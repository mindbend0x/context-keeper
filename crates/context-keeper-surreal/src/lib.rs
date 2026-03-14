pub mod client;
pub mod repository;
pub mod schema;
pub mod vector_store;

pub use client::{connect_memory, SurrealConfig};
pub use repository::Repository;
pub use schema::apply_schema;
