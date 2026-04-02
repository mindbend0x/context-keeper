//! Terminal UI and backends for Context Keeper.

pub mod backend;
pub mod bootstrap;
pub mod error;
pub mod types;
pub mod ui;

#[cfg(feature = "remote-mcp")]
pub use backend::McpHttpBackend;
pub use backend::{LocalBackend, TuiBackend};
