use anyhow::Result;
use surrealdb::engine::local::Db;
use surrealdb::Surreal;

use crate::client::SurrealConfig;

/// Generate the full SurrealQL schema dynamically from config.
fn build_schema(config: &SurrealConfig) -> String {
    let dim = config.embedding_dimensions;
    let dist = config.distance_metric.to_string();

    format!(
        r#"
-- ── Node tables ──────────────────────────────────────────────────────

DEFINE TABLE IF NOT EXISTS episode SCHEMAFULL CHANGEFEED 30d;
DEFINE FIELD IF NOT EXISTS content ON episode TYPE string;
DEFINE FIELD IF NOT EXISTS source ON episode TYPE string;
DEFINE FIELD IF NOT EXISTS session_id ON episode TYPE option<string>;
DEFINE FIELD IF NOT EXISTS created_at ON episode TYPE datetime;

DEFINE TABLE IF NOT EXISTS entity SCHEMAFULL CHANGEFEED 30d;
DEFINE FIELD IF NOT EXISTS name ON entity TYPE string;
DEFINE FIELD IF NOT EXISTS entity_type ON entity TYPE string;
DEFINE FIELD IF NOT EXISTS summary ON entity TYPE string;
DEFINE FIELD IF NOT EXISTS embedding ON entity TYPE array<float>;
DEFINE FIELD IF NOT EXISTS valid_from ON entity TYPE datetime;
DEFINE FIELD IF NOT EXISTS valid_until ON entity TYPE option<datetime>;

DEFINE TABLE IF NOT EXISTS memory SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS content ON memory TYPE string;
DEFINE FIELD IF NOT EXISTS embedding ON memory TYPE array<float>;
DEFINE FIELD IF NOT EXISTS created_at ON memory TYPE datetime;

-- ── Graph edge tables (TYPE RELATION) ────────────────────────────────

DEFINE TABLE IF NOT EXISTS relates_to TYPE RELATION SCHEMAFULL CHANGEFEED 30d;
DEFINE FIELD IF NOT EXISTS relation_type ON relates_to TYPE string;
DEFINE FIELD IF NOT EXISTS confidence ON relates_to TYPE int;
DEFINE FIELD IF NOT EXISTS valid_from ON relates_to TYPE datetime;
DEFINE FIELD IF NOT EXISTS valid_until ON relates_to TYPE option<datetime>;

DEFINE TABLE IF NOT EXISTS sourced_from TYPE RELATION SCHEMAFULL;

DEFINE TABLE IF NOT EXISTS references TYPE RELATION SCHEMAFULL;

-- ── HNSW Vector Indexes ──────────────────────────────────────────────

DEFINE INDEX IF NOT EXISTS entity_embedding_idx ON entity FIELDS embedding
  HNSW DIMENSION {dim} DIST {dist};
DEFINE INDEX IF NOT EXISTS memory_embedding_idx ON memory FIELDS embedding
  HNSW DIMENSION {dim} DIST {dist};

-- ── BM25 Full-Text Search ────────────────────────────────────────────

DEFINE ANALYZER IF NOT EXISTS context_analyzer TOKENIZERS blank,class FILTERS lowercase,ascii,snowball(english);

DEFINE INDEX IF NOT EXISTS entity_name_ft ON entity FIELDS name
  FULLTEXT ANALYZER context_analyzer BM25;
DEFINE INDEX IF NOT EXISTS entity_summary_ft ON entity FIELDS summary
  FULLTEXT ANALYZER context_analyzer BM25;
DEFINE INDEX IF NOT EXISTS memory_content_ft ON memory FIELDS content
  FULLTEXT ANALYZER context_analyzer BM25;
DEFINE INDEX IF NOT EXISTS episode_content_ft ON episode FIELDS content
  FULLTEXT ANALYZER context_analyzer BM25;

-- ── Unique index for entity upsert by name ───────────────────────────

DEFINE INDEX IF NOT EXISTS entity_name_unique ON entity FIELDS name UNIQUE;
"#
    )
}

/// Apply the Context Keeper graph schema to SurrealDB.
///
/// Creates SCHEMAFULL node tables (episode, entity, memory), graph edge
/// tables (relates_to, sourced_from, references), HNSW vector indexes,
/// BM25 full-text indexes, and changefeeds for temporal auditing.
pub async fn apply_schema(db: &Surreal<Db>, config: &SurrealConfig) -> Result<()> {
    tracing::info!(
        dim = config.embedding_dimensions,
        dist = %config.distance_metric,
        "Applying Context Keeper graph schema"
    );
    let schema = build_schema(config);
    db.query(schema).await?.check()?;
    tracing::info!("Schema applied successfully");
    Ok(())
}
