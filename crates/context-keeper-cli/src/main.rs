use anyhow::Result;
use chrono::Utc;
use clap::{Parser, Subcommand};
use context_keeper_core::{
    ingestion,
    models::{AgentInfo, Episode},
    search::fuse_rrf,
    traits::*,
};
use context_keeper_rig::{
    embeddings::RigEmbedder,
    extraction::{RigEntityExtractor, RigRelationExtractor},
};
use context_keeper_surreal::{
    apply_schema, connect, default_storage_string, parse_storage_backend, Repository,
    StorageBackend, SurrealConfig,
};
use dotenv::dotenv;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info, warn};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

mod telemetry;

#[derive(Parser)]
#[command(
    name = "context-keeper",
    about = "Temporal knowledge graph memory tool",
    version
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

    /// Override API URL for embeddings (falls back to OPENAI_API_URL)
    #[arg(long, env = "EMBEDDING_API_URL", global = true)]
    embedding_api_url: Option<String>,
    /// Override API key for embeddings (falls back to OPENAI_API_KEY)
    #[arg(long, env = "EMBEDDING_API_KEY", global = true)]
    embedding_api_key: Option<String>,
    #[arg(
        short = 'f',
        long,
        env = "DB_FILE_PATH",
        global = true,
        default_value = "context.sql"
    )]
    db_file_path: String,

    /// Storage backend: "rocksdb:<path>" (default: ~/.context-keeper/data), "memory", or "remote:<ws_url>"
    #[arg(long, env = "STORAGE_BACKEND", global = true, default_value_t = default_storage_string())]
    storage: String,

    /// Namespace to scope operations to (omit for global/default)
    #[arg(long, env = "CK_NAMESPACE", global = true)]
    namespace: Option<String>,

    /// Agent identifier for provenance tracking
    #[arg(long, env = "CK_AGENT_ID", global = true)]
    agent_id: Option<String>,

    /// SurrealDB root username (for remote connections)
    #[arg(long, env = "SURREAL_USER", global = true)]
    surreal_user: Option<String>,

    /// SurrealDB root password (for remote connections)
    #[arg(long, env = "SURREAL_PASS", global = true)]
    surreal_pass: Option<String>,

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
    /// Delete all data and reset the knowledge graph to an empty state
    Reset {
        /// Skip the confirmation prompt
        #[arg(long)]
        force: bool,
    },
    /// Delete all data within a specific namespace
    DeleteNamespace {
        /// The namespace to delete
        #[arg(short, long)]
        namespace: String,
        /// Skip the confirmation prompt
        #[arg(long)]
        force: bool,
    },
    /// Manage anonymous telemetry (opt-in). See README for what is collected.
    Telemetry {
        #[command(subcommand)]
        action: TelemetryAction,
    },
}

#[derive(Subcommand)]
enum TelemetryAction {
    /// Print the current telemetry state and install id.
    Status,
    /// Enable anonymous telemetry.
    Enable,
    /// Disable anonymous telemetry.
    Disable,
}

/// Stable classification strings for telemetry. We never emit the error's
/// `.to_string()` because that can leak paths, arguments or other user data.
fn classify_error(e: &anyhow::Error) -> &'static str {
    // Walk the chain and look for well-known types.
    for cause in e.chain() {
        if cause.downcast_ref::<std::io::Error>().is_some() {
            return "io";
        }
        if cause.downcast_ref::<serde_json::Error>().is_some() {
            return "serde_json";
        }
    }
    "unknown"
}

fn subcommand_name(cmd: &Commands) -> &'static str {
    match cmd {
        Commands::Add { .. } => "add",
        Commands::Search { .. } => "search",
        Commands::Entity { .. } => "entity",
        Commands::Recent { .. } => "recent",
        Commands::Reset { .. } => "reset",
        Commands::DeleteNamespace { .. } => "delete-namespace",
        Commands::Telemetry { .. } => "telemetry",
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("context_keeper=info,warn")),
        )
        .init();

    let _ = dotenv();

    let cli = Cli::parse();

    // Short-circuit the telemetry management subcommand before touching the
    // database or running the consent flow — users may want to configure
    // telemetry before anything else happens.
    if let Commands::Telemetry { action } = &cli.command {
        return handle_telemetry_subcommand(action);
    }

    // Resolve consent (prompts on first run if stdin is a TTY) and
    // initialise the OTLP pipeline. The handle is inert when telemetry is
    // disabled, so it is always safe to call its `record_*` methods.
    let consent = telemetry::resolve_consent()?;
    let telemetry_handle = telemetry::init(&consent.config);
    if consent.first_run {
        telemetry_handle.record_install();
    }
    telemetry_handle.record_invoke(subcommand_name(&cli.command));

    match run(cli, &telemetry_handle).await {
        Ok(()) => {
            telemetry_handle.shutdown();
            Ok(())
        }
        Err(err) => {
            telemetry_handle.record_error(classify_error(&err));
            telemetry_handle.shutdown();
            Err(err)
        }
    }
}

fn handle_telemetry_subcommand(action: &TelemetryAction) -> Result<()> {
    match action {
        TelemetryAction::Status => {
            match telemetry::load_config()? {
                Some(cfg) => {
                    let active = telemetry::is_active(&cfg);
                    println!("telemetry: {}", if cfg.telemetry.enabled { "enabled" } else { "disabled" });
                    println!("install_id: {}", cfg.telemetry.install_id);
                    if cfg.telemetry.enabled && !active {
                        println!(
                            "note: {}=1 is overriding consent for this process",
                            telemetry::DISABLE_ENV_VAR
                        );
                    }
                }
                None => {
                    println!("telemetry: not configured (will prompt on first run)");
                }
            }
        }
        TelemetryAction::Enable => {
            let mut cfg = telemetry::load_config()?.unwrap_or_default();
            cfg.telemetry.enabled = true;
            if cfg.telemetry.install_id.is_empty() {
                cfg.telemetry.install_id = Uuid::new_v4().to_string();
            }
            telemetry::save_config(&cfg)?;
            println!("telemetry enabled. install_id={}", cfg.telemetry.install_id);
        }
        TelemetryAction::Disable => {
            let mut cfg = telemetry::load_config()?.unwrap_or_default();
            cfg.telemetry.enabled = false;
            telemetry::save_config(&cfg)?;
            println!("telemetry disabled.");
        }
    }
    Ok(())
}

async fn run(cli: Cli, _telemetry: &telemetry::TelemetryHandle) -> Result<()> {
    let embedding_dims = cli.embedding_dims.unwrap_or(1536);
    let config = SurrealConfig {
        embedding_dimensions: embedding_dims,
        storage: parse_storage_backend(&cli.storage),
        username: cli.surreal_user,
        password: cli.surreal_pass,
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

    // Embedding URL/key can be overridden separately (e.g., OpenAI for embeddings, HuggingFace for extraction)
    let emb_api_url = cli.embedding_api_url.as_deref().or(cli.api_url.as_deref());
    let emb_api_key = cli.embedding_api_key.as_deref().or(cli.api_key.as_deref());

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
            let emb_url = emb_api_url.unwrap_or(api_url);
            let emb_key = emb_api_key.unwrap_or(api_key);
            info!(
                extraction_url = api_url,
                embedding_url = emb_url,
                "Using LLM-powered extraction"
            );
            (
                Arc::new(RigEmbedder::new(
                    emb_url,
                    emb_key,
                    emb_model,
                    embedding_dims,
                )),
                Arc::new(RigEntityExtractor::new(api_url, api_key, ext_model)),
                Arc::new(RigRelationExtractor::new(api_url, api_key, ext_model)),
            )
        }
        _ => {
            let set: Vec<_> = llm_fields
                .iter()
                .filter(|(_, v)| v.is_some())
                .map(|(k, _)| *k)
                .collect();
            if !set.is_empty() {
                let missing: Vec<_> = llm_fields
                    .iter()
                    .filter(|(_, v)| v.is_none())
                    .map(|(k, _)| *k)
                    .collect();
                warn!(
                    "Partial LLM config (have {}, missing {}) — falling back to mock extraction",
                    set.join(", "),
                    missing.join(", ")
                );
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
            let entities = repo.find_entities_by_name(&name, None, ns).await?;
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
        Commands::Reset { force } => {
            if !force {
                eprint!("This will permanently delete all data. Continue? [y/N] ");
                let mut answer = String::new();
                std::io::stdin().read_line(&mut answer)?;
                if !answer.trim().eq_ignore_ascii_case("y") {
                    info!("Aborted.");
                    return Ok(());
                }
            }

            let entities = repo.count_active_entities().await.unwrap_or(0);
            let memories = repo.count_memories().await.unwrap_or(0);
            let episodes = repo.count_episodes().await.unwrap_or(0);
            let relations = repo.count_active_relations().await.unwrap_or(0);

            repo.reset_graph().await?;

            info!(
                "Reset complete — removed {} entities, {} relations, {} memories, {} episodes",
                entities, relations, memories, episodes
            );
        }
        Commands::DeleteNamespace { namespace, force } => {
            if !force {
                eprint!(
                    "This will permanently delete all data in namespace '{}'. Continue? [y/N] ",
                    namespace
                );
                let mut answer = String::new();
                std::io::stdin().read_line(&mut answer)?;
                if !answer.trim().eq_ignore_ascii_case("y") {
                    info!("Aborted.");
                    return Ok(());
                }
            }
            let result = repo.delete_namespace(&namespace).await?;
            info!(
                "Deleted namespace '{}' — removed {} entities, {} memories, {} episodes",
                namespace,
                result.entities_deleted,
                result.memories_deleted,
                result.episodes_deleted
            );
        }
        Commands::Telemetry { .. } => {
            // Handled earlier in `main`; unreachable here.
            unreachable!("telemetry subcommand should be handled before run()");
        }
    }

    if matches!(config.storage, StorageBackend::Memory) {
        repo.export(&cli.db_file_path).await?;
    }

    Ok(())
}
