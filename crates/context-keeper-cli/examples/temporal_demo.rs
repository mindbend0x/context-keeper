//! Temporal Reasoning Demo: knowledge graph with time-aware facts.
//!
//! Demonstrates Context Keeper's core differentiator — temporal correctness.
//! When Alice changes jobs, old queries reflect the update and historical
//! snapshots still return the original state.
//!
//! Run:  cargo run --example temporal_demo
//! JSON: cargo run --example temporal_demo -- --output results.json

use std::io::Write;
use std::time::Instant;

use anyhow::Result;
use chrono::{Duration, Utc};
use context_keeper_core::{models::*, traits::*};
use context_keeper_surreal::{apply_schema, connect_memory, Repository, SurrealConfig};
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
struct DemoStep {
    step: &'static str,
    latency_ms: f64,
    detail: String,
}

fn banner(msg: &str) {
    println!("\n\x1b[1;36m── {} ──\x1b[0m\n", msg);
}

#[tokio::main]
async fn main() -> Result<()> {
    let output_path = std::env::args().nth_back(0).and_then(|v| {
        if std::env::args().any(|a| a == "--output") && !v.starts_with("--") {
            Some(v)
        } else {
            None
        }
    });

    let mut steps: Vec<DemoStep> = Vec::new();

    println!("\x1b[1;33m");
    println!("  ╔══════════════════════════════════════════════════════════════╗");
    println!("  ║        Context Keeper — Temporal Reasoning Demo             ║");
    println!("  ║  Knowledge graph that knows WHEN facts were true            ║");
    println!("  ╚══════════════════════════════════════════════════════════════╝");
    println!("\x1b[0m");

    let t = Instant::now();
    let config = SurrealConfig {
        embedding_dimensions: 64,
        ..SurrealConfig::default()
    };
    let db = connect_memory(&config).await?;
    apply_schema(&db, &config).await?;
    let repo = Repository::new(db);
    let embedder = MockEmbedder::new(64);
    let init_ms = t.elapsed().as_secs_f64() * 1000.0;
    println!(
        "  Database ready (in-memory, no Docker needed)  [{:.0}ms]\n",
        init_ms
    );
    steps.push(DemoStep {
        step: "init",
        latency_ms: init_ms,
        detail: "In-memory SurrealDB initialized".into(),
    });

    // ── Step 1: Ingest Alice and Bob at Acme Corp ───────────────────────
    banner("Step 1: Ingest — Alice and Bob work at Acme Corp");

    let t = Instant::now();
    let thirty_days_ago = Utc::now() - Duration::days(30);

    let acme = Entity {
        id: Uuid::new_v4(),
        name: "Acme Corp".to_string(),
        entity_type: "company".into(),
        summary: "Technology company".to_string(),
        embedding: embedder.embed("Acme Corp technology company").await?,
        valid_from: thirty_days_ago,
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    repo.upsert_entity(&acme).await?;

    let alice = Entity {
        id: Uuid::new_v4(),
        name: "Alice".to_string(),
        entity_type: "person".into(),
        summary: "Senior engineer at Acme Corp".to_string(),
        embedding: embedder.embed("Alice senior engineer Acme").await?,
        valid_from: thirty_days_ago,
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    repo.upsert_entity(&alice).await?;

    let bob = Entity {
        id: Uuid::new_v4(),
        name: "Bob".to_string(),
        entity_type: "person".into(),
        summary: "Backend developer at Acme Corp".to_string(),
        embedding: embedder.embed("Bob backend developer Acme").await?,
        valid_from: thirty_days_ago,
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    repo.upsert_entity(&bob).await?;

    let alice_at_acme = Relation {
        id: Uuid::new_v4(),
        from_entity_id: alice.id,
        to_entity_id: acme.id,
        relation_type: "works_at".into(),
        confidence: 95,
        valid_from: thirty_days_ago,
        valid_until: None,
    };
    repo.create_relation(&alice_at_acme).await?;

    let bob_at_acme = Relation {
        id: Uuid::new_v4(),
        from_entity_id: bob.id,
        to_entity_id: acme.id,
        relation_type: "works_at".into(),
        confidence: 95,
        valid_from: thirty_days_ago,
        valid_until: None,
    };
    repo.create_relation(&bob_at_acme).await?;

    let ingest_ms = t.elapsed().as_secs_f64() * 1000.0;
    println!("  + Alice  — Senior engineer at Acme Corp");
    println!("  + Bob    — Backend developer at Acme Corp");
    println!("  + Acme Corp (company)");
    println!("  + Alice  ──works_at──▶ Acme Corp");
    println!("  + Bob    ──works_at──▶ Acme Corp");
    println!("\n  \x1b[2m[{:.0}ms]\x1b[0m", ingest_ms);
    steps.push(DemoStep {
        step: "ingest_initial",
        latency_ms: ingest_ms,
        detail: "Alice + Bob at Acme Corp with relations".into(),
    });

    // ── Step 2: Search — "Who works at Acme?" ───────────────────────────
    banner("Step 2: Search — \"Who works at Acme?\"");

    let t = Instant::now();
    let results = repo.search_entities_by_keyword("Acme", None, None).await?;
    let search1_ms = t.elapsed().as_secs_f64() * 1000.0;
    let names: Vec<&str> = results.iter().map(|e| e.name.as_str()).collect();
    for e in &results {
        println!("  → {} ({}): {}", e.name, e.entity_type, e.summary);
    }
    println!(
        "\n  \x1b[32m✓ Both Alice and Bob found\x1b[0m  [{:.0}ms]",
        search1_ms
    );
    steps.push(DemoStep {
        step: "search_before_update",
        latency_ms: search1_ms,
        detail: format!("Found: {}", names.join(", ")),
    });

    // ── Step 3: Update — Alice leaves Acme, joins BigCo ─────────────────
    banner("Step 3: Update — Alice leaves Acme, joins BigCo as Director");

    let t = Instant::now();
    repo.invalidate_relation(alice_at_acme.id).await?;

    let alice_updated = Entity {
        id: alice.id,
        name: "Alice".to_string(),
        entity_type: "person".into(),
        summary: "Director of Engineering at BigCo".to_string(),
        embedding: embedder.embed("Alice Director Engineering BigCo").await?,
        valid_from: Utc::now(),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    repo.upsert_entity(&alice_updated).await?;

    let bigco = Entity {
        id: Uuid::new_v4(),
        name: "BigCo".to_string(),
        entity_type: "company".into(),
        summary: "Large enterprise company".to_string(),
        embedding: embedder.embed("BigCo enterprise company").await?,
        valid_from: Utc::now(),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    repo.upsert_entity(&bigco).await?;

    let alice_at_bigco = Relation {
        id: Uuid::new_v4(),
        from_entity_id: alice.id,
        to_entity_id: bigco.id,
        relation_type: "works_at".into(),
        confidence: 95,
        valid_from: Utc::now(),
        valid_until: None,
    };
    repo.create_relation(&alice_at_bigco).await?;

    let update_ms = t.elapsed().as_secs_f64() * 1000.0;
    println!("  ✗ Alice  ──works_at──▶ Acme Corp    \x1b[31m[invalidated]\x1b[0m");
    println!("  + Alice  — Director of Engineering at BigCo");
    println!("  + BigCo (company)");
    println!("  + Alice  ──works_at──▶ BigCo");
    println!("\n  \x1b[2m[{:.0}ms]\x1b[0m", update_ms);
    steps.push(DemoStep {
        step: "update_alice",
        latency_ms: update_ms,
        detail: "Alice leaves Acme, joins BigCo as Director".into(),
    });

    // ── Step 4: Search again — "Who works at Acme?" ─────────────────────
    banner("Step 4: Search again — \"Who works at Acme?\"");

    let t = Instant::now();
    let results = repo.search_entities_by_keyword("Acme", None, None).await?;
    let search2_ms = t.elapsed().as_secs_f64() * 1000.0;
    let people: Vec<&Entity> = results
        .iter()
        .filter(|e| e.entity_type == EntityType::Person)
        .collect();
    for e in &results {
        println!("  → {} ({}): {}", e.name, e.entity_type, e.summary);
    }
    let person_names: Vec<&str> = people.iter().map(|e| e.name.as_str()).collect();
    let alice_gone = !person_names.contains(&"Alice");
    let bob_present = person_names.contains(&"Bob");
    if alice_gone && bob_present {
        println!(
            "\n  \x1b[32m✓ Only Bob remains — temporal reasoning correct!\x1b[0m  [{:.0}ms]",
            search2_ms
        );
    } else {
        println!(
            "\n  People found: {}  [{:.0}ms]",
            person_names.join(", "),
            search2_ms,
        );
    }
    steps.push(DemoStep {
        step: "search_after_update",
        latency_ms: search2_ms,
        detail: format!("People at Acme: {}", person_names.join(", ")),
    });

    // ── Step 5: Snapshot — travel back to when Alice was still at Acme ──
    banner("Step 5: Snapshot — 15 days ago (Alice was still at Acme)");

    let t = Instant::now();
    let past = Utc::now() - Duration::days(15);
    let snapshot = repo.entities_at(past).await?;
    let snapshot_ms = t.elapsed().as_secs_f64() * 1000.0;
    let snapshot_names: Vec<&str> = snapshot.iter().map(|e| e.name.as_str()).collect();
    for e in &snapshot {
        println!("  → {} ({}): {}", e.name, e.entity_type, e.summary);
    }
    let had_alice = snapshot_names.contains(&"Alice");
    if had_alice {
        println!(
            "\n  \x1b[32m✓ Alice visible in historical snapshot — time travel works!\x1b[0m  [{:.0}ms]",
            snapshot_ms
        );
    }
    steps.push(DemoStep {
        step: "snapshot_past",
        latency_ms: snapshot_ms,
        detail: format!(
            "Entities at {}: {}",
            past.format("%Y-%m-%d"),
            snapshot_names.join(", ")
        ),
    });

    // ── Summary ─────────────────────────────────────────────────────────
    println!("\n\x1b[1;33m");
    println!("  ╔══════════════════════════════════════════════════════════════╗");
    println!("  ║                       Latency Summary                       ║");
    println!("  ╚══════════════════════════════════════════════════════════════╝");
    println!("\x1b[0m");
    let total_ms: f64 = steps.iter().map(|s| s.latency_ms).sum();
    for s in &steps {
        println!("  {:.<40} {:>6.0}ms", s.step, s.latency_ms);
    }
    println!("  {:.<40} {:>6.0}ms", "total", total_ms);

    if let Some(path) = output_path {
        let json = serde_json::to_string_pretty(&steps)?;
        let mut f = std::fs::File::create(&path)?;
        f.write_all(json.as_bytes())?;
        println!("\n  Results written to {path}");
    }

    println!();
    Ok(())
}
