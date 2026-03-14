use anyhow::Result;
use chrono::Utc;
use clap::{Parser, Subcommand};
use context_keeper_core::{
    ingestion, models::Episode, search::fuse_rrf, traits::*,
};
use context_keeper_surreal::{connect_memory, apply_schema, Repository, SurrealConfig};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "context-keeper", about = "Temporal knowledge graph memory tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Add a memory from text input
    Add {
        /// The text content to ingest
        #[arg(short, long)]
        text: String,
        /// Source label for the episode
        #[arg(short, long, default_value = "cli")]
        source: String,
    },
    /// Search memories
    Search {
        /// Search query
        #[arg(short, long)]
        query: String,
        /// Maximum results
        #[arg(short, long, default_value = "5")]
        limit: usize,
    },
    /// Get entity details
    Entity {
        /// Entity name to look up
        #[arg(short, long)]
        name: String,
    },
    /// List recent memories
    Recent {
        /// Number of recent items
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let config = SurrealConfig::default();
    let db = connect_memory(&config).await?;
    apply_schema(&db).await?;
    let repo = Repository::new(db);

    let embedder = MockEmbedder::new(64);
    let entity_extractor = MockEntityExtractor;
    let relation_extractor = MockRelationExtractor;

    match cli.command {
        Commands::Add { text, source } => {
            let episode = Episode {
                id: Uuid::new_v4(),
                content: text,
                source,
                session_id: None,
                created_at: Utc::now(),
            };
            let result = ingestion::ingest(
                &episode,
                &embedder,
                &entity_extractor,
                &relation_extractor,
            )
            .await?;

            repo.create_episode(&episode).await?;
            for entity in &result.entities {
                repo.upsert_entity(entity).await?;
            }
            for relation in &result.relations {
                repo.create_relation(relation).await?;
            }
            for memory in &result.memories {
                repo.create_memory(memory).await?;
            }

            println!(
                "Ingested: {} entities, {} relations, {} memories",
                result.entities.len(),
                result.relations.len(),
                result.memories.len()
            );
        }
        Commands::Search { query, limit } => {
            let query_embedding = embedder.embed(&query).await?;
            let vector_results = repo
                .search_entities_by_vector(&query_embedding, limit)
                .await?;
            let keyword_results = repo.search_entities_by_keyword(&query).await?;

            let fused = fuse_rrf(vec![
                vector_results.into_iter().map(|(e, _)| e).collect(),
                keyword_results,
            ]);

            if fused.is_empty() {
                println!("No results found.");
            } else {
                for (i, result) in fused.iter().take(limit).enumerate() {
                    if let Some(ref entity) = result.entity {
                        println!(
                            "{}. {} ({}) — score: {:.4}",
                            i + 1,
                            entity.name,
                            entity.entity_type,
                            result.score
                        );
                        println!("   {}", entity.summary);
                    }
                }
            }
        }
        Commands::Entity { name } => {
            let entities = repo.find_entities_by_name(&name).await?;
            if entities.is_empty() {
                println!("No entity found with name '{}'", name);
            } else {
                for entity in &entities {
                    println!("Name: {}", entity.name);
                    println!("Type: {}", entity.entity_type);
                    println!("Summary: {}", entity.summary);
                    println!("Valid from: {}", entity.valid_from);
                    if let Some(until) = entity.valid_until {
                        println!("Valid until: {}", until);
                    }
                    println!("---");
                }
            }
        }
        Commands::Recent { limit } => {
            let memories = repo.list_recent_memories(limit).await?;
            if memories.is_empty() {
                println!("No memories found.");
            } else {
                for (i, memory) in memories.iter().enumerate() {
                    println!("{}. [{}] {}", i + 1, memory.created_at, memory.content);
                }
            }
        }
    }

    Ok(())
}
