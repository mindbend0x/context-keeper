//! Pluggable backends: direct repository access or remote MCP.

mod local;
#[cfg(feature = "remote-mcp")]
mod mcp;
#[cfg(test)]
pub mod mock;

pub use local::LocalBackend;
#[cfg(feature = "remote-mcp")]
pub use mcp::McpHttpBackend;

use async_trait::async_trait;

use crate::error::TuiError;
use crate::types::{
    AddMemoryResult, AgentInfoRow, AgentRunRow, EntityDetail, EntitySummary, EpisodeRow, GraphStats,
    MemoryRow, NamespaceInfo, NoteRow, SearchHit, SnapshotResult,
};

#[async_trait]
pub trait TuiBackend: Send + Sync {
    async fn add_memory(&self, text: &str, source: &str) -> Result<AddMemoryResult, TuiError>;

    async fn search_memory(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, TuiError>;

    async fn expand_search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, TuiError>;

    async fn list_recent(&self, limit: usize) -> Result<Vec<MemoryRow>, TuiError>;

    async fn get_entity(&self, name: &str) -> Result<Option<EntityDetail>, TuiError>;

    async fn list_entities(&self, limit: usize) -> Result<Vec<EntitySummary>, TuiError>;

    async fn get_stats(&self) -> Result<GraphStats, TuiError>;

    async fn list_namespaces(&self) -> Result<Vec<NamespaceInfo>, TuiError>;

    async fn delete_namespace(&self, namespace: &str) -> Result<String, TuiError>;

    async fn list_agents(&self) -> Result<Vec<AgentInfoRow>, TuiError>;

    async fn agent_activity(
        &self,
        agent_id: &str,
        limit: usize,
    ) -> Result<Vec<EpisodeRow>, TuiError>;

    async fn cross_namespace_search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchHit>, TuiError>;

    async fn snapshot(&self, iso_timestamp: &str) -> Result<SnapshotResult, TuiError>;

    async fn list_notes(
        &self,
        tag: Option<&str>,
        limit: usize,
    ) -> Result<Vec<NoteRow>, TuiError>;

    async fn query_agent_runs(
        &self,
        status: Option<&str>,
        agent_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<AgentRunRow>, TuiError>;
}
