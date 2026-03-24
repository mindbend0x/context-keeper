use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

// ── Entity type taxonomy ────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
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

impl fmt::Display for EntityType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Person => "person",
            Self::Organization => "organization",
            Self::Location => "location",
            Self::Event => "event",
            Self::Product => "product",
            Self::Service => "service",
            Self::Concept => "concept",
            Self::File => "file",
            Self::Other => "other",
        };
        f.write_str(s)
    }
}

impl From<&str> for EntityType {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "person" => Self::Person,
            "organization" | "org" | "company" => Self::Organization,
            "location" | "place" | "city" | "country" => Self::Location,
            "event" => Self::Event,
            "product" => Self::Product,
            "service" => Self::Service,
            "concept" | "idea" | "topic" => Self::Concept,
            "file" | "document" => Self::File,
            _ => Self::Other,
        }
    }
}

impl Default for EntityType {
    fn default() -> Self {
        Self::Other
    }
}

// ── Canonical relation types ────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RelationType {
    WorksAt,
    LocatedIn,
    PartOf,
    Uses,
    CreatedBy,
    Knows,
    DependsOn,
    RelatedTo,
}

impl RelationType {
    /// Map free-form LLM predicates to the canonical set.
    pub fn canonicalize(raw: &str) -> Self {
        match raw.to_lowercase().replace(['-', ' '], "_").as_str() {
            "works_at" | "employed_at" | "works_for" | "employee_of" | "employed_by" => {
                Self::WorksAt
            }
            "located_in" | "based_in" | "headquartered_in" | "lives_in" | "resides_in" => {
                Self::LocatedIn
            }
            "part_of" | "is_part_of" | "belongs_to" | "member_of" | "subset_of" => Self::PartOf,
            "uses" | "utilizes" | "leverages" | "adopts" => Self::Uses,
            "created_by" | "authored_by" | "built_by" | "developed_by" | "founded_by"
            | "invented_by" | "designed_by" => Self::CreatedBy,
            "knows" | "met" | "collaborates_with" | "works_with" | "mentors" | "manages" => {
                Self::Knows
            }
            "depends_on" | "requires" | "needs" | "relies_on" => Self::DependsOn,
            "related_to" => Self::RelatedTo,
            _ => Self::RelatedTo,
        }
    }

    pub fn is_symmetric(&self) -> bool {
        matches!(self, Self::Knows | Self::RelatedTo)
    }
}

impl fmt::Display for RelationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::WorksAt => "works_at",
            Self::LocatedIn => "located_in",
            Self::PartOf => "part_of",
            Self::Uses => "uses",
            Self::CreatedBy => "created_by",
            Self::Knows => "knows",
            Self::DependsOn => "depends_on",
            Self::RelatedTo => "related_to",
        };
        f.write_str(s)
    }
}

impl From<&str> for RelationType {
    fn from(s: &str) -> Self {
        Self::canonicalize(s)
    }
}

impl Default for RelationType {
    fn default() -> Self {
        Self::RelatedTo
    }
}

// ── Core domain types ───────────────────────────────────────────────────

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
    pub entity_type: EntityType,
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
    pub relation_type: RelationType,
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
