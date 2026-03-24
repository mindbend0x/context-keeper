use anyhow::Result;
use chrono::Utc;
use context_keeper_core::{models::*, traits::*};
use context_keeper_test::harness::{TestEnv, EMBED_DIM};
use uuid::Uuid;

// ── Entity roundtrip fidelity ────────────────────────────────────────────

#[tokio::test]
async fn test_entity_roundtrip_fidelity() -> Result<()> {
    let env = TestEnv::new().await?;
    let embedding = env.embedder.embed("TestEntity").await?;
    let now = Utc::now();

    let entity = Entity {
        id: Uuid::new_v4(),
        name: "TestEntity".to_string(),
        entity_type: EntityType::Concept,
        summary: "A concept for testing roundtrip fidelity".to_string(),
        embedding: embedding.clone(),
        valid_from: now,
        valid_until: None,
    };
    env.repo.upsert_entity(&entity).await?;

    let fetched = env.repo.get_entity(entity.id).await?.expect("entity should exist");
    assert_eq!(fetched.name, entity.name);
    assert_eq!(fetched.entity_type, entity.entity_type);
    assert_eq!(fetched.summary, entity.summary);
    assert!(fetched.valid_until.is_none());

    Ok(())
}

// ── Embedding preservation ───────────────────────────────────────────────

#[tokio::test]
async fn test_embedding_preservation() -> Result<()> {
    let env = TestEnv::new().await?;
    let embedding = env.embedder.embed("EmbeddingTest").await?;
    assert_eq!(embedding.len(), EMBED_DIM);

    let entity = Entity {
        id: Uuid::new_v4(),
        name: "EmbeddingTest".to_string(),
        entity_type: EntityType::Other,
        summary: "test".to_string(),
        embedding: embedding.clone(),
        valid_from: Utc::now(),
        valid_until: None,
    };
    env.repo.upsert_entity(&entity).await?;

    let fetched = env.repo.get_entity(entity.id).await?.expect("entity should exist");
    assert_eq!(fetched.embedding.len(), EMBED_DIM);

    for (i, (orig, stored)) in embedding.iter().zip(fetched.embedding.iter()).enumerate() {
        assert!(
            (orig - stored).abs() < 1e-10,
            "Embedding dimension {i} differs: {orig} vs {stored}"
        );
    }

    Ok(())
}

// ── Relation graph integrity ─────────────────────────────────────────────

#[tokio::test]
async fn test_relation_graph_integrity() -> Result<()> {
    let env = TestEnv::new().await?;
    let result = env
        .ingest_text("Alice works at Acme Corp in Berlin", "test")
        .await?;

    for entity in &result.entities {
        let rels = env.repo.get_relations_for_entity(entity.id).await?;
        for rel in &rels {
            assert!(
                rel.from_entity_id == entity.id || rel.to_entity_id == entity.id,
                "Relation for entity {} references unrelated entity IDs",
                entity.name
            );
        }
    }

    let total_stored_rels: usize = {
        let mut seen = std::collections::HashSet::new();
        for entity in &result.entities {
            let rels = env.repo.get_relations_for_entity(entity.id).await?;
            for rel in rels {
                seen.insert(rel.id);
            }
        }
        seen.len()
    };

    assert_eq!(
        total_stored_rels,
        result.relations.len(),
        "Number of stored relations should match ingestion result"
    );

    Ok(())
}

// ── Memory-episode link ──────────────────────────────────────────────────

#[tokio::test]
async fn test_memory_episode_link() -> Result<()> {
    let env = TestEnv::new().await?;

    let r1 = env.ingest_text("First episode content", "test").await?;
    let r2 = env.ingest_text("Second episode content", "test").await?;

    let memories = env.repo.list_recent_memories(10).await?;
    assert_eq!(memories.len(), 2);

    let mem_episode_ids: std::collections::HashSet<Uuid> =
        memories.iter().map(|m| m.source_episode_id).collect();

    let expected_episode_ids: std::collections::HashSet<Uuid> = r1
        .memories
        .iter()
        .chain(r2.memories.iter())
        .map(|m| m.source_episode_id)
        .collect();

    assert_eq!(
        mem_episode_ids, expected_episode_ids,
        "Memories should reference their source episodes"
    );

    Ok(())
}

// ── Memory-entity links ──────────────────────────────────────────────────

#[tokio::test]
async fn test_memory_entity_links() -> Result<()> {
    let env = TestEnv::new().await?;
    let result = env
        .ingest_text("Alice works at Acme Corp", "test")
        .await?;

    assert!(!result.entities.is_empty());
    assert_eq!(result.memories.len(), 1);

    let memory = &result.memories[0];
    assert_eq!(
        memory.entity_ids.len(),
        result.entities.len(),
        "Memory should reference all extracted entities"
    );

    for entity in &result.entities {
        assert!(
            memory.entity_ids.contains(&entity.id),
            "Memory should reference entity {}",
            entity.name
        );
    }

    Ok(())
}

// ── Upsert idempotency ──────────────────────────────────────────────────

#[tokio::test]
async fn test_upsert_idempotency() -> Result<()> {
    let env = TestEnv::new().await?;
    let id = Uuid::new_v4();

    for version in 1..=3 {
        let entity = Entity {
            id,
            name: "StableEntity".to_string(),
            entity_type: EntityType::Other,
            summary: format!("Version {version}"),
            embedding: env.embedder.embed("StableEntity").await?,
            valid_from: Utc::now(),
            valid_until: None,
        };
        env.repo.upsert_entity(&entity).await?;
    }

    let all = env.repo.get_all_active_entities().await?;
    let matching: Vec<_> = all.iter().filter(|e| e.id == id).collect();
    assert_eq!(matching.len(), 1, "Should have exactly one entity after 3 upserts");
    assert_eq!(matching[0].summary, "Version 3", "Should have the latest summary");

    Ok(())
}

// ── Export/import consistency ─────────────────────────────────────────────

#[tokio::test]
async fn test_export_import_consistency() -> Result<()> {
    let env = TestEnv::new().await?;
    env.ingest_text("Alice works at Acme Corp in Berlin", "test").await?;

    let original_entities = env.repo.get_all_active_entities().await?;
    let original_episodes = env.repo.list_recent_episodes(100).await?;
    let original_memories = env.repo.list_recent_memories(100).await?;

    let export_path = std::env::temp_dir().join("ck_test_export.surql");
    let export_path_str = export_path.to_str().unwrap();
    env.repo.export(export_path_str).await?;

    let env2 = TestEnv::new().await?;
    env2.repo.import_from_file(export_path_str).await?;

    let imported_entities = env2.repo.get_all_active_entities().await?;
    let imported_episodes = env2.repo.list_recent_episodes(100).await?;
    let imported_memories = env2.repo.list_recent_memories(100).await?;

    assert_eq!(
        original_entities.len(),
        imported_entities.len(),
        "Entity count should match after export/import"
    );
    assert_eq!(
        original_episodes.len(),
        imported_episodes.len(),
        "Episode count should match after export/import"
    );
    assert_eq!(
        original_memories.len(),
        imported_memories.len(),
        "Memory count should match after export/import"
    );

    let _ = std::fs::remove_file(&export_path);

    Ok(())
}
