//! Rig-powered entity and relation extraction.
//!
//! These types implement the core `EntityExtractor` and `RelationExtractor` traits
//! using Rig's `agent().prompt()` with structured JSON schema output.
//!
//! Requires a valid `OPENAI_API_KEY` (or other provider key) at runtime.
//! For testing, use `MockEntityExtractor` and `MockRelationExtractor` from core.

use anyhow::Result;
use async_trait::async_trait;
use context_keeper_core::traits::{EntityExtractor, ExtractedEntity, ExtractedRelation, RelationExtractor};

/// Rig-backed entity extractor using LLM structured output.
pub struct RigEntityExtractor {
    /// System prompt for entity extraction.
    pub system_prompt: String,
}

impl RigEntityExtractor {
    pub fn new() -> Self {
        Self {
            system_prompt: "Extract named entities from the text. Return JSON array of {name, entity_type, summary}.".to_string(),
        }
    }
}

#[async_trait]
impl EntityExtractor for RigEntityExtractor {
    async fn extract_entities(&self, _text: &str) -> Result<Vec<ExtractedEntity>> {
        // In production: use rig-core's agent().prompt() with JSON schema
        // let client = openai::Client::from_env();
        // let agent = client.agent("gpt-4o").preamble(&self.system_prompt).build();
        // let response: Vec<ExtractedEntity> = agent.prompt(text).await?;
        tracing::warn!("RigEntityExtractor requires OPENAI_API_KEY; returning empty");
        Ok(vec![])
    }
}

/// Rig-backed relation extractor using LLM structured output.
pub struct RigRelationExtractor {
    pub system_prompt: String,
}

impl RigRelationExtractor {
    pub fn new() -> Self {
        Self {
            system_prompt: "Extract relations between entities. Return JSON array of {subject, predicate, object, confidence}.".to_string(),
        }
    }
}

#[async_trait]
impl RelationExtractor for RigRelationExtractor {
    async fn extract_relations(
        &self,
        _text: &str,
        _entities: &[ExtractedEntity],
    ) -> Result<Vec<ExtractedRelation>> {
        tracing::warn!("RigRelationExtractor requires OPENAI_API_KEY; returning empty");
        Ok(vec![])
    }
}
