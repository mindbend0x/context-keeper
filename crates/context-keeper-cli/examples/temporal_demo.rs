//! Temporal Demo: Show how facts are versioned over time.
//!
//! Run with: cargo run --example temporal_demo

use anyhow::Result;
use chrono::{Duration, Utc};
use context_keeper_core::{models::*, temporal::staleness_score, traits::*};
use context_keeper_surreal::{apply_schema, connect_memory, Repository, SurrealConfig};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    let config = SurrealConfig {
        embedding_dimensions: 64,
        ..SurrealConfig::default()
    };
    let db = connect_memory(&config).await?;
    apply_schema(&db, &config).await?;
    let repo = Repository::new(db);
    let embedder = MockEmbedder::new(64);

    // 1. Create initial fact: Alice works at Acme
    let alice_v1 = Entity {
        id: Uuid::new_v4(),
        name: "Alice".to_string(),
        entity_type: "person".to_string(),
        summary: "Software engineer at Acme Corp".to_string(),
        embedding: embedder.embed("Alice Acme").await?,
        valid_from: Utc::now() - Duration::days(30),
        valid_until: None,
    };
    repo.upsert_entity(&alice_v1).await?;
    println!("✓ Created: Alice works at Acme Corp (30 days ago)");
    println!("  Staleness score: {:.1} days", staleness_score(&alice_v1));

    // 2. Create relation: Alice → Acme
    let acme = Entity {
        id: Uuid::new_v4(),
        name: "Acme".to_string(),
        entity_type: "company".to_string(),
        summary: "Technology company".to_string(),
        embedding: embedder.embed("Acme").await?,
        valid_from: Utc::now() - Duration::days(30),
        valid_until: None,
    };
    repo.upsert_entity(&acme).await?;

    let works_at = Relation {
        id: Uuid::new_v4(),
        from_entity_id: alice_v1.id,
        to_entity_id: acme.id,
        relation_type: "works_at".to_string(),
        confidence: 95,
        valid_from: Utc::now() - Duration::days(30),
        valid_until: None,
    };
    repo.create_relation(&works_at).await?;
    println!("✓ Created relation: Alice → works_at → Acme");

    // 3. Snapshot from 15 days ago: Alice should be visible
    let past = Utc::now() - Duration::days(15);
    let snapshot = repo.entities_at(past).await?;
    println!(
        "\n--- Snapshot at {} ---",
        past.format("%Y-%m-%d")
    );
    for e in &snapshot {
        println!("  {} ({}): {}", e.name, e.entity_type, e.summary);
    }

    // 4. Invalidate old relation (Alice no longer at Acme)
    repo.invalidate_relation(works_at.id).await?;
    println!("\n✓ Invalidated: Alice no longer works at Acme");

    // 5. Create new fact: Alice now works at NewCo
    let newco = Entity {
        id: Uuid::new_v4(),
        name: "NewCo".to_string(),
        entity_type: "company".to_string(),
        summary: "Startup company".to_string(),
        embedding: embedder.embed("NewCo").await?,
        valid_from: Utc::now(),
        valid_until: None,
    };
    repo.upsert_entity(&newco).await?;

    let works_at_new = Relation {
        id: Uuid::new_v4(),
        from_entity_id: alice_v1.id,
        to_entity_id: newco.id,
        relation_type: "works_at".to_string(),
        confidence: 95,
        valid_from: Utc::now(),
        valid_until: None,
    };
    repo.create_relation(&works_at_new).await?;
    println!("✓ Created: Alice now works at NewCo");

    // 6. Current active relations for Alice
    let active_rels = repo.get_relations_for_entity(alice_v1.id).await?;
    println!("\n--- Active relations for Alice ---");
    for rel in &active_rels {
        println!("  {} → {}", rel.relation_type, rel.to_entity_id);
    }
    println!("  (old Acme relation is invalidated, not shown)");

    Ok(())
}
