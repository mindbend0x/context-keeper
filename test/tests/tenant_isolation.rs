use anyhow::Result;
use chrono::Utc;
use context_keeper_core::models::*;
use context_keeper_surreal::{SurrealConfig, TenantRouter, DEFAULT_TENANT_ID};
use uuid::Uuid;

fn mem_config() -> SurrealConfig {
    SurrealConfig {
        embedding_dimensions: 3,
        ..SurrealConfig::default()
    }
}

fn make_episode(content: &str, namespace: Option<&str>) -> Episode {
    Episode {
        id: Uuid::new_v4(),
        content: content.to_string(),
        source: "test".to_string(),
        session_id: None,
        agent: None,
        namespace: namespace.map(String::from),
        created_at: Utc::now(),
    }
}

fn make_entity(name: &str, namespace: Option<&str>) -> Entity {
    Entity {
        id: Uuid::new_v4(),
        name: name.to_string(),
        entity_type: EntityType::Concept,
        summary: format!("{name} summary"),
        embedding: vec![0.1, 0.2, 0.3],
        valid_from: Utc::now(),
        valid_until: None,
        namespace: namespace.map(String::from),
        created_by_agent: None,
    }
}

// ── Tenant Isolation ────────────────────────────────────────────────────

#[tokio::test]
async fn tenant_entity_isolation() -> Result<()> {
    let router = TenantRouter::new(mem_config(), 10);
    let repo_a = router.get_or_create("tenant_a").await?;
    let repo_b = router.get_or_create("tenant_b").await?;

    let entity = make_entity("SecretProject", None);
    repo_a.upsert_entity(&entity).await?;

    let found_a = repo_a
        .find_entities_by_name("SecretProject", None, None)
        .await?;
    assert_eq!(found_a.len(), 1, "tenant_a should see its own entity");

    let found_b = repo_b
        .find_entities_by_name("SecretProject", None, None)
        .await?;
    assert!(
        found_b.is_empty(),
        "tenant_b must NOT see tenant_a's entity"
    );

    Ok(())
}

#[tokio::test]
async fn tenant_episode_isolation() -> Result<()> {
    let router = TenantRouter::new(mem_config(), 10);
    let repo_a = router.get_or_create("tenant_a").await?;
    let repo_b = router.get_or_create("tenant_b").await?;

    let ep = make_episode("classified intel", None);
    let ep_id = ep.id;
    repo_a.create_episode(&ep).await?;

    assert!(
        repo_a.get_episode(ep_id).await?.is_some(),
        "tenant_a should see its own episode"
    );
    assert!(
        repo_b.get_episode(ep_id).await?.is_none(),
        "tenant_b must NOT see tenant_a's episode"
    );

    Ok(())
}

#[tokio::test]
async fn tenant_memory_isolation() -> Result<()> {
    let router = TenantRouter::new(mem_config(), 10);
    let repo_a = router.get_or_create("tenant_a").await?;
    let repo_b = router.get_or_create("tenant_b").await?;

    let ep = make_episode("mem source", None);
    repo_a.create_episode(&ep).await?;
    let mem = Memory {
        id: Uuid::new_v4(),
        content: "top secret memory".to_string(),
        embedding: vec![0.5, 0.5, 0.5],
        source_episode_id: ep.id,
        entity_ids: vec![],
        created_at: Utc::now(),
        namespace: None,
        created_by_agent: None,
    };
    repo_a.create_memory(&mem).await?;

    let a_mems = repo_a.list_recent_memories(10).await?;
    assert_eq!(a_mems.len(), 1);

    let b_mems = repo_b.list_recent_memories(10).await?;
    assert!(
        b_mems.is_empty(),
        "tenant_b must NOT see tenant_a's memories"
    );

    Ok(())
}

// ── Default Tenant Backward Compatibility ────────────────────────────────

#[tokio::test]
async fn default_tenant_uses_main_database() -> Result<()> {
    let router = TenantRouter::new(mem_config(), 10);
    let repo = router.get_or_create(DEFAULT_TENANT_ID).await?;

    let entity = make_entity("BackwardCompatEntity", None);
    repo.upsert_entity(&entity).await?;

    let found = repo
        .find_entities_by_name("BackwardCompatEntity", None, None)
        .await?;
    assert_eq!(found.len(), 1);

    Ok(())
}

#[tokio::test]
async fn default_tenant_idempotent_creation() -> Result<()> {
    let router = TenantRouter::new(mem_config(), 10);
    let r1 = router.get_or_create(DEFAULT_TENANT_ID).await?;
    let r2 = router.get_or_create(DEFAULT_TENANT_ID).await?;

    let entity = make_entity("SharedEntity", None);
    r1.upsert_entity(&entity).await?;

    let found = r2.find_entities_by_name("SharedEntity", None, None).await?;
    assert_eq!(found.len(), 1, "same tenant repo should share state");

    assert_eq!(router.tenant_count(), 1, "only one tenant should exist");

    Ok(())
}

// ── Tenant Limit ────────────────────────────────────────────────────────

#[tokio::test]
async fn tenant_limit_enforced() -> Result<()> {
    let router = TenantRouter::new(mem_config(), 2);
    router.get_or_create("t1").await?;
    router.get_or_create("t2").await?;

    let result = router.get_or_create("t3").await;
    assert!(result.is_err(), "should reject when limit reached");

    // Existing tenants still work
    router.get_or_create("t1").await?;
    router.get_or_create("t2").await?;

    Ok(())
}

// ── Concurrent Tenant Access ────────────────────────────────────────────

#[tokio::test]
async fn concurrent_tenant_creation_no_cross_contamination() -> Result<()> {
    use std::sync::Arc;
    let router = Arc::new(TenantRouter::new(mem_config(), 100));

    let mut handles = Vec::new();
    for i in 0..10 {
        let r = router.clone();
        handles.push(tokio::spawn(async move {
            let tenant_id = format!("concurrent_{}", i);
            let repo = r.get_or_create(&tenant_id).await.unwrap();

            let entity = make_entity(&format!("Entity_{}", i), None);
            repo.upsert_entity(&entity).await.unwrap();

            let found = repo
                .find_entities_by_name(&format!("Entity_{}", i), None, None)
                .await
                .unwrap();
            assert_eq!(found.len(), 1);

            // Verify no other tenant's data is visible
            for j in 0..10 {
                if j != i {
                    let others = repo
                        .find_entities_by_name(&format!("Entity_{}", j), None, None)
                        .await
                        .unwrap();
                    assert!(
                        others.is_empty(),
                        "tenant {} should not see entity from tenant {}",
                        i,
                        j
                    );
                }
            }
        }));
    }

    for h in handles {
        h.await?;
    }

    assert_eq!(router.tenant_count(), 10);

    Ok(())
}
