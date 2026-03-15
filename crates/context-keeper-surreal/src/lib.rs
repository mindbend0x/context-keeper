pub mod client;
pub mod repository;
pub mod schema;
pub mod vector_store;

pub use client::{connect, connect_memory, StorageBackend, SurrealConfig};
pub use repository::Repository;
pub use schema::apply_schema;
