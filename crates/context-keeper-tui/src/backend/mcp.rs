//! Remote backend via MCP streamable HTTP (`rmcp` client).

use std::borrow::Cow;
use std::sync::Arc;

use async_trait::async_trait;
use rmcp::model::{CallToolRequestParam, JsonObject, ReadResourceRequestParam, ResourceContents};
use rmcp::service::ServiceError;
use rmcp::service::{Peer, RoleClient, RunningService, ServiceExt};
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::StreamableHttpClientTransport;
use serde_json::json;

use super::TuiBackend;
use crate::error::TuiError;
use crate::types::{
    AddMemoryResult, AgentInfoRow, AgentRunRow, EntityDetail, EntitySummary, EpisodeRow,
    GraphStats, MemoryItemJson, MemoryRow, NamespaceInfo, NoteRow, RelationDirection, RelationRow,
    SearchHit, SearchHitJson, SnapshotResult,
};

fn map_service_err(e: ServiceError) -> TuiError {
    TuiError::Mcp(e.to_string())
}

fn tool_text(res: rmcp::model::CallToolResult) -> Result<String, TuiError> {
    if res.is_error == Some(true) {
        let mut msg = String::new();
        for c in &res.content {
            if let Some(t) = c.as_text() {
                msg.push_str(&t.text);
                msg.push('\n');
            }
        }
        return Err(TuiError::Mcp(msg.trim().to_string()));
    }

    let mut out = String::new();
    for c in &res.content {
        if let Some(t) = c.as_text() {
            out.push_str(&t.text);
            out.push('\n');
        }
    }
    Ok(out.trim().to_string())
}

async fn call_tool_json_args(
    peer: &Peer<RoleClient>,
    name: &'static str,
    args: serde_json::Value,
) -> Result<String, TuiError> {
    let obj: JsonObject = args
        .as_object()
        .cloned()
        .ok_or_else(|| TuiError::Mcp("internal: expected JSON object".into()))?;

    let res = peer
        .call_tool(CallToolRequestParam {
            name: Cow::Borrowed(name),
            arguments: Some(obj),
        })
        .await
        .map_err(map_service_err)?;

    tool_text(res)
}

async fn call_tool_no_args(
    peer: &Peer<RoleClient>,
    name: &'static str,
) -> Result<String, TuiError> {
    let res = peer
        .call_tool(CallToolRequestParam {
            name: Cow::Borrowed(name),
            arguments: None,
        })
        .await
        .map_err(map_service_err)?;

    tool_text(res)
}

async fn read_resource_text(peer: &Peer<RoleClient>, uri: &str) -> Result<String, TuiError> {
    let result = peer
        .read_resource(ReadResourceRequestParam {
            uri: uri.to_string(),
        })
        .await
        .map_err(map_service_err)?;

    for content in &result.contents {
        if let ResourceContents::TextResourceContents { text, .. } = content {
            return Ok(text.clone());
        }
    }
    Err(TuiError::Mcp("No text content in resource response".into()))
}

struct Inner {
    #[allow(dead_code)]
    running: RunningService<RoleClient, ()>,
    peer: Peer<RoleClient>,
    namespace: Option<String>,
    agent_id: Option<String>,
}

/// Connects to `http://host:port/mcp` and keeps the MCP session alive.
pub struct McpHttpBackend {
    inner: Arc<Inner>,
}

impl McpHttpBackend {
    pub async fn connect(
        uri: impl AsRef<str>,
        bearer_token: Option<&str>,
        namespace: Option<String>,
        agent_id: Option<String>,
    ) -> Result<Self, TuiError> {
        let uri: std::sync::Arc<str> = std::sync::Arc::from(uri.as_ref());
        let mut config = StreamableHttpClientTransportConfig::with_uri(uri);
        if let Some(t) = bearer_token {
            config = config.auth_header(t);
        }

        let transport = StreamableHttpClientTransport::from_config(config);
        let running = ().serve(transport).await.map_err(|e| TuiError::Mcp(e.to_string()))?;
        let peer = running.peer().clone();

        Ok(Self {
            inner: Arc::new(Inner {
                running,
                peer,
                namespace,
                agent_id,
            }),
        })
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(default)]
struct AddMemoryMcp {
    memories_created: usize,
    relations_created: usize,
    entity_names: Vec<String>,
}

impl Default for AddMemoryMcp {
    fn default() -> Self {
        Self {
            memories_created: 0,
            relations_created: 0,
            entity_names: Vec::new(),
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct StatsResource {
    #[serde(default)]
    entity_count: usize,
    #[serde(default)]
    memory_count: usize,
}

#[derive(Debug, serde::Deserialize)]
struct EntitySummaryJson {
    name: String,
    #[serde(alias = "type", default)]
    entity_type: String,
}

#[async_trait]
impl TuiBackend for McpHttpBackend {
    async fn add_memory(&self, text: &str, source: &str) -> Result<AddMemoryResult, TuiError> {
        let mut args = json!({
            "text": text,
            "source": source,
        });
        if let Some(ns) = &self.inner.namespace {
            args["namespace"] = json!(ns);
        }
        if let Some(id) = &self.inner.agent_id {
            args["agent_id"] = json!(id);
        }

        let raw = call_tool_json_args(&self.inner.peer, "add_memory", args).await?;
        let parsed: AddMemoryMcp = serde_json::from_str(&raw).unwrap_or_default();
        Ok(AddMemoryResult {
            entity_count: parsed.entity_names.len(),
            relation_count: parsed.relations_created,
            memory_count: parsed.memories_created,
            entity_names: parsed.entity_names,
        })
    }

    async fn search_memory(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, TuiError> {
        let mut args = json!({ "query": query, "limit": limit });
        if let Some(ns) = &self.inner.namespace {
            args["namespace"] = json!(ns);
        }
        let raw = call_tool_json_args(&self.inner.peer, "search_memory", args).await?;
        let items: Vec<SearchHitJson> = serde_json::from_str(&raw).map_err(TuiError::from)?;
        Ok(items.into_iter().map(SearchHit::from).collect())
    }

    async fn expand_search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, TuiError> {
        let mut args = json!({ "query": query, "limit": limit });
        if let Some(ns) = &self.inner.namespace {
            args["namespace"] = json!(ns);
        }
        let raw = call_tool_json_args(&self.inner.peer, "expand_search", args).await?;
        let items: Vec<SearchHitJson> = serde_json::from_str(&raw).map_err(TuiError::from)?;
        Ok(items.into_iter().map(SearchHit::from).collect())
    }

    async fn list_recent(&self, limit: usize) -> Result<Vec<MemoryRow>, TuiError> {
        let args = json!({ "limit": limit });
        let raw = call_tool_json_args(&self.inner.peer, "list_recent", args).await?;
        let items: Vec<MemoryItemJson> = serde_json::from_str(&raw).map_err(TuiError::from)?;
        Ok(items.into_iter().map(MemoryRow::from).collect())
    }

    async fn get_entity(&self, name: &str) -> Result<Option<EntityDetail>, TuiError> {
        let mut args = json!({ "name": name });
        if let Some(ns) = &self.inner.namespace {
            args["namespace"] = json!(ns);
        }
        let raw = call_tool_json_args(&self.inner.peer, "get_entity", args).await?;

        let details: Vec<crate::types::EntityDetailJson> = match serde_json::from_str(&raw) {
            Ok(d) => d,
            Err(_) => return Ok(None),
        };
        let detail = match details.into_iter().next() {
            Some(d) => d,
            None => return Ok(None),
        };

        let queried = name.to_lowercase();
        Ok(Some(EntityDetail {
            name: detail.name.clone(),
            entity_type: detail.entity_type,
            summary: detail.summary,
            valid_from: detail.valid_from,
            valid_until: detail.valid_until,
            relations: detail
                .relations
                .into_iter()
                .map(|r| {
                    let from_lower = r.from_entity_name.to_lowercase();
                    let (direction, target_name) = if from_lower == queried {
                        (RelationDirection::Outgoing, r.to_entity_name)
                    } else if !r.to_entity_name.is_empty() {
                        (RelationDirection::Incoming, r.from_entity_name)
                    } else {
                        (RelationDirection::Outgoing, r.to_entity_id)
                    };
                    RelationRow {
                        relation_type: r.relation_type,
                        target_name,
                        direction,
                        confidence: r.confidence as u8,
                    }
                })
                .collect(),
        }))
    }

    async fn list_entities(&self, limit: usize) -> Result<Vec<EntitySummary>, TuiError> {
        if let Ok(raw) = read_resource_text(&self.inner.peer, "memory://entities/summary").await {
            if let Ok(items) = serde_json::from_str::<Vec<EntitySummaryJson>>(&raw) {
                return Ok(items
                    .into_iter()
                    .take(limit)
                    .map(|j| EntitySummary {
                        name: j.name,
                        entity_type: j.entity_type,
                        summary: String::new(),
                    })
                    .collect());
            }
        }

        let mut args = json!({ "query": "*", "limit": limit });
        if let Some(ns) = &self.inner.namespace {
            args["namespace"] = json!(ns);
        }
        let raw = call_tool_json_args(&self.inner.peer, "search_memory", args).await?;
        let items: Vec<SearchHitJson> = serde_json::from_str(&raw).unwrap_or_default();
        Ok(items
            .into_iter()
            .map(|j| EntitySummary {
                name: j.name,
                entity_type: j.entity_type,
                summary: j.summary,
            })
            .collect())
    }

    async fn get_stats(&self) -> Result<GraphStats, TuiError> {
        if let Ok(raw) = read_resource_text(&self.inner.peer, "memory://stats").await {
            if let Ok(stats) = serde_json::from_str::<StatsResource>(&raw) {
                let ns = self.list_namespaces().await.unwrap_or_default();
                let ag = self.list_agents().await.unwrap_or_default();
                return Ok(GraphStats {
                    entities: stats.entity_count,
                    memories: stats.memory_count,
                    namespaces: ns.len(),
                    agents: ag.len(),
                });
            }
        }

        let ns = self.list_namespaces().await.unwrap_or_default();
        let ag = self.list_agents().await.unwrap_or_default();
        Ok(GraphStats {
            entities: 0,
            memories: 0,
            namespaces: ns.len(),
            agents: ag.len(),
        })
    }

    async fn list_namespaces(&self) -> Result<Vec<NamespaceInfo>, TuiError> {
        let raw = call_tool_no_args(&self.inner.peer, "list_namespaces").await?;
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    async fn delete_namespace(&self, namespace: &str) -> Result<String, TuiError> {
        let args = json!({ "namespace": namespace });
        let raw = call_tool_json_args(&self.inner.peer, "delete_namespace", args).await?;
        Ok(raw)
    }

    async fn list_agents(&self) -> Result<Vec<AgentInfoRow>, TuiError> {
        let raw = call_tool_no_args(&self.inner.peer, "list_agents").await?;
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    async fn agent_activity(
        &self,
        agent_id: &str,
        limit: usize,
    ) -> Result<Vec<EpisodeRow>, TuiError> {
        let args = json!({ "agent_id": agent_id, "limit": limit });
        let raw = call_tool_json_args(&self.inner.peer, "agent_activity", args).await?;
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    async fn cross_namespace_search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchHit>, TuiError> {
        let args = json!({ "query": query, "limit": limit });
        let raw = call_tool_json_args(&self.inner.peer, "cross_namespace_search", args).await?;
        let items: Vec<SearchHitJson> = serde_json::from_str(&raw).map_err(TuiError::from)?;
        Ok(items.into_iter().map(SearchHit::from).collect())
    }

    async fn snapshot(&self, iso_timestamp: &str) -> Result<SnapshotResult, TuiError> {
        let args = json!({ "timestamp": iso_timestamp });
        let raw = call_tool_json_args(&self.inner.peer, "snapshot", args).await?;
        serde_json::from_str(&raw)
            .map_err(|e| TuiError::Mcp(format!("Failed to parse snapshot response: {e}")))
    }

    async fn list_notes(&self, tag: Option<&str>, limit: usize) -> Result<Vec<NoteRow>, TuiError> {
        let mut args = json!({ "limit": limit });
        if let Some(t) = tag {
            args["tags"] = json!([t]);
        }
        if let Some(ns) = &self.inner.namespace {
            args["namespace"] = json!(ns);
        }
        let raw = call_tool_json_args(&self.inner.peer, "list_notes", args).await?;
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    async fn query_agent_runs(
        &self,
        status: Option<&str>,
        agent_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<AgentRunRow>, TuiError> {
        let mut args = json!({ "limit": limit });
        if let Some(s) = status {
            args["status"] = json!(s);
        }
        if let Some(id) = agent_id {
            args["agent_id"] = json!(id);
        }
        if let Some(ns) = &self.inner.namespace {
            args["namespace"] = json!(ns);
        }
        let raw = call_tool_json_args(&self.inner.peer, "query_agent_runs", args).await?;
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }
}
