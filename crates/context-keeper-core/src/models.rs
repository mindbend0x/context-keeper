use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
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

/// A directed relationship between two entities with temporal bounds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relation {
    pub id: Uuid,
    pub source_entity_id: Uuid,
    pub target_entity_id: Uuid,
    pub relation_type: String,
    pub confidence: u8,
    pub valid_from: DateTime<Utc>,
    pub valid_until: Option<DateTime<Utc>>,
}

/// A distilled, searchable fact unit derived from episodes and entity extraction.
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
