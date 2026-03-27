use anyhow::Result;
use chrono::Utc;
use context_keeper_core::models::*;
use context_keeper_core::traits::*;
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
        namespace: None,
        created_by_agent: None,
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
        namespace: None,
        created_by_agent: None,
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
            namespace: None,
            created_by_agent: None,
        };
        env.repo.upsert_entity(&entity).await?;
    }

    let all = env.repo.get_all_active_entities().await?;
    let matching: Vec<_> = all.iter().filter(|e| e.id == id).collect();
    assert_eq!(matching.len(), 1, "Should have exactly one entity after 3 upserts");
    assert_eq!(matching[0].summary, "Version 3", "Should have the latest summary");

    Ok(())
}

// ── Composite entity identity: (name, entity_type) ───────────────────────
// "Alice (Person)" and "Alice (Organization)" are distinct graph nodes.

#[tokio::test]
async fn test_composite_entity_identity_coexistence() -> Result<()> {
    let env = TestEnv::new().await?;

    let alice_person = Entity {
        id: Uuid::new_v4(),
        name: "Alice".to_string(),
        entity_type: EntityType::Person,
        summary: "Software engineer".to_string(),
        embedding: env.embedder.embed("Alice person").await?,
        valid_from: Utc::now(),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    let alice_org = Entity {
        id: Uuid::new_v4(),
        name: "Alice".to_string(),
        entity_type: EntityType::Organization,
        summary: "A startup company".to_string(),
        embedding: env.embedder.embed("Alice org").await?,
        valid_from: Utc::now(),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };

    env.repo.upsert_entity(&alice_person).await?;
    env.repo.upsert_entity(&alice_org).await?;

    let all_alice = env.repo.find_entities_by_name("Alice", None, None).await?;
    assert_eq!(
        all_alice.len(),
        2,
        "Two distinct Alice entities (Person vs Organization) should coexist"
    );

    let persons = env
        .repo
        .find_entities_by_name("Alice", Some(&EntityType::Person), None)
        .await?;
    assert_eq!(persons.len(), 1);
    assert_eq!(persons[0].entity_type, EntityType::Person);
    assert_eq!(persons[0].summary, "Software engineer");

    let orgs = env
        .repo
        .find_entities_by_name("Alice", Some(&EntityType::Organization), None)
        .await?;
    assert_eq!(orgs.len(), 1);
    assert_eq!(orgs[0].entity_type, EntityType::Organization);
    assert_eq!(orgs[0].summary, "A startup company");

    Ok(())
}

// ── Negation invalidates old entity ───────────────────────────────────────
// "Alice works at Acme" followed by "Alice left Acme" should invalidate
// the original Alice entity and create a new one.

#[tokio::test]
async fn test_negation_invalidates_entity() -> Result<()> {
    let env = TestEnv::new().await?;

    env.ingest_text_with_resolver("Alice works at Acme", "test", true).await?;

    let before = env.repo.find_entities_by_name("Alice", None, None).await?;
    assert_eq!(before.len(), 1, "Alice should exist after first ingestion");
    let original_id = before[0].id;

    env.ingest_text_with_resolver("Alice left Acme", "test", true).await?;

    let after = env.repo.find_entities_by_name("Alice", None, None).await?;
    assert_eq!(after.len(), 1, "Should have one active Alice after negation");
    assert_ne!(
        after[0].id, original_id,
        "New Alice entity should have a different ID than the invalidated one"
    );

    let original = env.repo.get_entity(original_id).await?.expect("original should still exist in DB");
    assert!(
        original.valid_until.is_some(),
        "Original Alice entity should be soft-deleted (valid_until set)"
    );

    Ok(())
}

// ── Invalidated entities don't appear in active queries ──────────────────

#[tokio::test]
async fn test_invalidated_entities_excluded_from_active() -> Result<()> {
    let env = TestEnv::new().await?;

    env.ingest_text_with_resolver("Alice works at Acme", "test", true).await?;
    env.ingest_text_with_resolver("Alice quit Acme", "test", true).await?;

    let all_active = env.repo.get_all_active_entities().await?;
    let active_alice: Vec<_> = all_active.iter().filter(|e| e.name == "Alice").collect();
    assert_eq!(
        active_alice.len(),
        1,
        "Only one active Alice should exist after negation"
    );
    assert!(
        active_alice[0].valid_until.is_none(),
        "Active Alice should have no valid_until"
    );

    Ok(())
}

// ── Relations invalidated along with entities ────────────────────────────

#[tokio::test]
async fn test_relations_invalidated_with_entity() -> Result<()> {
    let env = TestEnv::new().await?;

    let r1 = env.ingest_text_with_resolver("Alice works at Acme", "test", true).await?;

    let alice_before: Vec<_> = r1.entities.iter().filter(|e| e.name == "Alice").collect();
    assert!(!alice_before.is_empty(), "Alice should be extracted");
    let alice_id = alice_before[0].id;

    let rels_before = env.repo.get_relations_for_entity(alice_id).await?;
    assert!(
        !rels_before.is_empty(),
        "Alice should have relations after first ingestion"
    );

    env.ingest_text_with_resolver("Alice left Acme", "test", true).await?;

    let rels_after = env.repo.get_relations_for_entity(alice_id).await?;
    assert!(
        rels_after.is_empty(),
        "Old Alice's relations should be invalidated after negation"
    );

    Ok(())
}

// ── Deduplication: same entity ingested twice → single entity updated ────

#[tokio::test]
async fn test_deduplication_updates_entity() -> Result<()> {
    let env = TestEnv::new().await?;

    env.ingest_text_with_resolver("Alice works at Acme", "test", true).await?;

    let before = env.repo.find_entities_by_name("Alice", None, None).await?;
    assert_eq!(before.len(), 1);
    let original_id = before[0].id;
    let original_summary = before[0].summary.clone();

    env.ingest_text_with_resolver("Alice leads engineering at Acme", "test", true).await?;

    let after = env.repo.find_entities_by_name("Alice", None, None).await?;
    assert_eq!(
        after.len(),
        1,
        "Should still have one Alice after dedup (no contradiction)"
    );
    assert_eq!(
        after[0].id, original_id,
        "Entity ID should be preserved through dedup update"
    );
    assert_ne!(
        after[0].summary, original_summary,
        "Summary should be updated with merged info"
    );

    Ok(())
}

// ── Invalidate relations for entity (repository method) ──────────────────

#[tokio::test]
async fn test_invalidate_relations_for_entity() -> Result<()> {
    let env = TestEnv::new().await?;
    let embedder = &env.embedder;

    let alice = Entity {
        id: Uuid::new_v4(),
        name: "Alice".to_string(),
        entity_type: EntityType::Person,
        summary: "Engineer".to_string(),
        embedding: embedder.embed("Alice").await?,
        valid_from: Utc::now(),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    let acme = Entity {
        id: Uuid::new_v4(),
        name: "Acme".to_string(),
        entity_type: EntityType::Organization,
        summary: "Company".to_string(),
        embedding: embedder.embed("Acme").await?,
        valid_from: Utc::now(),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    let bob = Entity {
        id: Uuid::new_v4(),
        name: "Bob".to_string(),
        entity_type: EntityType::Person,
        summary: "Manager".to_string(),
        embedding: embedder.embed("Bob").await?,
        valid_from: Utc::now(),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };

    env.repo.upsert_entity(&alice).await?;
    env.repo.upsert_entity(&acme).await?;
    env.repo.upsert_entity(&bob).await?;

    let rel1 = Relation {
        id: Uuid::new_v4(),
        from_entity_id: alice.id,
        to_entity_id: acme.id,
        relation_type: RelationType::WorksAt,
        confidence: 90,
        valid_from: Utc::now(),
        valid_until: None,
    };
    let rel2 = Relation {
        id: Uuid::new_v4(),
        from_entity_id: alice.id,
        to_entity_id: bob.id,
        relation_type: RelationType::Knows,
        confidence: 80,
        valid_from: Utc::now(),
        valid_until: None,
    };
    env.repo.create_relation(&rel1).await?;
    env.repo.create_relation(&rel2).await?;

    let before = env.repo.get_relations_for_entity(alice.id).await?;
    assert_eq!(before.len(), 2, "Alice should have 2 relations before invalidation");

    let count = env.repo.invalidate_relations_for_entity(alice.id).await?;
    assert_eq!(count, 2, "Should invalidate 2 relations");

    let after = env.repo.get_relations_for_entity(alice.id).await?;
    assert!(after.is_empty(), "Alice should have no active relations after invalidation");

    let bob_rels = env.repo.get_relations_for_entity(bob.id).await?;
    assert!(bob_rels.is_empty(), "Bob's relation to Alice should also be invalidated");

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

// ── Relation dedup with canonical types ─────────────────────────────────
// Different raw predicates that canonicalize to the same type should merge.

#[tokio::test]
async fn test_relation_dedup_canonical_types() -> Result<()> {
    let env = TestEnv::new().await?;
    let now = Utc::now();
    let embedding = env.embedder.embed("dedup_test").await?;

    let alice = Entity {
        id: Uuid::new_v4(),
        name: "Alice".to_string(),
        entity_type: EntityType::Person,
        summary: "Engineer".to_string(),
        embedding: embedding.clone(),
        valid_from: now,
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    let acme = Entity {
        id: Uuid::new_v4(),
        name: "Acme".to_string(),
        entity_type: EntityType::Organization,
        summary: "Company".to_string(),
        embedding: embedding.clone(),
        valid_from: now,
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    env.repo.upsert_entity(&alice).await?;
    env.repo.upsert_entity(&acme).await?;

    let rel1 = Relation {
        id: Uuid::new_v4(),
        from_entity_id: alice.id,
        to_entity_id: acme.id,
        relation_type: RelationType::from("works_at"),
        confidence: 80,
        valid_from: now,
        valid_until: None,
    };
    let created = env.repo.create_relation(&rel1).await?;
    assert!(created, "first relation should be created");

    // "employed_at" canonicalizes to WorksAt — should deduplicate
    let rel2 = Relation {
        id: Uuid::new_v4(),
        from_entity_id: alice.id,
        to_entity_id: acme.id,
        relation_type: RelationType::from("employed_at"),
        confidence: 90,
        valid_from: now,
        valid_until: None,
    };
    let created = env.repo.create_relation(&rel2).await?;
    assert!(!created, "duplicate canonical type should merge, not create");

    let rels = env.repo.get_relations_for_entity(alice.id).await?;
    assert_eq!(rels.len(), 1, "only one relation should exist after dedup");
    assert_eq!(rels[0].confidence, 85, "confidence should be averaged");

    Ok(())
}

// ── Symmetric relation dedup ────────────────────────────────────────────

#[tokio::test]
async fn test_symmetric_relation_dedup() -> Result<()> {
    let env = TestEnv::new().await?;
    let now = Utc::now();
    let embedding = env.embedder.embed("sym_test").await?;

    let alice = Entity {
        id: Uuid::new_v4(),
        name: "Alice".to_string(),
        entity_type: EntityType::Person,
        summary: "Person A".to_string(),
        embedding: embedding.clone(),
        valid_from: now,
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    let bob = Entity {
        id: Uuid::new_v4(),
        name: "Bob".to_string(),
        entity_type: EntityType::Person,
        summary: "Person B".to_string(),
        embedding: embedding.clone(),
        valid_from: now,
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    env.repo.upsert_entity(&alice).await?;
    env.repo.upsert_entity(&bob).await?;

    let rel_fwd = Relation {
        id: Uuid::new_v4(),
        from_entity_id: alice.id,
        to_entity_id: bob.id,
        relation_type: RelationType::Knows,
        confidence: 70,
        valid_from: now,
        valid_until: None,
    };
    let created = env.repo.create_relation(&rel_fwd).await?;
    assert!(created, "forward relation should be created");

    // Reverse direction of a symmetric type should deduplicate
    let rel_rev = Relation {
        id: Uuid::new_v4(),
        from_entity_id: bob.id,
        to_entity_id: alice.id,
        relation_type: RelationType::Knows,
        confidence: 90,
        valid_from: now,
        valid_until: None,
    };
    let created = env.repo.create_relation(&rel_rev).await?;
    assert!(!created, "reverse symmetric relation should merge");

    let rels = env.repo.get_relations_for_entity(alice.id).await?;
    assert_eq!(rels.len(), 1, "only one relation should exist for symmetric dedup");
    assert_eq!(rels[0].confidence, 80, "confidence should be averaged");

    Ok(())
}
