//! Rig-powered entity and relation extraction.
//!
//! Uses Rig's `agent().prompt()` to get raw LLM output, then parses JSON
//! ourselves for maximum tolerance of malformed responses from OSS models.
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
use rig::client::CompletionClient;
use rig::completion::Prompt;
use rig::providers::openai;
use regex::Regex;
use serde::Deserialize;
use std::sync::LazyLock;
use std::time::Duration;

const ENTITY_EXTRACTION_PROMPT: &str = "\
Extract only significant named entities from the text — proper nouns that refer \
to specific people, organizations, places, products, events, or technical concepts. \
Do NOT extract generic roles (e.g. \"engineer\", \"manager\"), common nouns, adjectives, \
or pronouns. Each entity must be a distinct, identifiable thing with a proper name.

Do NOT extract numeric specifications or measurements as entities. Values like \
\"4 CPU cores\", \"75G disk\", \"16GB RAM\", or \"3.5 GHz\" are attributes of a parent \
entity, not standalone entities.

Classify each entity using exactly one of these types:

Level 1 (coarse — prefer these when uncertain):
- person: named individuals (e.g. \"Alice\", \"Linus Torvalds\")
- organization: companies, institutions, agencies (e.g. \"Acme Corp\", \"Google\", \"MIT\")
- location: cities, countries, geographic places (e.g. \"Berlin\", \"Germany\", \"AWS us-east-1\")
- event: named events, conferences, releases (e.g. \"PyCon 2025\", \"World Cup\")
- product: software, databases, frameworks, languages, OSes, hardware (e.g. \"SurrealDB\", \"Rust\", \"PostgreSQL\", \"Debian 13\")
- service: hosted services, APIs, registries, platforms-as-a-service (e.g. \"AWS Lambda\", \"GitHub Actions\", \"crates.io\", \"npm\")
- concept: abstract ideas, methodologies, protocols, algorithms (e.g. \"RAG\", \"microservices\", \"OAuth\")
- file: specific files, documents, libraries, crates (e.g. \"pipeline.rs\", \"tokio\", \"serde\")

Level 2 (use when confident — these refine a Level 1 parent):
- project: named projects, repositories, codebases (e.g. \"Context Keeper\", \"linux kernel\")
- group: teams, departments, working groups (e.g. \"Platform Team\", \"IETF Working Group\")
- specification: standards, RFCs, specs (e.g. \"HTTP/2\", \"RFC 7231\", \"OpenAPI 3.0\")
- other: only when none of the above fit

Disambiguation rules:
- Databases and frameworks are \"product\", not \"organization\"
- Hosted platforms like crates.io and npm are \"service\", not \"location\"
- Named repositories and codebases are \"project\", not \"file\"
- Named teams and departments are \"group\", not \"organization\"
- When uncertain between a Level 2 type and its parent, prefer the Level 1 parent

Write each summary as a concise description of what the entity does or represents \
in the context of THIS text. Each summary must capture what makes the entity distinct — \
avoid generic descriptions like \"a crate in the workspace\" and instead describe the \
entity's specific role or responsibility.

Return ONLY a JSON object with an \"entities\" array of {\"name\", \"entity_type\", \"summary\"}. \
No markdown, no explanation — just the JSON object.";

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
Return ONLY a JSON object with a \"relations\" array of {\"subject\", \"predicate\", \"object\", \"confidence\"}. \
No markdown, no explanation — just the JSON object.";

const DEFAULT_MAX_RETRIES: u32 = 3;
const INITIAL_BACKOFF_MS: u64 = 100;

/// Tolerant intermediate types for LLM deserialization.
/// All fields are Option so nulls and missing fields are absorbed.
#[derive(Debug, Clone, Deserialize)]
struct RawExtractedEntity {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub entity_type: Option<EntityType>,
    #[serde(default)]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
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

/// Extract a JSON object from raw LLM text that may contain markdown fences or preamble.
fn extract_json_str(raw: &str) -> &str {
    let trimmed = raw.trim();

    // Strip markdown code fences
    if let Some(rest) = trimmed.strip_prefix("```json") {
        if let Some(inner) = rest.strip_suffix("```") {
            return inner.trim();
        }
    }
    if let Some(rest) = trimmed.strip_prefix("```") {
        if let Some(inner) = rest.strip_suffix("```") {
            return inner.trim();
        }
    }

    // Find the first '{' and last '}' to extract the JSON object
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if start < end {
            return &trimmed[start..=end];
        }
    }

    trimmed
}

/// Parse entities from a JSON string, tolerating nulls at any level.
fn parse_entities(json_str: &str) -> std::result::Result<Vec<RawExtractedEntity>, String> {
    let value: serde_json::Value =
        serde_json::from_str(json_str).map_err(|e| format!("JSON parse error: {e}"))?;

    // Try {"entities": [...]} wrapper
    let arr = if let Some(arr) = value.get("entities").and_then(|v| v.as_array()) {
        arr.clone()
    } else if let Some(arr) = value.as_array() {
        // Bare array
        arr.clone()
    } else {
        return Err(format!(
            "Expected object with 'entities' array or bare array, got: {}",
            &json_str[..json_str.len().min(200)]
        ));
    };

    // Deserialize each element individually, skipping nulls and failures
    let entities: Vec<RawExtractedEntity> = arr
        .into_iter()
        .filter(|v| !v.is_null())
        .filter_map(
            |v| match serde_json::from_value::<RawExtractedEntity>(v.clone()) {
                Ok(e) => Some(e),
                Err(e) => {
                    tracing::warn!(error = %e, value = %v, "Skipping malformed entity element");
                    None
                }
            },
        )
        .collect();

    Ok(entities)
}

/// Parse relations from a JSON string, tolerating nulls at any level.
fn parse_relations(json_str: &str) -> std::result::Result<Vec<RawExtractedRelation>, String> {
    let value: serde_json::Value =
        serde_json::from_str(json_str).map_err(|e| format!("JSON parse error: {e}"))?;

    let arr = if let Some(arr) = value.get("relations").and_then(|v| v.as_array()) {
        arr.clone()
    } else if let Some(arr) = value.as_array() {
        arr.clone()
    } else {
        return Err(format!(
            "Expected object with 'relations' array or bare array, got: {}",
            &json_str[..json_str.len().min(200)]
        ));
    };

    let relations: Vec<RawExtractedRelation> = arr
        .into_iter()
        .filter(|v| !v.is_null())
        .filter_map(
            |v| match serde_json::from_value::<RawExtractedRelation>(v.clone()) {
                Ok(r) => Some(r),
                Err(e) => {
                    tracing::warn!(error = %e, value = %v, "Skipping malformed relation element");
                    None
                }
            },
        )
        .collect();

    Ok(relations)
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

/// Rig-backed entity extractor using raw prompt + manual JSON parsing.
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

        let agent = self
            .client
            .agent(&self.model)
            .preamble(&self.system_prompt)
            .build();

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

            // Get raw text from LLM
            let raw_response: String = match agent.prompt(text).await {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(
                        attempt,
                        model = %self.model,
                        error = %e,
                        "Entity extraction LLM call failed"
                    );
                    last_err = Some(format!("LLM call failed: {e}"));
                    continue;
                }
            };

            tracing::debug!(
                attempt,
                response_len = raw_response.len(),
                response_preview = %&raw_response[..raw_response.len().min(500)],
                "Entity extraction raw LLM response"
            );

            // Parse JSON ourselves with null tolerance
            let json_str = extract_json_str(&raw_response);
            match parse_entities(json_str) {
                Ok(raw_entities) => {
                    tracing::debug!(raw_count = raw_entities.len(), "Parsed raw entities");
                    let coerced = coerce_entities(raw_entities);
                    let validated = validate_entities(coerced);
                    return Ok(validated);
                }
                Err(e) => {
                    tracing::warn!(
                        attempt,
                        model = %self.model,
                        error = %e,
                        response_preview = %&raw_response[..raw_response.len().min(500)],
                        "Entity extraction JSON parse failed"
                    );
                    last_err = Some(e);
                }
            }
        }

        let err_msg = last_err.unwrap_or_default();
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

/// Rig-backed relation extractor using raw prompt + manual JSON parsing.
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

        let preamble = [
            self.system_prompt.clone(),
            format!("Entities: {}", entity_names.join(", ")),
        ]
        .join("\n");

        let agent = self.client.agent(&self.model).preamble(&preamble).build();

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

            let raw_response: String = match agent.prompt(text).await {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(
                        attempt,
                        model = %self.model,
                        error = %e,
                        "Relation extraction LLM call failed"
                    );
                    last_err = Some(format!("LLM call failed: {e}"));
                    continue;
                }
            };

            tracing::debug!(
                attempt,
                response_len = raw_response.len(),
                response_preview = %&raw_response[..raw_response.len().min(500)],
                "Relation extraction raw LLM response"
            );

            let json_str = extract_json_str(&raw_response);
            match parse_relations(json_str) {
                Ok(raw_relations) => {
                    tracing::debug!(raw_count = raw_relations.len(), "Parsed raw relations");
                    let coerced = coerce_relations(raw_relations);
                    let validated = validate_relations(coerced, &entity_names);
                    return Ok(validated);
                }
                Err(e) => {
                    tracing::warn!(
                        attempt,
                        model = %self.model,
                        error = %e,
                        response_preview = %&raw_response[..raw_response.len().min(500)],
                        "Relation extraction JSON parse failed"
                    );
                    last_err = Some(e);
                }
            }
        }

        let err_msg = last_err.unwrap_or_default();
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

static MEASUREMENT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\d+[\.\d]*\s*(cpu|core|gb|mb|tb|ghz|mhz|ram|disk|cores?|threads?|vcpus?|gib|mib|tib)s?$").unwrap()
});

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
            if MEASUREMENT_RE.is_match(e.name.trim()) {
                tracing::warn!(name = %e.name, "Rejected entity: measurement/spec value");
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

    #[test]
    fn test_extract_json_str_plain() {
        let input = r#"{"entities": []}"#;
        assert_eq!(extract_json_str(input), input);
    }

    #[test]
    fn test_extract_json_str_markdown_fenced() {
        let input = "```json\n{\"entities\": []}\n```";
        assert_eq!(extract_json_str(input), r#"{"entities": []}"#);
    }

    #[test]
    fn test_extract_json_str_with_preamble() {
        let input = "Here are the entities:\n{\"entities\": []}";
        assert_eq!(extract_json_str(input), r#"{"entities": []}"#);
    }

    #[test]
    fn test_parse_entities_with_nulls() {
        let json = r#"{"entities": [{"name": "Alice", "entity_type": "person", "summary": "A person"}, null, {"name": "Bob", "entity_type": "person", "summary": "Another person"}]}"#;
        let parsed = parse_entities(json).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].name.as_deref(), Some("Alice"));
        assert_eq!(parsed[1].name.as_deref(), Some("Bob"));
    }

    #[test]
    fn test_parse_entities_bare_array() {
        let json = r#"[{"name": "Alice", "entity_type": "person", "summary": "A person"}]"#;
        let parsed = parse_entities(json).unwrap();
        assert_eq!(parsed.len(), 1);
    }

    #[test]
    fn test_parse_entities_null_fields() {
        let json = r#"{"entities": [{"name": "Alice", "entity_type": null, "summary": null}]}"#;
        let parsed = parse_entities(json).unwrap();
        assert_eq!(parsed.len(), 1);
        assert!(parsed[0].entity_type.is_none());
    }

    #[test]
    fn test_parse_relations_with_nulls() {
        let json = r#"{"relations": [null, {"subject": "Alice", "predicate": "works_at", "object": "Acme", "confidence": 90}]}"#;
        let parsed = parse_relations(json).unwrap();
        assert_eq!(parsed.len(), 1);
    }
}
