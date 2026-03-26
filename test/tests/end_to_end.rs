use std::collections::HashSet;

use anyhow::Result;
use context_keeper_core::traits::Embedder;
use context_keeper_test::{
    fixtures,
    harness::TestEnv,
    metrics::{self, f1},
};

// ── Conversation memory ──────────────────────────────────────────────────

#[tokio::test]
async fn test_conversation_memory() -> Result<()> {
    let env = TestEnv::new().await?;

    let episodes = [
        ("We discussed the project timeline today", "chat"),
        ("Alice mentioned the Kubernetes migration plan", "chat"),
        ("Bob raised concerns about the database performance", "chat"),
        ("The team decided to use SurrealDB for the new service", "chat"),
        ("Final decision was to deploy on Friday", "chat"),
    ];

    for (text, source) in &episodes {
        env.ingest_text(text, source).await?;
    }

    let results = env.search_hybrid("Kubernetes", 5).await?;
    assert!(
        !results.is_empty(),
        "Should find results related to Kubernetes"
    );

    let top_names: Vec<String> = results
        .iter()
        .filter_map(|r| r.entity.as_ref().map(|e| e.name.clone()))
        .collect();
    assert!(
        top_names.contains(&"Kubernetes".to_string()),
        "Kubernetes should appear in search results, got: {top_names:?}"
    );

    let memories = env.repo.list_recent_memories(10).await?;
    assert_eq!(memories.len(), 5, "Should have 5 memories from 5 episodes");

    let mem_vec = env.embedder.embed("Kubernetes").await?;
    let mem_results = env.repo.search_memories_by_vector(&mem_vec, 5, None).await?;
    assert!(
        !mem_results.is_empty(),
        "Should find memories related to Kubernetes via vector search"
    );

    Ok(())
}

// ── Knowledge graph construction ─────────────────────────────────────────

#[tokio::test]
async fn test_knowledge_graph_construction() -> Result<()> {
    let env = TestEnv::new().await?;

    env.ingest_text("Alice is the CTO of Acme Corp", "test").await?;
    env.ingest_text("Bob is the CEO of Acme Corp", "test").await?;
    env.ingest_text("Acme Corp is headquartered in Berlin", "test").await?;

    let entities = env.repo.get_all_active_entities().await?;
    let entity_names: HashSet<String> = entities.iter().map(|e| e.name.clone()).collect();

    assert!(entity_names.contains("Alice"), "Alice should be in the graph");
    assert!(entity_names.contains("Bob"), "Bob should be in the graph");
    assert!(entity_names.contains("Acme"), "Acme should be in the graph");

    let acme = entities.iter().find(|e| e.name == "Acme");
    if let Some(acme_entity) = acme {
        let neighbors = env
            .repo
            .get_graph_neighbors(&[acme_entity.id], 1)
            .await?;
        assert!(
            neighbors.len() > 1,
            "Acme should have graph neighbors, got {}",
            neighbors.len()
        );
    }

    Ok(())
}

// ── Incremental knowledge ────────────────────────────────────────────────

#[tokio::test]
async fn test_incremental_knowledge() -> Result<()> {
    let env = TestEnv::new().await?;

    env.ingest_text("Alice works at Acme Corp as an Engineer", "chat").await?;
    env.ingest_text("Alice left Acme and joined BigCo as Director", "chat").await?;

    let entities = env.repo.get_all_active_entities().await?;
    let names: HashSet<String> = entities.iter().map(|e| e.name.clone()).collect();

    assert!(names.contains("Alice"));
    assert!(names.contains("Acme"), "Acme should still exist from first episode");
    assert!(names.contains("BigCo"), "BigCo should exist from second episode");

    let memories = env.repo.list_recent_memories(10).await?;
    assert_eq!(memories.len(), 2, "Should have 2 memories from 2 episodes");

    let search_results = env.search_hybrid("Alice", 5).await?;
    assert!(
        !search_results.is_empty(),
        "Should find Alice in search results"
    );

    Ok(())
}

// ── Cross-episode entity linking ─────────────────────────────────────────

#[tokio::test]
async fn test_cross_episode_entity_linking() -> Result<()> {
    let env = TestEnv::new().await?;

    env.ingest_text_with_resolver("Rust is a systems programming language", "docs", true).await?;
    env.ingest_text_with_resolver("Rust has excellent memory safety guarantees", "docs", true).await?;

    let entities = env.repo.get_all_active_entities().await?;
    let rust_entities: Vec<_> = entities.iter().filter(|e| e.name == "Rust").collect();

    assert_eq!(
        rust_entities.len(),
        1,
        "Rust should appear exactly once via upsert dedup, got {}",
        rust_entities.len()
    );

    Ok(())
}

// ── Search after bulk ingest ─────────────────────────────────────────────

#[tokio::test]
async fn test_search_after_bulk_ingest() -> Result<()> {
    let env = TestEnv::new().await?;

    let episodes = [
        "Alice is the CTO of Acme Corp",
        "Bob is a Software Engineer at Google",
        "Charlie studies Computer Science at MIT",
        "Diana works at Amazon on AWS Lambda",
        "Eve is a Security Researcher at Microsoft",
        "Frank founded SpaceX alongside Tesla Motors",
        "Grace developed COBOL at IBM Research",
        "Heidi manages the Kubernetes team at RedHat",
        "Ivan leads the Python development at PSF",
        "Judy designs React components at Meta",
        "Karl runs the Rust compiler team at Mozilla",
        "Liam builds Docker images for Netflix",
        "Mia optimizes PostgreSQL queries at Supabase",
        "Noah deploys Terraform configurations at HashiCorp",
        "Olivia maintains Linux kernel modules at Canonical",
        "Pablo develops Android apps at Samsung",
        "Quinn writes Swift code at Apple",
        "Rosa trains ML models at DeepMind",
        "Sam builds blockchain systems at Ethereum Foundation",
        "Tina creates UI designs at Figma headquarters",
    ];

    for text in &episodes {
        env.ingest_text(text, "bulk").await?;
    }

    let queries_and_expected = [
        ("Alice", vec!["Alice"]),
        ("Kubernetes", vec!["Kubernetes"]),
        ("Rust", vec!["Rust"]),
        ("React", vec!["React"]),
        ("Python", vec!["Python"]),
    ];

    let mut total_mrr = 0.0;
    for (query, expected) in &queries_and_expected {
        let results = env.search_hybrid(query, 5).await?;
        let ranked: Vec<String> = results
            .iter()
            .filter_map(|r| r.entity.as_ref().map(|e| e.name.clone()))
            .collect();

        let relevant: HashSet<String> = expected.iter().map(|s| s.to_string()).collect();
        let query_mrr = metrics::mrr(&ranked, &relevant);
        total_mrr += query_mrr;

        eprintln!(
            "  query={:<15} mrr={:.2} top_results={:?}",
            query,
            query_mrr,
            &ranked[..ranked.len().min(3)]
        );
    }

    let avg_mrr = total_mrr / queries_and_expected.len() as f64;
    eprintln!("  Average MRR: {avg_mrr:.2}");

    assert!(
        avg_mrr >= 0.5,
        "Average MRR {avg_mrr:.2} should be >= 0.5 across bulk queries"
    );

    Ok(())
}

// ── Full pipeline metrics report ─────────────────────────────────────────

#[tokio::test]
async fn test_full_pipeline_metrics_report() -> Result<()> {
    let mut all_precision = Vec::new();
    let mut all_recall = Vec::new();
    let mut all_mrr = Vec::new();

    eprintln!("\n{}", "=".repeat(60));
    eprintln!("  FULL PIPELINE EFFECTIVENESS REPORT");
    eprintln!("{}\n", "=".repeat(60));

    for scenario in fixtures::all_scenarios() {
        let env = TestEnv::new().await?;
        let expected = scenario.expected_entity_set();

        let mut extracted_names: Vec<String> = Vec::new();
        for (text, source) in &scenario.episodes {
            let result = env.ingest_text(text, source).await?;
            for entity in &result.entities {
                extracted_names.push(entity.name.clone());
            }
        }

        if !expected.is_empty() {
            let p = metrics::precision(&extracted_names, &expected);
            let r = metrics::recall(&extracted_names, &expected);
            all_precision.push(p);
            all_recall.push(r);

            eprintln!(
                "  [Extraction] {:<25} P={:.2}  R={:.2}  F1={:.2}  entities={}",
                scenario.name,
                p,
                r,
                f1(p, r),
                extracted_names.len()
            );
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

            let query_mrr = metrics::mrr(&ranked, &relevant);
            let r_at_5 = metrics::recall_at_k(&ranked, &relevant, 5);
            all_mrr.push(query_mrr);

            eprintln!(
                "  [Search]     {:<25} query={:<15} MRR={:.2}  R@5={:.2}",
                scenario.name, query_exp.query, query_mrr, r_at_5
            );
        }
    }

    let avg_p = if all_precision.is_empty() {
        0.0
    } else {
        all_precision.iter().sum::<f64>() / all_precision.len() as f64
    };
    let avg_r = if all_recall.is_empty() {
        0.0
    } else {
        all_recall.iter().sum::<f64>() / all_recall.len() as f64
    };
    let avg_mrr = if all_mrr.is_empty() {
        0.0
    } else {
        all_mrr.iter().sum::<f64>() / all_mrr.len() as f64
    };
    let avg_f1 = f1(avg_p, avg_r);

    eprintln!("\n  ─── AGGREGATE ───");
    eprintln!("  Avg Precision:  {avg_p:.2}");
    eprintln!("  Avg Recall:     {avg_r:.2}");
    eprintln!("  Avg F1:         {avg_f1:.2}");
    eprintln!("  Avg MRR:        {avg_mrr:.2}");
    eprintln!();

    assert!(avg_f1 >= 0.5, "Aggregate F1 {avg_f1:.2} below threshold 0.5");
    assert!(avg_mrr >= 0.5, "Aggregate MRR {avg_mrr:.2} below threshold 0.5");

    Ok(())
}
