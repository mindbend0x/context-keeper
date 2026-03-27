//! Feature Showcase: A visual walkthrough of every major Context Keeper capability.
//!
//! Run with: cargo run --example feature_showcase
//!
//! No API keys required — uses mock LLM services throughout.

use anyhow::Result;
use chrono::{Duration, Utc};
use context_keeper_core::{
    ingestion,
    models::*,
    search::{fuse_rrf, QueryExpander},
    temporal::{staleness_score, TemporalSnapshot},
    traits::*,
};
use context_keeper_surreal::{
    apply_schema, connect_memory, vector_store::SurrealVectorStore, Repository, SurrealConfig,
};
use uuid::Uuid;

fn banner(title: &str) {
    let width: usize = 64;
    let pad = width.saturating_sub(title.len() + 4);
    let left = pad / 2;
    let right = pad - left;
    println!();
    println!("  ╔{}╗", "═".repeat(width));
    println!("  ║{} {} {}║", " ".repeat(left), title, " ".repeat(right));
    println!("  ╚{}╝", "═".repeat(width));
    println!();
}

fn section(title: &str) {
    println!();
    println!(
        "  ┌─── {} {}",
        title,
        "─".repeat(50usize.saturating_sub(title.len()))
    );
    println!("  │");
}

fn end_section() {
    println!("  │");
    println!("  └{}─", "─".repeat(56));
}

fn item(text: &str) {
    println!("  │  ▸ {text}");
}

fn blank() {
    println!("  │");
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .with_timer(tracing_subscriber::fmt::time::uptime())
        .init();

    banner("CONTEXT KEEPER  —  Feature Showcase");

    section("1 · In-Memory SurrealDB Setup");

    let config = SurrealConfig {
        embedding_dimensions: 64,
        ..SurrealConfig::default()
    };

    tracing::info!(
        ns = %config.namespace,
        db = %config.database,
        dims = config.embedding_dimensions,
        metric = %config.distance_metric,
        "Connecting to in-memory SurrealDB"
    );

    let db = connect_memory(&config).await?;
    apply_schema(&db, &config).await?;
    let repo = Repository::new(db);

    item(&format!("Namespace  : {}", config.namespace));
    item(&format!("Database   : {}", config.database));
    item(&format!("Dimensions : {}", config.embedding_dimensions));
    item(&format!("Metric     : {}", config.distance_metric));
    item("Storage    : in-memory (no disk)");
    item("Schema     : applied (HNSW indexes, BM25 analyzers, changefeeds)");

    end_section();

    section("2 · Mock LLM Services (no API key needed)");

    let embedder = MockEmbedder::new(64);
    let entity_extractor = MockEntityExtractor;
    let relation_extractor = MockRelationExtractor;
    let query_rewriter = MockQueryRewriter;

    item("MockEmbedder         → deterministic 64-dim unit vectors from text hashes");
    item("MockEntityExtractor  → extracts capitalized words as entities");
    item("MockRelationExtractor→ links consecutive entities with 'related_to'");
    item("MockQueryRewriter    → generates semantic query variants");

    end_section();

    section("3 · Ingestion Pipeline");

    let episodes_text = vec![
        (
            "conv-1",
            "Alice is a software engineer at Acme Corp in Berlin",
        ),
        (
            "conv-1",
            "Bob manages the Machine Learning team at Acme Corp",
        ),
        (
            "conv-2",
            "Charlie joined DataFlow Inc as a Data Scientist in Munich",
        ),
        (
            "conv-2",
            "Eve is the CTO of DataFlow Inc and mentors Charlie",
        ),
        (
            "conv-3",
            "Acme Corp acquired DataFlow Inc for its AI capabilities",
        ),
    ];

    let mut all_entities: Vec<Entity> = Vec::new();
    let mut all_relations: Vec<Relation> = Vec::new();

    for (session, text) in &episodes_text {
        let episode = Episode {
            id: Uuid::new_v4(),
            content: text.to_string(),
            source: "showcase".into(),
            session_id: Some(session.to_string()),
            agent: None,
            namespace: None,
            created_at: Utc::now(),
        };

        tracing::info!(session = session, "Ingesting episode");
        let resolver: &dyn EntityResolver = &repo;
        let result = ingestion::ingest(
            &episode,
            &embedder,
            &entity_extractor,
            &relation_extractor,
            Some(resolver),
            None,
        )
        .await?;

        repo.create_episode(&episode).await?;
        for entity in &result.entities {
            repo.upsert_entity(entity).await?;
        }
        for relation in &result.relations {
            repo.create_relation(relation).await?;
        }
        for memory in &result.memories {
            repo.create_memory(memory).await?;
        }

        let entity_names: Vec<&str> = result.entities.iter().map(|e| e.name.as_str()).collect();
        item(&format!(
            "[{}] \"{}\"",
            session,
            &text[..48.min(text.len())]
        ));
        item(&format!(
            "       → {} entities {:?}  ·  {} relations",
            result.entities.len(),
            entity_names,
            result.relations.len(),
        ));

        all_entities.extend(result.entities);
        all_relations.extend(result.relations);
    }

    blank();
    item(&format!(
        "Totals: {} episodes, {} entities, {} relations ingested",
        episodes_text.len(),
        all_entities.len(),
        all_relations.len(),
    ));

    end_section();

    section("4 · HNSW Vector Search (semantic similarity)");

    let query = "Machine Learning engineer";
    let query_embedding = embedder.embed(query).await?;

    tracing::info!(query, "Running HNSW vector search on entities");
    let vector_results = repo
        .search_entities_by_vector(&query_embedding, 5, None, None)
        .await?;

    item(&format!("Query: \"{query}\""));
    blank();
    for (i, (entity, score)) in vector_results.iter().enumerate() {
        item(&format!(
            "  {}. {:<16} ({:<12})  cosine: {:.4}",
            i + 1,
            entity.name,
            entity.entity_type,
            score,
        ));
    }

    blank();

    let mem_query = "data science Munich";
    let mem_embedding = embedder.embed(mem_query).await?;

    tracing::info!(query = mem_query, "Running HNSW vector search on memories");
    let mem_results = repo
        .search_memories_by_vector(&mem_embedding, 3, None)
        .await?;

    item(&format!("Query: \"{mem_query}\"  (memory search)"));
    blank();
    for (i, (memory, score)) in mem_results.iter().enumerate() {
        item(&format!(
            "  {}. \"{:.60}\"  cosine: {:.4}",
            i + 1,
            memory.content,
            score,
        ));
    }

    end_section();

    section("5 · BM25 Full-Text Search (keyword matching)");

    let keyword = "Acme";
    tracing::info!(keyword, "Running BM25 keyword search");

    let kw_entities = repo.search_entities_by_keyword(keyword, None, None).await?;
    let kw_episodes = repo.search_episodes_by_keyword(keyword).await?;

    item(&format!("Keyword: \"{keyword}\""));
    blank();
    item(&format!("  Entities matched : {}", kw_entities.len()));
    for e in &kw_entities {
        item(&format!(
            "    · {} ({}): {}",
            e.name, e.entity_type, e.summary
        ));
    }
    blank();
    item(&format!("  Episodes matched : {}", kw_episodes.len()));
    for ep in &kw_episodes {
        item(&format!("    · [{}] \"{:.50}\"", ep.source, ep.content));
    }

    end_section();

    section("6 · Hybrid Search — RRF Fusion (vector + keyword)");

    let hybrid_query = "DataFlow";
    let hybrid_embedding = embedder.embed(hybrid_query).await?;

    tracing::info!(
        query = hybrid_query,
        "Running hybrid search with RRF fusion"
    );

    let vec_hits = repo
        .search_entities_by_vector(&hybrid_embedding, 5, None, None)
        .await?;
    let kw_hits = repo
        .search_entities_by_keyword(hybrid_query, None, None)
        .await?;

    item(&format!("Query: \"{hybrid_query}\""));
    item(&format!(
        "  Vector hits : {}  ·  Keyword hits : {}",
        vec_hits.len(),
        kw_hits.len()
    ));
    blank();

    let fused = fuse_rrf(vec![
        vec_hits.into_iter().map(|(e, _)| e).collect(),
        kw_hits,
    ]);

    item("  Fused results (Reciprocal Rank Fusion, k=60):");
    blank();
    for (i, result) in fused.iter().take(5).enumerate() {
        if let Some(ref entity) = result.entity {
            item(&format!(
                "  {}. {:<16} — rrf score: {:.6}",
                i + 1,
                entity.name,
                result.score,
            ));
        }
    }

    end_section();

    section("7 · Query Expansion (sparse-result fallback)");

    let expander = QueryExpander::new(3);
    let sparse_query = "AI capabilities";

    tracing::info!(
        query = sparse_query,
        threshold = 3,
        "Checking if expansion is needed"
    );

    let sparse_embedding = embedder.embed(sparse_query).await?;
    let initial_results = repo
        .search_entities_by_vector(&sparse_embedding, 5, None, None)
        .await?;

    item(&format!("Query: \"{sparse_query}\""));
    item(&format!(
        "  Initial results : {}  ·  Threshold : {}",
        initial_results.len(),
        expander.threshold
    ));
    item(&format!(
        "  Needs expansion : {}",
        expander.should_expand(initial_results.len())
    ));
    blank();

    let variants = expander.expand(sparse_query, &query_rewriter).await?;
    item("  Expanded variants:");
    for (i, v) in variants.iter().enumerate() {
        item(&format!("    {i}. \"{v}\""));
    }

    blank();

    let mut expanded_lists: Vec<Vec<Entity>> =
        vec![initial_results.into_iter().map(|(e, _)| e).collect()];

    for variant in &variants[1..] {
        let emb = embedder.embed(variant).await?;
        let hits = repo.search_entities_by_vector(&emb, 5, None, None).await?;
        expanded_lists.push(hits.into_iter().map(|(e, _)| e).collect());
    }

    let expanded_fused = fuse_rrf(expanded_lists);
    item(&format!(
        "  After expansion + RRF: {} results",
        expanded_fused.len()
    ));
    for (i, r) in expanded_fused.iter().take(5).enumerate() {
        if let Some(ref entity) = r.entity {
            item(&format!(
                "    {}. {:<16} — rrf score: {:.6}",
                i + 1,
                entity.name,
                r.score,
            ));
        }
    }

    end_section();

    section("8 · Temporal Versioning (time-travel queries)");

    let old_date = Utc::now() - Duration::days(60);
    let alice_v1 = Entity {
        id: Uuid::new_v4(),
        name: "Alice_Temporal".into(),
        entity_type: EntityType::Person,
        summary: "Junior engineer at StartupX".into(),
        embedding: embedder.embed("Alice_Temporal StartupX").await?,
        valid_from: old_date,
        valid_until: Some(Utc::now() - Duration::days(10)),
        namespace: None,
        created_by_agent: None,
    };
    repo.upsert_entity(&alice_v1).await?;

    let alice_v2 = Entity {
        id: Uuid::new_v4(),
        name: "Alice_Temporal".into(),
        entity_type: EntityType::Person,
        summary: "Senior engineer at MegaCorp".into(),
        embedding: embedder.embed("Alice_Temporal MegaCorp").await?,
        valid_from: Utc::now() - Duration::days(10),
        valid_until: None,
        namespace: None,
        created_by_agent: None,
    };
    repo.upsert_entity(&alice_v2).await?;

    tracing::info!("Created two temporal versions of Alice_Temporal");

    item("Two versions of Alice_Temporal created:");
    item(&format!(
        "  v1: \"{}\" (valid {} → {})",
        alice_v1.summary,
        alice_v1.valid_from.format("%Y-%m-%d"),
        alice_v1
            .valid_until
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "present".into()),
    ));
    item(&format!(
        "  v2: \"{}\" (valid {} → present)",
        alice_v2.summary,
        alice_v2.valid_from.format("%Y-%m-%d"),
    ));
    blank();

    let snapshot_30d = Utc::now() - Duration::days(30);
    let past_entities = repo.entities_at(snapshot_30d).await?;
    tracing::info!(
        at = %snapshot_30d.format("%Y-%m-%d"),
        count = past_entities.len(),
        "Point-in-time entity snapshot"
    );

    item(&format!(
        "Snapshot at {} ({} entities active):",
        snapshot_30d.format("%Y-%m-%d"),
        past_entities.len()
    ));
    for e in &past_entities {
        item(&format!(
            "  · {} ({}): {}",
            e.name, e.entity_type, e.summary
        ));
    }
    blank();

    let snapshot_now = repo.entities_at(Utc::now()).await?;
    item(&format!(
        "Snapshot at now ({} entities active):",
        snapshot_now.len()
    ));
    for e in &snapshot_now {
        item(&format!(
            "  · {} ({}): {}",
            e.name, e.entity_type, e.summary
        ));
    }

    end_section();

    section("9 · Staleness Scoring");

    tracing::info!("Computing staleness scores for all active entities");

    item("How stale is each entity? (days since valid_from)");
    blank();

    let mut scored: Vec<(String, f64)> = Vec::new();
    scored.push((alice_v2.name.clone(), staleness_score(&alice_v2)));

    let active = repo.get_all_active_entities().await?;
    for e in &active {
        scored.push((e.name.clone(), staleness_score(e)));
    }
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    scored.dedup_by(|a, b| a.0 == b.0);

    for (name, score) in scored.iter().take(10) {
        let bar_len = (*score as usize).min(30);
        let bar: String = "█".repeat(bar_len);
        item(&format!("  {:<20} {:>5.1}d  {bar}", name, score));
    }

    end_section();

    section("10 · Graph Traversal (entity neighbors)");

    let acme_entities = repo.find_entities_by_name("Acme", None, None).await?;
    if let Some(acme) = acme_entities.first() {
        tracing::info!(entity = %acme.name, "Fetching graph neighbors");
        let neighbors = repo.get_graph_neighbors(&[acme.id], 1).await?;
        item(&format!("Neighbors of \"{}\" (1-hop):", acme.name));
        blank();
        for n in &neighbors {
            item(&format!(
                "  · {} ({}) — {}",
                n.name, n.entity_type, n.summary
            ));
        }
        if neighbors.is_empty() {
            item("  (none found — graph edges depend on RELATE statements)");
        }
    } else {
        item("  No 'Acme' entity found for traversal demo");
    }

    blank();

    let relations = repo.get_relations_for_entity(all_entities[0].id).await?;
    item(&format!("Relations for \"{}\":", all_entities[0].name));
    for rel in &relations {
        let target = all_entities.iter().find(|e| e.id == rel.to_entity_id);
        let target_name = target.map(|e| e.name.as_str()).unwrap_or("?");
        item(&format!(
            "  {} ──[{} ({}%)]──▸ {}",
            all_entities[0].name, rel.relation_type, rel.confidence, target_name,
        ));
    }

    end_section();

    section("11 · SurrealVectorStore (convenience top-k API)");

    let db2 = connect_memory(&config).await?;
    apply_schema(&db2, &config).await?;
    let repo2 = Repository::new(db2);

    for e in &all_entities {
        repo2.upsert_entity(e).await?;
    }

    let store = SurrealVectorStore::new(repo2);

    let store_query = "Berlin engineer";
    let store_emb = embedder.embed(store_query).await?;

    tracing::info!(
        query = store_query,
        "SurrealVectorStore top-k entity search"
    );
    let top_k = store.top_k_entities(&store_emb, 5).await?;

    item(&format!("Query: \"{store_query}\""));
    blank();
    for (i, r) in top_k.iter().enumerate() {
        item(&format!(
            "  {}. {:<20} score: {:.4}  │  \"{}\"",
            i + 1,
            r.entity.as_ref().map(|e| e.name.as_str()).unwrap_or("?"),
            r.score,
            &r.content[..60.min(r.content.len())],
        ));
    }

    end_section();

    section("12 · Recent Memories");

    let memories = repo.list_recent_memories(5).await?;
    tracing::info!(count = memories.len(), "Listing recent memories");

    for (i, m) in memories.iter().enumerate() {
        item(&format!("  {}. \"{:.70}\"", i + 1, m.content,));
        item(&format!(
            "     entities: {}  ·  created: {}",
            m.entity_ids.len(),
            m.created_at.format("%H:%M:%S"),
        ));
    }

    end_section();

    section("13 · Temporal Snapshot (entities + relations at a point in time)");

    let snap_time = Utc::now();
    let snap_entities = repo.entities_at(snap_time).await?;
    let snap_relations = repo.relations_at(snap_time).await?;

    let snapshot = TemporalSnapshot {
        entities: snap_entities,
        relations: snap_relations,
        timestamp: snap_time,
    };

    tracing::info!(
        at = %snapshot.timestamp.format("%Y-%m-%d"),
        entities = snapshot.entities.len(),
        relations = snapshot.relations.len(),
        "Built temporal snapshot"
    );

    item(&format!(
        "Snapshot at: {}",
        snapshot.timestamp.format("%Y-%m-%d %H:%M")
    ));
    item(&format!("  Entities  : {}", snapshot.entities.len()));
    item(&format!("  Relations : {}", snapshot.relations.len()));

    end_section();

    banner("Demo Complete — All Features Showcased");

    println!("  Demonstrated:");
    println!("    1.  In-memory SurrealDB setup with HNSW + BM25 indexes");
    println!("    2.  Mock LLM services (embedder, extractors, rewriter)");
    println!("    3.  Ingestion pipeline (episode → entities + relations + memories)");
    println!("    4.  HNSW vector search (entities and memories)");
    println!("    5.  BM25 full-text search (entities, memories, episodes)");
    println!("    6.  Hybrid RRF fusion (vector + keyword)");
    println!("    7.  Query expansion with semantic variants");
    println!("    8.  Temporal versioning with time-travel queries");
    println!("    9.  Staleness scoring");
    println!("   10.  Graph traversal (neighbors, relations)");
    println!("   11.  SurrealVectorStore convenience API");
    println!("   12.  Recent memory listing");
    println!("   13.  Temporal snapshots");
    println!();

    Ok(())
}
