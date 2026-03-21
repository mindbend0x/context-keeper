use std::collections::HashSet;

use anyhow::Result;
use context_keeper_core::traits::*;
use context_keeper_test::{
    fixtures,
    harness::TestEnv,
    metrics::{self, f1},
};

// ── Entity extraction precision ──────────────────────────────────────────

#[tokio::test]
async fn test_entity_extraction_precision() -> Result<()> {
    let env = TestEnv::new().await?;
    let scenario = fixtures::people_and_orgs();
    let expected = scenario.expected_entity_set();

    let mut extracted_names: Vec<String> = Vec::new();
    for (text, source) in &scenario.episodes {
        let result = env.ingest_text(text, source).await?;
        for entity in &result.entities {
            extracted_names.push(entity.name.clone());
        }
    }

    let p = metrics::precision(&extracted_names, &expected);
    assert!(
        p >= 0.5,
        "Precision {p:.2} is below 0.5 for people_and_orgs scenario"
    );
    Ok(())
}

// ── Entity extraction recall ─────────────────────────────────────────────

#[tokio::test]
async fn test_entity_extraction_recall() -> Result<()> {
    let env = TestEnv::new().await?;
    let scenario = fixtures::technical_domain();
    let expected = scenario.expected_entity_set();

    let mut extracted_names: Vec<String> = Vec::new();
    for (text, source) in &scenario.episodes {
        let result = env.ingest_text(text, source).await?;
        for entity in &result.entities {
            extracted_names.push(entity.name.clone());
        }
    }

    let r = metrics::recall(&extracted_names, &expected);
    assert!(
        r >= 0.5,
        "Recall {r:.2} is below 0.5 for technical_domain scenario"
    );
    Ok(())
}

// ── Entity deduplication ─────────────────────────────────────────────────

#[tokio::test]
async fn test_entity_deduplication() -> Result<()> {
    let env = TestEnv::new().await?;

    env.ingest_text("Alice works at Acme Corp in Berlin", "test").await?;
    env.ingest_text("Alice met Bob at Acme headquarters in Berlin", "test").await?;

    let entities = env.repo.get_all_active_entities().await?;
    let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();

    let unique_names: HashSet<&str> = names.iter().copied().collect();
    assert_eq!(
        names.len(),
        unique_names.len(),
        "Duplicate entities found: {names:?}"
    );

    Ok(())
}

// ── Relation extraction connectivity ─────────────────────────────────────

#[tokio::test]
async fn test_relation_extraction_connectivity() -> Result<()> {
    let env = TestEnv::new().await?;
    let result = env
        .ingest_text("Alice works at Acme Corp in Berlin", "test")
        .await?;

    let n_entities = result.entities.len();
    if n_entities > 1 {
        assert_eq!(
            result.relations.len(),
            n_entities - 1,
            "MockRelationExtractor should produce N-1 relations for N entities"
        );
    }

    for relation in &result.relations {
        let from_exists = result.entities.iter().any(|e| e.id == relation.from_entity_id);
        let to_exists = result.entities.iter().any(|e| e.id == relation.to_entity_id);
        assert!(from_exists, "Relation references non-existent from_entity");
        assert!(to_exists, "Relation references non-existent to_entity");
    }

    Ok(())
}

// ── Memory creation per episode ──────────────────────────────────────────

#[tokio::test]
async fn test_memory_creation_per_episode() -> Result<()> {
    let env = TestEnv::new().await?;

    let r1 = env.ingest_text("Alice works at Acme", "test").await?;
    let r2 = env.ingest_text("Bob lives in Berlin", "test").await?;

    assert_eq!(r1.memories.len(), 1, "Each episode should produce exactly 1 memory");
    assert_eq!(r2.memories.len(), 1, "Each episode should produce exactly 1 memory");

    let memories = env.repo.list_recent_memories(10).await?;
    assert_eq!(memories.len(), 2, "Two ingested episodes should yield 2 memories");

    assert_eq!(r1.memories[0].content, "Alice works at Acme");
    assert_eq!(r2.memories[0].content, "Bob lives in Berlin");

    Ok(())
}

// ── Entity embedding distinctness ────────────────────────────────────────

#[tokio::test]
async fn test_entity_embedding_distinctness() -> Result<()> {
    let embedder = MockEmbedder::new(8);

    let v_alice = embedder.embed("Alice").await?;
    let v_bob = embedder.embed("Bob").await?;
    let v_berlin = embedder.embed("Berlin").await?;

    let cos_ab = cosine_sim(&v_alice, &v_bob);
    let cos_abr = cosine_sim(&v_alice, &v_berlin);

    assert!(
        cos_ab < 0.99,
        "Alice and Bob embeddings should differ, got cosine {cos_ab:.4}"
    );
    assert!(
        cos_abr < 0.99,
        "Alice and Berlin embeddings should differ, got cosine {cos_abr:.4}"
    );

    let v_alice2 = embedder.embed("Alice").await?;
    let cos_self = cosine_sim(&v_alice, &v_alice2);
    assert!(
        (cos_self - 1.0).abs() < 1e-6,
        "Same input should produce identical embedding, got cosine {cos_self:.6}"
    );

    Ok(())
}

fn cosine_sim(a: &[f64], b: &[f64]) -> f64 {
    let dot: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let mag_a: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let mag_b: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
    if mag_a == 0.0 || mag_b == 0.0 {
        return 0.0;
    }
    dot / (mag_a * mag_b)
}

// ── Empty input graceful handling ────────────────────────────────────────

#[tokio::test]
async fn test_empty_input_graceful() -> Result<()> {
    let env = TestEnv::new().await?;

    let r1 = env.ingest_text("", "test").await?;
    assert!(r1.entities.is_empty(), "Empty input should produce no entities");
    assert!(r1.relations.is_empty(), "Empty input should produce no relations");
    assert_eq!(r1.memories.len(), 1, "Even empty input gets a memory record");

    let r2 = env.ingest_text("   ", "test").await?;
    assert!(r2.entities.is_empty(), "Whitespace-only input should produce no entities");
    assert!(r2.relations.is_empty(), "Whitespace-only input should produce no relations");

    Ok(())
}

// ── Aggregate F1 across all scenarios ────────────────────────────────────

#[tokio::test]
async fn test_extraction_f1_across_scenarios() -> Result<()> {
    let mut total_precision = 0.0;
    let mut total_recall = 0.0;
    let mut count = 0;

    for scenario in fixtures::all_scenarios() {
        let env = TestEnv::new().await?;
        let expected = scenario.expected_entity_set();

        if expected.is_empty() {
            continue;
        }

        let mut extracted_names: Vec<String> = Vec::new();
        for (text, source) in &scenario.episodes {
            let result = env.ingest_text(text, source).await?;
            for entity in &result.entities {
                extracted_names.push(entity.name.clone());
            }
        }

        let p = metrics::precision(&extracted_names, &expected);
        let r = metrics::recall(&extracted_names, &expected);
        total_precision += p;
        total_recall += r;
        count += 1;

        eprintln!(
            "  scenario={:<25} precision={:.2} recall={:.2} f1={:.2}",
            scenario.name, p, r, f1(p, r)
        );
    }

    let avg_p = total_precision / count as f64;
    let avg_r = total_recall / count as f64;
    let avg_f1 = f1(avg_p, avg_r);

    eprintln!("  AGGREGATE: precision={avg_p:.2} recall={avg_r:.2} f1={avg_f1:.2}");

    assert!(
        avg_f1 >= 0.5,
        "Aggregate F1 {avg_f1:.2} is below 0.5 threshold"
    );

    Ok(())
}
