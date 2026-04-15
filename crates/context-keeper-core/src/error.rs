use thiserror::Error;

/// Typed error hierarchy for Context Keeper.
///
/// Each variant represents a distinct failure mode so callers can
/// pattern-match and surface appropriate user-facing messages.
#[derive(Debug, Error)]
pub enum ContextKeeperError {
    #[error("LLM service unavailable: {0}")]
    LlmUnavailable(String),

    #[error("extraction failed: {0}")]
    ExtractionFailed(String),

    #[error("entity not found: {0}")]
    EntityNotFound(String),

    #[error("storage error: {0}")]
    StorageError(String),

    #[error("validation error: {0}")]
    ValidationError(String),

    #[error("budget exceeded: {0}")]
    BudgetExceeded(String),

    #[error("embedding failed: {0}")]
    EmbeddingFailed(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, ContextKeeperError>;
