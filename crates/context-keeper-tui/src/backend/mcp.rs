//! Remote backend via MCP streamable HTTP (`rmcp` client).

use std::borrow::Cow;
use std::sync::Arc;

use async_trait::async_trait;
use rmcp::model::{CallToolRequestParam, JsonObject};
use rmcp::service::ServiceError;
use rmcp::service::{Peer, RoleClient, RunningService, ServiceExt};
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::StreamableHttpClientTransport;
use serde_json::json;

use super::TuiBackend;
use crate::error::TuiError;
use crate::types::{
    AddMemoryResult, AgentInfoRow, EntityDetail, EntitySummary, EpisodeRow, GraphStats, MemoryRow,
    MemoryItemJson, NamespaceInfo, RelationDirection, RelationRow, SearchHit, SearchHitJson,
    SnapshotResult,
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
    entity_names: Vec<String>,
}

impl Default for AddMemoryMcp {
    fn default() -> Self {
        Self {
            memories_created: 0,
            entity_names: Vec::new(),
        }
    }
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
            relation_count: 0,
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

        if raw.contains("not found") || raw.contains("No entity") {
            return Ok(None);
        }

        let details: Vec<crate::types::EntityDetailJson> =
            serde_json::from_str(&raw).map_err(TuiError::from)?;
        let detail = match details.into_iter().next() {
            Some(d) => d,
            None => return Ok(None),
        };

        Ok(Some(EntityDetail {
            name: detail.name,
            entity_type: detail.entity_type,
            summary: detail.summary,
            valid_from: detail.valid_from,
            valid_until: detail.valid_until,
            relations: detail
                .relations
                .into_iter()
                .map(|r| RelationRow {
                    relation_type: r.relation_type,
                    target_name: r.to_entity_id.clone(),
                    direction: RelationDirection::Outgoing,
                    confidence: r.confidence as u8,
                })
                .collect(),
        }))
    }

    async fn list_entities(&self, limit: usize) -> Result<Vec<EntitySummary>, TuiError> {
        // MCP doesn't have a dedicated list_entities tool; use search with wildcard
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
        // Approximate from available tools
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
        Ok(serde_json::from_str(&raw).unwrap_or(SnapshotResult {
            timestamp: iso_timestamp.to_string(),
            entity_count: 0,
            relation_count: 0,
            entities: vec![],
        }))
    }
}
