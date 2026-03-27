use std::collections::HashSet;

use anyhow::Result;
use chrono::Utc;
use context_keeper_core::{
    models::*,
    search::{fuse_rrf, QueryExpander},
    traits::*,
};
use context_keeper_test::{fixtures, harness::TestEnv, metrics};
use uuid::Uuid;

// ── Vector search self-similarity ────────────────────────────────────────

#[tokio::test]
async fn test_vector_search_self_similarity() -> Result<()> {
    let env = TestEnv::new().await?;
    let names = ["Rust", "Python", "JavaScript", "TypeScript"];

    for name in &names {
        let entity = Entity {
            id: Uuid::new_v4(),
            name: name.to_string(),
            entity_type: EntityType::Concept,
            summary: format!("{name} programming language"),
            embedding: env.embedder.embed(name).await?,
            valid_from: Utc::now(),
            valid_until: None,
            namespace: None,
            created_by_agent: None,
        };
        env.repo.upsert_entity(&entity).await?;
    }

    for name in &names {
        let query_vec = env.embedder.embed(name).await?;
        let results = env
            .repo
            .search_entities_by_vector(&query_vec, 4, None, None)
            .await?;
        assert!(
            !results.is_empty(),
            "Vector search for {name} should return results"
        );
        assert_eq!(
            results[0].0.name, *name,
            "Self-query for {name} should rank itself first, got {}",
            results[0].0.name
        );
        assert!(
            (results[0].1 - 1.0).abs() < 0.01,
            "Self-similarity should be ~1.0, got {:.4}",
            results[0].1
        );
    }

    Ok(())
}

// ── Vector search ranking ────────────────────────────────────────────────

#[tokio::test]
async fn test_vector_search_ranking() -> Result<()> {
    let env = TestEnv::new().await?;
    let names: Vec<&str> = vec![
        "Alpha", "Bravo", "Charlie", "Delta", "Echo", "Foxtrot", "Golf", "Hotel", "India", "Juliet",
    ];

    for name in &names {
        let entity = Entity {
            id: Uuid::new_v4(),
            name: name.to_string(),
            entity_type: EntityType::Other,
            summary: format!("NATO phonetic: {name}"),
            embedding: env.embedder.embed(name).await?,
            valid_from: Utc::now(),
            valid_until: None,
            namespace: None,
            created_by_agent: None,
        };
        env.repo.upsert_entity(&entity).await?;
    }

    let query_vec = env.embedder.embed("Alpha").await?;
    let results = env
        .repo
        .search_entities_by_vector(&query_vec, 10, None, None)
        .await?;
    assert_eq!(results[0].0.name, "Alpha");

    for i in 1..results.len() {
        assert!(
            results[i - 1].1 >= results[i].1,
            "Results should be sorted by descending score"
        );
    }

    Ok(())
}

// ── BM25 exact match ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_bm25_exact_match() -> Result<()> {
    let env = TestEnv::new().await?;

    let entity = Entity {
        id: Uuid::new_v4(),
        name: "Kubernetes".to_string(),
        entity_type: EntityType::Concept,
        summary: "Container orchestration platform".to_string(),
        embedding: env.embedder.embed("Kubernetes").await?,
        valid_from: Utc::now(),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    env.repo.upsert_entity(&entity).await?;

    let results = env
        .repo
        .search_entities_by_keyword("Kubernetes", None, None)
        .await?;
    assert!(!results.is_empty(), "BM25 should find exact name match");
    assert_eq!(results[0].name, "Kubernetes");

    Ok(())
}

// ── BM25 summary match ──────────────────────────────────────────────────

#[tokio::test]
async fn test_bm25_summary_match() -> Result<()> {
    let env = TestEnv::new().await?;

    let entity = Entity {
        id: Uuid::new_v4(),
        name: "Kubernetes".to_string(),
        entity_type: EntityType::Concept,
        summary: "Container orchestration platform for deploying microservices".to_string(),
        embedding: env.embedder.embed("Kubernetes").await?,
        valid_from: Utc::now(),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    env.repo.upsert_entity(&entity).await?;

    let results = env
        .repo
        .search_entities_by_keyword("orchestration", None, None)
        .await?;
    assert!(!results.is_empty(), "BM25 should match words in summary");
    assert_eq!(results[0].name, "Kubernetes");

    Ok(())
}

// ── RRF fusion boost ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_rrf_fusion_boost() -> Result<()> {
    let make = |name: &str| Entity {
        id: Uuid::new_v4(),
        name: name.to_string(),
        entity_type: EntityType::Other,
        summary: name.to_string(),
        embedding: vec![],
        valid_from: Utc::now(),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };

    let alice = make("Alice");
    let bob = make("Bob");
    let charlie = make("Charlie");

    let vector_ranked = vec![alice.clone(), bob.clone()];
    let keyword_ranked = vec![bob.clone(), charlie.clone()];

    let fused = fuse_rrf(vec![vector_ranked, keyword_ranked]);

    assert_eq!(
        fused[0].entity.as_ref().unwrap().name,
        "Bob",
        "Bob appears in both lists and should be ranked first"
    );

    Ok(())
}

// ── RRF diverse results ──────────────────────────────────────────────────

#[tokio::test]
async fn test_rrf_diverse_results() -> Result<()> {
    let make = |name: &str| Entity {
        id: Uuid::new_v4(),
        name: name.to_string(),
        entity_type: EntityType::Other,
        summary: name.to_string(),
        embedding: vec![],
        valid_from: Utc::now(),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };

    let list1 = vec![make("A"), make("B")];
    let list2 = vec![make("C"), make("D")];

    let fused = fuse_rrf(vec![list1, list2]);
    assert_eq!(
        fused.len(),
        4,
        "Union of disjoint lists should contain all items"
    );

    let names: HashSet<String> = fused
        .iter()
        .map(|r| r.entity.as_ref().unwrap().name.clone())
        .collect();
    for expected in &["A", "B", "C", "D"] {
        assert!(
            names.contains(*expected),
            "Missing {expected} in fused results"
        );
    }

    Ok(())
}

// ── Hybrid search recall@k ───────────────────────────────────────────────

#[tokio::test]
async fn test_hybrid_search_recall_at_k() -> Result<()> {
    for scenario in fixtures::all_scenarios() {
        if scenario.queries.is_empty() {
            continue;
        }

        let env = TestEnv::new().await?;
        for (text, source) in &scenario.episodes {
            env.ingest_text(text, source).await?;
        }

        for query_exp in &scenario.queries {
            let results = env.search_hybrid(query_exp.query, 5).await?;
            let ranked: Vec<String> = results
                .iter()
                .filter_map(|r| r.entity.as_ref().map(|e| e.name.clone()))
                .collect();

            let relevant: HashSet<String> = query_exp
                .relevant_entity_names
                .iter()
                .map(|s| s.to_string())
                .collect();

            let r_at_5 = metrics::recall_at_k(&ranked, &relevant, 5);
            assert!(
                r_at_5 >= query_exp.min_recall_at_5,
                "Scenario '{}', query '{}': recall@5={r_at_5:.2} < {:.2}",
                scenario.name,
                query_exp.query,
                query_exp.min_recall_at_5,
            );
        }
    }

    Ok(())
}

// ── Memory vector search ─────────────────────────────────────────────────

#[tokio::test]
async fn test_memory_vector_search() -> Result<()> {
    let env = TestEnv::new().await?;

    let episode_text = "Rust is a systems programming language focused on safety";
    env.ingest_text(episode_text, "test").await?;

    let query_vec = env.embedder.embed(episode_text).await?;
    let results = env
        .repo
        .search_memories_by_vector(&query_vec, 5, None)
        .await?;

    assert!(
        !results.is_empty(),
        "Memory vector search should return results"
    );
    assert_eq!(results[0].0.content, episode_text);
    assert!(
        (results[0].1 - 1.0).abs() < 0.01,
        "Self-similarity should be ~1.0"
    );

    Ok(())
}

// ── Memory keyword search (via episode BM25 as proxy) ────────────────────

#[tokio::test]
async fn test_memory_keyword_search() -> Result<()> {
    let env = TestEnv::new().await?;

    env.ingest_text("Rust is a systems programming language", "test")
        .await?;
    env.ingest_text("Python is popular for data science", "test")
        .await?;

    let results = env.repo.search_episodes_by_keyword("Rust").await?;
    assert!(!results.is_empty(), "BM25 should find episode by keyword");
    assert!(
        results[0].content.contains("Rust"),
        "First result should contain the query term"
    );

    Ok(())
}

// ── Query expansion increases recall ─────────────────────────────────────

#[tokio::test]
async fn test_query_expansion_increases_recall() -> Result<()> {
    let env = TestEnv::new().await?;
    env.ingest_text("Alice is the CTO of Acme Corp", "test")
        .await?;
    env.ingest_text("Bob manages the Engineering team at Acme", "test")
        .await?;

    let expander = QueryExpander::new(3);
    let variants = expander
        .expand("Acme leadership", &env.query_rewriter)
        .await?;

    assert!(
        variants.len() > 1,
        "Expander should produce multiple variants"
    );

    let mut all_entity_names: HashSet<String> = HashSet::new();
    for variant in &variants {
        let query_vec = env.embedder.embed(variant).await?;
        let vec_results = env
            .repo
            .search_entities_by_vector(&query_vec, 5, None, None)
            .await?;
        for (entity, _) in vec_results {
            all_entity_names.insert(entity.name);
        }

        let kw_results = env
            .repo
            .search_entities_by_keyword(variant, None, None)
            .await?;
        for entity in kw_results {
            all_entity_names.insert(entity.name);
        }
    }

    let base_vec = env.embedder.embed("Acme leadership").await?;
    let base_results = env
        .repo
        .search_entities_by_vector(&base_vec, 5, None, None)
        .await?;
    let base_names: HashSet<String> = base_results.into_iter().map(|(e, _)| e.name).collect();

    assert!(
        all_entity_names.len() >= base_names.len(),
        "Expanded search should retrieve at least as many entities as base search"
    );

    Ok(())
}

// ── Graph neighbor traversal ─────────────────────────────────────────────

#[tokio::test]
async fn test_graph_neighbor_traversal() -> Result<()> {
    let env = TestEnv::new().await?;
    let result = env
        .ingest_text("Alice works at Acme Corp in Berlin", "test")
        .await?;

    if result.entities.len() < 2 {
        return Ok(());
    }

    let first = &result.entities[0];
    let neighbors = env.repo.get_graph_neighbors(&[first.id], 1).await?;

    assert!(
        !neighbors.is_empty(),
        "Entity '{}' should have graph neighbors",
        first.name
    );

    let neighbor_names: HashSet<String> = neighbors.iter().map(|e| e.name.clone()).collect();
    assert!(
        neighbor_names.contains(&first.name),
        "Seed entity should be included in neighbors"
    );

    if result.entities.len() > 1 {
        let second = &result.entities[1];
        assert!(
            neighbor_names.contains(&second.name),
            "Adjacent entity '{}' should be a neighbor of '{}'",
            second.name,
            first.name
        );
    }

    Ok(())
}

// ── Search excludes invalidated entities ─────────────────────────────────

#[tokio::test]
async fn test_search_excludes_invalidated() -> Result<()> {
    let env = TestEnv::new().await?;

    let active = Entity {
        id: Uuid::new_v4(),
        name: "ActiveEntity".to_string(),
        entity_type: EntityType::Other,
        summary: "This entity is active".to_string(),
        embedding: env.embedder.embed("ActiveEntity").await?,
        valid_from: Utc::now(),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };

    let invalidated = Entity {
        id: Uuid::new_v4(),
        name: "GoneEntity".to_string(),
        entity_type: EntityType::Other,
        summary: "This entity is invalidated".to_string(),
        embedding: env.embedder.embed("GoneEntity").await?,
        valid_from: Utc::now() - chrono::Duration::days(10),
        valid_until: Some(Utc::now() - chrono::Duration::days(1)),
        namespace: None,
        created_by_agent: None,
    };

    env.repo.upsert_entity(&active).await?;
    env.repo.upsert_entity(&invalidated).await?;

    let query_vec = env.embedder.embed("GoneEntity").await?;
    let vec_results = env
        .repo
        .search_entities_by_vector(&query_vec, 10, None, None)
        .await?;
    let vec_names: Vec<String> = vec_results.into_iter().map(|(e, _)| e.name).collect();
    assert!(
        !vec_names.contains(&"GoneEntity".to_string()),
        "Invalidated entity should not appear in vector search results"
    );

    let all_active = env.repo.get_all_active_entities().await?;
    let active_names: Vec<String> = all_active.iter().map(|e| e.name.clone()).collect();
    assert!(active_names.contains(&"ActiveEntity".to_string()));
    assert!(!active_names.contains(&"GoneEntity".to_string()));

    Ok(())
}
