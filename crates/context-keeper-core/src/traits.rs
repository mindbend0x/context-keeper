use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::error::Result;
use crate::models::{Entity, EntityType, RelationType};

// ── Extracted types (shared between traits and models) ──────────────────

/// Raw entity extracted from text by an LLM or mock.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExtractedEntity {
    pub name: String,
    pub entity_type: EntityType,
    pub summary: String,
}

/// Raw relation extracted from text by an LLM or mock.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExtractedRelation {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: u8,
}

impl ExtractedRelation {
    pub fn canonical_type(&self) -> RelationType {
        RelationType::canonicalize(&self.predicate)
    }
}

// ── Trait definitions ───────────────────────────────────────────────────

/// Generates embedding vectors for text.
#[async_trait]
pub trait Embedder: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f64>>;

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f64>>> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.embed(text).await?);
        }
        Ok(results)
    }
}

/// Extracts named entities from raw text.
#[async_trait]
pub trait EntityExtractor: Send + Sync {
    async fn extract_entities(&self, text: &str) -> Result<Vec<ExtractedEntity>>;
}

/// Extracts relations (subject, predicate, object) from raw text.
#[async_trait]
pub trait RelationExtractor: Send + Sync {
    async fn extract_relations(
        &self,
        text: &str,
        entities: &[ExtractedEntity],
    ) -> Result<Vec<ExtractedRelation>>;
}

/// Rewrites a search query into multiple semantic variants for expanded recall.
#[async_trait]
pub trait QueryRewriter: Send + Sync {
    async fn rewrite(&self, query: &str) -> Result<Vec<String>>;
}

/// Resolves newly extracted entities against existing graph nodes.
///
/// Resolution uses a composite key of (name, entity_type, namespace) to prevent
/// collisions across namespaces and between different entity types sharing a name.
/// When `namespace` is `None`, resolution searches the global (unscoped) graph.
#[async_trait]
pub trait EntityResolver: Send + Sync {
    /// Exact name + type match against active entities, scoped by namespace.
    async fn find_existing(
        &self,
        name: &str,
        entity_type: &EntityType,
        namespace: Option<&str>,
    ) -> Result<Option<Entity>>;

    /// Vector + string similarity match for alias resolution, optionally scoped by namespace.
    async fn find_similar(
        &self,
        name: &str,
        embedding: &[f64],
        threshold: f64,
        namespace: Option<&str>,
    ) -> Result<Vec<Entity>>;
}

/// Synthesizes a coherent merged summary from an existing and new description.
///
/// Used during entity resolution when a re-extracted entity needs its summary
/// updated. LLM implementations produce fluent prose; the mock concatenates.
#[async_trait]
pub trait SummarySynthesizer: Send + Sync {
    async fn synthesize(&self, existing: &str, new: &str, entity_name: &str) -> Result<String>;
}

// ── Mock implementations for testing ────────────────────────────────────

/// Deterministic embedder that produces vectors from text hashes.
/// Useful for tests and examples that should run without API keys.
pub struct MockEmbedder {
    pub dimension: usize,
}

impl MockEmbedder {
    pub fn new(dimension: usize) -> Self {
        Self { dimension }
    }
}

#[async_trait]
impl Embedder for MockEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f64>> {
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        let seed = hasher.finish();

        let mut vec = Vec::with_capacity(self.dimension);
        let mut state = seed;
        for _ in 0..self.dimension {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let val = ((state >> 33) as f64) / (u32::MAX as f64);
            vec.push(val * 2.0 - 1.0);
        }

        let magnitude: f64 = vec.iter().map(|x| x * x).sum::<f64>().sqrt();
        if magnitude > 0.0 {
            for v in &mut vec {
                *v /= magnitude;
            }
        }

        Ok(vec)
    }
}

/// Mock entity extractor that finds capitalized words as entities
/// and infers basic types from heuristics.
pub struct MockEntityExtractor;

#[async_trait]
impl EntityExtractor for MockEntityExtractor {
    async fn extract_entities(&self, text: &str) -> Result<Vec<ExtractedEntity>> {
        let trimmed: String = text.chars().take(80).collect();
        let context = trimmed.trim();
        let entities: Vec<ExtractedEntity> = text
            .split_whitespace()
            .filter(|w| {
                w.len() > 1
                    && w.chars().next().is_some_and(|c| c.is_uppercase())
                    && w.chars().all(|c| c.is_alphanumeric())
            })
            .map(|w| {
                let entity_type = infer_entity_type(w);
                ExtractedEntity {
                    name: w.to_string(),
                    entity_type,
                    summary: format!("{} — {}", w, context),
                }
            })
            .collect();

        let mut seen = std::collections::HashSet::new();
        Ok(entities
            .into_iter()
            .filter(|e| seen.insert(e.name.clone()))
            .collect())
    }
}

/// Simple heuristic type inference for mock extraction.
fn infer_entity_type(word: &str) -> EntityType {
    if word.contains('.') || word.ends_with("rs") || word.ends_with("py") || word.ends_with("js") {
        return EntityType::File;
    }
    if word.chars().all(|c| c.is_uppercase() || c.is_ascii_digit()) && word.len() <= 5 {
        return EntityType::Organization;
    }
    if word.ends_with("Corp") || word.ends_with("Inc") || word.ends_with("Co") {
        return EntityType::Organization;
    }
    EntityType::Other
}

/// Mock relation extractor that creates relations between consecutive entities.
pub struct MockRelationExtractor;

#[async_trait]
impl RelationExtractor for MockRelationExtractor {
    async fn extract_relations(
        &self,
        _text: &str,
        entities: &[ExtractedEntity],
    ) -> Result<Vec<ExtractedRelation>> {
        let mut relations = Vec::new();
        for pair in entities.windows(2) {
            relations.push(ExtractedRelation {
                subject: pair[0].name.clone(),
                predicate: "related_to".to_string(),
                object: pair[1].name.clone(),
                confidence: 80,
            });
        }
        Ok(relations)
    }
}

/// Mock entity extractor that fails the first N attempts, then succeeds.
///
/// Used for testing retry logic in the extraction pipeline.
pub struct MockFailingExtractor {
    fail_count: std::sync::atomic::AtomicU32,
    failures_remaining: std::sync::atomic::AtomicU32,
}

impl MockFailingExtractor {
    pub fn new(fail_first_n: u32) -> Self {
        Self {
            fail_count: std::sync::atomic::AtomicU32::new(0),
            failures_remaining: std::sync::atomic::AtomicU32::new(fail_first_n),
        }
    }

    pub fn attempts(&self) -> u32 {
        self.fail_count.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[async_trait]
impl EntityExtractor for MockFailingExtractor {
    async fn extract_entities(&self, text: &str) -> Result<Vec<ExtractedEntity>> {
        self.fail_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let remaining = self
            .failures_remaining
            .fetch_update(
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
                |v| if v > 0 { Some(v - 1) } else { Some(0) },
            )
            .unwrap_or(0);

        if remaining > 0 {
            return Err(crate::ContextKeeperError::ExtractionFailed(
                "simulated LLM failure".into(),
            ));
        }

        let inner = MockEntityExtractor;
        inner.extract_entities(text).await
    }
}

#[async_trait]
impl RelationExtractor for MockFailingExtractor {
    async fn extract_relations(
        &self,
        text: &str,
        entities: &[ExtractedEntity],
    ) -> Result<Vec<ExtractedRelation>> {
        self.fail_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let remaining = self
            .failures_remaining
            .fetch_update(
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
                |v| if v > 0 { Some(v - 1) } else { Some(0) },
            )
            .unwrap_or(0);

        if remaining > 0 {
            return Err(crate::ContextKeeperError::ExtractionFailed(
                "simulated LLM failure".into(),
            ));
        }

        let inner = MockRelationExtractor;
        inner.extract_relations(text, entities).await
    }
}

/// Mock summary synthesizer that concatenates summaries (same as the old merge_summaries logic).
pub struct MockSummarySynthesizer;

#[async_trait]
impl SummarySynthesizer for MockSummarySynthesizer {
    async fn synthesize(&self, existing: &str, new: &str, _entity_name: &str) -> Result<String> {
        let existing_lower = existing.to_lowercase();
        let new_lower = new.to_lowercase();

        let existing_words: std::collections::HashSet<&str> =
            existing_lower.split_whitespace().collect();
        let new_words: std::collections::HashSet<&str> = new_lower.split_whitespace().collect();

        let novel_count = new_words.difference(&existing_words).count();
        if novel_count > 0 && !existing.is_empty() && !new.is_empty() {
            Ok(format!("{}; {}", existing, new))
        } else {
            Ok(new.to_string())
        }
    }
}

/// Mock query rewriter that generates simple variants.
pub struct MockQueryRewriter;

#[async_trait]
impl QueryRewriter for MockQueryRewriter {
    async fn rewrite(&self, query: &str) -> Result<Vec<String>> {
        Ok(vec![
            query.to_string(),
            format!("information about {}", query),
            format!("{} details", query),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_embedder_deterministic() {
        let embedder = MockEmbedder::new(8);
        let v1 = embedder.embed("hello").await.unwrap();
        let v2 = embedder.embed("hello").await.unwrap();
        assert_eq!(v1, v2);
        assert_eq!(v1.len(), 8);
    }

    #[tokio::test]
    async fn test_mock_embedder_different_inputs() {
        let embedder = MockEmbedder::new(8);
        let v1 = embedder.embed("hello").await.unwrap();
        let v2 = embedder.embed("world").await.unwrap();
        assert_ne!(v1, v2);
    }

    #[tokio::test]
    async fn test_mock_embedder_unit_vector() {
        let embedder = MockEmbedder::new(64);
        let v = embedder.embed("test").await.unwrap();
        let magnitude: f64 = v.iter().map(|x| x * x).sum::<f64>().sqrt();
        assert!((magnitude - 1.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_mock_entity_extractor() {
        let extractor = MockEntityExtractor;
        let text = "Alice met Bob at the Park";
        let entities = extractor.extract_entities(text).await.unwrap();
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"Alice"));
        assert!(names.contains(&"Bob"));
        assert!(names.contains(&"Park"));

        let alice = entities.iter().find(|e| e.name == "Alice").unwrap();
        assert!(
            alice.summary.contains(text),
            "Summary should include input context"
        );
    }

    #[tokio::test]
    async fn test_mock_relation_extractor() {
        let entities = vec![
            ExtractedEntity {
                name: "Alice".into(),
                entity_type: EntityType::Person,
                summary: "".into(),
            },
            ExtractedEntity {
                name: "Bob".into(),
                entity_type: EntityType::Person,
                summary: "".into(),
            },
        ];
        let extractor = MockRelationExtractor;
        let relations = extractor.extract_relations("", &entities).await.unwrap();
        assert_eq!(relations.len(), 1);
        assert_eq!(relations[0].subject, "Alice");
        assert_eq!(relations[0].object, "Bob");
    }

    #[tokio::test]
    async fn test_mock_query_rewriter() {
        let rewriter = MockQueryRewriter;
        let variants = rewriter.rewrite("rust programming").await.unwrap();
        assert_eq!(variants.len(), 3);
        assert!(variants[0].contains("rust programming"));
    }

    #[test]
    fn test_relation_type_canonicalize() {
        assert_eq!(
            RelationType::canonicalize("works_at"),
            RelationType::WorksAt
        );
        assert_eq!(
            RelationType::canonicalize("employed_at"),
            RelationType::WorksAt
        );
        assert_eq!(
            RelationType::canonicalize("works_for"),
            RelationType::WorksAt
        );
        assert_eq!(
            RelationType::canonicalize("located_in"),
            RelationType::LocatedIn
        );
        assert_eq!(RelationType::canonicalize("knows"), RelationType::Knows);
        assert_eq!(
            RelationType::canonicalize("random_thing"),
            RelationType::RelatedTo
        );
    }

    #[test]
    fn test_entity_type_from_str() {
        assert_eq!(EntityType::from("person"), EntityType::Person);
        assert_eq!(EntityType::from("organization"), EntityType::Organization);
        assert_eq!(EntityType::from("company"), EntityType::Organization);
        assert_eq!(EntityType::from("unknown"), EntityType::Other);
    }

    #[tokio::test]
    async fn test_mock_failing_extractor_fails_then_succeeds() {
        let extractor = MockFailingExtractor::new(2);

        let r1 = extractor.extract_entities("Alice met Bob").await;
        assert!(r1.is_err(), "first attempt should fail");

        let r2 = extractor.extract_entities("Alice met Bob").await;
        assert!(r2.is_err(), "second attempt should fail");

        let r3 = extractor.extract_entities("Alice met Bob").await;
        assert!(r3.is_ok(), "third attempt should succeed");

        assert_eq!(extractor.attempts(), 3);
    }

    #[tokio::test]
    async fn test_mock_failing_extractor_zero_failures() {
        let extractor = MockFailingExtractor::new(0);
        let result = extractor.extract_entities("Alice met Bob").await;
        assert!(result.is_ok(), "should succeed immediately with 0 failures");
    }

    #[tokio::test]
    async fn test_mock_failing_relation_extractor() {
        let extractor = MockFailingExtractor::new(1);
        let entities = vec![
            ExtractedEntity {
                name: "Alice".into(),
                entity_type: EntityType::Person,
                summary: "A person".into(),
            },
            ExtractedEntity {
                name: "Bob".into(),
                entity_type: EntityType::Person,
                summary: "Another person".into(),
            },
        ];

        let r1 = extractor.extract_relations("text", &entities).await;
        assert!(r1.is_err(), "first attempt should fail");

        let r2 = extractor.extract_relations("text", &entities).await;
        assert!(r2.is_ok(), "second attempt should succeed");
    }
}
