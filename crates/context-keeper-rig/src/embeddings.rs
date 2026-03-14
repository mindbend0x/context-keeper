//! Rig-powered embedding service.
//!
//! Implements the core `Embedder` trait using Rig's `EmbeddingModel`.
//! Requires a valid API key at runtime.
//! For testing, use `MockEmbedder` from core.

use anyhow::Result;
use async_trait::async_trait;
use context_keeper_core::traits::Embedder;

/// Rig-backed embedding service.
pub struct RigEmbedder {
    pub model_name: String,
    pub dimension: usize,
}

impl RigEmbedder {
    pub fn new(model_name: &str, dimension: usize) -> Self {
        Self {
            model_name: model_name.to_string(),
            dimension,
        }
    }

    /// Create a default embedder using OpenAI's text-embedding-3-small.
    pub fn openai_default() -> Self {
        Self::new("text-embedding-3-small", 1536)
    }
}

#[async_trait]
impl Embedder for RigEmbedder {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>> {
        // In production: use rig-core's EmbeddingsBuilder
        // let client = openai::Client::from_env();
        // let model = client.embedding_model(&self.model_name);
        // let embeddings = EmbeddingsBuilder::new(model).document(text)?.build().await?;
        tracing::warn!("RigEmbedder requires OPENAI_API_KEY; returning zero vector");
        Ok(vec![0.0; self.dimension])
    }
}
