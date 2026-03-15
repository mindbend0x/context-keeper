use anyhow::Result;
use context_keeper_core::models::DistanceMetric;
use surrealdb::engine::local::{Db, Mem, RocksDb};
use surrealdb::Surreal;

/// Storage backend for SurrealDB.
#[derive(Debug, Clone)]
pub enum StorageBackend {
    Memory,
    RocksDb(String),
}

/// Configuration for connecting to SurrealDB.
#[derive(Debug, Clone)]
pub struct SurrealConfig {
    pub namespace: String,
    pub database: String,
    pub embedding_dimensions: usize,
    pub distance_metric: DistanceMetric,
    pub storage: StorageBackend,
}

impl Default for SurrealConfig {
    fn default() -> Self {
        Self {
            namespace: "context_keeper".to_string(),
            database: "main".to_string(),
            embedding_dimensions: 1536,
            distance_metric: DistanceMetric::default(),
            storage: StorageBackend::Memory,
        }
    }
}

/// Connect to SurrealDB using the configured storage backend.
pub async fn connect(config: &SurrealConfig) -> Result<Surreal<Db>> {
    let db = match &config.storage {
        StorageBackend::Memory => {
            tracing::info!("Connecting to in-memory SurrealDB");
            Surreal::new::<Mem>(()).await?
        }
        StorageBackend::RocksDb(path) => {
            tracing::info!(path = %path, "Connecting to RocksDB SurrealDB");
            Surreal::new::<RocksDb>(path.as_str()).await?
        }
    };

    db.use_ns(&config.namespace)
        .use_db(&config.database)
        .await?;

    tracing::info!(
        ns = %config.namespace,
        db = %config.database,
        "Connected to SurrealDB"
    );
    Ok(db)
}

/// Create an embedded in-memory SurrealDB instance (convenience wrapper).
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
