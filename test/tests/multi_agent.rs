use std::collections::HashSet;

use anyhow::Result;
use context_keeper_test::harness::TestEnv;

// ── Namespace isolation ────────────────────────────────────────────────────

#[tokio::test]
async fn test_namespace_isolation() -> Result<()> {
    let env = TestEnv::new().await?;

    env.ingest_as_agent("Alice works at Acme Corp", "chat", "agent-a", Some("project-alpha"))
        .await?;
    env.ingest_as_agent("Bob works at BigCo Inc", "chat", "agent-b", Some("project-beta"))
        .await?;

    let alpha_entities = env
        .repo
        .get_all_active_entities_in_namespace(Some("project-alpha"))
        .await?;
    let alpha_names: HashSet<String> = alpha_entities.iter().map(|e| e.name.clone()).collect();

    let beta_entities = env
        .repo
        .get_all_active_entities_in_namespace(Some("project-beta"))
        .await?;
    let beta_names: HashSet<String> = beta_entities.iter().map(|e| e.name.clone()).collect();

    assert!(
        alpha_names.contains("Alice"),
        "Alice should be in project-alpha"
    );
    assert!(
        !alpha_names.contains("Bob"),
        "Bob should NOT be in project-alpha"
    );

    assert!(
        beta_names.contains("Bob"),
        "Bob should be in project-beta"
    );
    assert!(
        !beta_names.contains("Alice"),
        "Alice should NOT be in project-beta"
    );

    Ok(())
}

// ── Cross-namespace global search ──────────────────────────────────────────

#[tokio::test]
async fn test_cross_namespace_global_search() -> Result<()> {
    let env = TestEnv::new().await?;

    env.ingest_as_agent("Alice works at Acme Corp", "chat", "agent-a", Some("ns-1"))
        .await?;
    env.ingest_as_agent("Bob works at Acme Corp", "chat", "agent-b", Some("ns-2"))
        .await?;

    let all_entities = env.repo.get_all_active_entities().await?;
    let all_names: HashSet<String> = all_entities.iter().map(|e| e.name.clone()).collect();

    assert!(
        all_names.contains("Alice"),
        "Global search should find Alice"
    );
    assert!(
        all_names.contains("Bob"),
        "Global search should find Bob"
    );
    assert!(
        all_names.contains("Acme"),
        "Global search should find Acme from both namespaces"
    );

    Ok(())
}

// ── Agent provenance tracking ──────────────────────────────────────────────

#[tokio::test]
async fn test_agent_provenance() -> Result<()> {
    let env = TestEnv::new().await?;

    env.ingest_as_agent("Alice works at Acme", "chat", "cursor-agent", Some("dev"))
        .await?;
    env.ingest_as_agent("Bob works at BigCo", "notes", "claude-agent", Some("dev"))
        .await?;

    let episodes = env.repo.list_episodes_by_agent("cursor-agent", 10).await?;
    assert_eq!(episodes.len(), 1, "cursor-agent should have 1 episode");
    assert!(
        episodes[0].content.contains("Alice"),
        "cursor-agent's episode should contain Alice"
    );

    let claude_episodes = env.repo.list_episodes_by_agent("claude-agent", 10).await?;
    assert_eq!(claude_episodes.len(), 1, "claude-agent should have 1 episode");
    assert!(
        claude_episodes[0].content.contains("Bob"),
        "claude-agent's episode should contain Bob"
    );

    Ok(())
}

// ── Entity namespace propagation ───────────────────────────────────────────

#[tokio::test]
async fn test_entity_namespace_propagation() -> Result<()> {
    let env = TestEnv::new().await?;

    env.ingest_as_agent("Alice is the CTO of Acme", "test", "agent-1", Some("work"))
        .await?;

    let entities = env
        .repo
        .get_all_active_entities_in_namespace(Some("work"))
        .await?;

    for entity in &entities {
        assert_eq!(
            entity.namespace.as_deref(),
            Some("work"),
            "Entity '{}' should inherit the 'work' namespace",
            entity.name
        );
        assert_eq!(
            entity.created_by_agent.as_deref(),
            Some("agent-1"),
            "Entity '{}' should track agent-1 as creator",
            entity.name
        );
    }

    Ok(())
}

// ── Namespace-scoped name search ───────────────────────────────────────────
// Entity identity is (name, entity_type). When the same entity (name + type)
// is ingested from two namespaces, the resolver finds the existing entity
// globally and updates it rather than creating a duplicate.

#[tokio::test]
async fn test_namespace_scoped_name_search() -> Result<()> {
    let env = TestEnv::new().await?;

    env.ingest_as_agent("Alice is a developer", "test", "agent-a", Some("team-a"))
        .await?;
    env.ingest_as_agent("Alice is a manager", "test", "agent-b", Some("team-b"))
        .await?;

    let global = env.repo.find_entities_by_name("Alice", None).await?;
    assert_eq!(
        global.len(),
        1,
        "Alice should be a single global entity under composite identity, got {}",
        global.len()
    );

    let team_a = env
        .repo
        .find_entities_by_name("Alice", Some("team-a"))
        .await?;
    assert_eq!(
        team_a.len(),
        1,
        "Alice should remain in its original namespace (team-a)"
    );

    Ok(())
}

// ── Mixed namespaced and global data ───────────────────────────────────────

#[tokio::test]
async fn test_mixed_namespaced_and_global() -> Result<()> {
    let env = TestEnv::new().await?;

    env.ingest_text("GlobalEntity exists everywhere", "test").await?;
    env.ingest_as_agent("ScopedEntity is in a namespace", "test", "agent-1", Some("scoped"))
        .await?;

    let global = env.repo.get_all_active_entities().await?;
    let names: HashSet<String> = global.iter().map(|e| e.name.clone()).collect();
    assert!(names.len() >= 2, "Should have entities from both global and scoped");

    let scoped = env
        .repo
        .get_all_active_entities_in_namespace(Some("scoped"))
        .await?;
    let scoped_names: HashSet<String> = scoped.iter().map(|e| e.name.clone()).collect();

    assert!(
        !scoped_names.contains("GlobalEntity"),
        "GlobalEntity should not appear in 'scoped' namespace filter"
    );

    Ok(())
}
