pub mod client;
pub mod repository;
pub mod schema;
pub mod vector_store;

pub use client::{connect, connect_memory, StorageBackend, SurrealConfig};
pub use repository::Repository;
pub use schema::apply_schema;

/// Returns the default storage backend string: `rocksdb:~/.context-keeper/data`
/// with `~` expanded to the actual home directory.
pub fn default_storage_string() -> String {
    match dirs::home_dir() {
        Some(home) => format!(
            "rocksdb:{}",
            home.join(".context-keeper").join("data").display()
        ),
        None => "memory".to_string(),
    }
}

pub fn parse_storage_backend(s: &str) -> StorageBackend {
    if let Some(path) = s.strip_prefix("rocksdb:") {
        StorageBackend::RocksDb(path.to_string())
    } else if let Some(url) = s.strip_prefix("remote:") {
        StorageBackend::Remote(url.to_string())
    } else {
        StorageBackend::Memory
    }
}
