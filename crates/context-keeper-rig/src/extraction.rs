//! Rig-powered entity and relation extraction.
//!
//! These types implement the core `EntityExtractor` and `RelationExtractor` traits
//! using Rig's `agent().prompt()` with structured JSON schema output.
//!
//! Includes retry logic with exponential backoff and output validation to reject
//! malformed extraction results.
//!
//! Requires a valid `OPENAI_API_KEY` (or other provider key) at runtime.
//! For testing, use `MockEntityExtractor` and `MockRelationExtractor` from core.

use async_trait::async_trait;
use context_keeper_core::error::Result;
use context_keeper_core::models::EntityType;
use context_keeper_core::traits::{
    EntityExtractor, ExtractedEntity, ExtractedRelation, RelationExtractor,
};
use context_keeper_core::ContextKeeperError;
use rig::providers::openai;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const ENTITY_EXTRACTION_PROMPT: &str = "\
Extract named entities from the text. Classify each entity as one of: \
person, organization, location, event, product, service, concept, file, other. \
Return JSON array of {name, entity_type, summary}.";

const RELATION_EXTRACTION_PROMPT: &str = "\
Extract relations between the given entities. \
The predicate MUST be one of the following canonical types: \
works_at, located_in, part_of, member_of, uses, created_by, knows, depends_on, related_to. \
Choose the most specific type that fits: \
- works_at: employment, reporting, management (e.g. \"Alice works at Acme\") \
- located_in: physical or geographic containment (e.g. \"Acme is based in NYC\") \
- part_of: structural containment or subsets (e.g. \"engine is part of car\") \
- member_of: membership in groups or organizations (e.g. \"Alice is a member of the board\") \
- uses: usage, adoption, utilization (e.g. \"Acme uses Rust\") \
- created_by: authorship, creation, founding (e.g. \"Linux was created by Linus\") \
- knows: personal acquaintance or collaboration (e.g. \"Alice knows Bob\") \
- depends_on: technical or logical dependency (e.g. \"service A depends on service B\") \
- related_to: ONLY use when no other type fits. \
Avoid defaulting to related_to when a more specific type applies. \
Assign a confidence score from 0 to 100. \
Return JSON array of {subject, predicate, object, confidence}.";

const DEFAULT_MAX_RETRIES: u32 = 3;
const INITIAL_BACKOFF_MS: u64 = 100;

/// Tolerant intermediate types for LLM deserialization.
/// Some models (especially OSS) return null for fields — these types absorb that
/// and get filtered/converted to the strict core types in validation.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct RawExtractedEntity {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub entity_type: Option<EntityType>,
    #[serde(default)]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct RawExtractedRelation {
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub predicate: Option<String>,
    #[serde(default)]
    pub object: Option<String>,
    #[serde(default)]
    pub confidence: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct RigExtractedEntities {
    #[serde(default)]
    entities: Vec<RawExtractedEntity>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct RigExtractedRelations {
    #[serde(default)]
    relations: Vec<RawExtractedRelation>,
}

/// Convert raw LLM output to strict core types, dropping entries with missing required fields.
fn coerce_entities(raw: Vec<RawExtractedEntity>) -> Vec<ExtractedEntity> {
    raw.into_iter()
        .filter_map(|r| {
            let name = r.name.filter(|s| !s.trim().is_empty())?;
            let summary = r.summary.unwrap_or_default();
            Some(ExtractedEntity {
                name,
                entity_type: r.entity_type.unwrap_or_default(),
                summary,
            })
        })
        .collect()
}

fn coerce_relations(raw: Vec<RawExtractedRelation>) -> Vec<ExtractedRelation> {
    raw.into_iter()
        .filter_map(|r| {
            let subject = r.subject.filter(|s| !s.trim().is_empty())?;
            let predicate = r.predicate.filter(|s| !s.trim().is_empty())?;
            let object = r.object.filter(|s| !s.trim().is_empty())?;
            Some(ExtractedRelation {
                subject,
                predicate,
                object,
                confidence: r.confidence.unwrap_or(50),
            })
        })
        .collect()
}

/// Retry configuration for LLM extraction calls.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_backoff: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: DEFAULT_MAX_RETRIES,
            initial_backoff: Duration::from_millis(INITIAL_BACKOFF_MS),
        }
    }
}

/// Rig-backed entity extractor using LLM structured output.
pub struct RigEntityExtractor {
    pub system_prompt: String,
    pub model: String,
    pub retry_config: RetryConfig,
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
            retry_config: RetryConfig::default(),
            client: openai_client,
        }
    }

    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = config;
        self
    }
}

#[async_trait]
impl EntityExtractor for RigEntityExtractor {
    async fn extract_entities(&self, text: &str) -> Result<Vec<ExtractedEntity>> {
        let mut last_err = None;

        tracing::debug!(
            model = %self.model,
            text_len = text.len(),
            text_preview = %&text[..text.len().min(200)],
            "Starting entity extraction"
        );

        for attempt in 0..=self.retry_config.max_retries {
            if attempt > 0 {
                let backoff = self.retry_config.initial_backoff * 4u32.pow(attempt - 1);
                tracing::warn!(
                    attempt,
                    backoff_ms = backoff.as_millis(),
                    "Retrying entity extraction"
                );
                tokio::time::sleep(backoff).await;
            }

            let builder = self.client.extractor::<RigExtractedEntities>(&self.model);

            match builder
                .preamble(&self.system_prompt)
                .build()
                .extract(text)
                .await
            {
                Ok(values) => {
                    tracing::debug!(
                        raw_count = values.entities.len(),
                        "Entity extraction raw response"
                    );
                    let coerced = coerce_entities(values.entities);
                    let validated = validate_entities(coerced);
                    return Ok(validated);
                }
                Err(e) => {
                    tracing::warn!(
                        attempt,
                        model = %self.model,
                        error = %e,
                        error_debug = ?e,
                        "Entity extraction attempt failed"
                    );
                    last_err = Some(e);
                }
            }
        }

        let err_msg = last_err.as_ref().map(|e| e.to_string()).unwrap_or_default();
        tracing::error!(
            model = %self.model,
            attempts = self.retry_config.max_retries + 1,
            last_error = %err_msg,
            text_len = text.len(),
            "Entity extraction exhausted all retries"
        );

        Err(ContextKeeperError::ExtractionFailed(format!(
            "entity extraction failed after {} attempts: {}",
            self.retry_config.max_retries + 1,
            err_msg
        )))
    }
}

/// Rig-backed relation extractor using LLM structured output.
pub struct RigRelationExtractor {
    pub system_prompt: String,
    pub model: String,
    pub retry_config: RetryConfig,
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
            retry_config: RetryConfig::default(),
            client: openai_client,
        }
    }

    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = config;
        self
    }
}

#[async_trait]
impl RelationExtractor for RigRelationExtractor {
    async fn extract_relations(
        &self,
        text: &str,
        entities: &[ExtractedEntity],
    ) -> Result<Vec<ExtractedRelation>> {
        let entity_names: Vec<String> = entities.iter().map(|e| e.name.clone()).collect();
        let mut last_err = None;

        tracing::debug!(
            model = %self.model,
            entity_count = entities.len(),
            entities = ?entity_names,
            "Starting relation extraction"
        );

        for attempt in 0..=self.retry_config.max_retries {
            if attempt > 0 {
                let backoff = self.retry_config.initial_backoff * 4u32.pow(attempt - 1);
                tracing::warn!(
                    attempt,
                    backoff_ms = backoff.as_millis(),
                    "Retrying relation extraction"
                );
                tokio::time::sleep(backoff).await;
            }

            let builder = self.client.extractor::<RigExtractedRelations>(&self.model);

            let preamble = [
                self.system_prompt.clone(),
                format!("Entities: {}", entity_names.join(", ")),
            ]
            .join("\n");

            match builder.preamble(&preamble).build().extract(text).await {
                Ok(values) => {
                    tracing::debug!(
                        raw_count = values.relations.len(),
                        "Relation extraction raw response"
                    );
                    let coerced = coerce_relations(values.relations);
                    let validated = validate_relations(coerced, &entity_names);
                    return Ok(validated);
                }
                Err(e) => {
                    tracing::warn!(
                        attempt,
                        model = %self.model,
                        error = %e,
                        error_debug = ?e,
                        "Relation extraction attempt failed"
                    );
                    last_err = Some(e);
                }
            }
        }

        let err_msg = last_err.as_ref().map(|e| e.to_string()).unwrap_or_default();
        tracing::error!(
            model = %self.model,
            attempts = self.retry_config.max_retries + 1,
            last_error = %err_msg,
            entity_count = entities.len(),
            "Relation extraction exhausted all retries"
        );

        Err(ContextKeeperError::ExtractionFailed(format!(
            "relation extraction failed after {} attempts: {}",
            self.retry_config.max_retries + 1,
            err_msg
        )))
    }
}

// ── Validation helpers ──────────────────────────────────────────────────

fn validate_entities(entities: Vec<ExtractedEntity>) -> Vec<ExtractedEntity> {
    entities
        .into_iter()
        .filter(|e| {
            if e.name.trim().is_empty() {
                tracing::warn!(entity = ?e, "Rejected entity: empty name");
                return false;
            }
            if e.summary.trim().is_empty() {
                tracing::warn!(name = %e.name, "Rejected entity: empty summary");
                return false;
            }
            if EntityType::from(e.entity_type.to_string().as_str()) == EntityType::Other
                && e.entity_type.to_string() != "other"
            {
                tracing::warn!(
                    name = %e.name,
                    entity_type = %e.entity_type,
                    "Entity has unrecognized type, defaulting to Other"
                );
            }
            true
        })
        .collect()
}

fn validate_relations(
    relations: Vec<ExtractedRelation>,
    known_entity_names: &[String],
) -> Vec<ExtractedRelation> {
    relations
        .into_iter()
        .filter(|r| {
            if r.subject == r.object {
                tracing::warn!(
                    subject = %r.subject,
                    "Rejected relation: self-referential"
                );
                return false;
            }
            if r.predicate.trim().is_empty() {
                tracing::warn!(
                    subject = %r.subject,
                    object = %r.object,
                    "Rejected relation: empty predicate"
                );
                return false;
            }
            if r.confidence > 100 {
                tracing::warn!(
                    subject = %r.subject,
                    object = %r.object,
                    confidence = r.confidence,
                    "Rejected relation: confidence out of range"
                );
                return false;
            }
            if !known_entity_names.iter().any(|n| n == &r.subject) {
                tracing::warn!(
                    subject = %r.subject,
                    "Rejected relation: subject not in extraction batch"
                );
                return false;
            }
            if !known_entity_names.iter().any(|n| n == &r.object) {
                tracing::warn!(
                    object = %r.object,
                    "Rejected relation: object not in extraction batch"
                );
                return false;
            }
            true
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_keeper_core::models::EntityType;

    #[test]
    fn test_validate_entities_rejects_empty_name() {
        let entities = vec![
            ExtractedEntity {
                name: "".into(),
                entity_type: EntityType::Person,
                summary: "A person".into(),
            },
            ExtractedEntity {
                name: "Alice".into(),
                entity_type: EntityType::Person,
                summary: "A person named Alice".into(),
            },
        ];
        let valid = validate_entities(entities);
        assert_eq!(valid.len(), 1);
        assert_eq!(valid[0].name, "Alice");
    }

    #[test]
    fn test_validate_entities_rejects_empty_summary() {
        let entities = vec![ExtractedEntity {
            name: "Bob".into(),
            entity_type: EntityType::Person,
            summary: "  ".into(),
        }];
        let valid = validate_entities(entities);
        assert!(valid.is_empty());
    }

    #[test]
    fn test_validate_relations_rejects_self_referential() {
        let relations = vec![ExtractedRelation {
            subject: "Alice".into(),
            predicate: "knows".into(),
            object: "Alice".into(),
            confidence: 90,
        }];
        let valid = validate_relations(relations, &["Alice".into()]);
        assert!(valid.is_empty());
    }

    #[test]
    fn test_validate_relations_rejects_empty_predicate() {
        let relations = vec![ExtractedRelation {
            subject: "Alice".into(),
            predicate: "".into(),
            object: "Bob".into(),
            confidence: 80,
        }];
        let valid = validate_relations(relations, &["Alice".into(), "Bob".into()]);
        assert!(valid.is_empty());
    }

    #[test]
    fn test_validate_relations_rejects_dangling_references() {
        let relations = vec![ExtractedRelation {
            subject: "Alice".into(),
            predicate: "knows".into(),
            object: "Charlie".into(),
            confidence: 80,
        }];
        let valid = validate_relations(relations, &["Alice".into(), "Bob".into()]);
        assert!(valid.is_empty());
    }

    #[test]
    fn test_validate_relations_keeps_valid() {
        let relations = vec![ExtractedRelation {
            subject: "Alice".into(),
            predicate: "works_at".into(),
            object: "Acme".into(),
            confidence: 95,
        }];
        let valid = validate_relations(relations, &["Alice".into(), "Acme".into()]);
        assert_eq!(valid.len(), 1);
    }

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_backoff, Duration::from_millis(100));
    }
}
