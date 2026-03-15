//! MCP tool handler definitions for the Context Keeper server.
//!
//! Each tool corresponds to a capability exposed to MCP clients
//! (Claude Desktop, Cursor, etc.).

use std::sync::Arc;

use chrono::{DateTime, Utc};
use context_keeper_core::{
    ingestion,
    models::Episode,
    search::{fuse_rrf, QueryExpander},
    traits::{Embedder, EntityExtractor, QueryRewriter, RelationExtractor},
};
use context_keeper_surreal::Repository;
use rmcp::{
    handler::server::{
        router::tool::ToolRouter,
        wrapper::Parameters,
    },
    model::*, schemars, tool, tool_handler, tool_router,
    ErrorData as McpError, ServerHandler,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Input schemas ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddMemoryInput {
    #[schemars(description = "The text content to ingest as a memory")]
    pub text: String,
    #[schemars(description = "Source label for the episode (e.g. 'chat', 'document')")]
    pub source: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchMemoryInput {
    #[schemars(description = "The search query string")]
    pub query: String,
    #[schemars(description = "Maximum number of results to return (default: 5)")]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ExpandSearchInput {
    #[schemars(description = "The search query to expand with semantic variants")]
    pub query: String,
    #[schemars(description = "Maximum number of results to return (default: 10)")]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetEntityInput {
    #[schemars(description = "The exact name of the entity to look up")]
    pub name: String,
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

// ── Serializable response types ──────────────────────────────────────────

#[derive(Debug, Serialize)]
struct AddMemoryResponse {
    entities_created: usize,
    relations_created: usize,
    memories_created: usize,
    entity_names: Vec<String>,
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
    #[tool(description = "Add a memory to the knowledge graph. Ingests text, extracts entities and relations, and stores everything with embeddings for later retrieval.")]
    async fn add_memory(
        &self,
        Parameters(input): Parameters<AddMemoryInput>,
    ) -> Result<String, McpError> {
        let source = input.source.unwrap_or_else(|| "mcp".to_string());
        let episode = Episode {
            id: Uuid::new_v4(),
            content: input.text,
            source,
            session_id: None,
            created_at: Utc::now(),
        };

        let result = ingestion::ingest(
            &episode,
            self.embedder.as_ref(),
            self.entity_extractor.as_ref(),
            self.relation_extractor.as_ref(),
        )
        .await
        .map_err(|e| McpError::internal_error(format!("Ingestion failed: {e}"), None))?;

        // Persist everything
        self.repo
            .create_episode(&episode)
            .await
            .map_err(|e| McpError::internal_error(format!("Failed to create episode: {e}"), None))?;

        for entity in &result.entities {
            self.repo
                .upsert_entity(entity)
                .await
                .map_err(|e| McpError::internal_error(format!("Failed to upsert entity: {e}"), None))?;
        }
        for relation in &result.relations {
            self.repo
                .create_relation(relation)
                .await
                .map_err(|e| McpError::internal_error(format!("Failed to create relation: {e}"), None))?;
        }
        for memory in &result.memories {
            self.repo
                .create_memory(memory)
                .await
                .map_err(|e| McpError::internal_error(format!("Failed to create memory: {e}"), None))?;
        }

        let response = AddMemoryResponse {
            entities_created: result.entities.len(),
            relations_created: result.relations.len(),
            memories_created: result.memories.len(),
            entity_names: result.entities.iter().map(|e| e.name.clone()).collect(),
        };

        serde_json::to_string_pretty(&response)
            .map_err(|e| McpError::internal_error(format!("Serialization failed: {e}"), None))
    }

    /// Search the knowledge graph using hybrid vector + keyword search with
    /// Reciprocal Rank Fusion.
    #[tool(description = "Search memories and entities in the knowledge graph using hybrid vector similarity and BM25 keyword search, fused with Reciprocal Rank Fusion.")]
    async fn search_memory(
        &self,
        Parameters(input): Parameters<SearchMemoryInput>,
    ) -> Result<String, McpError> {
        let limit = input.limit.unwrap_or(5);

        let query_embedding = self
            .embedder
            .embed(&input.query)
            .await
            .map_err(|e| McpError::internal_error(format!("Embedding failed: {e}"), None))?;

        let vector_results = self
            .repo
            .search_entities_by_vector(&query_embedding, limit)
            .await
            .map_err(|e| McpError::internal_error(format!("Vector search failed: {e}"), None))?;

        let keyword_results = self
            .repo
            .search_entities_by_keyword(&input.query)
            .await
            .map_err(|e| McpError::internal_error(format!("Keyword search failed: {e}"), None))?;

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
                    entity_type: e.entity_type.clone(),
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
    #[tool(description = "Expand a search query into semantic variants using LLM rewriting, then search each variant and merge results with RRF for improved recall.")]
    async fn expand_search(
        &self,
        Parameters(input): Parameters<ExpandSearchInput>,
    ) -> Result<String, McpError> {
        let limit = input.limit.unwrap_or(10);
        let expander = QueryExpander::new(3);

        let variants = expander
            .expand(&input.query, self.query_rewriter.as_ref())
            .await
            .map_err(|e| McpError::internal_error(format!("Query expansion failed: {e}"), None))?;

        let mut ranked_lists = Vec::new();
        for variant in &variants {
            let query_embedding = self
                .embedder
                .embed(variant)
                .await
                .map_err(|e| McpError::internal_error(format!("Embedding failed: {e}"), None))?;

            let vector_results = self
                .repo
                .search_entities_by_vector(&query_embedding, limit)
                .await
                .map_err(|e| McpError::internal_error(format!("Vector search failed: {e}"), None))?;

            let keyword_results = self
                .repo
                .search_entities_by_keyword(variant)
                .await
                .map_err(|e| McpError::internal_error(format!("Keyword search failed: {e}"), None))?;

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
                    entity_type: e.entity_type.clone(),
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
            .find_entities_by_name(&input.name)
            .await
            .map_err(|e| McpError::internal_error(format!("Entity lookup failed: {e}"), None))?;

        if entities.is_empty() {
            return Ok(format!("No entity found with name '{}'", input.name));
        }

        let mut details = Vec::new();
        for entity in &entities {
            let relations = self
                .repo
                .get_relations_for_entity(entity.id)
                .await
                .map_err(|e| McpError::internal_error(format!("Relation lookup failed: {e}"), None))?;

            details.push(EntityDetail {
                name: entity.name.clone(),
                entity_type: entity.entity_type.clone(),
                summary: entity.summary.clone(),
                valid_from: entity.valid_from.to_rfc3339(),
                valid_until: entity.valid_until.map(|d| d.to_rfc3339()),
                relations: relations
                    .iter()
                    .map(|r| RelationDetail {
                        relation_type: r.relation_type.clone(),
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
            .map_err(|e| McpError::internal_error(format!("Snapshot query failed: {e}"), None))?;

        let relations = self
            .repo
            .relations_at(at)
            .await
            .map_err(|e| McpError::internal_error(format!("Snapshot query failed: {e}"), None))?;

        let response = SnapshotResponse {
            timestamp: at.to_rfc3339(),
            entity_count: entities.len(),
            relation_count: relations.len(),
            entities: entities
                .iter()
                .map(|e| SnapshotEntity {
                    name: e.name.clone(),
                    entity_type: e.entity_type.clone(),
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
            .map_err(|e| McpError::internal_error(format!("Query failed: {e}"), None))?;

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
}

#[tool_handler]
impl ServerHandler for ContextKeeperServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "context-keeper".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                title: None,
                icons: None,
                website_url: None,
            },
            instructions: Some(
                "Context Keeper is a temporal knowledge graph memory system. \
                 Use add_memory to store information, search_memory or expand_search \
                 to retrieve it, get_entity for detailed entity lookups, snapshot for \
                 point-in-time queries, and list_recent for recent memories."
                    .into(),
            ),
        }
    }
}
