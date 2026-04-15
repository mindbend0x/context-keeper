use context_keeper_core::error::Result;
use context_keeper_core::models::DistanceMetric;
use context_keeper_core::ContextKeeperError;
use surrealdb::engine::any::Any;
use surrealdb::opt::auth::Root;
use surrealdb::Surreal;

/// Storage backend for SurrealDB.
#[derive(Debug, Clone)]
pub enum StorageBackend {
    Memory,
    RocksDb(String),
    /// Remote SurrealDB server via WebSocket (e.g. `ws://localhost:8000`).
    Remote(String),
}

/// Configuration for connecting to SurrealDB.
#[derive(Debug, Clone)]
pub struct SurrealConfig {
    pub namespace: String,
    pub database: String,
    pub embedding_dimensions: usize,
    pub distance_metric: DistanceMetric,
    pub storage: StorageBackend,
    /// Root credentials for remote connections (optional for embedded).
    pub username: Option<String>,
    pub password: Option<String>,
}

impl Default for SurrealConfig {
    fn default() -> Self {
        Self {
            namespace: "context_keeper".to_string(),
            database: "main".to_string(),
            embedding_dimensions: 1536,
            distance_metric: DistanceMetric::default(),
            storage: StorageBackend::Memory,
            username: None,
            password: None,
        }
    }
}

/// Connect to SurrealDB using the configured storage backend.
///
/// Supports in-memory, RocksDB, and remote WebSocket connections via `Surreal<Any>`.
pub async fn connect(config: &SurrealConfig) -> Result<Surreal<Any>> {
    let endpoint = match &config.storage {
        StorageBackend::Memory => {
            tracing::info!("Connecting to in-memory SurrealDB");
            "mem://".to_string()
        }
        StorageBackend::RocksDb(path) => {
            tracing::info!(path = %path, "Connecting to RocksDB SurrealDB");
            format!("rocksdb:{}", path)
        }
        StorageBackend::Remote(url) => {
            tracing::info!(url = %url, "Connecting to remote SurrealDB");
            url.clone()
        }
    };

    let db = surrealdb::engine::any::connect(&endpoint)
        .await
        .map_err(|e| ContextKeeperError::StorageError(e.to_string()))?;

    // Authenticate for remote connections
    if matches!(&config.storage, StorageBackend::Remote(_)) {
        if let (Some(user), Some(pass)) = (&config.username, &config.password) {
            tracing::info!("Authenticating with root credentials");
            db.signin(Root {
                username: user.to_string(),
                password: pass.to_string(),
            })
            .await
            .map_err(|e| {
                ContextKeeperError::StorageError(format!("Authentication failed: {}", e))
            })?;
        } else {
            tracing::warn!(
                "Remote connection without credentials — anonymous access may be denied"
            );
        }
    }

    db.use_ns(&config.namespace)
        .use_db(&config.database)
        .await
        .map_err(|e| ContextKeeperError::StorageError(e.to_string()))?;

    tracing::info!(
        ns = %config.namespace,
        db = %config.database,
        "Connected to SurrealDB"
    );
    Ok(db)
}

/// Create an embedded in-memory SurrealDB instance (convenience wrapper).
pub async fn connect_memory(config: &SurrealConfig) -> Result<Surreal<Any>> {
    let db = surrealdb::engine::any::connect("mem://")
        .await
        .map_err(|e| ContextKeeperError::StorageError(e.to_string()))?;
    db.use_ns(&config.namespace)
        .use_db(&config.database)
        .await
        .map_err(|e| ContextKeeperError::StorageError(e.to_string()))?;
    tracing::info!(
        ns = %config.namespace,
        db = %config.database,
        "Connected to in-memory SurrealDB"
    );
    Ok(db)
}
