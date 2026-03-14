use anyhow::Result;
use surrealdb::engine::local::Db;
use surrealdb::Surreal;

/// Apply the Context Keeper schema to SurrealDB.
///
/// Creates tables for episodes, entities, relations, and memories
/// with BM25 full-text indexes. Vector indexes are omitted for the
/// in-memory engine (HNSW requires specific SurrealDB builds);
/// vector search falls back to brute-force cosine in the repository layer.
pub async fn apply_schema(db: &Surreal<Db>) -> Result<()> {
    tracing::info!("Applying Context Keeper schema");
    db.query(SCHEMA_SURREALQL).await?.check()?;
    tracing::info!("Schema applied successfully");
    Ok(())
}

/// Schema definition as SurrealQL.
/// Uses SCHEMALESS tables for SDK compatibility. Vector search is done
/// via brute-force cosine in the repository layer (no HNSW in embedded mode).
pub const SCHEMA_SURREALQL: &str = "
DEFINE TABLE episode SCHEMALESS;
DEFINE TABLE entity SCHEMALESS;
DEFINE TABLE relation SCHEMALESS;
DEFINE TABLE memory SCHEMALESS;
";
