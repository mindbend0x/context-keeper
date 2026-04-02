//! TUI-specific errors (typed per project conventions).

use context_keeper_core::ContextKeeperError;

#[derive(Debug, thiserror::Error)]
pub enum TuiError {
    #[error("storage: {0}")]
    Storage(String),

    #[error("domain: {0}")]
    Domain(#[from] ContextKeeperError),

    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("MCP: {0}")]
    #[cfg(feature = "remote-mcp")]
    Mcp(String),

    #[error("{0}")]
    Other(String),
}

impl TuiError {
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}
