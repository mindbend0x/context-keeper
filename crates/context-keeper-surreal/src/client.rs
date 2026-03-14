use anyhow::Result;
use surrealdb::engine::local::{Db, Mem};
use surrealdb::Surreal;

/// Configuration for connecting to SurrealDB.
#[derive(Debug, Clone)]
pub struct SurrealConfig {
    pub namespace: String,
    pub database: String,
}

impl Default for SurrealConfig {
    fn default() -> Self {
        Self {
            namespace: "context_keeper".to_string(),
            database: "main".to_string(),
        }
    }
}

/// Create an embedded in-memory SurrealDB instance.
/// Perfect for testing and development.
pub async fn connect_memory(config: &SurrealConfig) -> Result<Surreal<Db>> {
    let db = Surreal::new::<Mem>(()).await?;
    db.use_ns(&config.namespace)
        .use_db(&config.database)
        .await?;
    tracing::info!(
        ns = %config.namespace,
        db = %config.database,
        "Connected to in-memory SurrealDB"
    );
    Ok(db)
}
