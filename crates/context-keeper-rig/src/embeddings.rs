//! Rig-powered embedding service.
//!
//! Implements the core `Embedder` trait using Rig's `EmbeddingModel`.

use anyhow::Result;
use async_trait::async_trait;
use context_keeper_core::traits::Embedder;
use rig::client::EmbeddingsClient;
use rig::embeddings::EmbeddingModel;
use rig::providers::openai;

/// Rig-backed embedding service.
pub struct RigEmbedder {
    pub model_name: String,
    pub dimension: usize,
    pub model: openai::EmbeddingModel,
}

impl RigEmbedder {
    pub fn new(api_url: &str, api_key: &str, model_name: &str, dimension: usize) -> Self {
        let openai_client = openai::Client::builder()
            .base_url(api_url)
            .api_key(api_key)
            .build()
            .expect("Failed to create OpenAI client");
        
        let model = openai_client
            .embedding_model_with_ndims(model_name, dimension);

        Self {
            model_name: model_name.to_string(),
            dimension,
            model,
        }
    }
}

#[async_trait]
impl Embedder for RigEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f64>> {
        let embeddings = self.model.embed_text(text).await?;
        Ok(embeddings.vec)   
    }
}
