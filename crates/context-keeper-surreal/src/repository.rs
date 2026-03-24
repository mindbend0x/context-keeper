use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use context_keeper_core::models::*;
use context_keeper_core::traits::EntityResolver;
use serde::{Deserialize, Serialize};
use surrealdb::engine::local::Db;
use surrealdb::types::SurrealValue;
use surrealdb::Surreal;
use tracing::debug;
use uuid::Uuid;

/// Typed repository for all Context Keeper data operations against SurrealDB.
///
/// Uses SurrealDB's native graph engine: RELATE for edges, HNSW for vector
/// search, BM25 for full-text, and recursive graph queries for traversal.
#[derive(Clone)]
pub struct Repository {
    db: Surreal<Db>,
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
}

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
struct EpisodeRow {
    id: surrealdb::types::RecordId,
    content: String,
    source: String,
    session_id: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
struct MemoryRow {
    id: surrealdb::types::RecordId,
    content: String,
    #[serde(default)]
    embedding: Vec<f64>,
    created_at: DateTime<Utc>,
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
    score: f64,
}

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
struct ScoredMemoryRow {
    id: surrealdb::types::RecordId,
    content: String,
    #[serde(default)]
    embedding: Vec<f64>,
    created_at: DateTime<Utc>,
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
    })
}

fn episode_from_row(r: EpisodeRow) -> Option<Episode> {
    Some(Episode {
        id: record_id_to_uuid(&r.id)?,
        content: r.content,
        source: r.source,
        session_id: r.session_id,
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
    async fn find_existing(&self, name: &str) -> Result<Option<Entity>> {
        let results = self.find_entities_by_name(name).await?;
        Ok(results.into_iter().next())
    }

    async fn find_similar(
        &self,
        _name: &str,
        embedding: &[f64],
        threshold: f64,
    ) -> Result<Vec<Entity>> {
        let candidates = self
            .search_entities_by_vector(embedding, 3, None)
            .await?;

        Ok(candidates
            .into_iter()
            .filter(|(_, score)| *score >= threshold)
            .map(|(e, _)| e)
            .collect())
    }
}

impl Repository {
    pub fn new(db: Surreal<Db>) -> Self {
        Self { db }
    }

    // ── File export/import ───────────────────────────────────────────

    pub async fn export(&self, path: &str) -> Result<()> {
        self.db.export(path).await?;
        Ok(())
    }

    pub async fn import_from_file(&self, path: &str) -> Result<()> {
        self.db.import(path).await?;
        Ok(())
    }

    // ── Episodes ─────────────────────────────────────────────────────

    pub async fn create_episode(&self, episode: &Episode) -> Result<()> {
        let q = format!(
            "CREATE episode:`{}` SET content = $content, source = $source, session_id = $session_id, created_at = <datetime>$created_at",
            episode.id
        );
        self.db
            .query(&q)
            .bind(("content", episode.content.clone()))
            .bind(("source", episode.source.clone()))
            .bind(("session_id", episode.session_id.clone()))
            .bind(("created_at", episode.created_at.to_rfc3339()))
            .await?;
        Ok(())
    }

    pub async fn get_episode(&self, id: Uuid) -> Result<Option<Episode>> {
        let q = format!("SELECT * FROM episode:`{}`", id);
        let mut response = self.db.query(&q).await?;
        let rows: Vec<EpisodeRow> = response.take(0)?;
        Ok(rows.into_iter().next().and_then(episode_from_row))
    }

    pub async fn list_recent_episodes(&self, limit: usize) -> Result<Vec<Episode>> {
        let mut response = self
            .db
            .query("SELECT * FROM episode ORDER BY created_at DESC LIMIT $limit")
            .bind(("limit", limit))
            .await?;
        let rows: Vec<EpisodeRow> = response.take(0)?;
        Ok(rows.into_iter().filter_map(episode_from_row).collect())
    }

    // ── Entities ─────────────────────────────────────────────────────

    /// True UPSERT: if an entity with the same ID exists, update it.
    pub async fn upsert_entity(&self, entity: &Entity) -> Result<()> {
        let q = format!(
            "UPSERT entity:`{}` SET name = $name, entity_type = $entity_type, summary = $summary, embedding = $embedding, valid_from = <datetime>$valid_from, valid_until = IF $valid_until THEN <datetime>$valid_until ELSE NONE END",
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
            .await?;
        Ok(())
    }

    pub async fn get_entity(&self, id: Uuid) -> Result<Option<Entity>> {
        let q = format!("SELECT * FROM entity:`{}`", id);
        let mut response = self.db.query(&q).await?;
        let rows: Vec<EntityRow> = response.take(0)?;
        Ok(rows.into_iter().next().and_then(entity_from_row))
    }

    pub async fn find_entities_by_name(&self, name: &str) -> Result<Vec<Entity>> {
        let mut response = self
            .db
            .query("SELECT * FROM entity WHERE name = $name AND valid_until IS NONE")
            .bind(("name", name.to_string()))
            .await?;
        debug!("find_entities_by_name");
        let rows: Vec<EntityRow> = response.take(0)?;
        Ok(rows.into_iter().filter_map(entity_from_row).collect())
    }

    pub async fn get_all_active_entities(&self) -> Result<Vec<Entity>> {
        let mut response = self
            .db
            .query("SELECT * FROM entity WHERE valid_until IS NONE")
            .await?;
        let rows: Vec<EntityRow> = response.take(0)?;
        Ok(rows.into_iter().filter_map(entity_from_row).collect())
    }

    /// Invalidate an entity by setting its `valid_until` to now.
    pub async fn invalidate_entity(&self, id: Uuid) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let q = format!(
            "UPDATE entity:`{}` SET valid_until = <datetime>$now",
            id
        );
        self.db.query(&q).bind(("now", now)).await?;
        Ok(())
    }

    /// Get active entities filtered by type.
    pub async fn get_entities_by_type(&self, entity_type: &str) -> Result<Vec<Entity>> {
        let mut response = self
            .db
            .query("SELECT * FROM entity WHERE entity_type = $etype AND valid_until IS NONE")
            .bind(("etype", entity_type.to_string()))
            .await?;
        let rows: Vec<EntityRow> = response.take(0)?;
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

        // Check for existing active relation with same (from, to, type)
        let check_q = format!(
            "SELECT id, in AS in_id, out AS out_id, relation_type, confidence, valid_from, valid_until FROM relates_to WHERE in = entity:`{}` AND out = entity:`{}` AND relation_type = $rel_type AND valid_until IS NONE LIMIT 1",
            relation.from_entity_id, relation.to_entity_id
        );
        let mut check_resp = self
            .db
            .query(&check_q)
            .bind(("rel_type", rel_type_str.clone()))
            .await?;
        let existing: Vec<RelationEdgeRow> = check_resp.take(0)?;

        if let Some(existing_row) = existing.into_iter().next() {
            let avg_conf = ((existing_row.confidence as u16 + relation.confidence as u16) / 2) as u8;
            let update_q = format!(
                "UPDATE relates_to:`{}` SET confidence = $confidence",
                record_id_to_uuid(&existing_row.id).unwrap_or(relation.id)
            );
            self.db
                .query(&update_q)
                .bind(("confidence", avg_conf))
                .await?;
            return Ok(false); // merged, not created
        }

        // For symmetric types, also check the reverse direction
        if relation.relation_type.is_symmetric() {
            let rev_q = format!(
                "SELECT id, in AS in_id, out AS out_id, relation_type, confidence, valid_from, valid_until FROM relates_to WHERE in = entity:`{}` AND out = entity:`{}` AND relation_type = $rel_type AND valid_until IS NONE LIMIT 1",
                relation.to_entity_id, relation.from_entity_id
            );
            let mut rev_resp = self
                .db
                .query(&rev_q)
                .bind(("rel_type", rel_type_str.clone()))
                .await?;
            let rev_existing: Vec<RelationEdgeRow> = rev_resp.take(0)?;

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
                    .await?;
                return Ok(false); // merged, not created
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
            .await?;
        Ok(true) // newly created
    }

    pub async fn invalidate_relation(&self, id: Uuid) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let q = format!("UPDATE relates_to:`{}` SET valid_until = <datetime>$now", id);
        self.db
            .query(&q)
            .bind(("now", now))
            .await?;
        Ok(())
    }

    /// Get active relations for an entity using graph traversal.
    pub async fn get_relations_for_entity(&self, entity_id: Uuid) -> Result<Vec<Relation>> {
        let q = format!(
            "SELECT id, in AS in_id, out AS out_id, relation_type, confidence, valid_from, valid_until FROM relates_to WHERE (in = entity:`{}` OR out = entity:`{}`) AND valid_until IS NONE",
            entity_id, entity_id
        );
        let mut response = self.db.query(&q).await?;
        let rows: Vec<RelationEdgeRow> = response.take(0)?;
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
            .await?;
        let affected: Vec<RelationEdgeRow> = response.take(0)?;
        Ok(affected.len())
    }

    // ── Memories (with graph edges) ──────────────────────────────────

    /// Create a memory and its graph edges to the source episode and entities.
    pub async fn create_memory(&self, memory: &Memory) -> Result<()> {
        let mem_id = memory.id;
        let ep_id = memory.source_episode_id;

        let q = format!(
            "CREATE memory:`{}` SET content = $content, embedding = $embedding, created_at = <datetime>$created_at",
            mem_id
        );
        self.db
            .query(&q)
            .bind(("content", memory.content.clone()))
            .bind(("embedding", memory.embedding.clone()))
            .bind(("created_at", memory.created_at.to_rfc3339()))
            .await?;

        let q = format!("RELATE memory:`{}`->sourced_from->episode:`{}`", mem_id, ep_id);
        self.db.query(&q).await?;

        for entity_id in &memory.entity_ids {
            let q = format!("RELATE memory:`{}`->references->entity:`{}`", mem_id, entity_id);
            self.db.query(&q).await?;
        }

        Ok(())
    }

    pub async fn list_recent_memories(&self, limit: usize) -> Result<Vec<Memory>> {
        let mut response = self
            .db
            .query("SELECT *, ->sourced_from->episode AS episode_ids, ->references->entity AS entity_ids FROM memory ORDER BY created_at DESC LIMIT $limit")
            .bind(("limit", limit))
            .await?;
        let rows: Vec<MemoryWithEdgesRow> = response.take(0)?;
        Ok(rows.into_iter().filter_map(memory_with_edges_from_row).collect())
    }

    // ── Vector Search (HNSW) ─────────────────────────────────────────

    /// Search entities by vector similarity using HNSW index.
    /// Optionally filter by entity_type.
    pub async fn search_entities_by_vector(
        &self,
        query_embedding: &[f64],
        limit: usize,
        entity_type_filter: Option<&str>,
    ) -> Result<Vec<(Entity, f64)>> {
        let q = match entity_type_filter {
            Some(_) => "SELECT *, vector::similarity::cosine(embedding, $query_vec) AS score FROM entity WHERE valid_until IS NONE AND entity_type = $etype ORDER BY score DESC LIMIT $limit".to_string(),
            None => "SELECT *, vector::similarity::cosine(embedding, $query_vec) AS score FROM entity WHERE valid_until IS NONE ORDER BY score DESC LIMIT $limit".to_string(),
        };

        let mut query = self.db.query(&q)
            .bind(("query_vec", query_embedding.to_vec()))
            .bind(("limit", limit));

        if let Some(etype) = entity_type_filter {
            query = query.bind(("etype", etype.to_string()));
        }

        let mut response = query.await?;

        let rows: Vec<ScoredEntityRow> = response.take(0)?;
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
                })?;
                Some((entity, score))
            })
            .collect())
    }

    /// Search memories by vector similarity using HNSW index.
    pub async fn search_memories_by_vector(
        &self,
        query_embedding: &[f64],
        limit: usize,
    ) -> Result<Vec<(Memory, f64)>> {
        let mut response = self
            .db
            .query("SELECT *, vector::similarity::cosine(embedding, $query_vec) AS score FROM memory ORDER BY score DESC LIMIT $limit")
            .bind(("query_vec", query_embedding.to_vec()))
            .bind(("limit", limit))
            .await?;

        let rows: Vec<ScoredMemoryRow> = response.take(0)?;
        Ok(rows
            .into_iter()
            .filter_map(|r| {
                let score = r.score;
                let memory = memory_from_row(MemoryRow {
                    id: r.id,
                    content: r.content,
                    embedding: r.embedding,
                    created_at: r.created_at,
                })?;
                Some((memory, score))
            })
            .collect())
    }

    // ── Full-Text Search (BM25) ──────────────────────────────────────

    /// BM25 full-text search over entity name and summary fields.
    /// Optionally filter by entity_type.
    pub async fn search_entities_by_keyword(
        &self,
        query: &str,
        entity_type_filter: Option<&str>,
    ) -> Result<Vec<Entity>> {
        debug!("search_entities_by_keyword");

        let q = match entity_type_filter {
            Some(_) => "SELECT *, search::score(1) + search::score(2) AS relevance FROM entity WHERE (name @1@ $query OR summary @2@ $query) AND entity_type = $etype ORDER BY relevance DESC",
            None => "SELECT *, search::score(1) + search::score(2) AS relevance FROM entity WHERE name @1@ $query OR summary @2@ $query ORDER BY relevance DESC",
        };

        let mut db_query = self.db.query(q).bind(("query", query.to_string()));

        if let Some(etype) = entity_type_filter {
            db_query = db_query.bind(("etype", etype.to_string()));
        }

        let mut response = db_query.await?;

        let rows: Vec<FtsEntityRow> = response.take(0)?;
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
                })
            })
            .collect())
    }

    /// BM25 full-text search over memory content.
    pub async fn search_memories_by_keyword(&self, query: &str) -> Result<Vec<Memory>> {
        let mut response = self
            .db
            .query("SELECT *, search::score(1) AS relevance FROM memory WHERE content @1@ $query ORDER BY relevance DESC")
            .bind(("query", query.to_string()))
            .await?;

        let rows: Vec<ScoredMemoryRow> = response.take(0)?;
        Ok(rows
            .into_iter()
            .filter_map(|r| {
                memory_from_row(MemoryRow {
                    id: r.id,
                    content: r.content,
                    embedding: r.embedding,
                    created_at: r.created_at,
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
            .await?;

        let rows: Vec<EpisodeRow> = response.take(0)?;
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
            let mut response = self.db.query(&q).await?;
            let rows: Vec<EntityRow> = response.take(0)?;
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
            .await?;
        let rows: Vec<EntityRow> = response.take(0)?;
        Ok(rows.into_iter().filter_map(entity_from_row).collect())
    }

    pub async fn relations_at(&self, at: DateTime<Utc>) -> Result<Vec<Relation>> {
        let at_str = at.to_rfc3339();
        let mut response = self
            .db
            .query("SELECT id, in AS in_id, out AS out_id, relation_type, confidence, valid_from, valid_until FROM relates_to WHERE valid_from <= <datetime>$at AND (valid_until IS NONE OR valid_until > <datetime>$at)")
            .bind(("at", at_str))
            .await?;
        let rows: Vec<RelationEdgeRow> = response.take(0)?;
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
            .await?;
        let changes: Vec<serde_json::Value> = response.take(0)?;
        Ok(changes)
    }

    /// Query recent changes to the relates_to edge table via changefeeds.
    pub async fn relation_changes_since(&self, since: DateTime<Utc>) -> Result<Vec<serde_json::Value>> {
        let mut response = self
            .db
            .query("SHOW CHANGES FOR TABLE relates_to SINCE $since")
            .bind(("since", since.to_rfc3339()))
            .await?;
        let changes: Vec<serde_json::Value> = response.take(0)?;
        Ok(changes)
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
