//! Test double for [`super::TuiBackend`].

use async_trait::async_trait;

use super::TuiBackend;
use crate::error::TuiError;
use crate::types::{
    AddMemoryResult, AgentInfoRow, AgentRunRow, EntityDetail, EntitySummary, EpisodeRow,
    GraphStats, MemoryRow, NamespaceInfo, NoteRow, SearchHit, SnapshotResult,
};

/// Minimal backend that returns empty success values.
#[derive(Debug, Default)]
pub struct MockTuiBackend;

#[async_trait]
impl TuiBackend for MockTuiBackend {
    async fn add_memory(&self, _text: &str, _source: &str) -> Result<AddMemoryResult, TuiError> {
        Ok(AddMemoryResult {
            entity_count: 0,
            relation_count: 0,
            memory_count: 0,
            entity_names: Vec::new(),
        })
    }

    async fn search_memory(&self, _query: &str, _limit: usize) -> Result<Vec<SearchHit>, TuiError> {
        Ok(vec![])
    }

    async fn expand_search(&self, _query: &str, _limit: usize) -> Result<Vec<SearchHit>, TuiError> {
        Ok(vec![])
    }

    async fn list_recent(&self, _limit: usize) -> Result<Vec<MemoryRow>, TuiError> {
        Ok(vec![])
    }

    async fn get_entity(&self, _name: &str) -> Result<Option<EntityDetail>, TuiError> {
        Ok(None)
    }

    async fn list_entities(&self, _limit: usize) -> Result<Vec<EntitySummary>, TuiError> {
        Ok(vec![])
    }

    async fn get_stats(&self) -> Result<GraphStats, TuiError> {
        Ok(GraphStats::default())
    }

    async fn list_namespaces(&self) -> Result<Vec<NamespaceInfo>, TuiError> {
        Ok(vec![])
    }

    async fn delete_namespace(&self, _namespace: &str) -> Result<String, TuiError> {
        Ok("Mock: namespace deleted".to_string())
    }

    async fn list_agents(&self) -> Result<Vec<AgentInfoRow>, TuiError> {
        Ok(vec![])
    }

    async fn agent_activity(
        &self,
        _agent_id: &str,
        _limit: usize,
    ) -> Result<Vec<EpisodeRow>, TuiError> {
        Ok(vec![])
    }

    async fn cross_namespace_search(
        &self,
        _query: &str,
        _limit: usize,
    ) -> Result<Vec<SearchHit>, TuiError> {
        Ok(vec![])
    }

    async fn snapshot(&self, ts: &str) -> Result<SnapshotResult, TuiError> {
        Ok(SnapshotResult {
            timestamp: ts.to_string(),
            entity_count: 0,
            relation_count: 0,
            entities: vec![],
        })
    }

    async fn list_notes(
        &self,
        _tag: Option<&str>,
        _limit: usize,
    ) -> Result<Vec<NoteRow>, TuiError> {
        Ok(vec![])
    }

    async fn query_agent_runs(
        &self,
        _status: Option<&str>,
        _agent_id: Option<&str>,
        _limit: usize,
    ) -> Result<Vec<AgentRunRow>, TuiError> {
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::TuiBackend;

    #[tokio::test]
    async fn mock_list_recent_is_empty() {
        let m = MockTuiBackend;
        assert!(m.list_recent(5).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn mock_add_memory_succeeds() {
        let m = MockTuiBackend;
        let r = m.add_memory("x", "y").await.unwrap();
        assert_eq!(r.memory_count, 0);
    }
}
