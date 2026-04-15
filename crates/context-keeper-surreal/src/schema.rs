use context_keeper_core::error::Result;
use context_keeper_core::ContextKeeperError;
use surrealdb::engine::any::Any;
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
DEFINE FIELD IF NOT EXISTS agent_id ON episode TYPE option<string>;
DEFINE FIELD IF NOT EXISTS agent_name ON episode TYPE option<string>;
DEFINE FIELD IF NOT EXISTS machine_id ON episode TYPE option<string>;
DEFINE FIELD IF NOT EXISTS namespace ON episode TYPE option<string>;
DEFINE FIELD IF NOT EXISTS created_at ON episode TYPE datetime;

DEFINE TABLE IF NOT EXISTS entity SCHEMAFULL CHANGEFEED 30d;
DEFINE FIELD IF NOT EXISTS name ON entity TYPE string;
DEFINE FIELD IF NOT EXISTS entity_type ON entity TYPE string;
DEFINE FIELD IF NOT EXISTS summary ON entity TYPE string;
DEFINE FIELD IF NOT EXISTS embedding ON entity TYPE array<float>;
DEFINE FIELD IF NOT EXISTS valid_from ON entity TYPE datetime;
DEFINE FIELD IF NOT EXISTS valid_until ON entity TYPE option<datetime>;
DEFINE FIELD IF NOT EXISTS namespace ON entity TYPE option<string>;
DEFINE FIELD IF NOT EXISTS created_by_agent ON entity TYPE option<string>;

DEFINE TABLE IF NOT EXISTS memory SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS content ON memory TYPE string;
DEFINE FIELD IF NOT EXISTS embedding ON memory TYPE array<float>;
DEFINE FIELD IF NOT EXISTS created_at ON memory TYPE datetime;
DEFINE FIELD IF NOT EXISTS namespace ON memory TYPE option<string>;
DEFINE FIELD IF NOT EXISTS created_by_agent ON memory TYPE option<string>;

DEFINE TABLE IF NOT EXISTS note SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS key ON note TYPE string;
DEFINE FIELD IF NOT EXISTS content ON note TYPE string;
DEFINE FIELD IF NOT EXISTS embedding ON note TYPE array<float>;
DEFINE FIELD IF NOT EXISTS tags ON note TYPE array<string>;
DEFINE FIELD IF NOT EXISTS namespace ON note TYPE option<string>;
DEFINE FIELD IF NOT EXISTS created_at ON note TYPE datetime;
DEFINE FIELD IF NOT EXISTS updated_at ON note TYPE datetime;

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
DEFINE INDEX IF NOT EXISTS note_embedding_idx ON note FIELDS embedding
  HNSW DIMENSION {dim} DIST {dist};

-- ── BM25 Full-Text Search ────────────────────────────────────────────

DEFINE ANALYZER IF NOT EXISTS context_analyzer TOKENIZERS blank,class FILTERS lowercase,ascii,snowball(english);

DEFINE INDEX IF NOT EXISTS entity_name_ft ON entity FIELDS name
  FULLTEXT ANALYZER context_analyzer BM25;
DEFINE INDEX IF NOT EXISTS entity_summary_ft ON entity FIELDS summary
  FULLTEXT ANALYZER context_analyzer BM25;
DEFINE INDEX IF NOT EXISTS memory_content_ft ON memory FIELDS content
  FULLTEXT ANALYZER context_analyzer BM25;
DEFINE INDEX IF NOT EXISTS note_content_ft ON note FIELDS content
  FULLTEXT ANALYZER context_analyzer BM25;
DEFINE INDEX IF NOT EXISTS episode_content_ft ON episode FIELDS content
  FULLTEXT ANALYZER context_analyzer BM25;

-- ── Composite identity index ─────────────────────────────────────────
-- Entity identity is (name, entity_type). "Alice (Person)" and
-- "Alice (Organization)" are distinct graph nodes.
-- Namespace further scopes: same (name, type) in different namespaces
-- are separate entities. Uniqueness enforced by the EntityResolver
-- at the application level because SurrealDB UNIQUE indexes treat
-- NONE (null namespace) values as distinct.

DEFINE INDEX IF NOT EXISTS entity_name_idx ON entity FIELDS name;
DEFINE INDEX IF NOT EXISTS entity_identity_idx ON entity FIELDS name, entity_type, namespace;

-- ── Entity type index for type-filtered queries ──────────────────────

DEFINE INDEX IF NOT EXISTS entity_type_idx ON entity FIELDS entity_type;

-- ── Namespace indexes for multi-agent scoping ────────────────────────

DEFINE INDEX IF NOT EXISTS episode_namespace_idx ON episode FIELDS namespace;
DEFINE INDEX IF NOT EXISTS entity_namespace_idx ON entity FIELDS namespace;
DEFINE INDEX IF NOT EXISTS memory_namespace_idx ON memory FIELDS namespace;
DEFINE INDEX IF NOT EXISTS episode_agent_idx ON episode FIELDS agent_id;

-- ── Note indexes ────────────────────────────────────────────────────

DEFINE INDEX IF NOT EXISTS note_key_ns_idx ON note FIELDS key, namespace;
DEFINE INDEX IF NOT EXISTS note_namespace_idx ON note FIELDS namespace;
DEFINE INDEX IF NOT EXISTS note_tags_idx ON note FIELDS tags;
"#
    )
}

/// Apply the Context Keeper graph schema to SurrealDB.
///
/// Creates SCHEMAFULL node tables (episode, entity, memory), graph edge
/// tables (relates_to, sourced_from, references), HNSW vector indexes,
/// BM25 full-text indexes, and changefeeds for temporal auditing.
pub async fn apply_schema(db: &Surreal<Any>, config: &SurrealConfig) -> Result<()> {
    tracing::info!(
        dim = config.embedding_dimensions,
        dist = %config.distance_metric,
        "Applying Context Keeper graph schema"
    );
    let schema = build_schema(config);
    db.query(schema)
        .await
        .map_err(|e| ContextKeeperError::StorageError(e.to_string()))?
        .check()
        .map_err(|e| ContextKeeperError::StorageError(e.to_string()))?;
    tracing::info!("Schema applied successfully");
    Ok(())
}
