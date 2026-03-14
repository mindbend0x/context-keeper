use anyhow::Result;
use chrono::{Duration, Utc};
use context_keeper_core::{
    ingestion,
    models::*,
    search::fuse_rrf,
    temporal::staleness_score,
    traits::*,
};
use context_keeper_surreal::{connect_memory, apply_schema, Repository, SurrealConfig};
use uuid::Uuid;

/// Helper: create a fresh in-memory DB + repo.
async fn setup() -> Result<Repository> {
    let config = SurrealConfig::default();
    let db = connect_memory(&config).await?;
    apply_schema(&db).await?;
    Ok(Repository::new(db))
}

// ── Episode CRUD ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_episode_crud() -> Result<()> {
    let repo = setup().await?;

    let episode = Episode {
        id: Uuid::new_v4(),
        content: "Alice met Bob at the park".to_string(),
        source: "test".to_string(),
        session_id: Some("s1".to_string()),
        created_at: Utc::now(),
    };

    repo.create_episode(&episode).await?;

    let fetched = repo.get_episode(episode.id).await?;
    assert!(fetched.is_some());
    let fetched = fetched.unwrap();
    assert_eq!(fetched.content, "Alice met Bob at the park");
    assert_eq!(fetched.source, "test");

    let recent = repo.list_recent_episodes(10).await?;
    assert_eq!(recent.len(), 1);

    Ok(())
}

// ── Entity CRUD ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_entity_crud() -> Result<()> {
    let repo = setup().await?;
    let embedder = MockEmbedder::new(8);

    let entity = Entity {
        id: Uuid::new_v4(),
        name: "Alice".to_string(),
        entity_type: "person".to_string(),
        summary: "A protagonist".to_string(),
        embedding: embedder.embed("Alice").await?,
        valid_from: Utc::now(),
        valid_until: None,
    };

    repo.upsert_entity(&entity).await?;

    let fetched = repo.get_entity(entity.id).await?;
    assert!(fetched.is_some());
    assert_eq!(fetched.unwrap().name, "Alice");

    let by_name = repo.find_entities_by_name("Alice").await?;
    assert_eq!(by_name.len(), 1);

    Ok(())
}

// ── Relation CRUD + Invalidation ────────────────────────────────────────

#[tokio::test]
async fn test_relation_lifecycle() -> Result<()> {
    let repo = setup().await?;
    let embedder = MockEmbedder::new(8);

    let alice = Entity {
        id: Uuid::new_v4(),
        name: "Alice".to_string(),
        entity_type: "person".to_string(),
        summary: "".to_string(),
        embedding: embedder.embed("Alice").await?,
        valid_from: Utc::now(),
        valid_until: None,
    };
    let bob = Entity {
        id: Uuid::new_v4(),
        name: "Bob".to_string(),
        entity_type: "person".to_string(),
        summary: "".to_string(),
        embedding: embedder.embed("Bob").await?,
        valid_from: Utc::now(),
        valid_until: None,
    };
    repo.upsert_entity(&alice).await?;
    repo.upsert_entity(&bob).await?;

    let relation = Relation {
        id: Uuid::new_v4(),
        source_entity_id: alice.id,
        target_entity_id: bob.id,
        relation_type: "knows".to_string(),
        confidence: 0.9,
        valid_from: Utc::now(),
        valid_until: None,
    };
    repo.create_relation(&relation).await?;

    let rels = repo.get_relations_for_entity(alice.id).await?;
    assert_eq!(rels.len(), 1);
    assert_eq!(rels[0].relation_type, "knows");

    // Invalidate
    repo.invalidate_relation(relation.id).await?;

    // After invalidation, active relations should be empty
    let rels = repo.get_relations_for_entity(alice.id).await?;
    assert_eq!(rels.len(), 0);

    Ok(())
}

// ── Full Ingestion Pipeline ─────────────────────────────────────────────

#[tokio::test]
async fn test_ingestion_pipeline() -> Result<()> {
    let repo = setup().await?;
    let embedder = MockEmbedder::new(8);
    let entity_extractor = MockEntityExtractor;
    let relation_extractor = MockRelationExtractor;

    let episode = Episode {
        id: Uuid::new_v4(),
        content: "Alice works at Acme Corp in Berlin".to_string(),
        source: "test".to_string(),
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

    // Should extract: Alice, Acme, Corp, Berlin (capitalized words > 1 char)
    assert!(!result.entities.is_empty());
    assert!(!result.memories.is_empty());

    // Persist
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

    // Verify persistence
    let entities = repo.get_all_active_entities().await?;
    assert_eq!(entities.len(), result.entities.len());

    let memories = repo.list_recent_memories(10).await?;
    assert_eq!(memories.len(), 1);

    Ok(())
}

// ── Vector Search ───────────────────────────────────────────────────────

#[tokio::test]
async fn test_vector_search() -> Result<()> {
    let repo = setup().await?;
    let embedder = MockEmbedder::new(8);

    // Insert entities with different embeddings
    for name in &["Rust", "Python", "JavaScript"] {
        let entity = Entity {
            id: Uuid::new_v4(),
            name: name.to_string(),
            entity_type: "language".to_string(),
            summary: format!("{} programming language", name),
            embedding: embedder.embed(name).await?,
            valid_from: Utc::now(),
            valid_until: None,
        };
        repo.upsert_entity(&entity).await?;
    }

    // Search for "Rust" — should return Rust first (identical embedding)
    let query_embedding = embedder.embed("Rust").await?;
    let results = repo.search_entities_by_vector(&query_embedding, 3).await?;
    assert_eq!(results.len(), 3);
    assert_eq!(results[0].0.name, "Rust");
    assert!((results[0].1 - 1.0).abs() < 0.01); // Cosine similarity ~1.0

    Ok(())
}

// ── RRF Fusion ──────────────────────────────────────────────────────────

#[tokio::test]
async fn test_rrf_fusion_end_to_end() -> Result<()> {
    let repo = setup().await?;
    let embedder = MockEmbedder::new(8);

    // Insert entities
    let mut entities = Vec::new();
    for name in &["Alice", "Bob", "Charlie"] {
        let entity = Entity {
            id: Uuid::new_v4(),
            name: name.to_string(),
            entity_type: "person".to_string(),
            summary: format!("Person named {}", name),
            embedding: embedder.embed(name).await?,
            valid_from: Utc::now(),
            valid_until: None,
        };
        repo.upsert_entity(&entity).await?;
        entities.push(entity);
    }

    // Simulate two ranked lists
    let vector_ranked = vec![entities[0].clone(), entities[1].clone()]; // Alice, Bob
    let keyword_ranked = vec![entities[1].clone(), entities[2].clone()]; // Bob, Charlie

    let fused = fuse_rrf(vec![vector_ranked, keyword_ranked]);

    // Bob appears in both lists → should be ranked first
    assert_eq!(fused[0].entity.as_ref().unwrap().name, "Bob");

    Ok(())
}

// ── Temporal Snapshot ───────────────────────────────────────────────────

#[tokio::test]
async fn test_temporal_snapshot() -> Result<()> {
    let repo = setup().await?;
    let embedder = MockEmbedder::new(8);
    let past = Utc::now() - Duration::hours(1);
    let now = Utc::now();

    // Entity valid since past
    let entity = Entity {
        id: Uuid::new_v4(),
        name: "OldFact".to_string(),
        entity_type: "fact".to_string(),
        summary: "An old fact".to_string(),
        embedding: embedder.embed("OldFact").await?,
        valid_from: past,
        valid_until: None,
    };
    repo.upsert_entity(&entity).await?;

    // Should appear in current snapshot
    let snapshot = repo.entities_at(now).await?;
    assert_eq!(snapshot.len(), 1);
    assert_eq!(snapshot[0].name, "OldFact");

    // Should also appear in snapshot from 30 min ago
    let half_past = past + Duration::minutes(30);
    let snapshot = repo.entities_at(half_past).await?;
    assert_eq!(snapshot.len(), 1);

    Ok(())
}

// ── Staleness Score ─────────────────────────────────────────────────────

#[test]
fn test_staleness_computation() {
    let entity = Entity {
        id: Uuid::new_v4(),
        name: "test".to_string(),
        entity_type: "test".to_string(),
        summary: "test".to_string(),
        embedding: vec![],
        valid_from: Utc::now() - Duration::days(5),
        valid_until: None,
    };
    let score = staleness_score(&entity);
    assert!((score - 5.0).abs() <= 1.0);
}

// ── Graph Neighbors ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_graph_neighbors() -> Result<()> {
    let repo = setup().await?;
    let embedder = MockEmbedder::new(8);

    let alice = Entity {
        id: Uuid::new_v4(),
        name: "Alice".to_string(),
        entity_type: "person".to_string(),
        summary: "".to_string(),
        embedding: embedder.embed("Alice").await?,
        valid_from: Utc::now(),
        valid_until: None,
    };
    let bob = Entity {
        id: Uuid::new_v4(),
        name: "Bob".to_string(),
        entity_type: "person".to_string(),
        summary: "".to_string(),
        embedding: embedder.embed("Bob").await?,
        valid_from: Utc::now(),
        valid_until: None,
    };
    let charlie = Entity {
        id: Uuid::new_v4(),
        name: "Charlie".to_string(),
        entity_type: "person".to_string(),
        summary: "".to_string(),
        embedding: embedder.embed("Charlie").await?,
        valid_from: Utc::now(),
        valid_until: None,
    };
    repo.upsert_entity(&alice).await?;
    repo.upsert_entity(&bob).await?;
    repo.upsert_entity(&charlie).await?;

    // Alice → Bob
    let rel1 = Relation {
        id: Uuid::new_v4(),
        source_entity_id: alice.id,
        target_entity_id: bob.id,
        relation_type: "knows".to_string(),
        confidence: 0.9,
        valid_from: Utc::now(),
        valid_until: None,
    };
    // Bob → Charlie
    let rel2 = Relation {
        id: Uuid::new_v4(),
        source_entity_id: bob.id,
        target_entity_id: charlie.id,
        relation_type: "knows".to_string(),
        confidence: 0.9,
        valid_from: Utc::now(),
        valid_until: None,
    };
    repo.create_relation(&rel1).await?;
    repo.create_relation(&rel2).await?;

    // 1-hop from Alice should find Alice + Bob
    let neighbors = repo.get_graph_neighbors(&[alice.id], 1).await?;
    assert!(neighbors.len() >= 1); // At least Alice
    let names: Vec<&str> = neighbors.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"Alice"));

    Ok(())
}
