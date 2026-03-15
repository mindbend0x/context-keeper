//! Feature Showcase (LLM): End-to-end demo using real LLM extraction and embeddings.
//!
//! Unlike `feature_showcase` (which uses mocks), this example calls a live
//! OpenAI-compatible API for entity extraction, relation extraction, and embeddings.
//!
//! Required environment variables (or `.env` file):
//!
//!   OPENAI_API_URL   — base URL (e.g. https://api.openai.com/v1)
//!   OPENAI_API_KEY   — your API key
//!   EMBEDDING_MODEL  — embedding model name (e.g. text-embedding-3-small)
//!   EMBEDDING_DIMS   — embedding dimensions (e.g. 1536)
//!   EXTRACTION_MODEL — chat model for extraction (e.g. gpt-4o-mini)
//!
//! Run with: cargo run --example feature_showcase_llm

use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use context_keeper_core::{
    ingestion,
    models::*,
    search::{fuse_rrf, QueryExpander},
    temporal::{staleness_score, TemporalSnapshot},
    traits::*,
};
use context_keeper_rig::{
    embeddings::RigEmbedder,
    extraction::{RigEntityExtractor, RigRelationExtractor},
};
use context_keeper_surreal::{
    apply_schema, connect_memory, vector_store::SurrealVectorStore, Repository, SurrealConfig,
};
use dotenv::dotenv;
use uuid::Uuid;

// ── Visual helpers ──────────────────────────────────────────────────────

fn banner(title: &str) {
    let width: usize = 72;
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
        "─".repeat(58usize.saturating_sub(title.len()))
    );
    println!("  │");
}

fn end_section() {
    println!("  │");
    println!("  └{}─", "─".repeat(64));
}

fn item(text: &str) {
    println!("  │  ▸ {text}");
}

fn detail(text: &str) {
    println!("  │    {text}");
}

fn blank() {
    println!("  │");
}

// ── Configuration from env ──────────────────────────────────────────────

struct LlmConfig {
    api_url: String,
    api_key: String,
    embedding_model: String,
    embedding_dims: usize,
    extraction_model: String,
}

impl LlmConfig {
    fn from_env() -> Result<Self> {
        Ok(Self {
            api_url: std::env::var("OPENAI_API_URL")
                .context("OPENAI_API_URL not set")?,
            api_key: std::env::var("OPENAI_API_KEY")
                .context("OPENAI_API_KEY not set")?,
            embedding_model: std::env::var("EMBEDDING_MODEL")
                .context("EMBEDDING_MODEL not set")?,
            embedding_dims: std::env::var("EMBEDDING_DIMS")
                .context("EMBEDDING_DIMS not set")?
                .parse()
                .context("EMBEDDING_DIMS must be a number")?,
            extraction_model: std::env::var("EXTRACTION_MODEL")
                .context("EXTRACTION_MODEL not set")?,
        })
    }
}

// ── Rich documents for ingestion ────────────────────────────────────────

fn sample_documents() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "meeting-notes",
            "During the Q1 planning meeting, CTO Sarah Chen announced that \
             Nextera Robotics will partner with the Technical University of Munich \
             to develop an open-source reinforcement learning framework called \
             RoboLearn. The project is funded by a 2.4 million euro grant from \
             the European Research Council and will be led by Professor Klaus Weber.",
        ),
        (
            "meeting-notes",
            "VP of Engineering David Park confirmed that the Berlin office will \
             serve as the primary development hub for RoboLearn. Senior engineer \
             Aisha Patel will relocate from the London office to lead the core \
             runtime team. The first public release is targeted for September 2026.",
        ),
        (
            "incident-report",
            "Post-mortem for the March 3rd outage: the authentication service \
             managed by team Cerberus experienced a cascading failure after a \
             misconfigured rate limiter in the API gateway allowed 50x normal \
             traffic to hit the PostgreSQL cluster. SRE lead Marcus Johnson \
             identified the root cause within 18 minutes. The Redis session cache, \
             maintained by contractor firm OpsForge, was not affected.",
        ),
        (
            "research-digest",
            "Dr. Elena Vasquez from the Stanford NLP Group published a new paper \
             titled 'Temporal Graph Networks for Evolving Knowledge Bases' in the \
             proceedings of NeurIPS 2025. The paper introduces a method called \
             ChronoGraph that outperforms existing approaches on the ICEWS and GDELT \
             benchmarks by 12% on link prediction tasks. Nextera Robotics has \
             expressed interest in integrating ChronoGraph into their knowledge \
             management pipeline.",
        ),
        (
            "hr-update",
            "Nextera Robotics announced the appointment of former Google DeepMind \
             researcher Dr. James Liu as Head of AI Research, effective April 1st 2026. \
             Dr. Liu previously led the multi-agent systems team at DeepMind and holds \
             a PhD from MIT. He will report directly to CTO Sarah Chen and oversee \
             the newly formed Frontier Research division based in San Francisco.",
        ),
        (
            "product-brief",
            "The RoboLearn framework will support three core modules: a simulation \
             environment built on top of MuJoCo, a policy optimization library \
             implementing PPO, SAC, and DreamerV3 algorithms, and a deployment \
             toolkit for transferring trained policies to physical robots. Integration \
             with NVIDIA Isaac Sim is planned for the v2.0 release.",
        ),
    ]
}

// ── Main ────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .with_timer(tracing_subscriber::fmt::time::uptime())
        .init();

    dotenv().ok();

    banner("CONTEXT KEEPER  —  LLM Feature Showcase");

    // ═══════════════════════════════════════════════════════════════════════
    // 1. Configuration
    // ═══════════════════════════════════════════════════════════════════════

    section("1 · LLM Configuration");

    let llm = LlmConfig::from_env()?;

    tracing::info!(
        api_url = %llm.api_url,
        embedding_model = %llm.embedding_model,
        extraction_model = %llm.extraction_model,
        dims = llm.embedding_dims,
        "Loaded LLM configuration"
    );

    item(&format!("API URL          : {}", llm.api_url));
    item(&format!("Embedding model  : {}", llm.embedding_model));
    item(&format!("Embedding dims   : {}", llm.embedding_dims));
    item(&format!("Extraction model : {}", llm.extraction_model));

    end_section();

    // ═══════════════════════════════════════════════════════════════════════
    // 2. Database Setup
    // ═══════════════════════════════════════════════════════════════════════

    section("2 · In-Memory SurrealDB Setup");

    let config = SurrealConfig {
        embedding_dimensions: llm.embedding_dims,
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
    item("Storage    : in-memory");
    item("Schema     : applied (HNSW indexes, BM25 analyzers, changefeeds)");

    end_section();

    // ═══════════════════════════════════════════════════════════════════════
    // 3. LLM Service Initialization
    // ═══════════════════════════════════════════════════════════════════════

    section("3 · LLM Services (Rig-powered)");

    let embedder = RigEmbedder::new(
        &llm.api_url,
        &llm.api_key,
        &llm.embedding_model,
        llm.embedding_dims,
    );
    let entity_extractor = RigEntityExtractor::new(
        &llm.api_url,
        &llm.api_key,
        &llm.extraction_model,
    );
    let relation_extractor = RigRelationExtractor::new(
        &llm.api_url,
        &llm.api_key,
        &llm.extraction_model,
    );
    let query_rewriter = MockQueryRewriter;

    tracing::info!("Initialized Rig-backed LLM services");

    item(&format!(
        "RigEmbedder          → {} ({}-dim vectors)",
        llm.embedding_model, llm.embedding_dims
    ));
    item(&format!(
        "RigEntityExtractor   → {} (structured JSON output)",
        llm.extraction_model
    ));
    item(&format!(
        "RigRelationExtractor → {} (structured JSON output)",
        llm.extraction_model
    ));
    item("MockQueryRewriter    → generates semantic query variants locally");

    end_section();

    // ═══════════════════════════════════════════════════════════════════════
    // 4. Ingestion Pipeline (LLM extraction + embeddings)
    // ═══════════════════════════════════════════════════════════════════════

    section("4 · Ingestion Pipeline (LLM-powered)");

    let documents = sample_documents();
    let mut all_entities: Vec<Entity> = Vec::new();
    let mut all_relations: Vec<Relation> = Vec::new();
    let mut total_memories = 0usize;

    for (i, (source, text)) in documents.iter().enumerate() {
        let episode = Episode {
            id: Uuid::new_v4(),
            content: text.to_string(),
            source: source.to_string(),
            session_id: Some(source.to_string()),
            created_at: Utc::now(),
        };

        tracing::info!(
            doc = i + 1,
            source = source,
            len = text.len(),
            "Ingesting document"
        );

        let result = ingestion::ingest(
            &episode,
            &embedder,
            &entity_extractor,
            &relation_extractor,
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
        let relation_descs: Vec<String> = result
            .relations
            .iter()
            .map(|r| {
                let src = result
                    .entities
                    .iter()
                    .find(|e| e.id == r.from_entity_id)
                    .map(|e| e.name.as_str())
                    .unwrap_or("?");
                let tgt = result
                    .entities
                    .iter()
                    .find(|e| e.id == r.to_entity_id)
                    .map(|e| e.name.as_str())
                    .unwrap_or("?");
                format!("{src} →[{}]→ {tgt}", r.relation_type)
            })
            .collect();

        item(&format!(
            "Doc {}: [{}] \"{}...\"",
            i + 1,
            source,
            &text[..60.min(text.len())],
        ));
        blank();
        detail(&format!("Entities ({}): {:?}", entity_names.len(), entity_names));
        blank();
        detail(&format!("Relations ({}):", relation_descs.len()));
        for rd in &relation_descs {
            detail(&format!("  {rd}"));
        }
        blank();

        total_memories += result.memories.len();
        all_entities.extend(result.entities);
        all_relations.extend(result.relations);
    }

    item(&format!(
        "Totals: {} docs ingested → {} entities, {} relations, {} memories",
        documents.len(),
        all_entities.len(),
        all_relations.len(),
        total_memories,
    ));

    end_section();

    // ═══════════════════════════════════════════════════════════════════════
    // 5. HNSW Vector Search (real embeddings)
    // ═══════════════════════════════════════════════════════════════════════

    section("5 · HNSW Vector Search (real embeddings)");

    let queries = [
        ("reinforcement learning robotics", 5),
        ("infrastructure outage database", 3),
    ];

    for (query, limit) in &queries {
        tracing::info!(query, "Running HNSW vector search on entities");
        let emb = embedder.embed(query).await?;
        let results = repo.search_entities_by_vector(&emb, *limit).await?;

        item(&format!("Query: \"{query}\""));
        blank();
        for (i, (entity, score)) in results.iter().enumerate() {
            detail(&format!(
                "{}. {:<24} ({:<14})  cosine: {:.4}",
                i + 1,
                entity.name,
                entity.entity_type,
                score,
            ));
        }
        blank();
    }

    tracing::info!("Running HNSW vector search on memories");
    let mem_q = "who is leading the AI research division";
    let mem_emb = embedder.embed(mem_q).await?;
    let mem_results = repo.search_memories_by_vector(&mem_emb, 3).await?;

    item(&format!("Query: \"{mem_q}\"  (memory search)"));
    blank();
    for (i, (memory, score)) in mem_results.iter().enumerate() {
        let truncated: String = memory.content.chars().take(80).collect();
        detail(&format!("{}. cosine: {:.4}", i + 1, score));
        detail(&format!("   \"{truncated}...\""));
    }

    end_section();

    // ═══════════════════════════════════════════════════════════════════════
    // 6. BM25 Full-Text Search
    // ═══════════════════════════════════════════════════════════════════════

    section("6 · BM25 Full-Text Search");

    let keywords = ["Nextera", "ChronoGraph", "PostgreSQL"];

    for keyword in &keywords {
        tracing::info!(keyword, "Running BM25 keyword search");
        let kw_entities = repo.search_entities_by_keyword(keyword).await?;
        let kw_episodes = repo.search_episodes_by_keyword(keyword).await?;

        item(&format!(
            "Keyword: \"{keyword}\"  →  {} entities, {} episodes",
            kw_entities.len(),
            kw_episodes.len(),
        ));
        for e in &kw_entities {
            detail(&format!(
                "  [entity] {} ({}): {}",
                e.name,
                e.entity_type,
                &e.summary[..80.min(e.summary.len())]
            ));
        }
        for ep in &kw_episodes {
            let truncated: String = ep.content.chars().take(70).collect();
            detail(&format!("  [episode] \"{truncated}...\""));
        }
        blank();
    }

    end_section();

    // ═══════════════════════════════════════════════════════════════════════
    // 7. Hybrid Search — RRF Fusion
    // ═══════════════════════════════════════════════════════════════════════

    section("7 · Hybrid Search — RRF Fusion (vector + keyword)");

    let hybrid_query = "Munich university robotics partnership";
    let hybrid_emb = embedder.embed(hybrid_query).await?;

    tracing::info!(query = hybrid_query, "Running hybrid search with RRF fusion");

    let vec_hits = repo.search_entities_by_vector(&hybrid_emb, 5).await?;
    let kw_hits = repo.search_entities_by_keyword("Munich").await?;

    item(&format!("Query: \"{hybrid_query}\""));
    item(&format!(
        "  Vector hits : {}  ·  Keyword hits (\"Munich\"): {}",
        vec_hits.len(),
        kw_hits.len(),
    ));
    blank();

    let fused = fuse_rrf(vec![
        vec_hits.into_iter().map(|(e, _)| e).collect(),
        kw_hits,
    ]);

    item("Fused results (Reciprocal Rank Fusion, k=60):");
    blank();
    for (i, result) in fused.iter().take(7).enumerate() {
        if let Some(ref entity) = result.entity {
            detail(&format!(
                "{}. {:<28} ({:<14})  rrf: {:.6}",
                i + 1,
                entity.name,
                entity.entity_type,
                result.score,
            ));
        }
    }

    end_section();

    // ═══════════════════════════════════════════════════════════════════════
    // 8. Query Expansion
    // ═══════════════════════════════════════════════════════════════════════

    section("8 · Query Expansion (sparse-result fallback)");

    let expander = QueryExpander::new(3);
    let sparse_query = "simulation physics engine";

    let sparse_emb = embedder.embed(sparse_query).await?;
    let initial = repo.search_entities_by_vector(&sparse_emb, 5).await?;

    tracing::info!(
        query = sparse_query,
        initial_hits = initial.len(),
        threshold = expander.threshold,
        "Evaluating query expansion"
    );

    item(&format!("Query: \"{sparse_query}\""));
    item(&format!(
        "  Initial results : {}  ·  Threshold : {}  ·  Expand? {}",
        initial.len(),
        expander.threshold,
        expander.should_expand(initial.len()),
    ));
    blank();

    let variants = expander.expand(sparse_query, &query_rewriter).await?;
    item("Expanded variants:");
    for (i, v) in variants.iter().enumerate() {
        detail(&format!("{i}. \"{v}\""));
    }
    blank();

    let mut expanded_lists: Vec<Vec<Entity>> =
        vec![initial.into_iter().map(|(e, _)| e).collect()];

    for variant in &variants[1..] {
        let emb = embedder.embed(variant).await?;
        let hits = repo.search_entities_by_vector(&emb, 5).await?;
        expanded_lists.push(hits.into_iter().map(|(e, _)| e).collect());
    }

    let expanded_fused = fuse_rrf(expanded_lists);
    item(&format!(
        "After expansion + RRF: {} results",
        expanded_fused.len()
    ));
    for (i, r) in expanded_fused.iter().take(5).enumerate() {
        if let Some(ref entity) = r.entity {
            detail(&format!(
                "{}. {:<28} — rrf: {:.6}",
                i + 1,
                entity.name,
                r.score,
            ));
        }
    }

    end_section();

    // ═══════════════════════════════════════════════════════════════════════
    // 9. Temporal Versioning
    // ═══════════════════════════════════════════════════════════════════════

    section("9 · Temporal Versioning (time-travel queries)");

    let old_date = Utc::now() - Duration::days(90);

    let nextera_v1 = Entity {
        id: Uuid::new_v4(),
        name: "Nextera Robotics".into(),
        entity_type: "organization".into(),
        summary: "Small robotics startup with 30 employees, pre-Series A".into(),
        embedding: embedder.embed("Nextera Robotics small startup pre-Series A").await?,
        valid_from: old_date,
        valid_until: Some(Utc::now() - Duration::days(30)),
    };
    repo.upsert_entity(&nextera_v1).await?;

    let nextera_v2 = Entity {
        id: Uuid::new_v4(),
        name: "Nextera Robotics".into(),
        entity_type: "organization".into(),
        summary: "Growing AI robotics company, 120 employees, Series B funded, partnering with TU Munich".into(),
        embedding: embedder
            .embed("Nextera Robotics growing company Series B TU Munich partner")
            .await?,
        valid_from: Utc::now() - Duration::days(30),
        valid_until: None,
    };
    repo.upsert_entity(&nextera_v2).await?;

    tracing::info!("Created two temporal versions of Nextera Robotics");

    item("Two versions of 'Nextera Robotics':");
    item(&format!(
        "  v1: \"{}\"  (valid {} → {})",
        nextera_v1.summary,
        nextera_v1.valid_from.format("%Y-%m-%d"),
        nextera_v1
            .valid_until
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or("present".into()),
    ));
    item(&format!(
        "  v2: \"{}\"  (valid {} → present)",
        nextera_v2.summary,
        nextera_v2.valid_from.format("%Y-%m-%d"),
    ));
    blank();

    let past = Utc::now() - Duration::days(60);
    let past_snap = repo.entities_at(past).await?;
    tracing::info!(
        at = %past.format("%Y-%m-%d"),
        count = past_snap.len(),
        "Point-in-time entity snapshot"
    );

    item(&format!(
        "Snapshot 60 days ago ({}) — {} entities active:",
        past.format("%Y-%m-%d"),
        past_snap.len(),
    ));
    for e in past_snap.iter().take(8) {
        detail(&format!("· {} ({})", e.name, e.entity_type));
    }
    if past_snap.len() > 8 {
        detail(&format!("  ... and {} more", past_snap.len() - 8));
    }
    blank();

    let now_snap = repo.entities_at(Utc::now()).await?;
    item(&format!(
        "Snapshot now — {} entities active:",
        now_snap.len(),
    ));
    for e in now_snap.iter().take(8) {
        detail(&format!("· {} ({})", e.name, e.entity_type));
    }
    if now_snap.len() > 8 {
        detail(&format!("  ... and {} more", now_snap.len() - 8));
    }

    end_section();

    // ═══════════════════════════════════════════════════════════════════════
    // 10. Staleness Scoring
    // ═══════════════════════════════════════════════════════════════════════

    section("10 · Staleness Scoring");

    tracing::info!("Computing staleness scores");

    item("How stale is each entity? (days since valid_from)");
    blank();

    let mut scored: Vec<(String, f64)> = Vec::new();
    scored.push((nextera_v2.name.clone(), staleness_score(&nextera_v2)));

    let active = repo.get_all_active_entities().await?;
    for e in &active {
        scored.push((e.name.clone(), staleness_score(e)));
    }
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    scored.dedup_by(|a, b| a.0 == b.0);

    for (name, score) in scored.iter().take(12) {
        let bar_len = (*score as usize).min(40);
        let bar: String = "█".repeat(bar_len);
        detail(&format!("{:<28} {:>5.1}d  {bar}", name, score));
    }

    end_section();

    // ═══════════════════════════════════════════════════════════════════════
    // 11. Graph Traversal
    // ═══════════════════════════════════════════════════════════════════════

    section("11 · Graph Traversal (entity neighbors & relations)");

    if let Some(first_entity) = all_entities.first() {
        tracing::info!(entity = %first_entity.name, "Fetching graph neighbors");
        let neighbors = repo.get_graph_neighbors(&[first_entity.id], 1).await?;
        item(&format!(
            "Neighbors of \"{}\" (1-hop): {} found",
            first_entity.name,
            neighbors.len(),
        ));
        for n in neighbors.iter().take(5) {
            detail(&format!("· {} ({}) — {}", n.name, n.entity_type, n.summary));
        }
        blank();

        let rels = repo.get_relations_for_entity(first_entity.id).await?;
        item(&format!(
            "Relations for \"{}\" ({} total):",
            first_entity.name,
            rels.len(),
        ));
        for rel in rels.iter().take(5) {
            let target = all_entities
                .iter()
                .find(|e| e.id == rel.to_entity_id)
                .map(|e| e.name.as_str())
                .unwrap_or("?");
            detail(&format!(
                "{} ──[{} ({}%)]──▸ {}",
                first_entity.name, rel.relation_type, rel.confidence, target,
            ));
        }
    } else {
        item("No entities available for graph traversal demo");
    }

    end_section();

    // ═══════════════════════════════════════════════════════════════════════
    // 12. SurrealVectorStore (convenience API)
    // ═══════════════════════════════════════════════════════════════════════

    section("12 · SurrealVectorStore (convenience top-k API)");

    let db2 = connect_memory(&config).await?;
    apply_schema(&db2, &config).await?;
    let repo2 = Repository::new(db2);

    for e in &all_entities {
        repo2.upsert_entity(e).await?;
    }

    let store = SurrealVectorStore::new(repo2);
    let store_query = "knowledge graph temporal reasoning";
    let store_emb = embedder.embed(store_query).await?;

    tracing::info!(query = store_query, "SurrealVectorStore top-k entity search");
    let top_k = store.top_k_entities(&store_emb, 5).await?;

    item(&format!("Query: \"{store_query}\""));
    blank();
    for (i, r) in top_k.iter().enumerate() {
        let name = r
            .entity
            .as_ref()
            .map(|e| e.name.as_str())
            .unwrap_or("?");
        detail(&format!(
            "{}. {:<28} score: {:.4}  │  \"{}\"",
            i + 1,
            name,
            r.score,
            &r.content[..70.min(r.content.len())],
        ));
    }

    end_section();

    // ═══════════════════════════════════════════════════════════════════════
    // 13. Recent Memories
    // ═══════════════════════════════════════════════════════════════════════

    section("13 · Recent Memories");

    let memories = repo.list_recent_memories(6).await?;
    tracing::info!(count = memories.len(), "Listing recent memories");

    for (i, m) in memories.iter().enumerate() {
        let truncated: String = m.content.chars().take(85).collect();
        item(&format!("{}. \"{truncated}...\"", i + 1));
        detail(&format!(
            "   entities: {}  ·  created: {}",
            m.entity_ids.len(),
            m.created_at.format("%H:%M:%S"),
        ));
    }

    end_section();

    // ═══════════════════════════════════════════════════════════════════════
    // 14. Temporal Snapshot (combined view)
    // ═══════════════════════════════════════════════════════════════════════

    section("14 · Temporal Snapshot (entities + relations at a point in time)");

    let snap_time = Utc::now();
    let snap_entities = repo.entities_at(snap_time).await?;
    let snap_relations = repo.relations_at(snap_time).await?;

    let snapshot = TemporalSnapshot {
        entities: snap_entities,
        relations: snap_relations,
        timestamp: snap_time,
    };

    tracing::info!(
        at = %snapshot.timestamp.format("%Y-%m-%d %H:%M"),
        entities = snapshot.entities.len(),
        relations = snapshot.relations.len(),
        "Built temporal snapshot"
    );

    item(&format!(
        "Snapshot at {}",
        snapshot.timestamp.format("%Y-%m-%d %H:%M")
    ));
    item(&format!("  Active entities  : {}", snapshot.entities.len()));
    item(&format!("  Active relations : {}", snapshot.relations.len()));

    end_section();

    // ═══════════════════════════════════════════════════════════════════════
    // Fin
    // ═══════════════════════════════════════════════════════════════════════

    banner("LLM Demo Complete — All Features Showcased");

    println!("  Demonstrated with live LLM calls:");
    println!("    1.  Environment-based LLM configuration");
    println!("    2.  In-memory SurrealDB with HNSW + BM25 indexes");
    println!("    3.  Rig-powered LLM services (embeddings, extraction)");
    println!("    4.  Full ingestion pipeline on rich documents");
    println!("    5.  HNSW vector search (entities and memories)");
    println!("    6.  BM25 full-text search (entities, episodes)");
    println!("    7.  Hybrid RRF fusion (vector + keyword)");
    println!("    8.  Query expansion with semantic variants");
    println!("    9.  Temporal versioning with time-travel queries");
    println!("   10.  Staleness scoring");
    println!("   11.  Graph traversal (neighbors, relations)");
    println!("   12.  SurrealVectorStore convenience API");
    println!("   13.  Recent memory listing");
    println!("   14.  Temporal snapshots");
    println!();

    Ok(())
}
