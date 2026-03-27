//! MCP tool handler definitions for the Context Keeper server.
//!
//! Each tool corresponds to a capability exposed to MCP clients
//! (Claude Desktop, Cursor, etc.).

use std::sync::Arc;

use chrono::{DateTime, Utc};
use context_keeper_core::{
    ingestion,
    models::{AgentInfo, Episode},
    search::{fuse_rrf, QueryExpander},
    traits::{Embedder, EntityExtractor, EntityResolver, QueryRewriter, RelationExtractor},
    ContextKeeperError,
};
use context_keeper_surreal::Repository;
use rmcp::{
    handler::server::{
        router::tool::ToolRouter,
        wrapper::Parameters,
    },
    model::*, schemars, tool, tool_handler, tool_router,
    ErrorData as McpError, RoleServer, ServerHandler,
    service::RequestContext,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    #[schemars(description = "Namespace to scope this memory to (e.g. 'project-alpha'). Omit for the default global namespace.")]
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
    #[schemars(description = "Filter by entity type: person, organization, location, event, product, service, concept, file, other")]
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
    #[schemars(description = "Filter by entity type: person, organization, location, event, product, service, concept, file, other")]
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
    #[schemars(description = "ISO 8601 timestamp for the point-in-time snapshot (e.g. '2025-01-15T12:00:00Z')")]
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
    #[schemars(description = "Filter by entity type: person, organization, location, event, product, service, concept, file, other")]
    pub entity_type: Option<String>,
}

// ── Serializable response types ──────────────────────────────────────────

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
    to_entity_id: String,
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

// ── MCP Server ───────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct ContextKeeperServer {
    tool_router: ToolRouter<Self>,
    repo: Repository,
    embedder: Arc<dyn Embedder>,
    entity_extractor: Arc<dyn EntityExtractor>,
    relation_extractor: Arc<dyn RelationExtractor>,
    query_rewriter: Arc<dyn QueryRewriter>,
}

impl ContextKeeperServer {
    pub fn new(
        repo: Repository,
        embedder: Arc<dyn Embedder>,
        entity_extractor: Arc<dyn EntityExtractor>,
        relation_extractor: Arc<dyn RelationExtractor>,
        query_rewriter: Arc<dyn QueryRewriter>,
    ) -> Self {
        Self {
            tool_router: Self::tool_router(),
            repo,
            embedder,
            entity_extractor,
            relation_extractor,
            query_rewriter,
        }
    }
}

#[tool_router]
impl ContextKeeperServer {
    /// Ingest a piece of text into the knowledge graph. Extracts entities and
    /// relations via LLM, generates embeddings, and stores everything in the
    /// graph database.
    #[tool(description = "Add a memory to the knowledge graph. Ingests text, extracts entities and relations, and stores everything with embeddings for later retrieval. Returns a diff of what was created, updated, or invalidated.")]
    async fn add_memory(
        &self,
        Parameters(input): Parameters<AddMemoryInput>,
    ) -> Result<String, McpError> {
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

        let resolver: &dyn EntityResolver = &self.repo;
        let result = ingestion::ingest(
            &episode,
            self.embedder.as_ref(),
            self.entity_extractor.as_ref(),
            self.relation_extractor.as_ref(),
            Some(resolver),
            None,
        )
        .await
        .map_err(to_mcp)?;

        let ep_ns = episode.namespace.as_deref();
        for inv in &result.diff.entities_invalidated {
            let existing = self
                .repo
                .find_entities_by_name(&inv.name, ep_ns)
                .await
                .map_err(to_mcp)?;
            for entity in &existing {
                self.repo
                    .invalidate_entity(entity.id)
                    .await
                    .map_err(to_mcp)?;
            }
        }

        let mut relations_invalidated = 0usize;
        for entity_id in &result.diff.entity_ids_to_invalidate_relations {
            relations_invalidated += self
                .repo
                .invalidate_relations_for_entity(*entity_id)
                .await
                .map_err(to_mcp)?;
        }

        self.repo
            .create_episode(&episode)
            .await
            .map_err(to_mcp)?;

        for entity in &result.entities {
            self.repo
                .upsert_entity(entity)
                .await
                .map_err(to_mcp)?;
        }

        let mut relations_merged = 0usize;
        for relation in &result.relations {
            let created = self
                .repo
                .create_relation(relation)
                .await
                .map_err(to_mcp)?;
            if !created {
                relations_merged += 1;
            }
        }
        for memory in &result.memories {
            self.repo
                .create_memory(memory)
                .await
                .map_err(to_mcp)?;
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
    #[tool(description = "Search memories and entities in the knowledge graph using hybrid vector similarity and BM25 keyword search, fused with Reciprocal Rank Fusion. Optionally filter by entity_type.")]
    async fn search_memory(
        &self,
        Parameters(input): Parameters<SearchMemoryInput>,
    ) -> Result<String, McpError> {
        let limit = input.limit.unwrap_or(5);
        let type_filter = input.entity_type.as_deref();
        let ns = input.namespace.as_deref();

        let query_embedding = self
            .embedder
            .embed(&input.query)
            .await
            .map_err(to_mcp)?;

        let vector_results = self
            .repo
            .search_entities_by_vector(&query_embedding, limit, type_filter, ns)
            .await
            .map_err(to_mcp)?;

        let keyword_results = self
            .repo
            .search_entities_by_keyword(&input.query, type_filter, ns)
            .await
            .map_err(to_mcp)?;

        let fused = fuse_rrf(vec![
            vector_results.into_iter().map(|(e, _)| e).collect(),
            keyword_results,
        ]);

        let items: Vec<SearchResultItem> = fused
            .iter()
            .take(limit)
            .filter_map(|r| {
                r.entity.as_ref().map(|e| SearchResultItem {
                    name: e.name.clone(),
                    entity_type: e.entity_type.to_string(),
                    summary: e.summary.clone(),
                    score: r.score,
                })
            })
            .collect();

        serde_json::to_string_pretty(&items)
            .map_err(|e| McpError::internal_error(format!("Serialization failed: {e}"), None))
    }

    /// Run query expansion to improve recall when initial search results are
    /// sparse. Generates semantic variants of the query and searches each.
    #[tool(description = "Expand a search query into semantic variants using LLM rewriting, then search each variant and merge results with RRF for improved recall. Optionally filter by entity_type.")]
    async fn expand_search(
        &self,
        Parameters(input): Parameters<ExpandSearchInput>,
    ) -> Result<String, McpError> {
        let limit = input.limit.unwrap_or(10);
        let type_filter = input.entity_type.as_deref();
        let ns = input.namespace.as_deref();
        let expander = QueryExpander::new(3);

        let variants = expander
            .expand(&input.query, self.query_rewriter.as_ref())
            .await
            .map_err(to_mcp)?;

        let mut ranked_lists = Vec::new();
        for variant in &variants {
            let query_embedding = self
                .embedder
                .embed(variant)
                .await
                .map_err(to_mcp)?;

            let vector_results = self
                .repo
                .search_entities_by_vector(&query_embedding, limit, type_filter, ns)
                .await
                .map_err(to_mcp)?;

            let keyword_results = self
                .repo
                .search_entities_by_keyword(variant, type_filter, ns)
                .await
                .map_err(to_mcp)?;

            ranked_lists.push(vector_results.into_iter().map(|(e, _)| e).collect());
            ranked_lists.push(keyword_results);
        }

        let fused = fuse_rrf(ranked_lists);

        let items: Vec<SearchResultItem> = fused
            .iter()
            .take(limit)
            .filter_map(|r| {
                r.entity.as_ref().map(|e| SearchResultItem {
                    name: e.name.clone(),
                    entity_type: e.entity_type.to_string(),
                    summary: e.summary.clone(),
                    score: r.score,
                })
            })
            .collect();

        serde_json::to_string_pretty(&items)
            .map_err(|e| McpError::internal_error(format!("Serialization failed: {e}"), None))
    }

    /// Look up an entity by name and return its details including relationships.
    #[tool(description = "Get detailed information about a named entity, including its type, summary, temporal bounds, and all active relationships.")]
    async fn get_entity(
        &self,
        Parameters(input): Parameters<GetEntityInput>,
    ) -> Result<String, McpError> {
        let entities = self
            .repo
            .find_entities_by_name(&input.name, input.namespace.as_deref())
            .await
            .map_err(to_mcp)?;

        if entities.is_empty() {
            return Ok(format!("No entity found with name '{}'", input.name));
        }

        let mut details = Vec::new();
        for entity in &entities {
            let relations = self
                .repo
                .get_relations_for_entity(entity.id)
                .await
                .map_err(to_mcp)?;

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
                        to_entity_id: r.to_entity_id.to_string(),
                        confidence: r.confidence,
                    })
                    .collect(),
            });
        }

        serde_json::to_string_pretty(&details)
            .map_err(|e| McpError::internal_error(format!("Serialization failed: {e}"), None))
    }

    /// Query the knowledge graph state at a specific point in time.
    #[tool(description = "Get a point-in-time snapshot of the knowledge graph at a specific timestamp, showing all entities and relations that were active at that moment.")]
    async fn snapshot(
        &self,
        Parameters(input): Parameters<SnapshotInput>,
    ) -> Result<String, McpError> {
        let at: DateTime<Utc> = input
            .timestamp
            .parse()
            .map_err(|e| McpError::invalid_params(format!("Invalid timestamp: {e}"), None))?;

        let entities = self
            .repo
            .entities_at(at)
            .await
            .map_err(to_mcp)?;

        let relations = self
            .repo
            .relations_at(at)
            .await
            .map_err(to_mcp)?;

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
    #[tool(description = "List the N most recently added memories from the knowledge graph, ordered by creation time.")]
    async fn list_recent(
        &self,
        Parameters(input): Parameters<ListRecentInput>,
    ) -> Result<String, McpError> {
        let limit = input.limit.unwrap_or(10);

        let memories = self
            .repo
            .list_recent_memories(limit)
            .await
            .map_err(to_mcp)?;

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
    #[tool(description = "List all AI agents that have contributed to the knowledge graph, including their namespaces and episode counts. Useful for multi-agent setups to see who has been writing memories.")]
    async fn list_agents(&self) -> Result<String, McpError> {
        let agents = self
            .repo
            .list_agents()
            .await
            .map_err(to_mcp)?;

        if agents.is_empty() {
            return Ok("No agents have contributed to the knowledge graph yet.".to_string());
        }

        serde_json::to_string_pretty(&agents)
            .map_err(|e| McpError::internal_error(format!("Serialization failed: {e}"), None))
    }

    /// List all namespaces in the knowledge graph.
    #[tool(description = "List all namespaces in the knowledge graph with entity counts. Namespaces partition the graph so different projects or teams can have isolated memory spaces.")]
    async fn list_namespaces(&self) -> Result<String, McpError> {
        let namespaces = self
            .repo
            .list_namespaces()
            .await
            .map_err(to_mcp)?;

        if namespaces.is_empty() {
            return Ok("No namespaces found. All data is in the default (global) namespace.".to_string());
        }

        serde_json::to_string_pretty(&namespaces)
            .map_err(|e| McpError::internal_error(format!("Serialization failed: {e}"), None))
    }

    /// Show recent activity from a specific agent.
    #[tool(description = "Show recent episodes ingested by a specific agent, identified by agent_id. Useful for auditing what a particular agent has been contributing.")]
    async fn agent_activity(
        &self,
        Parameters(input): Parameters<AgentActivityInput>,
    ) -> Result<String, McpError> {
        let limit = input.limit.unwrap_or(20);
        let episodes = self
            .repo
            .list_episodes_by_agent(&input.agent_id, limit)
            .await
            .map_err(to_mcp)?;

        if episodes.is_empty() {
            return Ok(format!("No activity found for agent '{}'", input.agent_id));
        }

        let items: Vec<serde_json::Value> = episodes
            .iter()
            .map(|e| serde_json::json!({
                "content": e.content,
                "source": e.source,
                "namespace": e.namespace,
                "created_at": e.created_at.to_rfc3339(),
            }))
            .collect();

        serde_json::to_string_pretty(&items)
            .map_err(|e| McpError::internal_error(format!("Serialization failed: {e}"), None))
    }

    /// Search across all namespaces regardless of the caller's default namespace.
    #[tool(description = "Search the entire knowledge graph across all namespaces using hybrid vector + keyword search. Unlike search_memory, this always searches globally, ignoring namespace scoping.")]
    async fn cross_namespace_search(
        &self,
        Parameters(input): Parameters<CrossNamespaceSearchInput>,
    ) -> Result<String, McpError> {
        let limit = input.limit.unwrap_or(10);
        let type_filter = input.entity_type.as_deref();

        let query_embedding = self
            .embedder
            .embed(&input.query)
            .await
            .map_err(to_mcp)?;

        let vector_results = self
            .repo
            .search_entities_by_vector(&query_embedding, limit, type_filter, None)
            .await
            .map_err(to_mcp)?;

        let keyword_results = self
            .repo
            .search_entities_by_keyword(&input.query, type_filter, None)
            .await
            .map_err(to_mcp)?;

        let fused = fuse_rrf(vec![
            vector_results.into_iter().map(|(e, _)| e).collect(),
            keyword_results,
        ]);

        let items: Vec<SearchResultItem> = fused
            .iter()
            .take(limit)
            .filter_map(|r| {
                r.entity.as_ref().map(|e| SearchResultItem {
                    name: e.name.clone(),
                    entity_type: e.entity_type.to_string(),
                    summary: e.summary.clone(),
                    score: r.score,
                })
            })
            .collect();

        serde_json::to_string_pretty(&items)
            .map_err(|e| McpError::internal_error(format!("Serialization failed: {e}"), None))
    }
}

#[tool_handler]
impl ServerHandler for ContextKeeperServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .build(),
            server_info: Implementation {
                name: "context-keeper".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                title: None,
                icons: None,
                website_url: None,
            },
            instructions: Some(
                "Context Keeper is a temporal knowledge graph memory system for multi-agent collaboration. \
                 Use add_memory to store information (with optional namespace and agent_id for provenance), \
                 search_memory or expand_search to retrieve it, get_entity for detailed entity lookups, \
                 snapshot for point-in-time queries, and list_recent for recent memories. \
                 For multi-agent workflows: list_agents shows contributing agents, list_namespaces \
                 shows available scopes, agent_activity shows a specific agent's contributions, and \
                 cross_namespace_search searches globally across all namespaces."
                    .into(),
            ),
        }
    }

    // ── MCP Resources ────────────────────────────────────────────────

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        let mut resources: Vec<Resource> = vec![
            RawResource {
                uri: "memory://recent".into(),
                name: "recent-memories".into(),
                title: None,
                description: Some("The 20 most recently added memories".into()),
                mime_type: Some("application/json".into()),
                size: None,
                icons: None,
            }.no_annotation(),
        ];

        let entities = self
            .repo
            .get_all_active_entities()
            .await
            .map_err(to_mcp)?;

        for entity in &entities {
            resources.push(
                RawResource {
                    uri: format!("memory://entity/{}", entity.name),
                    name: entity.name.clone(),
                    title: None,
                    description: Some(format!("{} ({})", entity.summary, entity.entity_type)),
                    mime_type: Some("application/json".into()),
                    size: None,
                    icons: None,
                }.no_annotation(),
            );
        }

        Ok(ListResourcesResult {
            resources,
            next_cursor: None,
        })
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        Ok(ListResourceTemplatesResult {
            resource_templates: vec![
                ResourceTemplate {
                    raw: RawResourceTemplate {
                        uri_template: "memory://entity/{name}".into(),
                        name: "entity-detail".into(),
                        title: Some("Entity Detail".into()),
                        description: Some("Get detailed information about a named entity including relationships".into()),
                        mime_type: Some("application/json".into()),
                    },
                    annotations: None,
                },
            ],
            next_cursor: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let uri = &request.uri;

        if uri == "memory://recent" {
            let memories = self
                .repo
                .list_recent_memories(20)
                .await
                .map_err(to_mcp)?;

            let items: Vec<MemoryItem> = memories
                .iter()
                .map(|m| MemoryItem {
                    content: m.content.clone(),
                    created_at: m.created_at.to_rfc3339(),
                })
                .collect();

            let text = serde_json::to_string_pretty(&items)
                .map_err(|e| McpError::internal_error(format!("Serialization failed: {e}"), None))?;

            return Ok(ReadResourceResult {
                contents: vec![ResourceContents::text(text, uri)],
            });
        }

        if let Some(name) = uri.strip_prefix("memory://entity/") {
            let entities = self
                .repo
                .find_entities_by_name(name, None)
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
                let relations = self
                    .repo
                    .get_relations_for_entity(entity.id)
                    .await
                    .map_err(to_mcp)?;

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
                            to_entity_id: r.to_entity_id.to_string(),
                            confidence: r.confidence,
                        })
                        .collect(),
                });
            }

            let text = serde_json::to_string_pretty(&details)
                .map_err(|e| McpError::internal_error(format!("Serialization failed: {e}"), None))?;

            return Ok(ReadResourceResult {
                contents: vec![ResourceContents::text(text, uri)],
            });
        }

        Err(McpError::resource_not_found(
            format!("Unknown resource URI: {uri}"),
            None,
        ))
    }

    // ── MCP Prompts ──────────────────────────────────────────────────

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        Ok(ListPromptsResult {
            prompts: vec![
                Prompt::new(
                    "summarize-topic",
                    Some("Summarize everything known about a topic from the knowledge graph"),
                    Some(vec![PromptArgument {
                        name: "topic".into(),
                        title: None,
                        description: Some("The topic to summarize".into()),
                        required: Some(true),
                    }]),
                ),
                Prompt::new(
                    "what-changed",
                    Some("Describe what changed in the knowledge graph since a given date"),
                    Some(vec![PromptArgument {
                        name: "since".into(),
                        title: None,
                        description: Some("ISO 8601 date/time to look back from (e.g. '2025-01-15T00:00:00Z')".into()),
                        required: Some(true),
                    }]),
                ),
                Prompt::new(
                    "add-context",
                    Some("Ingest context from the current conversation into the knowledge graph"),
                    Some(vec![PromptArgument {
                        name: "context".into(),
                        title: None,
                        description: Some("The conversation context or notes to remember".into()),
                        required: Some(true),
                    }]),
                ),
            ],
            next_cursor: None,
        })
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParam,
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
                Ok(GetPromptResult {
                    description: Some(format!("Summarize what is known about '{}'", topic)),
                    messages: vec![PromptMessage::new_text(
                        PromptMessageRole::User,
                        format!(
                            "Search the knowledge graph for everything related to '{}' using search_memory and expand_search, \
                             then provide a comprehensive summary of all entities, relationships, and memories found. \
                             Include temporal information about when things were recorded.",
                            topic
                        ),
                    )],
                })
            }
            "what-changed" => {
                let since = get_str("since");
                Ok(GetPromptResult {
                    description: Some(format!("Changes since {}", since)),
                    messages: vec![PromptMessage::new_text(
                        PromptMessageRole::User,
                        format!(
                            "Use list_recent to get recent memories, then use snapshot with timestamp '{}' \
                             to see the graph state at that point. Compare with the current state and describe \
                             what entities, relationships, or memories have been added or changed since then.",
                            since
                        ),
                    )],
                })
            }
            "add-context" => {
                let context = get_str("context");
                Ok(GetPromptResult {
                    description: Some("Add context to the knowledge graph".into()),
                    messages: vec![PromptMessage::new_text(
                        PromptMessageRole::User,
                        format!(
                            "Use add_memory to ingest the following context into the knowledge graph. \
                             After ingestion, briefly confirm what entities and relations were extracted:\n\n{}",
                            context
                        ),
                    )],
                })
            }
            other => Err(McpError::invalid_params(
                format!("Unknown prompt: '{}'", other),
                None,
            )),
        }
    }
}
