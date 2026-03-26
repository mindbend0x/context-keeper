use anyhow::Result;
use chrono::{Duration, Utc};
use context_keeper_core::{models::*, temporal::staleness_score, traits::Embedder};
use context_keeper_test::harness::TestEnv;
use uuid::Uuid;

// ── Snapshot before creation ─────────────────────────────────────────────

#[tokio::test]
async fn test_snapshot_before_creation() -> Result<()> {
    let env = TestEnv::new().await?;
    let before = Utc::now() - Duration::hours(2);

    let entity = Entity {
        id: Uuid::new_v4(),
        name: "FutureEntity".to_string(),
        entity_type: EntityType::Other,
        summary: "Created after the snapshot time".to_string(),
        embedding: env.embedder.embed("FutureEntity").await?,
        valid_from: Utc::now(),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    env.repo.upsert_entity(&entity).await?;

    let snapshot = env.repo.entities_at(before).await?;
    assert!(
        snapshot.is_empty(),
        "Snapshot before entity creation should be empty, got {} entities",
        snapshot.len()
    );

    Ok(())
}

// ── Snapshot at creation ─────────────────────────────────────────────────

#[tokio::test]
async fn test_snapshot_at_creation() -> Result<()> {
    let env = TestEnv::new().await?;
    let created = Utc::now() - Duration::hours(1);

    let entity = Entity {
        id: Uuid::new_v4(),
        name: "PastEntity".to_string(),
        entity_type: EntityType::Other,
        summary: "Created in the past".to_string(),
        embedding: env.embedder.embed("PastEntity").await?,
        valid_from: created,
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    env.repo.upsert_entity(&entity).await?;

    let snapshot = env.repo.entities_at(Utc::now()).await?;
    assert_eq!(snapshot.len(), 1);
    assert_eq!(snapshot[0].name, "PastEntity");

    let snapshot_at = env.repo.entities_at(created + Duration::minutes(1)).await?;
    assert_eq!(snapshot_at.len(), 1);

    Ok(())
}

// ── Snapshot after invalidation ──────────────────────────────────────────

#[tokio::test]
async fn test_snapshot_after_invalidation() -> Result<()> {
    let env = TestEnv::new().await?;

    let entity = Entity {
        id: Uuid::new_v4(),
        name: "Expired".to_string(),
        entity_type: EntityType::Other,
        summary: "This entity has expired".to_string(),
        embedding: env.embedder.embed("Expired").await?,
        valid_from: Utc::now() - Duration::days(10),
        valid_until: Some(Utc::now() - Duration::days(5)),
        namespace: None,
        created_by_agent: None,
    };
    env.repo.upsert_entity(&entity).await?;

    let snapshot_now = env.repo.entities_at(Utc::now()).await?;
    assert!(
        snapshot_now.is_empty(),
        "Expired entity should not appear in current snapshot"
    );

    let snapshot_past = env.repo.entities_at(Utc::now() - Duration::days(7)).await?;
    assert_eq!(
        snapshot_past.len(),
        1,
        "Entity should appear in snapshot during its valid period"
    );
    assert_eq!(snapshot_past[0].name, "Expired");

    Ok(())
}

// ── Temporal entity evolution ────────────────────────────────────────────

#[tokio::test]
async fn test_temporal_entity_evolution() -> Result<()> {
    let env = TestEnv::new().await?;

    let id = Uuid::new_v4();
    let t0 = Utc::now() - Duration::days(20);

    let v1 = Entity {
        id,
        name: "Alice".to_string(),
        entity_type: EntityType::Person,
        summary: "Engineer at Acme".to_string(),
        embedding: env.embedder.embed("Alice").await?,
        valid_from: t0,
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    env.repo.upsert_entity(&v1).await?;

    let snap_early = env.repo.entities_at(t0 + Duration::days(1)).await?;
    let alice_early: Vec<_> = snap_early.iter().filter(|e| e.name == "Alice").collect();
    assert_eq!(alice_early.len(), 1);
    assert_eq!(alice_early[0].summary, "Engineer at Acme");

    let t1 = Utc::now() - Duration::days(5);
    let v2 = Entity {
        id,
        name: "Alice".to_string(),
        entity_type: EntityType::Person,
        summary: "Director at Acme".to_string(),
        embedding: env.embedder.embed("Alice").await?,
        valid_from: t1,
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    env.repo.upsert_entity(&v2).await?;

    let snap_late = env.repo.entities_at(Utc::now()).await?;
    let alice_late: Vec<_> = snap_late.iter().filter(|e| e.name == "Alice").collect();
    assert_eq!(alice_late.len(), 1);
    assert_eq!(alice_late[0].summary, "Director at Acme");

    let snap_between = env.repo.entities_at(t0 + Duration::days(1)).await?;
    assert!(
        snap_between.is_empty(),
        "After upsert moved valid_from forward, entity should not appear at the old time"
    );

    Ok(())
}

// ── Temporal relation lifecycle ──────────────────────────────────────────

#[tokio::test]
async fn test_temporal_relation_lifecycle() -> Result<()> {
    let env = TestEnv::new().await?;

    let alice = Entity {
        id: Uuid::new_v4(),
        name: "Alice".to_string(),
        entity_type: EntityType::Person,
        summary: "".to_string(),
        embedding: env.embedder.embed("Alice").await?,
        valid_from: Utc::now() - Duration::days(30),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    let acme = Entity {
        id: Uuid::new_v4(),
        name: "Acme".to_string(),
        entity_type: EntityType::Organization,
        summary: "".to_string(),
        embedding: env.embedder.embed("Acme").await?,
        valid_from: Utc::now() - Duration::days(30),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    env.repo.upsert_entity(&alice).await?;
    env.repo.upsert_entity(&acme).await?;

    let rel = Relation {
        id: Uuid::new_v4(),
        from_entity_id: alice.id,
        to_entity_id: acme.id,
        relation_type: RelationType::WorksAt,
        confidence: 95,
        valid_from: Utc::now() - Duration::days(30),
        valid_until: None,
    };
    env.repo.create_relation(&rel).await?;

    let before_invalidation = env.repo.relations_at(Utc::now()).await?;
    assert!(
        before_invalidation.iter().any(|r| r.relation_type == RelationType::WorksAt),
        "Relation should exist before invalidation"
    );

    env.repo.invalidate_relation(rel.id).await?;

    let active_rels = env.repo.get_relations_for_entity(alice.id).await?;
    assert!(
        active_rels.is_empty(),
        "No active relations should remain after invalidation"
    );

    Ok(())
}

// ── Staleness ordering ───────────────────────────────────────────────────

#[tokio::test]
async fn test_staleness_ordering() -> Result<()> {
    let entities = vec![
        Entity {
            id: Uuid::new_v4(),
            name: "Ancient".to_string(),
            entity_type: EntityType::Other,
            summary: "".to_string(),
            embedding: vec![],
            valid_from: Utc::now() - Duration::days(100),
            valid_until: None,
            namespace: None,
            created_by_agent: None,
        },
        Entity {
            id: Uuid::new_v4(),
            name: "Old".to_string(),
            entity_type: EntityType::Other,
            summary: "".to_string(),
            embedding: vec![],
            valid_from: Utc::now() - Duration::days(30),
            valid_until: None,
            namespace: None,
            created_by_agent: None,
        },
        Entity {
            id: Uuid::new_v4(),
            name: "Recent".to_string(),
            entity_type: EntityType::Other,
            summary: "".to_string(),
            embedding: vec![],
            valid_from: Utc::now() - Duration::days(1),
            valid_until: None,
            namespace: None,
            created_by_agent: None,
        },
    ];

    let scores: Vec<f64> = entities.iter().map(|e| staleness_score(e)).collect();

    assert!(
        scores[0] > scores[1],
        "Ancient ({:.1}) should be staler than Old ({:.1})",
        scores[0],
        scores[1]
    );
    assert!(
        scores[1] > scores[2],
        "Old ({:.1}) should be staler than Recent ({:.1})",
        scores[1],
        scores[2]
    );

    Ok(())
}

// ── Staleness of fresh entity ────────────────────────────────────────────

#[tokio::test]
async fn test_staleness_fresh_entity() -> Result<()> {
    let entity = Entity {
        id: Uuid::new_v4(),
        name: "Fresh".to_string(),
        entity_type: EntityType::Other,
        summary: "".to_string(),
        embedding: vec![],
        valid_from: Utc::now(),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };

    let score = staleness_score(&entity);
    assert_eq!(score, 0.0, "Entity created now should have staleness 0");

    Ok(())
}
