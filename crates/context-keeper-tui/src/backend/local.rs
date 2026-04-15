//! Direct `Repository` access (same path as CLI / MCP server).

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use context_keeper_core::{
    ingestion,
    models::{AgentInfo, Episode},
    search::{fuse_rrf, QueryExpander},
    traits::{Embedder, EntityExtractor, EntityResolver, QueryRewriter, RelationExtractor},
};
use context_keeper_surreal::Repository;
use tracing::debug;
use uuid::Uuid;

use super::TuiBackend;
use crate::error::TuiError;
use crate::types::{
    AddMemoryResult, AgentInfoRow, AgentRunRow, EntityDetail, EntitySummary, EpisodeRow,
    GraphStats, MemoryRow, NamespaceInfo, NoteRow, RelationDirection, RelationRow, SearchHit,
    SnapshotResult,
};

#[derive(Clone)]
pub struct LocalBackend {
    repo: Repository,
    embedder: Arc<dyn Embedder>,
    entity_extractor: Arc<dyn EntityExtractor>,
    relation_extractor: Arc<dyn RelationExtractor>,
    query_rewriter: Arc<dyn QueryRewriter>,
    namespace: Option<String>,
    agent_id: Option<String>,
}

impl LocalBackend {
    pub fn new(
        repo: Repository,
        embedder: Arc<dyn Embedder>,
        entity_extractor: Arc<dyn EntityExtractor>,
        relation_extractor: Arc<dyn RelationExtractor>,
        query_rewriter: Arc<dyn QueryRewriter>,
        namespace: Option<String>,
        agent_id: Option<String>,
    ) -> Self {
        Self {
            repo,
            embedder,
            entity_extractor,
            relation_extractor,
            query_rewriter,
            namespace,
            agent_id,
        }
    }

    fn ns(&self) -> Option<&str> {
        self.namespace.as_deref()
    }
}

#[async_trait]
impl TuiBackend for LocalBackend {
    async fn add_memory(&self, text: &str, source: &str) -> Result<AddMemoryResult, TuiError> {
        let agent = self.agent_id.as_ref().map(|id| AgentInfo {
            agent_id: id.clone(),
            agent_name: None,
            machine_id: None,
        });
        let episode = Episode {
            id: Uuid::new_v4(),
            content: text.to_string(),
            source: source.to_string(),
            session_id: None,
            agent,
            namespace: self.namespace.clone(),
            created_at: Utc::now(),
        };

        let resolver: &dyn EntityResolver = &self.repo;
        let result = ingestion::ingest(
            &episode,
            self.embedder.as_ref(),
            self.entity_extractor.as_ref(),
            self.relation_extractor.as_ref(),
            Some(resolver),
            None,
            None,
        )
        .await?;

        for inv in &result.diff.entities_invalidated {
            self.repo.invalidate_entity(inv.invalidated_id).await?;
            let relations = self
                .repo
                .get_relations_for_entity(inv.invalidated_id)
                .await?;
            for rel in &relations {
                self.repo.invalidate_relation(rel.id).await?;
            }
        }

        for entity_id in &result.diff.entity_ids_to_invalidate_relations {
            self.repo
                .invalidate_relations_for_entity(*entity_id)
                .await?;
        }

        self.repo.create_episode(&episode).await?;

        for entity in &result.entities {
            self.repo.upsert_entity(entity).await?;
            debug!("Upserted entity: {}", entity.name);
        }
        for relation in &result.relations {
            self.repo.create_relation(relation).await?;
        }
        for memory in &result.memories {
            self.repo.create_memory(memory).await?;
        }

        Ok(AddMemoryResult {
            entity_count: result.entities.len(),
            relation_count: result.relations.len(),
            memory_count: result.memories.len(),
            entity_names: result.entities.iter().map(|e| e.name.clone()).collect(),
        })
    }

    async fn search_memory(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, TuiError> {
        let ns = self.ns();
        let query_embedding = self.embedder.embed(query).await?;
        let vector_results = self
            .repo
            .search_entities_by_vector(&query_embedding, limit, None, ns)
            .await?;
        let keyword_results = self
            .repo
            .search_entities_by_keyword(query, None, ns)
            .await?;

        let fused = fuse_rrf(vec![
            vector_results.into_iter().map(|(e, _)| e).collect(),
            keyword_results,
        ]);

        Ok(fused
            .iter()
            .take(limit)
            .filter_map(|r| {
                r.entity.as_ref().map(|e| SearchHit {
                    name: e.name.clone(),
                    entity_type: e.entity_type.to_string(),
                    summary: e.summary.clone(),
                    score: f64::from(r.score),
                })
            })
            .collect())
    }

    async fn expand_search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, TuiError> {
        let ns = self.ns();
        let expander = QueryExpander::new(3);
        let variants = expander.expand(query, self.query_rewriter.as_ref()).await?;

        let mut ranked_lists = Vec::new();
        for variant in &variants {
            let query_embedding = self.embedder.embed(variant).await?;
            let vector_results = self
                .repo
                .search_entities_by_vector(&query_embedding, limit, None, ns)
                .await?;
            let keyword_results = self
                .repo
                .search_entities_by_keyword(variant, None, ns)
                .await?;
            ranked_lists.push(vector_results.into_iter().map(|(e, _)| e).collect());
            ranked_lists.push(keyword_results);
        }

        let fused = fuse_rrf(ranked_lists);
        Ok(fused
            .iter()
            .take(limit)
            .filter_map(|r| {
                r.entity.as_ref().map(|e| SearchHit {
                    name: e.name.clone(),
                    entity_type: e.entity_type.to_string(),
                    summary: e.summary.clone(),
                    score: f64::from(r.score),
                })
            })
            .collect())
    }

    async fn list_recent(&self, limit: usize) -> Result<Vec<MemoryRow>, TuiError> {
        let memories = self.repo.list_recent_memories(limit).await?;
        Ok(memories
            .into_iter()
            .map(|m| MemoryRow {
                content: m.content,
                created_at: m.created_at.to_rfc3339(),
            })
            .collect())
    }

    async fn get_entity(&self, name: &str) -> Result<Option<EntityDetail>, TuiError> {
        let entities = self
            .repo
            .find_entities_by_name(name, None, self.ns())
            .await?;

        let entity = match entities.into_iter().next() {
            Some(e) => e,
            None => return Ok(None),
        };

        let relations = self.repo.get_relations_for_entity(entity.id).await?;
        let mut relation_rows = Vec::with_capacity(relations.len());

        for rel in &relations {
            let (target_id, direction) = if rel.from_entity_id == entity.id {
                (rel.to_entity_id, RelationDirection::Outgoing)
            } else {
                (rel.from_entity_id, RelationDirection::Incoming)
            };
            let target_name = match self.repo.get_entity(target_id).await? {
                Some(e) => e.name,
                None => target_id.to_string(),
            };
            relation_rows.push(RelationRow {
                relation_type: rel.relation_type.to_string(),
                target_name,
                direction,
                confidence: rel.confidence,
            });
        }

        Ok(Some(EntityDetail {
            name: entity.name,
            entity_type: entity.entity_type.to_string(),
            summary: entity.summary,
            valid_from: entity.valid_from.to_rfc3339(),
            valid_until: entity.valid_until.map(|d| d.to_rfc3339()),
            relations: relation_rows,
        }))
    }

    async fn list_entities(&self, limit: usize) -> Result<Vec<EntitySummary>, TuiError> {
        let entities = self
            .repo
            .get_all_active_entities_in_namespace(self.ns())
            .await?;
        Ok(entities
            .into_iter()
            .take(limit)
            .map(|e| EntitySummary {
                name: e.name,
                entity_type: e.entity_type.to_string(),
                summary: e.summary,
            })
            .collect())
    }

    async fn get_stats(&self) -> Result<GraphStats, TuiError> {
        let entities = self
            .repo
            .get_all_active_entities_in_namespace(self.ns())
            .await?;
        let memories = self.repo.list_recent_memories(10000).await?;
        let namespaces = self.repo.list_namespaces().await?;
        let agents = self.repo.list_agents().await?;
        Ok(GraphStats {
            entities: entities.len(),
            memories: memories.len(),
            namespaces: namespaces.len(),
            agents: agents.len(),
        })
    }

    async fn list_namespaces(&self) -> Result<Vec<NamespaceInfo>, TuiError> {
        let raw = self.repo.list_namespaces().await?;
        Ok(raw
            .into_iter()
            .map(|v| {
                let name = v
                    .get("namespace")
                    .and_then(|n| n.as_str())
                    .unwrap_or("(default)")
                    .to_string();
                let entity_count =
                    v.get("entity_count").and_then(|c| c.as_u64()).unwrap_or(0) as usize;
                NamespaceInfo { name, entity_count }
            })
            .collect())
    }

    async fn delete_namespace(&self, namespace: &str) -> Result<String, TuiError> {
        let result = self.repo.delete_namespace(namespace).await?;
        Ok(format!(
            "Deleted namespace — removed {} entities, {} memories, {} episodes",
            result.entities_deleted, result.memories_deleted, result.episodes_deleted
        ))
    }

    async fn list_agents(&self) -> Result<Vec<AgentInfoRow>, TuiError> {
        let raw = self.repo.list_agents().await?;
        Ok(raw
            .into_iter()
            .map(|v| {
                let agent_id = v
                    .get("agent_id")
                    .and_then(|a| a.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let agent_name = v
                    .get("agent_name")
                    .and_then(|a| a.as_str())
                    .map(String::from);
                let episode_count =
                    v.get("episode_count").and_then(|c| c.as_u64()).unwrap_or(0) as usize;
                AgentInfoRow {
                    agent_id,
                    agent_name,
                    episode_count,
                }
            })
            .collect())
    }

    async fn agent_activity(
        &self,
        agent_id: &str,
        limit: usize,
    ) -> Result<Vec<EpisodeRow>, TuiError> {
        let episodes = self.repo.list_episodes_by_agent(agent_id, limit).await?;
        Ok(episodes
            .into_iter()
            .map(|e| EpisodeRow {
                content: e.content,
                source: e.source,
                namespace: e.namespace,
                created_at: e.created_at.to_rfc3339(),
            })
            .collect())
    }

    async fn cross_namespace_search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchHit>, TuiError> {
        let query_embedding = self.embedder.embed(query).await?;
        let vector_results = self
            .repo
            .search_entities_by_vector(&query_embedding, limit, None, None)
            .await?;
        let keyword_results = self
            .repo
            .search_entities_by_keyword(query, None, None)
            .await?;

        let fused = fuse_rrf(vec![
            vector_results.into_iter().map(|(e, _)| e).collect(),
            keyword_results,
        ]);

        Ok(fused
            .iter()
            .take(limit)
            .filter_map(|r| {
                r.entity.as_ref().map(|e| SearchHit {
                    name: e.name.clone(),
                    entity_type: e.entity_type.to_string(),
                    summary: e.summary.clone(),
                    score: f64::from(r.score),
                })
            })
            .collect())
    }

    async fn snapshot(&self, iso_timestamp: &str) -> Result<SnapshotResult, TuiError> {
        let at: chrono::DateTime<chrono::Utc> = iso_timestamp
            .parse()
            .map_err(|e| TuiError::other(format!("Invalid timestamp: {e}")))?;

        let entities = self.repo.entities_at(at).await?;
        let relations = self.repo.relations_at(at).await?;

        Ok(SnapshotResult {
            timestamp: at.to_rfc3339(),
            entity_count: entities.len(),
            relation_count: relations.len(),
            entities: entities
                .into_iter()
                .map(|e| EntitySummary {
                    name: e.name,
                    entity_type: e.entity_type.to_string(),
                    summary: e.summary,
                })
                .collect(),
        })
    }

    async fn list_notes(&self, tag: Option<&str>, limit: usize) -> Result<Vec<NoteRow>, TuiError> {
        let tags: Option<Vec<String>> = tag.map(|t| vec![t.to_string()]);
        let notes = self
            .repo
            .list_notes(tags.as_deref(), limit, self.ns())
            .await?;
        Ok(notes
            .into_iter()
            .map(|n| NoteRow {
                key: n.key,
                content: n.content,
                tags: n.tags,
                namespace: n.namespace,
                created_at: n.created_at.to_rfc3339(),
                updated_at: n.updated_at.to_rfc3339(),
            })
            .collect())
    }

    async fn query_agent_runs(
        &self,
        status: Option<&str>,
        agent_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<AgentRunRow>, TuiError> {
        let episodes = self
            .repo
            .list_episodes_by_source("agent_status", agent_id, limit)
            .await?;

        let mut items = Vec::new();
        for episode in &episodes {
            let parsed: serde_json::Value = serde_json::from_str(&episode.content)
                .unwrap_or_else(|_| serde_json::json!({"raw": episode.content}));

            let ep_status = parsed
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            if let Some(filter_status) = status {
                if ep_status != filter_status {
                    continue;
                }
            }

            items.push(AgentRunRow {
                agent_id: episode.agent.as_ref().map(|a| a.agent_id.clone()),
                session_id: episode.session_id.clone(),
                status: ep_status.to_string(),
                summary: parsed
                    .get("summary")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                namespace: episode.namespace.clone(),
                created_at: episode.created_at.to_rfc3339(),
            });
        }
        Ok(items)
    }
}

#[cfg(test)]
mod tests {
    use crate::backend::TuiBackend;
    use crate::bootstrap::build_local_backend;

    #[tokio::test]
    async fn memory_backend_add_and_list_recent() {
        let backend = build_local_backend(
            "memory",
            "/nonexistent/no-import-for-tui-test.sql",
            1536,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .expect("local backend");

        backend
            .add_memory("TUI integration test memory", "test")
            .await
            .expect("ingest");

        let recent = backend.list_recent(10).await.expect("recent");
        assert!(
            recent.iter().any(|m| m.content.contains("TUI integration")),
            "expected ingested memory in list"
        );
    }
}
