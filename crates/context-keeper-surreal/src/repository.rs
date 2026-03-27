use async_trait::async_trait;
use chrono::{DateTime, Utc};
use context_keeper_core::error::Result;
use context_keeper_core::models::*;
use context_keeper_core::traits::EntityResolver;
use context_keeper_core::ContextKeeperError;
use serde::{Deserialize, Serialize};
use surrealdb::engine::any::Any;
use surrealdb::types::SurrealValue;
use surrealdb::Surreal;
use tracing::debug;
use uuid::Uuid;

fn storage_err(e: impl std::fmt::Display) -> ContextKeeperError {
    ContextKeeperError::StorageError(e.to_string())
}

/// Typed repository for all Context Keeper data operations against SurrealDB.
///
/// Uses SurrealDB's native graph engine: RELATE for edges, HNSW for vector
/// search, BM25 for full-text, and recursive graph queries for traversal.
/// Accepts `Surreal<Any>` to support in-memory, RocksDB, and remote backends.
#[derive(Clone)]
pub struct Repository {
    db: Surreal<Any>,
}

// ── Deserialization helpers for SurrealDB query results ──────────────────

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
struct EntityRow {
    id: surrealdb::types::RecordId,
    name: String,
    entity_type: String,
    summary: String,
    #[serde(default)]
    embedding: Vec<f64>,
    valid_from: DateTime<Utc>,
    valid_until: Option<DateTime<Utc>>,
    #[serde(default)]
    namespace: Option<String>,
    #[serde(default)]
    created_by_agent: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
struct EpisodeRow {
    id: surrealdb::types::RecordId,
    content: String,
    source: String,
    session_id: Option<String>,
    #[serde(default)]
    agent_id: Option<String>,
    #[serde(default)]
    agent_name: Option<String>,
    #[serde(default)]
    machine_id: Option<String>,
    #[serde(default)]
    namespace: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
struct MemoryRow {
    id: surrealdb::types::RecordId,
    content: String,
    #[serde(default)]
    embedding: Vec<f64>,
    created_at: DateTime<Utc>,
    #[serde(default)]
    namespace: Option<String>,
    #[serde(default)]
    created_by_agent: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
struct RelationEdgeRow {
    id: surrealdb::types::RecordId,
    #[serde(rename = "in")]
    in_id: surrealdb::types::RecordId,
    #[serde(rename = "out")]
    out_id: surrealdb::types::RecordId,
    relation_type: String,
    confidence: u8,
    valid_from: DateTime<Utc>,
    valid_until: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
struct ScoredEntityRow {
    id: surrealdb::types::RecordId,
    name: String,
    entity_type: String,
    summary: String,
    #[serde(default)]
    embedding: Vec<f64>,
    valid_from: DateTime<Utc>,
    valid_until: Option<DateTime<Utc>>,
    #[serde(default)]
    namespace: Option<String>,
    #[serde(default)]
    created_by_agent: Option<String>,
    score: f64,
}

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
struct ScoredMemoryRow {
    id: surrealdb::types::RecordId,
    content: String,
    #[serde(default)]
    embedding: Vec<f64>,
    created_at: DateTime<Utc>,
    #[serde(default)]
    namespace: Option<String>,
    #[serde(default)]
    created_by_agent: Option<String>,
    score: f64,
}

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
struct FtsEntityRow {
    id: surrealdb::types::RecordId,
    name: String,
    entity_type: String,
    summary: String,
    #[serde(default)]
    embedding: Vec<f64>,
    valid_from: DateTime<Utc>,
    valid_until: Option<DateTime<Utc>>,
    #[serde(default)]
    namespace: Option<String>,
    #[serde(default)]
    created_by_agent: Option<String>,
    relevance: f64,
}

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
struct MemoryWithEdgesRow {
    id: surrealdb::types::RecordId,
    content: String,
    #[serde(default)]
    embedding: Vec<f64>,
    created_at: DateTime<Utc>,
    #[serde(default)]
    namespace: Option<String>,
    #[serde(default)]
    created_by_agent: Option<String>,
    #[serde(default)]
    episode_ids: Vec<surrealdb::types::RecordId>,
    #[serde(default)]
    entity_ids: Vec<surrealdb::types::RecordId>,
}

// ── RecordId <-> Uuid helpers ────────────────────────────────────────────

fn record_id_to_uuid(rid: &surrealdb::types::RecordId) -> Option<Uuid> {
    use surrealdb::types::RecordIdKey;
    match &rid.key {
        RecordIdKey::String(s) => Uuid::parse_str(s).ok(),
        RecordIdKey::Uuid(u) => Some(**u),
        _ => None,
    }
}

fn entity_from_row(r: EntityRow) -> Option<Entity> {
    Some(Entity {
        id: record_id_to_uuid(&r.id)?,
        name: r.name,
        entity_type: EntityType::from(r.entity_type.as_str()),
        summary: r.summary,
        embedding: r.embedding,
        valid_from: r.valid_from,
        valid_until: r.valid_until,
        namespace: r.namespace,
        created_by_agent: r.created_by_agent,
    })
}

fn episode_from_row(r: EpisodeRow) -> Option<Episode> {
    let agent = r.agent_id.map(|id| context_keeper_core::AgentInfo {
        agent_id: id,
        agent_name: r.agent_name,
        machine_id: r.machine_id,
    });
    Some(Episode {
        id: record_id_to_uuid(&r.id)?,
        content: r.content,
        source: r.source,
        session_id: r.session_id,
        agent,
        namespace: r.namespace,
        created_at: r.created_at,
    })
}

fn memory_from_row(r: MemoryRow) -> Option<Memory> {
    Some(Memory {
        id: record_id_to_uuid(&r.id)?,
        content: r.content,
        embedding: r.embedding,
        source_episode_id: Uuid::nil(),
        entity_ids: vec![],
        created_at: r.created_at,
        namespace: r.namespace,
        created_by_agent: r.created_by_agent,
    })
}

fn memory_with_edges_from_row(r: MemoryWithEdgesRow) -> Option<Memory> {
    let episode_id = r.episode_ids.first().and_then(record_id_to_uuid).unwrap_or(Uuid::nil());
    let entity_ids: Vec<Uuid> = r.entity_ids.iter().filter_map(record_id_to_uuid).collect();
    Some(Memory {
        id: record_id_to_uuid(&r.id)?,
        content: r.content,
        embedding: r.embedding,
        source_episode_id: episode_id,
        entity_ids,
        created_at: r.created_at,
        namespace: r.namespace,
        created_by_agent: r.created_by_agent,
    })
}

fn relation_from_edge_row(r: RelationEdgeRow) -> Option<Relation> {
    Some(Relation {
        id: record_id_to_uuid(&r.id)?,
        from_entity_id: record_id_to_uuid(&r.in_id)?,
        to_entity_id: record_id_to_uuid(&r.out_id)?,
        relation_type: RelationType::from(r.relation_type.as_str()),
        confidence: r.confidence,
        valid_from: r.valid_from,
        valid_until: r.valid_until,
    })
}

// ── EntityResolver implementation ────────────────────────────────────────

#[async_trait]
impl EntityResolver for Repository {
    async fn find_existing(&self, name: &str, entity_type: &EntityType, _namespace: Option<&str>) -> Result<Option<Entity>> {
        let etype_str = entity_type.to_string();
        let q = "SELECT * FROM entity WHERE name = $name AND entity_type = $etype AND valid_until IS NONE LIMIT 1";
        let mut response = self.db.query(q)
            .bind(("name", name.to_string()))
            .bind(("etype", etype_str))
            .await
            .map_err(storage_err)?;
        let rows: Vec<EntityRow> = response.take(0).map_err(storage_err)?;
        Ok(rows.into_iter().next().and_then(entity_from_row))
    }

    async fn find_similar(
        &self,
        _name: &str,
        embedding: &[f64],
        threshold: f64,
        namespace: Option<&str>,
    ) -> Result<Vec<Entity>> {
        let candidates = self
            .search_entities_by_vector(embedding, 3, None, namespace)
            .await?;

        Ok(candidates
            .into_iter()
            .filter(|(_, score)| *score >= threshold)
            .map(|(e, _)| e)
            .collect())
    }
}

impl Repository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    // ── File export/import ───────────────────────────────────────────

    pub async fn export(&self, path: &str) -> Result<()> {
        self.db.export(path).await.map_err(storage_err)?;
        Ok(())
    }

    pub async fn import_from_file(&self, path: &str) -> Result<()> {
        self.db.import(path).await.map_err(storage_err)?;
        Ok(())
    }

    // ── Episodes ─────────────────────────────────────────────────────

    pub async fn create_episode(&self, episode: &Episode) -> Result<()> {
        let q = format!(
            "CREATE episode:`{}` SET content = $content, source = $source, session_id = $session_id, agent_id = $agent_id, agent_name = $agent_name, machine_id = $machine_id, namespace = $namespace, created_at = <datetime>$created_at",
            episode.id
        );
        self.db
            .query(&q)
            .bind(("content", episode.content.clone()))
            .bind(("source", episode.source.clone()))
            .bind(("session_id", episode.session_id.clone()))
            .bind(("agent_id", episode.agent.as_ref().map(|a| a.agent_id.clone())))
            .bind(("agent_name", episode.agent.as_ref().and_then(|a| a.agent_name.clone())))
            .bind(("machine_id", episode.agent.as_ref().and_then(|a| a.machine_id.clone())))
            .bind(("namespace", episode.namespace.clone()))
            .bind(("created_at", episode.created_at.to_rfc3339()))
            .await
            .map_err(storage_err)?;
        Ok(())
    }

    pub async fn get_episode(&self, id: Uuid) -> Result<Option<Episode>> {
        let q = format!("SELECT * FROM episode:`{}`", id);
        let mut response = self.db.query(&q).await.map_err(storage_err)?;
        let rows: Vec<EpisodeRow> = response.take(0).map_err(storage_err)?;
        Ok(rows.into_iter().next().and_then(episode_from_row))
    }

    pub async fn list_recent_episodes(&self, limit: usize) -> Result<Vec<Episode>> {
        let mut response = self
            .db
            .query("SELECT * FROM episode ORDER BY created_at DESC LIMIT $limit")
            .bind(("limit", limit))
            .await
            .map_err(storage_err)?;
        let rows: Vec<EpisodeRow> = response.take(0).map_err(storage_err)?;
        Ok(rows.into_iter().filter_map(episode_from_row).collect())
    }

    // ── Entities ─────────────────────────────────────────────────────

    /// True UPSERT: if an entity with the same ID exists, update it.
    pub async fn upsert_entity(&self, entity: &Entity) -> Result<()> {
        let q = format!(
            "UPSERT entity:`{}` SET name = $name, entity_type = $entity_type, summary = $summary, embedding = $embedding, valid_from = <datetime>$valid_from, valid_until = IF $valid_until THEN <datetime>$valid_until ELSE NONE END, namespace = $namespace, created_by_agent = $created_by_agent",
            entity.id
        );
        self.db
            .query(&q)
            .bind(("name", entity.name.clone()))
            .bind(("entity_type", entity.entity_type.to_string()))
            .bind(("summary", entity.summary.clone()))
            .bind(("embedding", entity.embedding.clone()))
            .bind(("valid_from", entity.valid_from.to_rfc3339()))
            .bind(("valid_until", entity.valid_until.map(|d| d.to_rfc3339())))
            .bind(("namespace", entity.namespace.clone()))
            .bind(("created_by_agent", entity.created_by_agent.clone()))
            .await
            .map_err(storage_err)?;
        Ok(())
    }

    pub async fn get_entity(&self, id: Uuid) -> Result<Option<Entity>> {
        let q = format!("SELECT * FROM entity:`{}`", id);
        let mut response = self.db.query(&q).await.map_err(storage_err)?;
        let rows: Vec<EntityRow> = response.take(0).map_err(storage_err)?;
        Ok(rows.into_iter().next().and_then(entity_from_row))
    }

    pub async fn find_entities_by_name(
        &self,
        name: &str,
        namespace: Option<&str>,
    ) -> Result<Vec<Entity>> {
        self.find_entities_by_name_and_type(name, None, namespace).await
    }

    pub async fn find_entities_by_name_and_type(
        &self,
        name: &str,
        entity_type: Option<&EntityType>,
        namespace: Option<&str>,
    ) -> Result<Vec<Entity>> {
        let mut conditions = vec!["name = $name", "valid_until IS NONE"];
        if entity_type.is_some() {
            conditions.push("entity_type = $etype");
        }
        if namespace.is_some() {
            conditions.push("namespace = $ns");
        }
        let q = format!("SELECT * FROM entity WHERE {}", conditions.join(" AND "));

        let mut query = self.db.query(&q).bind(("name", name.to_string()));
        if let Some(etype) = entity_type {
            query = query.bind(("etype", etype.to_string()));
        }
        if let Some(ns) = namespace {
            query = query.bind(("ns", ns.to_string()));
        }
        let mut response = query.await.map_err(storage_err)?;
        debug!("find_entities_by_name_and_type");
        let rows: Vec<EntityRow> = response.take(0).map_err(storage_err)?;
        Ok(rows.into_iter().filter_map(entity_from_row).collect())
    }

    pub async fn get_all_active_entities(&self) -> Result<Vec<Entity>> {
        self.get_all_active_entities_in_namespace(None).await
    }

    pub async fn get_all_active_entities_in_namespace(&self, namespace: Option<&str>) -> Result<Vec<Entity>> {
        let q = match namespace {
            Some(_) => "SELECT * FROM entity WHERE valid_until IS NONE AND namespace = $ns",
            None => "SELECT * FROM entity WHERE valid_until IS NONE",
        };
        let mut query = self.db.query(q);
        if let Some(ns) = namespace {
            query = query.bind(("ns", ns.to_string()));
        }
        let mut response = query.await.map_err(storage_err)?;
        let rows: Vec<EntityRow> = response.take(0).map_err(storage_err)?;
        Ok(rows.into_iter().filter_map(entity_from_row).collect())
    }

    /// Invalidate an entity by setting its `valid_until` to now.
    pub async fn invalidate_entity(&self, id: Uuid) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let q = format!(
            "UPDATE entity:`{}` SET valid_until = <datetime>$now",
            id
        );
        self.db.query(&q).bind(("now", now)).await.map_err(storage_err)?;
        Ok(())
    }

    /// Get active entities filtered by type.
    pub async fn get_entities_by_type(&self, entity_type: &str) -> Result<Vec<Entity>> {
        let mut response = self
            .db
            .query("SELECT * FROM entity WHERE entity_type = $etype AND valid_until IS NONE")
            .bind(("etype", entity_type.to_string()))
            .await
            .map_err(storage_err)?;
        let rows: Vec<EntityRow> = response.take(0).map_err(storage_err)?;
        Ok(rows.into_iter().filter_map(entity_from_row).collect())
    }

    // ── Relations (Graph Edges) ──────────────────────────────────────

    /// Create or deduplicate a graph edge between two entities.
    ///
    /// Checks for existing active relations with the same (from, to, type).
    /// For symmetric relation types (Knows, RelatedTo), also checks the
    /// reverse direction. Averages confidence on merge.
    pub async fn create_relation(&self, relation: &Relation) -> Result<bool> {
        let rel_type_str = relation.relation_type.to_string();

        let check_q = format!(
            "SELECT id, in AS in_id, out AS out_id, relation_type, confidence, valid_from, valid_until FROM relates_to WHERE in = entity:`{}` AND out = entity:`{}` AND relation_type = $rel_type AND valid_until IS NONE LIMIT 1",
            relation.from_entity_id, relation.to_entity_id
        );
        let mut check_resp = self
            .db
            .query(&check_q)
            .bind(("rel_type", rel_type_str.clone()))
            .await
            .map_err(storage_err)?;
        let existing: Vec<RelationEdgeRow> = check_resp.take(0).map_err(storage_err)?;

        if let Some(existing_row) = existing.into_iter().next() {
            let avg_conf = ((existing_row.confidence as u16 + relation.confidence as u16) / 2) as u8;
            let update_q = format!(
                "UPDATE relates_to:`{}` SET confidence = $confidence",
                record_id_to_uuid(&existing_row.id).unwrap_or(relation.id)
            );
            self.db
                .query(&update_q)
                .bind(("confidence", avg_conf))
                .await
                .map_err(storage_err)?;
            return Ok(false);
        }

        if relation.relation_type.is_symmetric() {
            let rev_q = format!(
                "SELECT id, in AS in_id, out AS out_id, relation_type, confidence, valid_from, valid_until FROM relates_to WHERE in = entity:`{}` AND out = entity:`{}` AND relation_type = $rel_type AND valid_until IS NONE LIMIT 1",
                relation.to_entity_id, relation.from_entity_id
            );
            let mut rev_resp = self
                .db
                .query(&rev_q)
                .bind(("rel_type", rel_type_str.clone()))
                .await
                .map_err(storage_err)?;
            let rev_existing: Vec<RelationEdgeRow> = rev_resp.take(0).map_err(storage_err)?;

            if let Some(rev_row) = rev_existing.into_iter().next() {
                let avg_conf =
                    ((rev_row.confidence as u16 + relation.confidence as u16) / 2) as u8;
                let update_q = format!(
                    "UPDATE relates_to:`{}` SET confidence = $confidence",
                    record_id_to_uuid(&rev_row.id).unwrap_or(relation.id)
                );
                self.db
                    .query(&update_q)
                    .bind(("confidence", avg_conf))
                    .await
                    .map_err(storage_err)?;
                return Ok(false);
            }
        }

        let q = format!(
            "RELATE entity:`{}`->relates_to:`{}`->entity:`{}` SET relation_type = $rel_type, confidence = $confidence, valid_from = <datetime>$valid_from, valid_until = IF $valid_until THEN <datetime>$valid_until ELSE NONE END",
            relation.from_entity_id,
            relation.id,
            relation.to_entity_id,
        );
        self.db
            .query(&q)
            .bind(("rel_type", rel_type_str))
            .bind(("confidence", relation.confidence))
            .bind(("valid_from", relation.valid_from.to_rfc3339()))
            .bind(("valid_until", relation.valid_until.map(|d| d.to_rfc3339())))
            .await
            .map_err(storage_err)?;
        Ok(true)
    }

    pub async fn invalidate_relation(&self, id: Uuid) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let q = format!("UPDATE relates_to:`{}` SET valid_until = <datetime>$now", id);
        self.db
            .query(&q)
            .bind(("now", now))
            .await
            .map_err(storage_err)?;
        Ok(())
    }

    /// Invalidate all active relations where the given entity is either endpoint.
    /// Returns the number of relations invalidated.
    pub async fn invalidate_relations_for_entity(&self, entity_id: Uuid) -> Result<usize> {
        let now = Utc::now().to_rfc3339();
        let q = format!(
            "UPDATE relates_to SET valid_until = <datetime>$now WHERE (in = entity:`{}` OR out = entity:`{}`) AND valid_until IS NONE",
            entity_id, entity_id
        );
        let mut response = self
            .db
            .query(&q)
            .bind(("now", now))
            .await
            .map_err(storage_err)?;
        let affected: Vec<RelationEdgeRow> = response.take(0).map_err(storage_err)?;
        Ok(affected.len())
    }

    /// Get active relations for an entity using graph traversal.
    pub async fn get_relations_for_entity(&self, entity_id: Uuid) -> Result<Vec<Relation>> {
        let q = format!(
            "SELECT id, in AS in_id, out AS out_id, relation_type, confidence, valid_from, valid_until FROM relates_to WHERE (in = entity:`{}` OR out = entity:`{}`) AND valid_until IS NONE",
            entity_id, entity_id
        );
        let mut response = self.db.query(&q).await.map_err(storage_err)?;
        let rows: Vec<RelationEdgeRow> = response.take(0).map_err(storage_err)?;
        Ok(rows.into_iter().filter_map(relation_from_edge_row).collect())
    }

    /// Batch-prune relations below a confidence threshold.
    pub async fn prune_low_confidence_relations(&self, threshold: u8) -> Result<usize> {
        let now = Utc::now().to_rfc3339();
        let mut response = self
            .db
            .query("UPDATE relates_to SET valid_until = <datetime>$now WHERE confidence < $threshold AND valid_until IS NONE")
            .bind(("now", now))
            .bind(("threshold", threshold))
            .await
            .map_err(storage_err)?;
        let affected: Vec<RelationEdgeRow> = response.take(0).map_err(storage_err)?;
        Ok(affected.len())
    }

    // ── Memories (with graph edges) ──────────────────────────────────

    /// Create a memory and its graph edges to the source episode and entities.
    pub async fn create_memory(&self, memory: &Memory) -> Result<()> {
        let mem_id = memory.id;
        let ep_id = memory.source_episode_id;

        let q = format!(
            "CREATE memory:`{}` SET content = $content, embedding = $embedding, created_at = <datetime>$created_at, namespace = $namespace, created_by_agent = $created_by_agent",
            mem_id
        );
        self.db
            .query(&q)
            .bind(("content", memory.content.clone()))
            .bind(("embedding", memory.embedding.clone()))
            .bind(("created_at", memory.created_at.to_rfc3339()))
            .bind(("namespace", memory.namespace.clone()))
            .bind(("created_by_agent", memory.created_by_agent.clone()))
            .await
            .map_err(storage_err)?;

        let q = format!("RELATE memory:`{}`->sourced_from->episode:`{}`", mem_id, ep_id);
        self.db.query(&q).await.map_err(storage_err)?;

        for entity_id in &memory.entity_ids {
            let q = format!("RELATE memory:`{}`->references->entity:`{}`", mem_id, entity_id);
            self.db.query(&q).await.map_err(storage_err)?;
        }

        Ok(())
    }

    pub async fn list_recent_memories(&self, limit: usize) -> Result<Vec<Memory>> {
        let mut response = self
            .db
            .query("SELECT *, ->sourced_from->episode AS episode_ids, ->references->entity AS entity_ids FROM memory ORDER BY created_at DESC LIMIT $limit")
            .bind(("limit", limit))
            .await
            .map_err(storage_err)?;
        let rows: Vec<MemoryWithEdgesRow> = response.take(0).map_err(storage_err)?;
        Ok(rows.into_iter().filter_map(memory_with_edges_from_row).collect())
    }

    // ── Vector Search (HNSW) ─────────────────────────────────────────

    /// Search entities by vector similarity using HNSW index.
    /// Optionally filter by entity_type and/or namespace.
    pub async fn search_entities_by_vector(
        &self,
        query_embedding: &[f64],
        limit: usize,
        entity_type_filter: Option<&str>,
        namespace: Option<&str>,
    ) -> Result<Vec<(Entity, f64)>> {
        let mut conditions = vec!["valid_until IS NONE"];
        let mut cond_parts = Vec::new();
        if entity_type_filter.is_some() {
            cond_parts.push("entity_type = $etype".to_string());
        }
        if namespace.is_some() {
            cond_parts.push("namespace = $ns".to_string());
        }
        conditions.extend(cond_parts.iter().map(|s| s.as_str()));

        let q = format!(
            "SELECT *, vector::similarity::cosine(embedding, $query_vec) AS score FROM entity WHERE {} ORDER BY score DESC LIMIT $limit",
            conditions.join(" AND ")
        );

        let mut query = self.db.query(&q)
            .bind(("query_vec", query_embedding.to_vec()))
            .bind(("limit", limit));

        if let Some(etype) = entity_type_filter {
            query = query.bind(("etype", etype.to_string()));
        }
        if let Some(ns) = namespace {
            query = query.bind(("ns", ns.to_string()));
        }

        let mut response = query.await.map_err(storage_err)?;

        let rows: Vec<ScoredEntityRow> = response.take(0).map_err(storage_err)?;
        Ok(rows
            .into_iter()
            .filter_map(|r| {
                let score = r.score;
                let entity = entity_from_row(EntityRow {
                    id: r.id,
                    name: r.name,
                    entity_type: r.entity_type,
                    summary: r.summary,
                    embedding: r.embedding,
                    valid_from: r.valid_from,
                    valid_until: r.valid_until,
                    namespace: r.namespace,
                    created_by_agent: r.created_by_agent,
                })?;
                Some((entity, score))
            })
            .collect())
    }

    /// Search memories by vector similarity using HNSW index.
    /// Optionally scoped by namespace.
    pub async fn search_memories_by_vector(
        &self,
        query_embedding: &[f64],
        limit: usize,
        namespace: Option<&str>,
    ) -> Result<Vec<(Memory, f64)>> {
        let q = match namespace {
            Some(_) => "SELECT *, vector::similarity::cosine(embedding, $query_vec) AS score FROM memory WHERE namespace = $ns ORDER BY score DESC LIMIT $limit",
            None => "SELECT *, vector::similarity::cosine(embedding, $query_vec) AS score FROM memory ORDER BY score DESC LIMIT $limit",
        };
        let mut query = self.db.query(q)
            .bind(("query_vec", query_embedding.to_vec()))
            .bind(("limit", limit));
        if let Some(ns) = namespace {
            query = query.bind(("ns", ns.to_string()));
        }
        let mut response = query.await.map_err(storage_err)?;

        let rows: Vec<ScoredMemoryRow> = response.take(0).map_err(storage_err)?;
        Ok(rows
            .into_iter()
            .filter_map(|r| {
                let score = r.score;
                let memory = memory_from_row(MemoryRow {
                    id: r.id,
                    content: r.content,
                    embedding: r.embedding,
                    created_at: r.created_at,
                    namespace: r.namespace,
                    created_by_agent: r.created_by_agent,
                })?;
                Some((memory, score))
            })
            .collect())
    }

    // ── Full-Text Search (BM25) ──────────────────────────────────────

    /// BM25 full-text search over entity name and summary fields.
    /// Optionally filter by entity_type and/or namespace.
    pub async fn search_entities_by_keyword(
        &self,
        query: &str,
        entity_type_filter: Option<&str>,
        namespace: Option<&str>,
    ) -> Result<Vec<Entity>> {
        debug!("search_entities_by_keyword");

        let mut extra_filters = Vec::new();
        if entity_type_filter.is_some() {
            extra_filters.push("AND entity_type = $etype");
        }
        if namespace.is_some() {
            extra_filters.push("AND namespace = $ns");
        }
        let suffix = extra_filters.join(" ");

        let q = format!(
            "SELECT *, search::score(1) + search::score(2) AS relevance FROM entity WHERE (name @1@ $query OR summary @2@ $query) {} ORDER BY relevance DESC",
            suffix
        );

        let mut db_query = self.db.query(&q).bind(("query", query.to_string()));

        if let Some(etype) = entity_type_filter {
            db_query = db_query.bind(("etype", etype.to_string()));
        }
        if let Some(ns) = namespace {
            db_query = db_query.bind(("ns", ns.to_string()));
        }

        let mut response = db_query.await.map_err(storage_err)?;

        let rows: Vec<FtsEntityRow> = response.take(0).map_err(storage_err)?;
        Ok(rows
            .into_iter()
            .filter_map(|r| {
                entity_from_row(EntityRow {
                    id: r.id,
                    name: r.name,
                    entity_type: r.entity_type,
                    summary: r.summary,
                    embedding: r.embedding,
                    valid_from: r.valid_from,
                    valid_until: r.valid_until,
                    namespace: r.namespace,
                    created_by_agent: r.created_by_agent,
                })
            })
            .collect())
    }

    /// BM25 full-text search over memory content.
    /// Optionally scoped by namespace.
    pub async fn search_memories_by_keyword(&self, query: &str, namespace: Option<&str>) -> Result<Vec<Memory>> {
        let q = match namespace {
            Some(_) => "SELECT *, search::score(1) AS relevance FROM memory WHERE content @1@ $query AND namespace = $ns ORDER BY relevance DESC",
            None => "SELECT *, search::score(1) AS relevance FROM memory WHERE content @1@ $query ORDER BY relevance DESC",
        };
        let mut db_query = self.db.query(q).bind(("query", query.to_string()));
        if let Some(ns) = namespace {
            db_query = db_query.bind(("ns", ns.to_string()));
        }
        let mut response = db_query.await.map_err(storage_err)?;

        let rows: Vec<ScoredMemoryRow> = response.take(0).map_err(storage_err)?;
        Ok(rows
            .into_iter()
            .filter_map(|r| {
                memory_from_row(MemoryRow {
                    id: r.id,
                    content: r.content,
                    embedding: r.embedding,
                    created_at: r.created_at,
                    namespace: r.namespace,
                    created_by_agent: r.created_by_agent,
                })
            })
            .collect())
    }

    /// BM25 full-text search over episode content.
    pub async fn search_episodes_by_keyword(&self, query: &str) -> Result<Vec<Episode>> {
        let mut response = self
            .db
            .query("SELECT *, search::score(1) AS relevance FROM episode WHERE content @1@ $query ORDER BY relevance DESC")
            .bind(("query", query.to_string()))
            .await
            .map_err(storage_err)?;

        let rows: Vec<EpisodeRow> = response.take(0).map_err(storage_err)?;
        Ok(rows.into_iter().filter_map(episode_from_row).collect())
    }

    // ── Graph Traversal ──────────────────────────────────────────────

    /// Get graph neighbors of given entities using SurrealDB graph traversal.
    pub async fn get_graph_neighbors(
        &self,
        entity_ids: &[Uuid],
        _depth: usize,
    ) -> Result<Vec<Entity>> {
        let mut all_entities = Vec::new();

        for eid in entity_ids {
            if let Some(seed) = self.get_entity(*eid).await? {
                all_entities.push(seed);
            }

            let q = format!(
                "SELECT * FROM entity:`{}`<->relates_to<->entity WHERE valid_until IS NONE",
                eid
            );
            let mut response = self.db.query(&q).await.map_err(storage_err)?;
            let rows: Vec<EntityRow> = response.take(0).map_err(storage_err)?;
            for row in rows {
                if let Some(entity) = entity_from_row(row) {
                    if !all_entities.iter().any(|e| e.id == entity.id) {
                        all_entities.push(entity);
                    }
                }
            }
        }

        Ok(all_entities)
    }

    // ── Temporal Queries ─────────────────────────────────────────────

    pub async fn entities_at(&self, at: DateTime<Utc>) -> Result<Vec<Entity>> {
        let at_str = at.to_rfc3339();
        let mut response = self
            .db
            .query("SELECT * FROM entity WHERE valid_from <= <datetime>$at AND (valid_until IS NONE OR valid_until > <datetime>$at)")
            .bind(("at", at_str))
            .await
            .map_err(storage_err)?;
        let rows: Vec<EntityRow> = response.take(0).map_err(storage_err)?;
        Ok(rows.into_iter().filter_map(entity_from_row).collect())
    }

    pub async fn relations_at(&self, at: DateTime<Utc>) -> Result<Vec<Relation>> {
        let at_str = at.to_rfc3339();
        let mut response = self
            .db
            .query("SELECT id, in AS in_id, out AS out_id, relation_type, confidence, valid_from, valid_until FROM relates_to WHERE valid_from <= <datetime>$at AND (valid_until IS NONE OR valid_until > <datetime>$at)")
            .bind(("at", at_str))
            .await
            .map_err(storage_err)?;
        let rows: Vec<RelationEdgeRow> = response.take(0).map_err(storage_err)?;
        Ok(rows.into_iter().filter_map(relation_from_edge_row).collect())
    }

    // ── Changefeed Queries ───────────────────────────────────────────

    /// Query recent changes to the entity table via changefeeds.
    pub async fn entity_changes_since(&self, since: DateTime<Utc>) -> Result<Vec<serde_json::Value>> {
        let since_str = since.to_rfc3339();
        let mut response = self
            .db
            .query("SHOW CHANGES FOR TABLE entity SINCE $since")
            .bind(("since", since_str))
            .await
            .map_err(storage_err)?;
        let changes: Vec<serde_json::Value> = response.take(0).map_err(storage_err)?;
        Ok(changes)
    }

    /// Query recent changes to the relates_to edge table via changefeeds.
    pub async fn relation_changes_since(&self, since: DateTime<Utc>) -> Result<Vec<serde_json::Value>> {
        let mut response = self
            .db
            .query("SHOW CHANGES FOR TABLE relates_to SINCE $since")
            .bind(("since", since.to_rfc3339()))
            .await
            .map_err(storage_err)?;
        let changes: Vec<serde_json::Value> = response.take(0).map_err(storage_err)?;
        Ok(changes)
    }

    // ── Multi-Agent Discovery ─────────────────────────────────────────

    /// List all distinct agent IDs that have contributed episodes.
    pub async fn list_agents(&self) -> Result<Vec<serde_json::Value>> {
        let mut response = self
            .db
            .query("SELECT agent_id, agent_name, array::group(namespace) AS namespaces, count() AS episode_count FROM episode WHERE agent_id IS NOT NONE GROUP BY agent_id, agent_name")
            .await
            .map_err(storage_err)?;
        let rows: Vec<serde_json::Value> = response.take(0).map_err(storage_err)?;
        Ok(rows)
    }

    /// List all distinct namespaces across episodes, entities, and memories.
    pub async fn list_namespaces(&self) -> Result<Vec<serde_json::Value>> {
        let mut response = self
            .db
            .query("SELECT namespace, count() AS entity_count FROM entity WHERE namespace IS NOT NONE AND valid_until IS NONE GROUP BY namespace")
            .await
            .map_err(storage_err)?;
        let rows: Vec<serde_json::Value> = response.take(0).map_err(storage_err)?;
        Ok(rows)
    }

    /// List recent episodes from a specific agent.
    pub async fn list_episodes_by_agent(&self, agent_id: &str, limit: usize) -> Result<Vec<Episode>> {
        let mut response = self
            .db
            .query("SELECT * FROM episode WHERE agent_id = $agent_id ORDER BY created_at DESC LIMIT $limit")
            .bind(("agent_id", agent_id.to_string()))
            .bind(("limit", limit))
            .await
            .map_err(storage_err)?;
        let rows: Vec<EpisodeRow> = response.take(0).map_err(storage_err)?;
        Ok(rows.into_iter().filter_map(episode_from_row).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_id_to_uuid_via_string_key() {
        use surrealdb::types::RecordIdKey;
        let uuid = Uuid::new_v4();
        let rid = surrealdb::types::RecordId {
            table: "entity".into(),
            key: RecordIdKey::String(uuid.to_string()),
        };
        assert_eq!(record_id_to_uuid(&rid), Some(uuid));
    }
}
