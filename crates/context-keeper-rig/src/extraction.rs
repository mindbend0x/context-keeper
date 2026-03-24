//! Rig-powered entity and relation extraction.
//!
//! These types implement the core `EntityExtractor` and `RelationExtractor` traits
//! using Rig's `agent().prompt()` with structured JSON schema output.
//!
//! Requires a valid `OPENAI_API_KEY` (or other provider key) at runtime.
//! For testing, use `MockEntityExtractor` and `MockRelationExtractor` from core.

use anyhow::Result;
use async_trait::async_trait;
use context_keeper_core::traits::{
    EntityExtractor, ExtractedEntity,
    ExtractedRelation, RelationExtractor
};
use rig::providers::openai;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

const ENTITY_EXTRACTION_PROMPT: &str = "\
Extract named entities from the text. Classify each entity as one of: \
person, organization, location, event, product, service, concept, file, other. \
Return JSON array of {name, entity_type, summary}.";

const RELATION_EXTRACTION_PROMPT: &str = "\
Extract relations between the given entities. \
The predicate MUST be one of: works_at, located_in, part_of, uses, \
created_by, knows, depends_on, related_to. \
Assign a confidence score from 0 to 100. \
Return JSON array of {subject, predicate, object, confidence}.";

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RigExtractedEntities {
    pub entities: Vec<ExtractedEntity>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RigExtractedRelations {
    pub relations: Vec<ExtractedRelation>,
}

/// Rig-backed entity extractor using LLM structured output.
pub struct RigEntityExtractor {
    pub system_prompt: String,
    pub model: String,
    client: openai::Client,
}

impl RigEntityExtractor {
    pub fn new(api_url: &str, api_key: &str, model_name: &str) -> Self {
        let prompt = ENTITY_EXTRACTION_PROMPT.to_string();

        let openai_client = openai::Client::builder()
            .base_url(api_url)
            .api_key(api_key)
            .build()
            .expect("Failed to create OpenAI client");

        Self {
            system_prompt: prompt,
            model: model_name.to_string(),
            client: openai_client,
        }
    }
}

#[async_trait]
impl EntityExtractor for RigEntityExtractor {
    async fn extract_entities(&self, text: &str) -> anyhow::Result<Vec<ExtractedEntity>> {
        let builder = self
            .client
            .extractor::<RigExtractedEntities>(&self.model);

        let values: RigExtractedEntities = builder
            .preamble(&self.system_prompt)
            .build()
            .extract(text)
            .await?;

        Ok(values.entities)
    }
}

/// Rig-backed relation extractor using LLM structured output.
pub struct RigRelationExtractor {
    pub system_prompt: String,
    pub model: String,
    client: openai::Client,
}

impl RigRelationExtractor {
    pub fn new(api_url: &str, api_key: &str, model_name: &str) -> Self {
        let prompt = RELATION_EXTRACTION_PROMPT.to_string();

        let openai_client = openai::Client::builder()
            .base_url(api_url)
            .api_key(api_key)
            .build()
            .expect("Failed to create OpenAI client");

        Self {
            system_prompt: prompt,
            model: model_name.to_string(),
            client: openai_client,
        }
    }
}

#[async_trait]
impl RelationExtractor for RigRelationExtractor {
    async fn extract_relations(
        &self,
        text: &str,
        entities: &[ExtractedEntity],
    ) -> Result<Vec<ExtractedRelation>> {
        let builder = self
            .client
            .extractor::<RigExtractedRelations>(&self.model);

        let preamble = [
            self.system_prompt.clone(),
            format!("Entities: {}", entities.iter().map(|e| e.name.clone()).collect::<Vec<String>>().join(", ")),
        ].join("\n");

        let values: RigExtractedRelations = builder
            .preamble(&preamble)
            .build()
            .extract(text)
            .await?;

        Ok(values.relations)
    }
}
