//! MCP tool handler definitions for the Context Keeper server.
//!
//! Each tool corresponds to a capability exposed to MCP clients
//! (Claude Desktop, Cursor, etc.).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use context_keeper_core::{
    ingestion,
    models::{AgentInfo, Episode, Note},
    search::{fuse_rrf_mixed, QueryExpander},
    traits::{Embedder, EntityExtractor, EntityResolver, QueryRewriter, RelationExtractor},
    ContextKeeperError,
};
use context_keeper_surreal::{Repository, TenantRouter, DEFAULT_TENANT_ID};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars,
    service::RequestContext,
    tool, tool_handler, tool_router, ErrorData as McpError, RoleServer, ServerHandler,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::tenant::TenantContext;

fn to_mcp(err: ContextKeeperError) -> McpError {
    match err {
        ContextKeeperError::EntityNotFound(msg) => McpError::resource_not_found(msg, None),
        ContextKeeperError::ValidationError(msg) => McpError::invalid_params(msg, None),
        other => McpError::internal_error(other.to_string(), None),
    }
}

// ── Input schemas ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddMemoryInput {
    #[schemars(description = "The text content to ingest as a memory")]
    pub text: String,
    #[schemars(description = "Source label for the episode (e.g. 'chat', 'document')")]
    pub source: Option<String>,
    #[schemars(
        description = "Namespace to scope this memory to (e.g. 'project-alpha'). Omit for the default global namespace."
    )]
    pub namespace: Option<String>,
    #[schemars(description = "Identifier of the agent adding this memory (e.g. 'cursor-agent-1')")]
    pub agent_id: Option<String>,
    #[schemars(description = "Human-readable name of the agent")]
    pub agent_name: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchMemoryInput {
    #[schemars(description = "The search query string")]
    pub query: String,
    #[schemars(description = "Maximum number of results to return (default: 5)")]
    pub limit: Option<usize>,
    #[schemars(
        description = "Filter by entity type: person, organization, location, event, product, service, concept, file, other"
    )]
    pub entity_type: Option<String>,
    #[schemars(description = "Namespace to search within. Omit to search all namespaces.")]
    pub namespace: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ExpandSearchInput {
    #[schemars(description = "The search query to expand with semantic variants")]
    pub query: String,
    #[schemars(description = "Maximum number of results to return (default: 10)")]
    pub limit: Option<usize>,
    #[schemars(
        description = "Filter by entity type: person, organization, location, event, product, service, concept, file, other"
    )]
    pub entity_type: Option<String>,
    #[schemars(description = "Namespace to search within. Omit to search all namespaces.")]
    pub namespace: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetEntityInput {
    #[schemars(description = "The exact name of the entity to look up")]
    pub name: String,
    #[schemars(description = "Namespace to look up in. Omit to search all namespaces.")]
    pub namespace: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SnapshotInput {
    #[schemars(
        description = "ISO 8601 timestamp for the point-in-time snapshot (e.g. '2025-01-15T12:00:00Z')"
    )]
    pub timestamp: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListRecentInput {
    #[schemars(description = "Maximum number of recent memories to return (default: 10)")]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AgentActivityInput {
    #[schemars(description = "The agent_id to look up activity for")]
    pub agent_id: String,
    #[schemars(description = "Maximum number of recent episodes to return (default: 20)")]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CrossNamespaceSearchInput {
    #[schemars(description = "The search query string")]
    pub query: String,
    #[schemars(description = "Maximum number of results to return (default: 10)")]
    pub limit: Option<usize>,
    #[schemars(
        description = "Filter by entity type: person, organization, location, event, product, service, concept, file, other"
    )]
    pub entity_type: Option<String>,
}

// ── Note tool inputs ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SaveNoteInput {
    #[schemars(
        description = "A unique key for this note. If a note with this key already exists in the same namespace, its content will be updated."
    )]
    pub key: String,
    #[schemars(description = "The text content to store as a note")]
    pub content: String,
    #[schemars(description = "Optional tags to categorize the note for filtering")]
    pub tags: Option<Vec<String>>,
    #[schemars(
        description = "Namespace to scope this note to. Omit for the default global namespace."
    )]
    pub namespace: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetNoteInput {
    #[schemars(description = "The key of the note to retrieve")]
    pub key: String,
    #[schemars(description = "Namespace to look up in. Omit for the default global namespace.")]
    pub namespace: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchNotesInput {
    #[schemars(description = "The search query string")]
    pub query: String,
    #[schemars(description = "Filter notes that have any of these tags")]
    pub tags: Option<Vec<String>>,
    #[schemars(description = "Maximum number of results to return (default: 10)")]
    pub limit: Option<usize>,
    #[schemars(description = "Namespace to search within. Omit for the default global namespace.")]
    pub namespace: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListNotesInput {
    #[schemars(description = "Filter notes that have any of these tags")]
    pub tags: Option<Vec<String>>,
    #[schemars(description = "Maximum number of notes to return (default: 20)")]
    pub limit: Option<usize>,
    #[schemars(
        description = "Namespace to list notes from. Omit for the default global namespace."
    )]
    pub namespace: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DeleteNoteInput {
    #[schemars(description = "The key of the note to delete")]
    pub key: String,
    #[schemars(description = "Namespace of the note. Omit for the default global namespace.")]
    pub namespace: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DeleteNamespaceInput {
    #[schemars(
        description = "The namespace to delete all data for (entities, memories, episodes, notes, and their relations)"
    )]
    pub namespace: String,
}

// ── Agent status inputs ─────────────────────────────────────────────────

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PostAgentStatusInput {
    #[schemars(description = "Unique identifier of the agent posting the status")]
    pub agent_id: String,
    #[schemars(description = "Optional session identifier to group related status updates")]
    pub session_id: Option<String>,
    #[schemars(
        description = "Status of the agent run: 'started', 'in_progress', 'blocked', 'completed', or 'failed'"
    )]
    pub status: String,
    #[schemars(
        description = "Optional human-readable summary of what the agent is doing or has done"
    )]
    pub summary: Option<String>,
    #[schemars(
        description = "Namespace to scope this status to. Omit for the default global namespace."
    )]
    pub namespace: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct QueryAgentRunsInput {
    #[schemars(
        description = "Filter by status: 'started', 'in_progress', 'blocked', 'completed', or 'failed'"
    )]
    pub status: Option<String>,
    #[schemars(description = "Filter by a specific agent's ID")]
    pub agent_id: Option<String>,
    #[schemars(description = "Maximum number of results to return (default: 20)")]
    pub limit: Option<usize>,
    #[schemars(description = "Namespace to filter by. Omit to search all namespaces.")]
    pub namespace: Option<String>,
}

#[derive(Debug, Serialize)]
struct AddMemoryResponse {
    entities_created: usize,
    entities_updated: usize,
    entities_invalidated: usize,
    relations_created: usize,
    relations_merged: usize,
    relations_pruned: usize,
    relations_invalidated: usize,
    memories_created: usize,
    entity_names: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    updates: Vec<UpdateSummary>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    invalidations: Vec<InvalidationSummary>,
}

#[derive(Debug, Serialize)]
struct UpdateSummary {
    name: String,
    old_summary: String,
    new_summary: String,
}

#[derive(Debug, Serialize)]
struct InvalidationSummary {
    name: String,
    reason: String,
}

#[derive(Debug, Serialize)]
struct SearchResultItem {
    name: String,
    entity_type: String,
    summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    memory_content: Option<String>,
    score: f32,
}

#[derive(Debug, Serialize)]
struct EntityDetail {
    name: String,
    entity_type: String,
    summary: String,
    valid_from: String,
    valid_until: Option<String>,
    relations: Vec<RelationDetail>,
}

#[derive(Debug, Serialize)]
struct RelationDetail {
    relation_type: String,
    from_entity_id: String,
    from_entity_name: String,
    to_entity_id: String,
    to_entity_name: String,
    confidence: u8,
}

#[derive(Debug, Serialize)]
struct SnapshotResponse {
    timestamp: String,
    entity_count: usize,
    relation_count: usize,
    entities: Vec<SnapshotEntity>,
}

#[derive(Debug, Serialize)]
struct SnapshotEntity {
    name: String,
    entity_type: String,
    summary: String,
}

#[derive(Debug, Serialize)]
struct MemoryItem {
    content: String,
    created_at: String,
}

#[derive(Debug, Serialize)]
struct NoteItem {
    key: String,
    content: String,
    tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    namespace: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize)]
struct ScoredNoteItem {
    key: String,
    content: String,
    tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    namespace: Option<String>,
    score: f64,
}

// ── Helpers ──────────────────────────────────────────────────────────────

fn search_result_to_item(
    r: &context_keeper_core::models::SearchResult,
) -> Option<SearchResultItem> {
    if let Some(e) = r.entity.as_ref() {
        Some(SearchResultItem {
            name: e.name.clone(),
            entity_type: e.entity_type.to_string(),
            summary: e.summary.clone(),
            memory_content: None,
            score: r.score,
        })
    } else if let Some(m) = r.memory.as_ref() {
        Some(SearchResultItem {
            name: "[memory]".to_string(),
            entity_type: "memory".to_string(),
            summary: m.content.clone(),
            memory_content: Some(m.content.clone()),
            score: r.score,
        })
    } else {
        None
    }
}

// ── MCP Server ───────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct ContextKeeperServer {
    tool_router: ToolRouter<Self>,
    tenant_router: Arc<TenantRouter>,
    embedder: Arc<dyn Embedder>,
    entity_extractor: Arc<dyn EntityExtractor>,
    relation_extractor: Arc<dyn RelationExtractor>,
    query_rewriter: Arc<dyn QueryRewriter>,
}

impl ContextKeeperServer {
    pub fn new(
        tenant_router: Arc<TenantRouter>,
        embedder: Arc<dyn Embedder>,
        entity_extractor: Arc<dyn EntityExtractor>,
        relation_extractor: Arc<dyn RelationExtractor>,
        query_rewriter: Arc<dyn QueryRewriter>,
    ) -> Self {
        Self {
            tool_router: Self::tool_router(),
            tenant_router,
            embedder,
            entity_extractor,
            relation_extractor,
            query_rewriter,
        }
    }

    /// Extract the tenant-scoped [`Repository`] from rmcp request extensions.
    ///
    /// For HTTP: `TenantContext` was inserted into `http::request::Parts` by
    /// auth middleware; rmcp stores Parts in `RequestContext.extensions`.
    /// For stdio: falls back to the default tenant.
    async fn repo_for(&self, ctx: &RequestContext<RoleServer>) -> Result<Repository, McpError> {
        let tenant_id = ctx
            .extensions
            .get::<http::request::Parts>()
            .and_then(|parts| parts.extensions.get::<TenantContext>())
            .map(|tc| tc.tenant_id.as_str())
            .unwrap_or(DEFAULT_TENANT_ID);
        self.tenant_router
            .get_or_create(tenant_id)
            .await
            .map_err(to_mcp)
    }
}

#[tool_router]
impl ContextKeeperServer {
    /// Ingest a piece of text into the knowledge graph. Extracts entities and
    /// relations via LLM, generates embeddings, and stores everything in the
    /// graph database.
    #[tool(
        description = "Add a memory to the knowledge graph. Ingests text, extracts entities and relations, and stores everything with embeddings for later retrieval. Returns a diff of what was created, updated, or invalidated."
    )]
    async fn add_memory(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(input): Parameters<AddMemoryInput>,
    ) -> Result<String, McpError> {
        let repo = self.repo_for(&ctx).await?;
        let source = input.source.unwrap_or_else(|| "mcp".to_string());
        let agent = input.agent_id.map(|id| AgentInfo {
            agent_id: id,
            agent_name: input.agent_name,
            machine_id: None,
        });
        let episode = Episode {
            id: Uuid::new_v4(),
            content: input.text,
            source,
            session_id: None,
            agent,
            namespace: input.namespace,
            created_at: Utc::now(),
        };

        let resolver: &dyn EntityResolver = &repo;
        let result = ingestion::ingest(
            &episode,
            self.embedder.as_ref(),
            self.entity_extractor.as_ref(),
            self.relation_extractor.as_ref(),
            Some(resolver),
            None,
            None,
        )
        .await
        .map_err(to_mcp)?;

        for inv in &result.diff.entities_invalidated {
            repo.invalidate_entity(inv.invalidated_id)
                .await
                .map_err(to_mcp)?;
        }

        let mut relations_invalidated = 0usize;
        for entity_id in &result.diff.entity_ids_to_invalidate_relations {
            relations_invalidated += repo
                .invalidate_relations_for_entity(*entity_id)
                .await
                .map_err(to_mcp)?;
        }

        repo.create_episode(&episode).await.map_err(to_mcp)?;

        for entity in &result.entities {
            repo.upsert_entity(entity).await.map_err(to_mcp)?;
        }

        let mut relations_merged = 0usize;
        for relation in &result.relations {
            let created = repo.create_relation(relation).await.map_err(to_mcp)?;
            if !created {
                relations_merged += 1;
            }
        }
        for memory in &result.memories {
            repo.create_memory(memory).await.map_err(to_mcp)?;
        }

        let response = AddMemoryResponse {
            entities_created: result.diff.entities_created.len(),
            entities_updated: result.diff.entities_updated.len(),
            entities_invalidated: result.diff.entities_invalidated.len(),
            relations_created: result.diff.relations_created,
            relations_merged,
            relations_pruned: result.diff.relations_pruned,
            relations_invalidated,
            memories_created: result.memories.len(),
            entity_names: result.entities.iter().map(|e| e.name.clone()).collect(),
            updates: result
                .diff
                .entities_updated
                .iter()
                .map(|u| UpdateSummary {
                    name: u.name.clone(),
                    old_summary: u.old_summary.clone(),
                    new_summary: u.new_summary.clone(),
                })
                .collect(),
            invalidations: result
                .diff
                .entities_invalidated
                .iter()
                .map(|i| InvalidationSummary {
                    name: i.name.clone(),
                    reason: i.reason.clone(),
                })
                .collect(),
        };

        serde_json::to_string_pretty(&response)
            .map_err(|e| McpError::internal_error(format!("Serialization failed: {e}"), None))
    }

    /// Search the knowledge graph using hybrid vector + keyword search with
    /// Reciprocal Rank Fusion.
    #[tool(
        description = "Search memories and entities in the knowledge graph using hybrid vector similarity and BM25 keyword search, fused with Reciprocal Rank Fusion. Optionally filter by entity_type."
    )]
    async fn search_memory(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(input): Parameters<SearchMemoryInput>,
    ) -> Result<String, McpError> {
        let repo = self.repo_for(&ctx).await?;
        let limit = input.limit.unwrap_or(5);
        let type_filter = input.entity_type.as_deref();
        let ns = input.namespace.as_deref();

        let query_embedding = self.embedder.embed(&input.query).await.map_err(to_mcp)?;

        let entity_vector = repo
            .search_entities_by_vector(&query_embedding, limit, type_filter, ns)
            .await
            .map_err(to_mcp)?;
        let entity_keyword = repo
            .search_entities_by_keyword(&input.query, type_filter, ns)
            .await
            .map_err(to_mcp)?;

        let memory_vector = repo
            .search_memories_by_vector(&query_embedding, limit, ns)
            .await
            .map_err(to_mcp)?;
        let memory_keyword = repo
            .search_memories_by_keyword(&input.query, ns)
            .await
            .map_err(to_mcp)?;

        let fused = fuse_rrf_mixed(
            vec![
                entity_vector.into_iter().map(|(e, _)| e).collect(),
                entity_keyword,
            ],
            vec![
                memory_vector.into_iter().map(|(m, _)| m).collect(),
                memory_keyword,
            ],
        );

        let items: Vec<SearchResultItem> = fused
            .iter()
            .take(limit)
            .filter_map(search_result_to_item)
            .collect();

        serde_json::to_string_pretty(&items)
            .map_err(|e| McpError::internal_error(format!("Serialization failed: {e}"), None))
    }

    /// Run query expansion to improve recall when initial search results are
    /// sparse. Generates semantic variants of the query and searches each.
    #[tool(
        description = "Expand a search query into semantic variants using LLM rewriting, then search each variant and merge results with RRF for improved recall. Optionally filter by entity_type."
    )]
    async fn expand_search(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(input): Parameters<ExpandSearchInput>,
    ) -> Result<String, McpError> {
        let repo = self.repo_for(&ctx).await?;
        let limit = input.limit.unwrap_or(10);
        let type_filter = input.entity_type.as_deref();
        let ns = input.namespace.as_deref();
        let expander = QueryExpander::new(3);

        let variants = expander
            .expand(&input.query, self.query_rewriter.as_ref())
            .await
            .map_err(to_mcp)?;

        let mut entity_lists = Vec::new();
        let mut memory_lists = Vec::new();
        for variant in &variants {
            let query_embedding = self.embedder.embed(variant).await.map_err(to_mcp)?;

            let entity_vector = repo
                .search_entities_by_vector(&query_embedding, limit, type_filter, ns)
                .await
                .map_err(to_mcp)?;
            let entity_keyword = repo
                .search_entities_by_keyword(variant, type_filter, ns)
                .await
                .map_err(to_mcp)?;

            let memory_vector = repo
                .search_memories_by_vector(&query_embedding, limit, ns)
                .await
                .map_err(to_mcp)?;
            let memory_keyword = repo
                .search_memories_by_keyword(variant, ns)
                .await
                .map_err(to_mcp)?;

            entity_lists.push(entity_vector.into_iter().map(|(e, _)| e).collect());
            entity_lists.push(entity_keyword);
            memory_lists.push(memory_vector.into_iter().map(|(m, _)| m).collect());
            memory_lists.push(memory_keyword);
        }

        let fused = fuse_rrf_mixed(entity_lists, memory_lists);

        let items: Vec<SearchResultItem> = fused
            .iter()
            .take(limit)
            .filter_map(search_result_to_item)
            .collect();

        serde_json::to_string_pretty(&items)
            .map_err(|e| McpError::internal_error(format!("Serialization failed: {e}"), None))
    }

    /// Look up an entity by name and return its details including relationships.
    #[tool(
        description = "Get detailed information about a named entity, including its type, summary, temporal bounds, and all active relationships."
    )]
    async fn get_entity(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(input): Parameters<GetEntityInput>,
    ) -> Result<String, McpError> {
        let repo = self.repo_for(&ctx).await?;
        let entities = repo
            .find_entities_by_name(&input.name, None, input.namespace.as_deref())
            .await
            .map_err(to_mcp)?;

        if entities.is_empty() {
            return Ok(format!("No entity found with name '{}'", input.name));
        }

        let mut details = Vec::new();
        for entity in &entities {
            let relations = repo
                .get_relations_for_entity(entity.id)
                .await
                .map_err(to_mcp)?;

            let related_ids: Vec<uuid::Uuid> = relations
                .iter()
                .flat_map(|r| [r.from_entity_id, r.to_entity_id])
                .collect();
            let related_entities = repo
                .get_entities_by_ids(&related_ids)
                .await
                .map_err(to_mcp)?;
            let name_map: HashMap<uuid::Uuid, String> = related_entities
                .into_iter()
                .map(|e| (e.id, e.name))
                .collect();

            details.push(EntityDetail {
                name: entity.name.clone(),
                entity_type: entity.entity_type.to_string(),
                summary: entity.summary.clone(),
                valid_from: entity.valid_from.to_rfc3339(),
                valid_until: entity.valid_until.map(|d| d.to_rfc3339()),
                relations: relations
                    .iter()
                    .map(|r| RelationDetail {
                        relation_type: r.relation_type.to_string(),
                        from_entity_id: r.from_entity_id.to_string(),
                        from_entity_name: name_map
                            .get(&r.from_entity_id)
                            .cloned()
                            .unwrap_or_default(),
                        to_entity_id: r.to_entity_id.to_string(),
                        to_entity_name: name_map.get(&r.to_entity_id).cloned().unwrap_or_default(),
                        confidence: r.confidence,
                    })
                    .collect(),
            });
        }

        serde_json::to_string_pretty(&details)
            .map_err(|e| McpError::internal_error(format!("Serialization failed: {e}"), None))
    }

    /// Query the knowledge graph state at a specific point in time.
    #[tool(
        description = "Get a point-in-time snapshot of the knowledge graph at a specific timestamp, showing all entities and relations that were active at that moment."
    )]
    async fn snapshot(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(input): Parameters<SnapshotInput>,
    ) -> Result<String, McpError> {
        let repo = self.repo_for(&ctx).await?;
        let at: DateTime<Utc> = input
            .timestamp
            .parse()
            .map_err(|e| McpError::invalid_params(format!("Invalid timestamp: {e}"), None))?;

        let entities = repo.entities_at(at).await.map_err(to_mcp)?;

        let relations = repo.relations_at(at).await.map_err(to_mcp)?;

        let response = SnapshotResponse {
            timestamp: at.to_rfc3339(),
            entity_count: entities.len(),
            relation_count: relations.len(),
            entities: entities
                .iter()
                .map(|e| SnapshotEntity {
                    name: e.name.clone(),
                    entity_type: e.entity_type.to_string(),
                    summary: e.summary.clone(),
                })
                .collect(),
        };

        serde_json::to_string_pretty(&response)
            .map_err(|e| McpError::internal_error(format!("Serialization failed: {e}"), None))
    }

    /// List the most recently added memories.
    #[tool(
        description = "List the N most recently added memories from the knowledge graph, ordered by creation time."
    )]
    async fn list_recent(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(input): Parameters<ListRecentInput>,
    ) -> Result<String, McpError> {
        let repo = self.repo_for(&ctx).await?;
        let limit = input.limit.unwrap_or(10);

        let memories = repo.list_recent_memories(limit).await.map_err(to_mcp)?;

        let items: Vec<MemoryItem> = memories
            .iter()
            .map(|m| MemoryItem {
                content: m.content.clone(),
                created_at: m.created_at.to_rfc3339(),
            })
            .collect();

        serde_json::to_string_pretty(&items)
            .map_err(|e| McpError::internal_error(format!("Serialization failed: {e}"), None))
    }

    /// List all agents that have contributed memories to the knowledge graph.
    #[tool(
        description = "List all AI agents that have contributed to the knowledge graph, including their namespaces and episode counts. Useful for multi-agent setups to see who has been writing memories."
    )]
    async fn list_agents(&self, ctx: RequestContext<RoleServer>) -> Result<String, McpError> {
        let repo = self.repo_for(&ctx).await?;
        let agents = repo.list_agents().await.map_err(to_mcp)?;

        if agents.is_empty() {
            return Ok("No agents have contributed to the knowledge graph yet.".to_string());
        }

        serde_json::to_string_pretty(&agents)
            .map_err(|e| McpError::internal_error(format!("Serialization failed: {e}"), None))
    }

    /// List all namespaces in the knowledge graph.
    #[tool(
        description = "List all namespaces in the knowledge graph with entity counts. Namespaces partition the graph so different projects or teams can have isolated memory spaces."
    )]
    async fn list_namespaces(&self, ctx: RequestContext<RoleServer>) -> Result<String, McpError> {
        let repo = self.repo_for(&ctx).await?;
        let namespaces = repo.list_namespaces().await.map_err(to_mcp)?;

        if namespaces.is_empty() {
            return Ok(
                "No namespaces found. All data is in the default (global) namespace.".to_string(),
            );
        }

        serde_json::to_string_pretty(&namespaces)
            .map_err(|e| McpError::internal_error(format!("Serialization failed: {e}"), None))
    }

    /// Show recent activity from a specific agent.
    #[tool(
        description = "Show recent episodes ingested by a specific agent, identified by agent_id. Useful for auditing what a particular agent has been contributing."
    )]
    async fn agent_activity(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(input): Parameters<AgentActivityInput>,
    ) -> Result<String, McpError> {
        let repo = self.repo_for(&ctx).await?;
        let limit = input.limit.unwrap_or(20);
        let episodes = repo
            .list_episodes_by_agent(&input.agent_id, limit)
            .await
            .map_err(to_mcp)?;

        if episodes.is_empty() {
            return Ok(format!("No activity found for agent '{}'", input.agent_id));
        }

        let items: Vec<serde_json::Value> = episodes
            .iter()
            .map(|e| {
                serde_json::json!({
                    "content": e.content,
                    "source": e.source,
                    "namespace": e.namespace,
                    "created_at": e.created_at.to_rfc3339(),
                })
            })
            .collect();

        serde_json::to_string_pretty(&items)
            .map_err(|e| McpError::internal_error(format!("Serialization failed: {e}"), None))
    }

    /// Search across all namespaces regardless of the caller's default namespace.
    #[tool(
        description = "Search across all application-level namespaces within the current tenant's knowledge graph using hybrid vector + keyword search. Unlike search_memory, this always searches globally across namespaces, ignoring namespace scoping. Scoped to the authenticated tenant's data boundary."
    )]
    async fn cross_namespace_search(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(input): Parameters<CrossNamespaceSearchInput>,
    ) -> Result<String, McpError> {
        let repo = self.repo_for(&ctx).await?;
        let limit = input.limit.unwrap_or(10);
        let type_filter = input.entity_type.as_deref();

        let query_embedding = self.embedder.embed(&input.query).await.map_err(to_mcp)?;

        let entity_vector = repo
            .search_entities_by_vector(&query_embedding, limit, type_filter, None)
            .await
            .map_err(to_mcp)?;
        let entity_keyword = repo
            .search_entities_by_keyword(&input.query, type_filter, None)
            .await
            .map_err(to_mcp)?;

        let memory_vector = repo
            .search_memories_by_vector(&query_embedding, limit, None)
            .await
            .map_err(to_mcp)?;
        let memory_keyword = repo
            .search_memories_by_keyword(&input.query, None)
            .await
            .map_err(to_mcp)?;

        let fused = fuse_rrf_mixed(
            vec![
                entity_vector.into_iter().map(|(e, _)| e).collect(),
                entity_keyword,
            ],
            vec![
                memory_vector.into_iter().map(|(m, _)| m).collect(),
                memory_keyword,
            ],
        );

        let items: Vec<SearchResultItem> = fused
            .iter()
            .take(limit)
            .filter_map(search_result_to_item)
            .collect();

        serde_json::to_string_pretty(&items)
            .map_err(|e| McpError::internal_error(format!("Serialization failed: {e}"), None))
    }

    // ── Note Tools (Long-Term Memory) ─────────────────────────────────

    #[tool(
        description = "Save a text note with a unique key for later retrieval. Lightweight alternative to add_memory that skips entity/relation extraction. If a note with the same key exists in the same namespace, its content is updated."
    )]
    async fn save_note(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(input): Parameters<SaveNoteInput>,
    ) -> Result<String, McpError> {
        let repo = self.repo_for(&ctx).await?;
        let embedding = self.embedder.embed(&input.content).await.map_err(to_mcp)?;
        let now = Utc::now();

        let existing = repo
            .get_note_by_key(&input.key, input.namespace.as_deref())
            .await
            .map_err(to_mcp)?;

        let note = Note {
            id: existing.as_ref().map(|n| n.id).unwrap_or_else(Uuid::new_v4),
            key: input.key,
            content: input.content,
            embedding,
            tags: input.tags.unwrap_or_default(),
            namespace: input.namespace,
            created_at: existing.as_ref().map(|n| n.created_at).unwrap_or(now),
            updated_at: now,
        };

        repo.upsert_note(&note).await.map_err(to_mcp)?;

        let action = if existing.is_some() {
            "updated"
        } else {
            "created"
        };
        let response = serde_json::json!({
            "status": action,
            "key": note.key,
            "tags": note.tags,
            "namespace": note.namespace,
        });

        serde_json::to_string_pretty(&response)
            .map_err(|e| McpError::internal_error(format!("Serialization failed: {e}"), None))
    }

    #[tool(
        description = "Retrieve a saved note by its unique key. Returns the note content, tags, and timestamps."
    )]
    async fn get_note(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(input): Parameters<GetNoteInput>,
    ) -> Result<String, McpError> {
        let repo = self.repo_for(&ctx).await?;
        let note = repo
            .get_note_by_key(&input.key, input.namespace.as_deref())
            .await
            .map_err(to_mcp)?;

        match note {
            Some(n) => {
                let item = NoteItem {
                    key: n.key,
                    content: n.content,
                    tags: n.tags,
                    namespace: n.namespace,
                    created_at: n.created_at.to_rfc3339(),
                    updated_at: n.updated_at.to_rfc3339(),
                };
                serde_json::to_string_pretty(&item).map_err(|e| {
                    McpError::internal_error(format!("Serialization failed: {e}"), None)
                })
            }
            None => Ok(format!("No note found with key '{}'", input.key)),
        }
    }

    #[tool(
        description = "Search saved notes by content similarity using hybrid vector + keyword search. Optionally filter by tags."
    )]
    async fn search_notes(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(input): Parameters<SearchNotesInput>,
    ) -> Result<String, McpError> {
        let repo = self.repo_for(&ctx).await?;
        let limit = input.limit.unwrap_or(10);
        let tags = input.tags.as_deref();
        let ns = input.namespace.as_deref();

        let query_embedding = self.embedder.embed(&input.query).await.map_err(to_mcp)?;

        let vector_results = repo
            .search_notes_by_vector(&query_embedding, limit, tags, ns)
            .await
            .map_err(to_mcp)?;

        let keyword_results = repo
            .search_notes_by_keyword(&input.query, tags, ns)
            .await
            .map_err(to_mcp)?;

        let mut seen = HashSet::new();
        let mut items: Vec<ScoredNoteItem> = Vec::new();

        for (note, score) in &vector_results {
            if seen.insert(note.key.clone()) {
                items.push(ScoredNoteItem {
                    key: note.key.clone(),
                    content: note.content.clone(),
                    tags: note.tags.clone(),
                    namespace: note.namespace.clone(),
                    score: *score,
                });
            }
        }

        for note in &keyword_results {
            if seen.insert(note.key.clone()) {
                items.push(ScoredNoteItem {
                    key: note.key.clone(),
                    content: note.content.clone(),
                    tags: note.tags.clone(),
                    namespace: note.namespace.clone(),
                    score: 0.0,
                });
            }
        }

        items.truncate(limit);

        serde_json::to_string_pretty(&items)
            .map_err(|e| McpError::internal_error(format!("Serialization failed: {e}"), None))
    }

    #[tool(
        description = "List all saved notes, optionally filtered by tags. Returns notes sorted by most recently updated."
    )]
    async fn list_notes(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(input): Parameters<ListNotesInput>,
    ) -> Result<String, McpError> {
        let repo = self.repo_for(&ctx).await?;
        let limit = input.limit.unwrap_or(20);
        let tags = input.tags.as_deref();
        let ns = input.namespace.as_deref();

        let notes = repo.list_notes(tags, limit, ns).await.map_err(to_mcp)?;

        let items: Vec<NoteItem> = notes
            .into_iter()
            .map(|n| NoteItem {
                key: n.key,
                content: n.content,
                tags: n.tags,
                namespace: n.namespace,
                created_at: n.created_at.to_rfc3339(),
                updated_at: n.updated_at.to_rfc3339(),
            })
            .collect();

        serde_json::to_string_pretty(&items)
            .map_err(|e| McpError::internal_error(format!("Serialization failed: {e}"), None))
    }

    #[tool(description = "Delete a saved note by its unique key.")]
    async fn delete_note(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(input): Parameters<DeleteNoteInput>,
    ) -> Result<String, McpError> {
        let repo = self.repo_for(&ctx).await?;
        let deleted = repo
            .delete_note(&input.key, input.namespace.as_deref())
            .await
            .map_err(to_mcp)?;

        if deleted {
            Ok(format!("Note '{}' deleted.", input.key))
        } else {
            Ok(format!("No note found with key '{}'.", input.key))
        }
    }

    #[tool(
        description = "Permanently delete ALL data within a namespace: entities, relations, memories, episodes, and notes. This is irreversible."
    )]
    async fn delete_namespace(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(input): Parameters<DeleteNamespaceInput>,
    ) -> Result<String, McpError> {
        let repo = self.repo_for(&ctx).await?;
        let result = repo
            .delete_namespace(&input.namespace)
            .await
            .map_err(to_mcp)?;
        serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(format!("Serialization failed: {e}"), None))
    }

    // ── Agent Status Tools ──────────────────────────────────────────────

    #[tool(
        description = "Record an agent lifecycle status event (started, in_progress, blocked, completed, failed). Useful for multi-agent coordination and observability."
    )]
    async fn post_agent_status(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(input): Parameters<PostAgentStatusInput>,
    ) -> Result<String, McpError> {
        let repo = self.repo_for(&ctx).await?;

        const VALID_STATUSES: &[&str] =
            &["started", "in_progress", "blocked", "completed", "failed"];
        if !VALID_STATUSES.contains(&input.status.as_str()) {
            return Err(McpError::invalid_params(
                format!(
                    "Invalid status '{}'. Must be one of: {}",
                    input.status,
                    VALID_STATUSES.join(", ")
                ),
                None,
            ));
        }

        let content = serde_json::json!({
            "status": input.status,
            "summary": input.summary,
        });

        let episode = Episode {
            id: Uuid::new_v4(),
            content: content.to_string(),
            source: "agent_status".to_string(),
            session_id: input.session_id,
            agent: Some(AgentInfo {
                agent_id: input.agent_id.clone(),
                agent_name: None,
                machine_id: None,
            }),
            namespace: input.namespace,
            created_at: Utc::now(),
        };

        repo.create_episode(&episode).await.map_err(to_mcp)?;

        let response = serde_json::json!({
            "recorded": true,
            "agent_id": input.agent_id,
            "status": input.status,
            "episode_id": episode.id.to_string(),
        });

        serde_json::to_string_pretty(&response)
            .map_err(|e| McpError::internal_error(format!("Serialization failed: {e}"), None))
    }

    #[tool(
        description = "Query recent agent run statuses. Returns lifecycle events posted by agents, optionally filtered by status or agent_id. Useful for monitoring agent health and coordinating multi-agent workflows."
    )]
    async fn query_agent_runs(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(input): Parameters<QueryAgentRunsInput>,
    ) -> Result<String, McpError> {
        let repo = self.repo_for(&ctx).await?;
        let limit = input.limit.unwrap_or(20);

        let episodes = repo
            .list_episodes_by_source("agent_status", input.agent_id.as_deref(), limit)
            .await
            .map_err(to_mcp)?;

        let mut items: Vec<serde_json::Value> = Vec::new();
        for episode in &episodes {
            let parsed: serde_json::Value = serde_json::from_str(&episode.content)
                .unwrap_or_else(|_| serde_json::json!({"raw": episode.content}));

            let status = parsed
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            if let Some(ref filter_status) = input.status {
                if status != filter_status.as_str() {
                    continue;
                }
            }

            if let Some(ref filter_ns) = input.namespace {
                match &episode.namespace {
                    Some(ns) if ns == filter_ns => {}
                    _ => continue,
                }
            }

            items.push(serde_json::json!({
                "agent_id": episode.agent.as_ref().map(|a| &a.agent_id),
                "session_id": episode.session_id,
                "status": status,
                "summary": parsed.get("summary").and_then(|v| v.as_str()),
                "namespace": episode.namespace,
                "created_at": episode.created_at.to_rfc3339(),
            }));
        }

        if items.is_empty() {
            return Ok("No agent status updates found matching the criteria.".to_string());
        }

        serde_json::to_string_pretty(&items)
            .map_err(|e| McpError::internal_error(format!("Serialization failed: {e}"), None))
    }
}

#[tool_handler]
impl ServerHandler for ContextKeeperServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .build(),
        )
        .with_server_info(Implementation::new(
            "context-keeper",
            env!("CARGO_PKG_VERSION"),
        ))
        .with_instructions(
            "Context Keeper is a temporal knowledge graph memory system for multi-agent collaboration. \
             Use add_memory to store information (with optional namespace and agent_id for provenance), \
             search_memory or expand_search to retrieve it, get_entity for detailed entity lookups, \
             snapshot for point-in-time queries, and list_recent for recent memories. \
             For lightweight long-term memory: save_note stores a text note with a unique key, \
             get_note retrieves it by key, search_notes finds notes by content similarity, \
             list_notes lists all notes (optionally filtered by tags), delete_note removes a note, \
             and delete_namespace permanently deletes all data in a namespace (entities, relations, memories, episodes, notes). \
             Notes are simpler than add_memory — they skip entity/relation extraction and are ideal \
             for storing preferences, decisions, context, or any text you want to recall later. \
             For multi-agent workflows: list_agents shows contributing agents, list_namespaces \
             shows available scopes, agent_activity shows a specific agent's contributions, and \
             cross_namespace_search searches globally across all namespaces. \
             For agent run tracking: post_agent_status records lifecycle events (started, in_progress, \
             blocked, completed, failed) from agent sessions, and query_agent_runs retrieves recent \
             statuses filtered by agent or status for coordination and observability. \
             Resources: memory://recent (recent memories), memory://entities/summary (compact entity \
             list), memory://stats (graph-wide counts and namespaces). Use memory://entity/{name} \
             via resource templates to fetch full entity details.",
        )
    }

    // ── MCP Resources ────────────────────────────────────────────────

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        let resources: Vec<Resource> = vec![
            RawResource {
                uri: "memory://recent".into(),
                name: "recent-memories".into(),
                title: None,
                description: Some("The 20 most recently added memories".into()),
                mime_type: Some("application/json".into()),
                size: None,
                icons: None,
                meta: None,
            }
            .no_annotation(),
            RawResource {
                uri: "memory://entities/summary".into(),
                name: "entities-summary".into(),
                title: None,
                description: Some(
                    "Compact summary of all active entities (names and types)".into(),
                ),
                mime_type: Some("application/json".into()),
                size: None,
                icons: None,
                meta: None,
            }
            .no_annotation(),
            RawResource {
                uri: "memory://stats".into(),
                name: "graph-stats".into(),
                title: None,
                description: Some(
                    "Knowledge graph statistics: entity, memory, episode, and relation counts with namespace list".into(),
                ),
                mime_type: Some("application/json".into()),
                size: None,
                icons: None,
                meta: None,
            }
            .no_annotation(),
        ];

        Ok(ListResourcesResult {
            resources,
            next_cursor: None,
            meta: None,
        })
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        Ok(ListResourceTemplatesResult {
            resource_templates: vec![ResourceTemplate {
                raw: RawResourceTemplate {
                    uri_template: "memory://entity/{name}".into(),
                    name: "entity-detail".into(),
                    title: Some("Entity Detail".into()),
                    description: Some(
                        "Get detailed information about a named entity including relationships"
                            .into(),
                    ),
                    mime_type: Some("application/json".into()),
                    icons: None,
                },
                annotations: None,
            }],
            next_cursor: None,
            meta: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let repo = self.repo_for(&context).await?;
        let uri = &request.uri;

        if uri == "memory://recent" {
            let memories = repo.list_recent_memories(20).await.map_err(to_mcp)?;

            let items: Vec<MemoryItem> = memories
                .iter()
                .map(|m| MemoryItem {
                    content: m.content.clone(),
                    created_at: m.created_at.to_rfc3339(),
                })
                .collect();

            let text = serde_json::to_string_pretty(&items).map_err(|e| {
                McpError::internal_error(format!("Serialization failed: {e}"), None)
            })?;

            return Ok(ReadResourceResult::new(vec![ResourceContents::text(
                text, uri,
            )]));
        }

        if uri == "memory://entities/summary" {
            let entities = repo.get_all_active_entities().await.map_err(to_mcp)?;
            let summary: Vec<serde_json::Value> = entities
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "name": e.name,
                        "type": e.entity_type.to_string(),
                    })
                })
                .collect();
            let text = serde_json::to_string_pretty(&summary).map_err(|e| {
                McpError::internal_error(format!("Serialization failed: {e}"), None)
            })?;
            return Ok(ReadResourceResult::new(vec![ResourceContents::text(
                text, uri,
            )]));
        }

        if uri == "memory://stats" {
            let entity_count = repo.count_active_entities().await.map_err(to_mcp)?;
            let memory_count = repo.count_memories().await.map_err(to_mcp)?;
            let episode_count = repo.count_episodes().await.map_err(to_mcp)?;
            let relation_count = repo.count_active_relations().await.map_err(to_mcp)?;
            let namespaces = repo.list_namespaces().await.map_err(to_mcp)?;

            let stats = serde_json::json!({
                "entity_count": entity_count,
                "memory_count": memory_count,
                "episode_count": episode_count,
                "relation_count": relation_count,
                "namespaces": namespaces,
            });
            let text = serde_json::to_string_pretty(&stats).map_err(|e| {
                McpError::internal_error(format!("Serialization failed: {e}"), None)
            })?;
            return Ok(ReadResourceResult::new(vec![ResourceContents::text(
                text, uri,
            )]));
        }

        if let Some(name) = uri.strip_prefix("memory://entity/") {
            let entities = repo
                .find_entities_by_name(name, None, None)
                .await
                .map_err(to_mcp)?;

            if entities.is_empty() {
                return Err(McpError::resource_not_found(
                    format!("No entity found with name '{}'", name),
                    None,
                ));
            }

            let mut details = Vec::new();
            for entity in &entities {
                let relations = repo
                    .get_relations_for_entity(entity.id)
                    .await
                    .map_err(to_mcp)?;

                let related_ids: Vec<uuid::Uuid> = relations
                    .iter()
                    .flat_map(|r| [r.from_entity_id, r.to_entity_id])
                    .collect();
                let related_entities = repo
                    .get_entities_by_ids(&related_ids)
                    .await
                    .map_err(to_mcp)?;
                let name_map: HashMap<uuid::Uuid, String> = related_entities
                    .into_iter()
                    .map(|e| (e.id, e.name))
                    .collect();

                details.push(EntityDetail {
                    name: entity.name.clone(),
                    entity_type: entity.entity_type.to_string(),
                    summary: entity.summary.clone(),
                    valid_from: entity.valid_from.to_rfc3339(),
                    valid_until: entity.valid_until.map(|d| d.to_rfc3339()),
                    relations: relations
                        .iter()
                        .map(|r| RelationDetail {
                            relation_type: r.relation_type.to_string(),
                            from_entity_id: r.from_entity_id.to_string(),
                            from_entity_name: name_map
                                .get(&r.from_entity_id)
                                .cloned()
                                .unwrap_or_default(),
                            to_entity_id: r.to_entity_id.to_string(),
                            to_entity_name: name_map
                                .get(&r.to_entity_id)
                                .cloned()
                                .unwrap_or_default(),
                            confidence: r.confidence,
                        })
                        .collect(),
                });
            }

            let text = serde_json::to_string_pretty(&details).map_err(|e| {
                McpError::internal_error(format!("Serialization failed: {e}"), None)
            })?;

            return Ok(ReadResourceResult::new(vec![ResourceContents::text(
                text, uri,
            )]));
        }

        Err(McpError::resource_not_found(
            format!("Unknown resource URI: {uri}"),
            None,
        ))
    }

    // ── MCP Prompts ──────────────────────────────────────────────────

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        Ok(ListPromptsResult {
            prompts: vec![
                Prompt::new(
                    "summarize-topic",
                    Some("Summarize everything known about a topic from the knowledge graph"),
                    Some(vec![PromptArgument::new("topic")
                        .with_description("The topic to summarize")
                        .with_required(true)]),
                ),
                Prompt::new(
                    "what-changed",
                    Some("Describe what changed in the knowledge graph since a given date"),
                    Some(vec![PromptArgument::new("since")
                        .with_description(
                            "ISO 8601 date/time to look back from (e.g. '2025-01-15T00:00:00Z')",
                        )
                        .with_required(true)]),
                ),
                Prompt::new(
                    "add-context",
                    Some("Ingest context from the current conversation into the knowledge graph"),
                    Some(vec![PromptArgument::new("context")
                        .with_description("The conversation context or notes to remember")
                        .with_required(true)]),
                ),
            ],
            next_cursor: None,
            meta: None,
        })
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, McpError> {
        let args = request.arguments.unwrap_or_default();
        let get_str = |key: &str| -> String {
            args.get(key)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string()
        };

        match request.name.as_str() {
            "summarize-topic" => {
                let topic = get_str("topic");
                Ok(GetPromptResult::new(vec![PromptMessage::new_text(
                    PromptMessageRole::User,
                    format!(
                        "Search the knowledge graph for everything related to '{}' using search_memory and expand_search, \
                         then provide a comprehensive summary of all entities, relationships, and memories found. \
                         Include temporal information about when things were recorded.",
                        topic
                    ),
                )])
                .with_description(format!("Summarize what is known about '{}'", topic)))
            }
            "what-changed" => {
                let since = get_str("since");
                Ok(GetPromptResult::new(vec![PromptMessage::new_text(
                    PromptMessageRole::User,
                    format!(
                        "Use list_recent to get recent memories, then use snapshot with timestamp '{}' \
                         to see the graph state at that point. Compare with the current state and describe \
                         what entities, relationships, or memories have been added or changed since then.",
                        since
                    ),
                )])
                .with_description(format!("Changes since {}", since)))
            }
            "add-context" => {
                let context = get_str("context");
                Ok(GetPromptResult::new(vec![PromptMessage::new_text(
                    PromptMessageRole::User,
                    format!(
                        "Use add_memory to ingest the following context into the knowledge graph. \
                         After ingestion, briefly confirm what entities and relations were extracted:\n\n{}",
                        context
                    ),
                )])
                .with_description("Add context to the knowledge graph"))
            }
            other => Err(McpError::invalid_params(
                format!("Unknown prompt: '{}'", other),
                None,
            )),
        }
    }
}
