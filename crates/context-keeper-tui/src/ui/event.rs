//! Application events produced by async backend operations and user input.

use crate::types::{
    AgentInfoRow, EntityDetail, EntitySummary, EpisodeRow, GraphStats, MemoryRow, NamespaceInfo,
    SearchHit, SnapshotResult,
};

pub enum AppEvent {
    Quit,

    // Dashboard
    StatsReady(Result<GraphStats, String>),
    RecentReady(Result<Vec<MemoryRow>, String>),

    // Search
    SearchReady(Result<Vec<SearchHit>, String>),
    ExpandReady(Result<Vec<SearchHit>, String>),

    // Entity
    EntityReady(Result<Option<EntityDetail>, String>),
    EntityListReady(Result<Vec<EntitySummary>, String>),

    // Ingest
    IngestReady(Result<String, String>),

    // Admin
    NamespacesReady(Result<Vec<NamespaceInfo>, String>),
    AgentsReady(Result<Vec<AgentInfoRow>, String>),
    CrossSearchReady(Result<Vec<SearchHit>, String>),
    SnapshotReady(Result<SnapshotResult, String>),
    ActivityReady(Result<Vec<EpisodeRow>, String>),
}
