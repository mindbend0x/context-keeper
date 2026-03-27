use std::path::Path;
use std::sync::Arc;
use anyhow::Result;
use chrono::Utc;
use clap::{Parser, Subcommand};
use context_keeper_core::{ingestion, models::{AgentInfo, Episode}, search::fuse_rrf, traits::*};
use context_keeper_rig::{
    embeddings::RigEmbedder,
    extraction::{RigEntityExtractor, RigRelationExtractor},
};
use context_keeper_surreal::{apply_schema, connect, Repository, StorageBackend, SurrealConfig};
use dotenv::dotenv;
use tracing::{debug, info, warn};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

/// Returns the default storage backend string: `rocksdb:~/.context-keeper/data`
/// with `~` expanded to the actual home directory.
fn default_storage() -> String {
    match dirs::home_dir() {
        Some(home) => format!("rocksdb:{}", home.join(".context-keeper").join("data").display()),
        None => "memory".to_string(),
    }
}

#[derive(Parser)]
#[command(
    name = "context-keeper",
    about = "Temporal knowledge graph memory tool"
)]
struct Cli {
    #[arg(short = 'e', long, env = "EMBEDDING_MODEL", global = true)]
    embedding_model_name: Option<String>,
    #[arg(short = 'd', long, env = "EMBEDDING_DIMS", global = true)]
    embedding_dims: Option<usize>,
    #[arg(short = 'x', long, env = "EXTRACTION_MODEL", global = true)]
    extraction_model_name: Option<String>,
    #[arg(short = 'u', long, env = "OPENAI_API_URL", global = true)]
    api_url: Option<String>,
    #[arg(short = 'k', long, env = "OPENAI_API_KEY", global = true)]
    api_key: Option<String>,
    #[arg(short = 'f', long, env = "DB_FILE_PATH", global = true, default_value = "context.sql")]
    db_file_path: String,

    /// Storage backend: "rocksdb:<path>" (default: ~/.context-keeper/data), "memory", or "remote:<ws_url>"
    #[arg(long, env = "STORAGE_BACKEND", global = true, default_value_t = default_storage())]
    storage: String,

    /// Namespace to scope operations to (omit for global/default)
    #[arg(long, env = "CK_NAMESPACE", global = true)]
    namespace: Option<String>,

    /// Agent identifier for provenance tracking
    #[arg(long, env = "CK_AGENT_ID", global = true)]
    agent_id: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Add a memory from text input
    Add {
        #[arg(short, long)]
        text: String,
        #[arg(short, long, default_value = "cli")]
        source: String,
    },
    /// Search memories
    Search {
        #[arg(short, long)]
        query: String,
        #[arg(short, long, default_value = "5")]
        limit: usize,
    },
    /// Get entity details
    Entity {
        #[arg(short, long)]
        name: String,
    },
    /// List recent memories
    Recent {
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
}

fn parse_storage_backend(s: &str) -> StorageBackend {
    if let Some(path) = s.strip_prefix("rocksdb:") {
        StorageBackend::RocksDb(path.to_string())
    } else if let Some(url) = s.strip_prefix("remote:") {
        StorageBackend::Remote(url.to_string())
    } else {
        StorageBackend::Memory
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let _ = dotenv();

    let cli = Cli::parse();

    let embedding_dims = cli.embedding_dims.unwrap_or(1536);
    let config = SurrealConfig {
        embedding_dimensions: embedding_dims,
        storage: parse_storage_backend(&cli.storage),
        ..SurrealConfig::default()
    };

    if let StorageBackend::RocksDb(ref path) = config.storage {
        if let Some(parent) = Path::new(path).parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::create_dir_all(path).ok();
    }

    let db = connect(&config).await?;
    apply_schema(&db, &config).await?;
    let repo = Repository::new(db);

    if Path::new(&cli.db_file_path).exists() && matches!(config.storage, StorageBackend::Memory) {
        repo.import_from_file(&cli.db_file_path).await?;
    }

    let llm_fields = [
        ("OPENAI_API_URL", cli.api_url.as_deref()),
        ("OPENAI_API_KEY", cli.api_key.as_deref()),
        ("EMBEDDING_MODEL", cli.embedding_model_name.as_deref()),
        ("EXTRACTION_MODEL", cli.extraction_model_name.as_deref()),
    ];

    let (embedder, entity_extractor, relation_extractor): (
        Arc<dyn Embedder>,
        Arc<dyn EntityExtractor>,
        Arc<dyn RelationExtractor>,
    ) = match (
        cli.api_url.as_deref(),
        cli.api_key.as_deref(),
        cli.embedding_model_name.as_deref(),
        cli.extraction_model_name.as_deref(),
    ) {
        (Some(api_url), Some(api_key), Some(emb_model), Some(ext_model)) => {
            info!("Using LLM-powered extraction");
            (
                Arc::new(RigEmbedder::new(api_url, api_key, emb_model, embedding_dims)),
                Arc::new(RigEntityExtractor::new(api_url, api_key, ext_model)),
                Arc::new(RigRelationExtractor::new(api_url, api_key, ext_model)),
            )
        }
        _ => {
            let set: Vec<_> = llm_fields.iter().filter(|(_, v)| v.is_some()).map(|(k, _)| *k).collect();
            if !set.is_empty() {
                let missing: Vec<_> = llm_fields.iter().filter(|(_, v)| v.is_none()).map(|(k, _)| *k).collect();
                warn!("Partial LLM config (have {}, missing {}) — falling back to mock extraction",
                    set.join(", "), missing.join(", "));
            } else {
                info!("No LLM config — using mock extraction (set OPENAI_API_URL, OPENAI_API_KEY, EMBEDDING_MODEL, EXTRACTION_MODEL for real LLM)");
            }
            (
                Arc::new(MockEmbedder::new(embedding_dims)),
                Arc::new(MockEntityExtractor),
                Arc::new(MockRelationExtractor),
            )
        }
    };

    let ns = cli.namespace.as_deref();

    match cli.command {
        Commands::Add { text, source } => {
            let agent = cli.agent_id.as_ref().map(|id| AgentInfo {
                agent_id: id.clone(),
                agent_name: None,
                machine_id: None,
            });
            let episode = Episode {
                id: Uuid::new_v4(),
                content: text,
                source,
                session_id: None,
                agent,
                namespace: cli.namespace.clone(),
                created_at: Utc::now(),
            };

            let resolver: &dyn EntityResolver = &repo;
            let result = ingestion::ingest(
                &episode,
                embedder.as_ref(),
                entity_extractor.as_ref(),
                relation_extractor.as_ref(),
                Some(resolver),
                None,
            )
            .await?;

            for inv in &result.diff.entities_invalidated {
                repo.invalidate_entity(inv.invalidated_id).await?;
                let relations = repo.get_relations_for_entity(inv.invalidated_id).await?;
                for rel in &relations {
                    repo.invalidate_relation(rel.id).await?;
                }
            }

            repo.create_episode(&episode).await?;
            for entity in &result.entities {
                repo.upsert_entity(entity).await?;
                debug!("Upserted entity: {}", entity.name);
            }
            for relation in &result.relations {
                repo.create_relation(relation).await?;
                debug!("Created relation: {}", relation.from_entity_id);
            }
            for memory in &result.memories {
                repo.create_memory(memory).await?;
                debug!("Created memory: {}", memory.content);
            }

            info!(
                "Ingested: {} entities ({} new, {} updated, {} invalidated), {} relations, {} memories",
                result.entities.len(),
                result.diff.entities_created.len(),
                result.diff.entities_updated.len(),
                result.diff.entities_invalidated.len(),
                result.relations.len(),
                result.memories.len()
            );
        }
        Commands::Search { query, limit } => {
            let query_embedding = embedder.embed(&query).await?;
            let vector_results = repo
                .search_entities_by_vector(&query_embedding, limit, None, ns)
                .await?;
            let keyword_results = repo.search_entities_by_keyword(&query, None, ns).await?;

            let fused = fuse_rrf(vec![
                vector_results.into_iter().map(|(e, _)| e).collect(),
                keyword_results,
            ]);

            if fused.is_empty() {
                info!("No results found.");
            } else {
                for (i, result) in fused.iter().take(limit).enumerate() {
                    if let Some(ref entity) = result.entity {
                        info!(
                            "{}. {} ({}) -- score: {:.4}",
                            i + 1,
                            entity.name,
                            entity.entity_type,
                            result.score
                        );
                        debug!("   {}", entity.summary);
                    }
                }
            }
        }
        Commands::Entity { name } => {
            let entities = repo.find_entities_by_name(&name, ns).await?;
            if entities.is_empty() {
                info!("No entity found with name '{}'", name);
            } else {
                for entity in &entities {
                    info!("Name: {}", entity.name);
                    info!("Type: {}", entity.entity_type);
                    info!("Summary: {}", entity.summary);
                    info!("Valid from: {}", entity.valid_from);
                    if let Some(until) = entity.valid_until {
                        info!("Valid until: {}", until);
                    }
                }
            }
        }
        Commands::Recent { limit } => {
            let memories = repo.list_recent_memories(limit).await?;
            if memories.is_empty() {
                info!("No memories found.");
            } else {
                for (i, memory) in memories.iter().enumerate() {
                    info!("{}. [{}] {}", i + 1, memory.created_at, memory.content);
                }
            }
        }
    }

    if matches!(config.storage, StorageBackend::Memory) {
        repo.export(&cli.db_file_path).await?;
    }

    Ok(())
}
