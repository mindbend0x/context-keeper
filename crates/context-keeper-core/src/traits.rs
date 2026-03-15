use anyhow::Result;
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

// ── Extracted types (shared between traits and models) ──────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum ExtractedEntityType {
    Person,
    Organization,
    Location,
    Event,
    Product,
    Service,
    Concept,
    File,
    Other,
}

impl From<&str> for ExtractedEntityType {
    fn from(s: &str) -> Self {
        match s {
            "person" => Self::Person,
            "organization" => Self::Organization,
            "location" => Self::Location,
            "event" => Self::Event,
            "product" => Self::Product,
            "service" => Self::Service,
            "concept" => Self::Concept,
            "file" => Self::File,
            _ => Self::Other,
        }
    }
}

impl From<ExtractedEntityType> for String {
    fn from(entity_type: ExtractedEntityType) -> Self {
        match entity_type {
            ExtractedEntityType::Person => "person".to_string(),
            ExtractedEntityType::Organization => "organization".to_string(),
            ExtractedEntityType::Location => "location".to_string(),
            ExtractedEntityType::Event => "event".to_string(),
            ExtractedEntityType::Product => "product".to_string(),
            ExtractedEntityType::Service => "service".to_string(),
            ExtractedEntityType::Concept => "concept".to_string(),
            ExtractedEntityType::File => "file".to_string(),
            ExtractedEntityType::Other => "other".to_string(),
        }
    }
}

/// Raw entity extracted from text by an LLM or mock.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExtractedEntity {
    pub name: String,
    pub entity_type: ExtractedEntityType,
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
            // Simple LCG for deterministic pseudo-random floats
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let val = ((state >> 33) as f64) / (u32::MAX as f64);
            vec.push(val * 2.0 - 1.0); // Range [-1, 1]
        }

        // Normalize to unit vector
        let magnitude: f64 = vec.iter().map(|x| x * x).sum::<f64>().sqrt();
        if magnitude > 0.0 {
            for v in &mut vec {
                *v /= magnitude;
            }
        }

        Ok(vec)
    }
}

/// Mock entity extractor that finds capitalized words as entities.
pub struct MockEntityExtractor;

#[async_trait]
impl EntityExtractor for MockEntityExtractor {
    async fn extract_entities(&self, text: &str) -> Result<Vec<ExtractedEntity>> {
        let entities: Vec<ExtractedEntity> = text
            .split_whitespace()
            .filter(|w| {
                w.len() > 1
                    && w.chars().next().map_or(false, |c| c.is_uppercase())
                    && w.chars().all(|c| c.is_alphanumeric())
            })
            .map(|w| ExtractedEntity {
                name: w.to_string(),
                entity_type: ExtractedEntityType::Other,
                summary: format!("Entity: {}", w),
            })
            .collect();

        // Deduplicate by name
        let mut seen = std::collections::HashSet::new();
        Ok(entities
            .into_iter()
            .filter(|e| seen.insert(e.name.clone()))
            .collect())
    }
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
        let entities = extractor
            .extract_entities("Alice met Bob at the Park")
            .await
            .unwrap();
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"Alice"));
        assert!(names.contains(&"Bob"));
        assert!(names.contains(&"Park"));
    }

    #[tokio::test]
    async fn test_mock_relation_extractor() {
        let entities = vec![
            ExtractedEntity {
                name: "Alice".into(),
                entity_type: "person".into(),
                summary: "".into(),
            },
            ExtractedEntity {
                name: "Bob".into(),
                entity_type: "person".into(),
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
}
