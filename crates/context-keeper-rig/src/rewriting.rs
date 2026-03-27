//! Rig-powered query rewriting for search expansion.
//!
//! Implements the core `QueryRewriter` trait using Rig's LLM completions
//! to generate semantic variants of a search query.

use async_trait::async_trait;
use context_keeper_core::error::Result;
use context_keeper_core::ContextKeeperError;
use context_keeper_core::traits::QueryRewriter;
use rig::providers::openai;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const QUERY_REWRITE_PROMPT: &str = "\
You are a search query expansion assistant. Given a search query, generate 3-5 semantically \
related alternative queries that could help find relevant information. Return the variants as \
a JSON object with a \"variants\" array of strings. Include the original query as the first variant.";

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct QueryVariants {
    pub variants: Vec<String>,
}

/// Rig-backed query rewriter using LLM structured output.
pub struct RigQueryRewriter {
    pub model: String,
    client: openai::Client,
}

impl RigQueryRewriter {
    pub fn new(api_url: &str, api_key: &str, model_name: &str) -> Self {
        let openai_client = openai::Client::builder()
            .base_url(api_url)
            .api_key(api_key)
            .build()
            .expect("Failed to create OpenAI client");

        Self {
            model: model_name.to_string(),
            client: openai_client,
        }
    }
}

#[async_trait]
impl QueryRewriter for RigQueryRewriter {
    async fn rewrite(&self, query: &str) -> Result<Vec<String>> {
        let builder = self.client.extractor::<QueryVariants>(&self.model);

        let result: QueryVariants = builder
            .preamble(QUERY_REWRITE_PROMPT)
            .build()
            .extract(query)
            .await
            .map_err(|e| ContextKeeperError::ExtractionFailed(e.to_string()))?;

        Ok(result.variants)
    }
}
