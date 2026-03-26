//! Quickstart: Ingest episodes and search the knowledge graph.
//!
//! Run with: cargo run --example quickstart

use anyhow::Result;
use chrono::Utc;
use context_keeper_core::{ingestion, models::Episode, search::fuse_rrf, traits::*};
use context_keeper_surreal::{apply_schema, connect_memory, Repository, SurrealConfig};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    let config = SurrealConfig {
        embedding_dimensions: 64,
        ..SurrealConfig::default()
    };
    let db = connect_memory(&config).await?;
    apply_schema(&db, &config).await?;
    let repo = Repository::new(db);

    let embedder = MockEmbedder::new(64);
    let entity_extractor = MockEntityExtractor;
    let relation_extractor = MockRelationExtractor;

    let episodes = vec![
        "Alice is a software engineer at Acme Corp in Berlin",
        "Bob manages the Machine Learning team at Acme Corp",
        "Charlie joined Acme Corp as a Data Scientist in Munich",
    ];

    for text in &episodes {
        let episode = Episode {
            id: Uuid::new_v4(),
            content: text.to_string(),
            source: "quickstart".to_string(),
            session_id: None,
            agent: None,
            namespace: None,
            created_at: Utc::now(),
        };

        let resolver: &dyn EntityResolver = &repo;
        let result =
            ingestion::ingest(&episode, &embedder, &entity_extractor, &relation_extractor, Some(resolver), None).await?;

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
            "Ingested: {} entities, {} relations from: \"{}\"",
            result.entities.len(),
            result.relations.len(),
            &text[..40.min(text.len())]
        );
    }

    println!("\n--- Hybrid Search for 'Acme' ---");
    let query = "Acme";
    let query_embedding = embedder.embed(query).await?;

    let vector_results = repo.search_entities_by_vector(&query_embedding, 5, None, None).await?;
    let keyword_results = repo.search_entities_by_keyword(query, None, None).await?;

    let fused = fuse_rrf(vec![
        vector_results.into_iter().map(|(e, _)| e).collect(),
        keyword_results,
    ]);

    for (i, result) in fused.iter().take(5).enumerate() {
        if let Some(ref entity) = result.entity {
            println!(
                "  {}. {} ({}) -- score: {:.4}",
                i + 1,
                entity.name,
                entity.entity_type,
                result.score
            );
        }
    }

    println!("\n--- Recent Memories ---");
    let memories = repo.list_recent_memories(5).await?;
    for (i, memory) in memories.iter().enumerate() {
        println!("  {}. {}", i + 1, memory.content);
    }

    Ok(())
}
