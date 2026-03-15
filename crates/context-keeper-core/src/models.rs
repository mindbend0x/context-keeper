use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// A raw input unit representing a single piece of information ingested into the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    pub id: Uuid,
    pub content: String,
    pub source: String,
    pub session_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// An extracted named entity with temporal awareness and an embedding vector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: Uuid,
    pub name: String,
    pub entity_type: String,
    pub summary: String,
    pub embedding: Vec<f64>,
    pub valid_from: DateTime<Utc>,
    pub valid_until: Option<DateTime<Utc>>,
}

/// A directed graph edge between two entities with temporal bounds.
///
/// `from_entity_id` maps to SurrealDB's `in` field (the source node).
/// `to_entity_id` maps to SurrealDB's `out` field (the target node).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relation {
    pub id: Uuid,
    pub from_entity_id: Uuid,
    pub to_entity_id: Uuid,
    pub relation_type: String,
    pub confidence: u8,
    pub valid_from: DateTime<Utc>,
    pub valid_until: Option<DateTime<Utc>>,
}

/// A distilled, searchable fact unit derived from episodes and entity extraction.
///
/// Persistence is via graph edges: `memory->sourced_from->episode` and
/// `memory->references->entity`. The fields here are kept as convenience
/// accessors for in-memory use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: Uuid,
    pub content: String,
    pub embedding: Vec<f64>,
    pub source_episode_id: Uuid,
    pub entity_ids: Vec<Uuid>,
    pub created_at: DateTime<Utc>,
}

/// Result returned from hybrid search operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub entity: Option<Entity>,
    pub memory: Option<Memory>,
    pub score: f32,
}

/// Distance metric for vector similarity search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DistanceMetric {
    Cosine,
    Euclidean,
    Manhattan,
    Chebyshev,
    Hamming,
    Minkowski,
}

impl fmt::Display for DistanceMetric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cosine => write!(f, "COSINE"),
            Self::Euclidean => write!(f, "EUCLIDEAN"),
            Self::Manhattan => write!(f, "MANHATTAN"),
            Self::Chebyshev => write!(f, "CHEBYSHEV"),
            Self::Hamming => write!(f, "HAMMING"),
            Self::Minkowski => write!(f, "MINKOWSKI"),
        }
    }
}

impl Default for DistanceMetric {
    fn default() -> Self {
        Self::Cosine
    }
}

impl DistanceMetric {
    /// Returns the SurrealQL vector similarity function name for this metric.
    pub fn similarity_function(&self) -> &'static str {
        match self {
            Self::Cosine => "vector::similarity::cosine",
            Self::Euclidean => "vector::distance::euclidean",
            Self::Manhattan => "vector::distance::manhattan",
            Self::Chebyshev => "vector::distance::chebyshev",
            Self::Hamming => "vector::distance::hamming",
            Self::Minkowski => "vector::distance::minkowski",
        }
    }
}
