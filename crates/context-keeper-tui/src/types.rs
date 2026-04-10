//! View models shared by backends and the UI.

use serde::Deserialize;

// ---------------------------------------------------------------------------
// Existing view models
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct MemoryRow {
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub name: String,
    pub entity_type: String,
    pub summary: String,
    pub score: f64,
}

#[derive(Debug, Clone)]
pub struct AddMemoryResult {
    pub entity_count: usize,
    pub relation_count: usize,
    pub memory_count: usize,
    pub entity_names: Vec<String>,
}

// ---------------------------------------------------------------------------
// Structured view models (replacing raw JSON strings)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct GraphStats {
    pub entities: usize,
    pub memories: usize,
    pub namespaces: usize,
    pub agents: usize,
}

#[derive(Debug, Clone)]
pub struct EntityDetail {
    pub name: String,
    pub entity_type: String,
    pub summary: String,
    pub valid_from: String,
    pub valid_until: Option<String>,
    pub relations: Vec<RelationRow>,
}

#[derive(Debug, Clone)]
pub struct RelationRow {
    pub relation_type: String,
    pub target_name: String,
    pub direction: RelationDirection,
    pub confidence: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationDirection {
    Outgoing,
    Incoming,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EntitySummary {
    pub name: String,
    pub entity_type: String,
    pub summary: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NamespaceInfo {
    pub name: String,
    pub entity_count: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentInfoRow {
    pub agent_id: String,
    pub agent_name: Option<String>,
    pub episode_count: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EpisodeRow {
    pub content: String,
    pub source: String,
    pub namespace: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SnapshotResult {
    pub timestamp: String,
    pub entity_count: usize,
    pub relation_count: usize,
    pub entities: Vec<EntitySummary>,
}

// ---------------------------------------------------------------------------
// JSON helpers for MCP backend deserialization
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct SearchHitJson {
    pub name: String,
    pub entity_type: String,
    pub summary: String,
    pub score: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MemoryItemJson {
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RelationDetailJson {
    pub relation_type: String,
    pub from_entity_id: String,
    #[serde(default)]
    pub from_entity_name: String,
    pub to_entity_id: String,
    #[serde(default)]
    pub to_entity_name: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NoteRow {
    pub key: String,
    pub content: String,
    pub tags: Vec<String>,
    pub namespace: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentRunRow {
    pub agent_id: Option<String>,
    pub session_id: Option<String>,
    pub status: String,
    pub summary: Option<String>,
    pub namespace: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EntityDetailJson {
    pub name: String,
    pub entity_type: String,
    pub summary: String,
    pub valid_from: String,
    pub valid_until: Option<String>,
    pub relations: Vec<RelationDetailJson>,
}

impl From<SearchHitJson> for SearchHit {
    fn from(j: SearchHitJson) -> Self {
        Self {
            name: j.name,
            entity_type: j.entity_type,
            summary: j.summary,
            score: j.score,
        }
    }
}

impl From<MemoryItemJson> for MemoryRow {
    fn from(m: MemoryItemJson) -> Self {
        Self {
            content: m.content,
            created_at: m.created_at,
        }
    }
}
