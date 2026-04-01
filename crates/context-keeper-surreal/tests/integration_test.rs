use anyhow::Result;
use chrono::{Duration, Utc};
use context_keeper_core::{
    ingestion, models::*, search::fuse_rrf, temporal::staleness_score, traits::*,
};
use context_keeper_surreal::{apply_schema, connect_memory, Repository, SurrealConfig};
use uuid::Uuid;

/// Helper: create a fresh in-memory DB + repo with dim=8 for mock embeddings.
async fn setup() -> Result<Repository> {
    let config = SurrealConfig {
        embedding_dimensions: 8,
        ..SurrealConfig::default()
    };
    let db = connect_memory(&config).await?;
    apply_schema(&db, &config).await?;
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
        agent: None,
        namespace: None,
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
        entity_type: EntityType::Person,
        summary: "A protagonist".to_string(),
        embedding: embedder.embed("Alice").await?,
        valid_from: Utc::now(),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };

    repo.upsert_entity(&entity).await?;

    let fetched = repo.get_entity(entity.id).await?;
    assert!(fetched.is_some());
    assert_eq!(fetched.unwrap().name, "Alice");

    let by_name = repo.find_entities_by_name("Alice", None, None).await?;
    assert_eq!(by_name.len(), 1);

    Ok(())
}

// ── Entity UPSERT deduplication ─────────────────────────────────────────

#[tokio::test]
async fn test_entity_upsert_updates_existing() -> Result<()> {
    let repo = setup().await?;
    let embedder = MockEmbedder::new(8);

    let id = Uuid::new_v4();
    let entity_v1 = Entity {
        id,
        name: "Alice".to_string(),
        entity_type: EntityType::Person,
        summary: "Version 1".to_string(),
        embedding: embedder.embed("Alice v1").await?,
        valid_from: Utc::now(),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    repo.upsert_entity(&entity_v1).await?;

    let entity_v2 = Entity {
        id,
        name: "Alice".to_string(),
        entity_type: EntityType::Person,
        summary: "Version 2 - updated".to_string(),
        embedding: embedder.embed("Alice v2").await?,
        valid_from: Utc::now(),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    repo.upsert_entity(&entity_v2).await?;

    let fetched = repo.get_entity(id).await?;
    assert!(fetched.is_some());
    let fetched = fetched.unwrap();
    assert_eq!(fetched.summary, "Version 2 - updated");

    Ok(())
}

// ── Relation CRUD + Invalidation (Graph Edges) ─────────────────────────

#[tokio::test]
async fn test_relation_lifecycle() -> Result<()> {
    let repo = setup().await?;
    let embedder = MockEmbedder::new(8);

    let alice = Entity {
        id: Uuid::new_v4(),
        name: "Alice".to_string(),
        entity_type: EntityType::Person,
        summary: "".to_string(),
        embedding: embedder.embed("Alice").await?,
        valid_from: Utc::now(),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    let bob = Entity {
        id: Uuid::new_v4(),
        name: "Bob".to_string(),
        entity_type: EntityType::Person,
        summary: "".to_string(),
        embedding: embedder.embed("Bob").await?,
        valid_from: Utc::now(),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    repo.upsert_entity(&alice).await?;
    repo.upsert_entity(&bob).await?;

    let relation = Relation {
        id: Uuid::new_v4(),
        from_entity_id: alice.id,
        to_entity_id: bob.id,
        relation_type: RelationType::Knows,
        confidence: 90,
        valid_from: Utc::now(),
        valid_until: None,
    };
    repo.create_relation(&relation).await?;

    let rels = repo.get_relations_for_entity(alice.id).await?;
    assert_eq!(rels.len(), 1);
    assert_eq!(rels[0].relation_type, RelationType::Knows);

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
        agent: None,
        namespace: None,
        created_at: Utc::now(),
    };

    let result = ingestion::ingest(
        &episode,
        &embedder,
        &entity_extractor,
        &relation_extractor,
        None,
        None,
        None,
    )
    .await?;

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

// ── Vector Search (HNSW-backed) ─────────────────────────────────────────

#[tokio::test]
async fn test_vector_search() -> Result<()> {
    let repo = setup().await?;
    let embedder = MockEmbedder::new(8);

    for name in &["Rust", "Python", "JavaScript"] {
        let entity = Entity {
            id: Uuid::new_v4(),
            name: name.to_string(),
            entity_type: EntityType::Concept,
            summary: format!("{} programming language", name),
            embedding: embedder.embed(name).await?,
            valid_from: Utc::now(),
            valid_until: None,
            namespace: None,
            created_by_agent: None,
        };
        repo.upsert_entity(&entity).await?;
    }

    let query_embedding = embedder.embed("Rust").await?;
    let results = repo
        .search_entities_by_vector(&query_embedding, 3, None, None)
        .await?;
    assert_eq!(results.len(), 3);
    assert_eq!(results[0].0.name, "Rust");
    assert!((results[0].1 - 1.0).abs() < 0.01);

    Ok(())
}

// ── Memory Vector Search ────────────────────────────────────────────────

#[tokio::test]
async fn test_memory_vector_search() -> Result<()> {
    let repo = setup().await?;
    let embedder = MockEmbedder::new(8);

    let episode = Episode {
        id: Uuid::new_v4(),
        content: "Rust is a systems programming language".to_string(),
        source: "test".to_string(),
        session_id: None,
        agent: None,
        namespace: None,
        created_at: Utc::now(),
    };
    repo.create_episode(&episode).await?;

    let memory = Memory {
        id: Uuid::new_v4(),
        content: "Rust is fast and memory safe".to_string(),
        embedding: embedder.embed("Rust fast memory safe").await?,
        source_episode_id: episode.id,
        entity_ids: vec![],
        created_at: Utc::now(),
        namespace: None,
        created_by_agent: None,
    };
    repo.create_memory(&memory).await?;

    let query_embedding = embedder.embed("Rust fast memory safe").await?;
    let results = repo
        .search_memories_by_vector(&query_embedding, 5, None)
        .await?;
    assert_eq!(results.len(), 1);
    assert!((results[0].1 - 1.0).abs() < 0.01);

    Ok(())
}

// ── BM25 Full-Text Search ───────────────────────────────────────────────

#[tokio::test]
async fn test_bm25_entity_search() -> Result<()> {
    let repo = setup().await?;
    let embedder = MockEmbedder::new(8);

    let entity = Entity {
        id: Uuid::new_v4(),
        name: "Kubernetes".to_string(),
        entity_type: EntityType::Concept,
        summary: "Container orchestration platform for deploying microservices".to_string(),
        embedding: embedder.embed("Kubernetes").await?,
        valid_from: Utc::now(),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    repo.upsert_entity(&entity).await?;

    let results = repo
        .search_entities_by_keyword("Kubernetes", None, None)
        .await?;
    assert!(!results.is_empty());
    assert_eq!(results[0].name, "Kubernetes");

    let results = repo
        .search_entities_by_keyword("orchestration", None, None)
        .await?;
    assert!(!results.is_empty());

    Ok(())
}

#[tokio::test]
async fn test_bm25_episode_search() -> Result<()> {
    let repo = setup().await?;

    let episode = Episode {
        id: Uuid::new_v4(),
        content: "The distributed database handles millions of transactions".to_string(),
        source: "test".to_string(),
        session_id: None,
        agent: None,
        namespace: None,
        created_at: Utc::now(),
    };
    repo.create_episode(&episode).await?;

    let results = repo.search_episodes_by_keyword("database").await?;
    assert!(!results.is_empty());

    Ok(())
}

// ── RRF Fusion ──────────────────────────────────────────────────────────

#[tokio::test]
async fn test_rrf_fusion_end_to_end() -> Result<()> {
    let repo = setup().await?;
    let embedder = MockEmbedder::new(8);

    let mut entities = Vec::new();
    for name in &["Alice", "Bob", "Charlie"] {
        let entity = Entity {
            id: Uuid::new_v4(),
            name: name.to_string(),
            entity_type: EntityType::Person,
            summary: format!("Person named {}", name),
            embedding: embedder.embed(name).await?,
            valid_from: Utc::now(),
            valid_until: None,
            namespace: None,
            created_by_agent: None,
        };
        repo.upsert_entity(&entity).await?;
        entities.push(entity);
    }

    let vector_ranked = vec![entities[0].clone(), entities[1].clone()];
    let keyword_ranked = vec![entities[1].clone(), entities[2].clone()];

    let fused = fuse_rrf(vec![vector_ranked, keyword_ranked]);

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

    let entity = Entity {
        id: Uuid::new_v4(),
        name: "OldFact".to_string(),
        entity_type: EntityType::Concept,
        summary: "An old fact".to_string(),
        embedding: embedder.embed("OldFact").await?,
        valid_from: past,
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    repo.upsert_entity(&entity).await?;

    let snapshot = repo.entities_at(now).await?;
    assert_eq!(snapshot.len(), 1);
    assert_eq!(snapshot[0].name, "OldFact");

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
        entity_type: EntityType::Other,
        summary: "test".to_string(),
        embedding: vec![],
        valid_from: Utc::now() - Duration::days(5),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
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
        entity_type: EntityType::Person,
        summary: "".to_string(),
        embedding: embedder.embed("Alice").await?,
        valid_from: Utc::now(),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    let bob = Entity {
        id: Uuid::new_v4(),
        name: "Bob".to_string(),
        entity_type: EntityType::Person,
        summary: "".to_string(),
        embedding: embedder.embed("Bob").await?,
        valid_from: Utc::now(),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    let charlie = Entity {
        id: Uuid::new_v4(),
        name: "Charlie".to_string(),
        entity_type: EntityType::Person,
        summary: "".to_string(),
        embedding: embedder.embed("Charlie").await?,
        valid_from: Utc::now(),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    repo.upsert_entity(&alice).await?;
    repo.upsert_entity(&bob).await?;
    repo.upsert_entity(&charlie).await?;

    let rel1 = Relation {
        id: Uuid::new_v4(),
        from_entity_id: alice.id,
        to_entity_id: bob.id,
        relation_type: RelationType::Knows,
        confidence: 90,
        valid_from: Utc::now(),
        valid_until: None,
    };
    let rel2 = Relation {
        id: Uuid::new_v4(),
        from_entity_id: bob.id,
        to_entity_id: charlie.id,
        relation_type: RelationType::Knows,
        confidence: 90,
        valid_from: Utc::now(),
        valid_until: None,
    };
    repo.create_relation(&rel1).await?;
    repo.create_relation(&rel2).await?;

    let neighbors = repo.get_graph_neighbors(&[alice.id], 1).await?;
    assert!(!neighbors.is_empty());
    let names: Vec<&str> = neighbors.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"Alice"));

    Ok(())
}

// ── Memory Graph Edges ──────────────────────────────────────────────────

#[tokio::test]
async fn test_memory_graph_edges() -> Result<()> {
    let repo = setup().await?;
    let embedder = MockEmbedder::new(8);

    let episode = Episode {
        id: Uuid::new_v4(),
        content: "Alice works at Acme".to_string(),
        source: "test".to_string(),
        session_id: None,
        agent: None,
        namespace: None,
        created_at: Utc::now(),
    };
    repo.create_episode(&episode).await?;

    let alice = Entity {
        id: Uuid::new_v4(),
        name: "AliceWorker".to_string(),
        entity_type: EntityType::Person,
        summary: "Works at Acme".to_string(),
        embedding: embedder.embed("AliceWorker").await?,
        valid_from: Utc::now(),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    repo.upsert_entity(&alice).await?;

    let memory = Memory {
        id: Uuid::new_v4(),
        content: "Alice works at Acme".to_string(),
        embedding: embedder.embed("Alice works at Acme").await?,
        source_episode_id: episode.id,
        entity_ids: vec![alice.id],
        created_at: Utc::now(),
        namespace: None,
        created_by_agent: None,
    };
    repo.create_memory(&memory).await?;

    let memories = repo.list_recent_memories(10).await?;
    assert_eq!(memories.len(), 1);
    assert_eq!(memories[0].content, "Alice works at Acme");

    Ok(())
}

// ── Temporal Relations Query ────────────────────────────────────────────

#[tokio::test]
async fn test_relations_at_temporal() -> Result<()> {
    let repo = setup().await?;
    let embedder = MockEmbedder::new(8);
    let now = Utc::now();

    let alice = Entity {
        id: Uuid::new_v4(),
        name: "AliceTemporal".to_string(),
        entity_type: EntityType::Person,
        summary: "".to_string(),
        embedding: embedder.embed("AliceTemporal").await?,
        valid_from: now - Duration::days(10),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    let acme = Entity {
        id: Uuid::new_v4(),
        name: "AcmeTemporal".to_string(),
        entity_type: EntityType::Organization,
        summary: "".to_string(),
        embedding: embedder.embed("AcmeTemporal").await?,
        valid_from: now - Duration::days(10),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    repo.upsert_entity(&alice).await?;
    repo.upsert_entity(&acme).await?;

    let rel = Relation {
        id: Uuid::new_v4(),
        from_entity_id: alice.id,
        to_entity_id: acme.id,
        relation_type: RelationType::WorksAt,
        confidence: 95,
        valid_from: now - Duration::days(10),
        valid_until: None,
    };
    repo.create_relation(&rel).await?;

    let rels = repo.relations_at(now).await?;
    assert!(rels
        .iter()
        .any(|r| r.relation_type == RelationType::WorksAt));

    Ok(())
}

// ── Entity type filter ──────────────────────────────────────────────────

#[tokio::test]
async fn test_entity_type_filter() -> Result<()> {
    let repo = setup().await?;
    let embedder = MockEmbedder::new(8);

    let alice = Entity {
        id: Uuid::new_v4(),
        name: "AliceFilter".to_string(),
        entity_type: EntityType::Person,
        summary: "A person".to_string(),
        embedding: embedder.embed("AliceFilter").await?,
        valid_from: Utc::now(),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    let acme = Entity {
        id: Uuid::new_v4(),
        name: "AcmeFilter".to_string(),
        entity_type: EntityType::Organization,
        summary: "A company".to_string(),
        embedding: embedder.embed("AcmeFilter").await?,
        valid_from: Utc::now(),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    repo.upsert_entity(&alice).await?;
    repo.upsert_entity(&acme).await?;

    let by_type = repo.get_entities_by_type("person").await?;
    assert_eq!(by_type.len(), 1);
    assert_eq!(by_type[0].name, "AliceFilter");

    let by_type_org = repo.get_entities_by_type("organization").await?;
    assert_eq!(by_type_org.len(), 1);
    assert_eq!(by_type_org[0].name, "AcmeFilter");

    Ok(())
}

// ── Symmetric relation dedup ────────────────────────────────────────────

#[tokio::test]
async fn test_symmetric_relation_dedup() -> Result<()> {
    let repo = setup().await?;
    let embedder = MockEmbedder::new(8);

    let alice = Entity {
        id: Uuid::new_v4(),
        name: "AliceSym".to_string(),
        entity_type: EntityType::Person,
        summary: "".to_string(),
        embedding: embedder.embed("AliceSym").await?,
        valid_from: Utc::now(),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    let bob = Entity {
        id: Uuid::new_v4(),
        name: "BobSym".to_string(),
        entity_type: EntityType::Person,
        summary: "".to_string(),
        embedding: embedder.embed("BobSym").await?,
        valid_from: Utc::now(),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    repo.upsert_entity(&alice).await?;
    repo.upsert_entity(&bob).await?;

    let rel1 = Relation {
        id: Uuid::new_v4(),
        from_entity_id: alice.id,
        to_entity_id: bob.id,
        relation_type: RelationType::Knows,
        confidence: 80,
        valid_from: Utc::now(),
        valid_until: None,
    };
    let created = repo.create_relation(&rel1).await?;
    assert!(created, "First relation should be newly created");

    // Reverse direction of a symmetric type should merge
    let rel2 = Relation {
        id: Uuid::new_v4(),
        from_entity_id: bob.id,
        to_entity_id: alice.id,
        relation_type: RelationType::Knows,
        confidence: 90,
        valid_from: Utc::now(),
        valid_until: None,
    };
    let created = repo.create_relation(&rel2).await?;
    assert!(!created, "Reverse symmetric relation should be merged");

    let rels = repo.get_relations_for_entity(alice.id).await?;
    assert_eq!(
        rels.len(),
        1,
        "Should have exactly one relation after dedup"
    );
    assert_eq!(rels[0].confidence, 85, "Confidence should be averaged");

    Ok(())
}
