use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

// ── Entity type taxonomy ────────────────────────────────────────────────

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    // Level 1: coarse extraction types
    Person,
    Organization,
    Location,
    Event,
    Product,
    Service,
    Concept,
    File,
    // Level 2: precision types (refine a Level 1 parent)
    Project,
    Group,
    Specification,
    #[default]
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
            Self::Project => "project",
            Self::Group => "group",
            Self::Specification => "specification",
            Self::Other => "other",
        };
        f.write_str(s)
    }
}

impl From<&str> for EntityType {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "person" => Self::Person,
            "organization" | "org" | "company" | "firm" | "institution" | "agency" | "corp"
            | "corporation" => Self::Organization,
            "location" | "place" | "city" | "country" | "region" | "state" | "continent" => {
                Self::Location
            }
            "event" | "conference" | "meeting" | "summit" | "workshop" => Self::Event,
            "product"
            | "tool"
            | "app"
            | "application"
            | "database"
            | "db"
            | "framework"
            | "language"
            | "programming language"
            | "software"
            | "hardware"
            | "engine"
            | "runtime"
            | "compiler"
            | "sdk"
            | "operating_system"
            | "os"
            | "distro"
            | "distribution" => Self::Product,
            "service" | "api" | "saas" | "cloud service" | "hosting" | "registry" | "platform" => {
                Self::Service
            }
            "concept" | "idea" | "topic" | "technology" | "methodology" | "protocol"
            | "pattern" | "paradigm" | "algorithm" | "technique" => Self::Concept,
            "file" | "document" | "lib" | "library" | "crate" | "module" | "package" => Self::File,
            "project" | "codebase" | "workspace" | "monorepo" | "repository" | "repo" => {
                Self::Project
            }
            "group" | "team" | "department" | "division" | "squad" => Self::Group,
            "specification" | "spec" | "rfc" | "standard" => Self::Specification,
            _ => Self::Other,
        }
    }
}

// ── Canonical relation types ────────────────────────────────────────────

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RelationType {
    WorksAt,
    LocatedIn,
    PartOf,
    MemberOf,
    Uses,
    CreatedBy,
    Knows,
    DependsOn,
    #[default]
    RelatedTo,
}

impl RelationType {
    /// Map free-form LLM predicates to the canonical set.
    pub fn canonicalize(raw: &str) -> Self {
        match raw.to_lowercase().replace(['-', ' '], "_").as_str() {
            "works_at" | "employed_at" | "works_for" | "employee_of" | "employed_by"
            | "reports_to" | "manages" => Self::WorksAt,
            "located_in" | "based_in" | "headquartered_in" | "lives_in" | "resides_in" => {
                Self::LocatedIn
            }
            "part_of" | "is_part_of" | "belongs_to" | "subset_of" => Self::PartOf,
            "member_of" | "affiliated_with" => Self::MemberOf,
            "uses" | "utilizes" | "leverages" | "adopts" | "employs" => Self::Uses,
            "created_by" | "authored_by" | "built_by" | "developed_by" | "founded_by"
            | "invented_by" | "designed_by" | "built" | "authored" | "created" => Self::CreatedBy,
            "knows" | "met" | "collaborates_with" | "works_with" | "mentors" => Self::Knows,
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
            Self::MemberOf => "member_of",
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

// ── Agent identity ──────────────────────────────────────────────────────

/// Identifies the agent and machine that produced a piece of data.
///
/// Enables multi-agent provenance tracking when multiple AI assistants
/// share a single Context Keeper instance.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentInfo {
    pub agent_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub machine_id: Option<String>,
}

// ── Core domain types ───────────────────────────────────────────────────

/// A raw input unit representing a single piece of information ingested into the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    pub id: Uuid,
    pub content: String,
    pub source: String,
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by_agent: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by_agent: Option<String>,
}

/// Result returned from hybrid search operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub entity: Option<Entity>,
    pub memory: Option<Memory>,
    pub score: f32,
}

/// Distance metric for vector similarity search.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DistanceMetric {
    #[default]
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── EntityType alias coverage ──────────────────────────────────────

    #[test]
    fn entity_type_from_canonical_names() {
        assert_eq!(EntityType::from("person"), EntityType::Person);
        assert_eq!(EntityType::from("organization"), EntityType::Organization);
        assert_eq!(EntityType::from("location"), EntityType::Location);
        assert_eq!(EntityType::from("event"), EntityType::Event);
        assert_eq!(EntityType::from("product"), EntityType::Product);
        assert_eq!(EntityType::from("service"), EntityType::Service);
        assert_eq!(EntityType::from("concept"), EntityType::Concept);
        assert_eq!(EntityType::from("file"), EntityType::File);
        assert_eq!(EntityType::from("project"), EntityType::Project);
        assert_eq!(EntityType::from("group"), EntityType::Group);
        assert_eq!(EntityType::from("specification"), EntityType::Specification);
        assert_eq!(EntityType::from("unknown_type"), EntityType::Other);
    }

    #[test]
    fn entity_type_organization_aliases() {
        for alias in [
            "org",
            "company",
            "firm",
            "Organization",
            "COMPANY",
            "institution",
            "agency",
            "corp",
            "corporation",
        ] {
            assert_eq!(
                EntityType::from(alias),
                EntityType::Organization,
                "alias: {alias}"
            );
        }
    }

    #[test]
    fn entity_type_location_aliases() {
        for alias in [
            "place",
            "city",
            "country",
            "LOCATION",
            "City",
            "region",
            "state",
            "continent",
        ] {
            assert_eq!(
                EntityType::from(alias),
                EntityType::Location,
                "alias: {alias}"
            );
        }
    }

    #[test]
    fn entity_type_product_aliases() {
        for alias in [
            "product",
            "tool",
            "app",
            "Tool",
            "APP",
            "database",
            "db",
            "framework",
            "language",
            "software",
            "hardware",
            "engine",
            "runtime",
            "compiler",
            "sdk",
            "operating_system",
            "os",
            "distro",
            "distribution",
        ] {
            assert_eq!(
                EntityType::from(alias),
                EntityType::Product,
                "alias: {alias}"
            );
        }
    }

    #[test]
    fn entity_type_concept_aliases() {
        for alias in [
            "concept",
            "idea",
            "topic",
            "technology",
            "Technology",
            "methodology",
            "protocol",
            "pattern",
            "paradigm",
            "algorithm",
        ] {
            assert_eq!(
                EntityType::from(alias),
                EntityType::Concept,
                "alias: {alias}"
            );
        }
    }

    #[test]
    fn entity_type_file_aliases() {
        for alias in [
            "file", "document", "lib", "library", "crate", "Library", "module", "package",
        ] {
            assert_eq!(EntityType::from(alias), EntityType::File, "alias: {alias}");
        }
    }

    #[test]
    fn entity_type_service_aliases() {
        for alias in ["service", "api", "saas", "hosting", "registry", "platform"] {
            assert_eq!(
                EntityType::from(alias),
                EntityType::Service,
                "alias: {alias}"
            );
        }
    }

    #[test]
    fn entity_type_project_aliases() {
        for alias in [
            "project",
            "codebase",
            "workspace",
            "monorepo",
            "repository",
            "repo",
            "Project",
            "REPO",
        ] {
            assert_eq!(
                EntityType::from(alias),
                EntityType::Project,
                "alias: {alias}"
            );
        }
    }

    #[test]
    fn entity_type_group_aliases() {
        for alias in ["group", "team", "department", "division", "squad", "Team"] {
            assert_eq!(EntityType::from(alias), EntityType::Group, "alias: {alias}");
        }
    }

    #[test]
    fn entity_type_specification_aliases() {
        for alias in ["specification", "spec", "rfc", "standard", "Spec", "RFC"] {
            assert_eq!(
                EntityType::from(alias),
                EntityType::Specification,
                "alias: {alias}"
            );
        }
    }

    // ── RelationType canonicalize coverage ──────────────────────────────

    #[test]
    fn relation_type_works_at_aliases() {
        for alias in [
            "works_at",
            "employed_at",
            "works_for",
            "employee_of",
            "employed_by",
            "reports_to",
            "manages",
            "Works At",
            "WORKS-FOR",
        ] {
            assert_eq!(
                RelationType::canonicalize(alias),
                RelationType::WorksAt,
                "alias: {alias}"
            );
        }
    }

    #[test]
    fn relation_type_located_in_aliases() {
        for alias in [
            "located_in",
            "based_in",
            "headquartered_in",
            "lives_in",
            "resides_in",
            "Located In",
            "BASED-IN",
        ] {
            assert_eq!(
                RelationType::canonicalize(alias),
                RelationType::LocatedIn,
                "alias: {alias}"
            );
        }
    }

    #[test]
    fn relation_type_part_of_aliases() {
        for alias in ["part_of", "is_part_of", "belongs_to", "subset_of"] {
            assert_eq!(
                RelationType::canonicalize(alias),
                RelationType::PartOf,
                "alias: {alias}"
            );
        }
    }

    #[test]
    fn relation_type_member_of_aliases() {
        for alias in ["member_of", "affiliated_with", "Member Of"] {
            assert_eq!(
                RelationType::canonicalize(alias),
                RelationType::MemberOf,
                "alias: {alias}"
            );
        }
    }

    #[test]
    fn relation_type_uses_aliases() {
        for alias in ["uses", "utilizes", "leverages", "adopts", "employs"] {
            assert_eq!(
                RelationType::canonicalize(alias),
                RelationType::Uses,
                "alias: {alias}"
            );
        }
    }

    #[test]
    fn relation_type_created_by_aliases() {
        for alias in [
            "created_by",
            "authored_by",
            "built_by",
            "developed_by",
            "founded_by",
            "invented_by",
            "designed_by",
            "built",
            "authored",
            "created",
        ] {
            assert_eq!(
                RelationType::canonicalize(alias),
                RelationType::CreatedBy,
                "alias: {alias}"
            );
        }
    }

    #[test]
    fn relation_type_knows_aliases() {
        for alias in ["knows", "met", "collaborates_with", "works_with", "mentors"] {
            assert_eq!(
                RelationType::canonicalize(alias),
                RelationType::Knows,
                "alias: {alias}"
            );
        }
    }

    #[test]
    fn relation_type_depends_on_aliases() {
        for alias in ["depends_on", "requires", "needs", "relies_on"] {
            assert_eq!(
                RelationType::canonicalize(alias),
                RelationType::DependsOn,
                "alias: {alias}"
            );
        }
    }

    #[test]
    fn relation_type_unknown_defaults_to_related_to() {
        assert_eq!(
            RelationType::canonicalize("foo_bar"),
            RelationType::RelatedTo
        );
        assert_eq!(RelationType::canonicalize("xyz"), RelationType::RelatedTo);
    }

    #[test]
    fn relation_type_from_delegates_to_canonicalize() {
        assert_eq!(RelationType::from("works_at"), RelationType::WorksAt);
        assert_eq!(RelationType::from("member_of"), RelationType::MemberOf);
        assert_eq!(RelationType::from("gibberish"), RelationType::RelatedTo);
    }

    #[test]
    fn relation_type_display_roundtrip() {
        let types = [
            RelationType::WorksAt,
            RelationType::LocatedIn,
            RelationType::PartOf,
            RelationType::MemberOf,
            RelationType::Uses,
            RelationType::CreatedBy,
            RelationType::Knows,
            RelationType::DependsOn,
            RelationType::RelatedTo,
        ];
        for rt in types {
            let display = rt.to_string();
            let back = RelationType::canonicalize(&display);
            assert_eq!(back, rt, "roundtrip failed for {display}");
        }
    }

    #[test]
    fn symmetric_types() {
        assert!(RelationType::Knows.is_symmetric());
        assert!(RelationType::RelatedTo.is_symmetric());
        assert!(!RelationType::WorksAt.is_symmetric());
        assert!(!RelationType::MemberOf.is_symmetric());
        assert!(!RelationType::PartOf.is_symmetric());
    }
}
