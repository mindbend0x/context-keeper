use anyhow::Result;
use chrono::{DateTime, Utc};
use context_keeper_core::models::*;
use serde::{Deserialize, Serialize};
use surrealdb::engine::local::Db;
use surrealdb::Surreal;
use surrealdb::types::{RecordId, SurrealValue};
use tracing::debug;
use uuid::Uuid;

/// Typed repository for all Context Keeper data operations against SurrealDB.
#[derive(Clone)]
pub struct Repository {
    db: Surreal<Db>,
}

// ── Internal record types for SurrealDB serialization ───────────────────

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
struct EpisodeRecord {
    #[allow(dead_code)]
    id: RecordId,
    content: String,
    source: String,
    session_id: Option<String>,
    created_at: String,
    uuid: String,
}

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
struct EntityRecord {
    #[allow(dead_code)]
    id: RecordId,
    name: String,
    entity_type: String,
    summary: String,
    embedding: Vec<f64>,
    valid_from: String,
    valid_until: Option<String>,
    uuid: String,
}

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
struct RelationRecord {
    #[allow(dead_code)]
    id: RecordId,
    source_entity: String,
    target_entity: String,
    rel_type: String,
    confidence: u8,
    valid_from: String,
    valid_until: Option<String>,
    uuid: String,
}

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
struct MemoryRecord {
    #[allow(dead_code)]
    id: RecordId,
    content: String,
    embedding: Vec<f64>,
    source_episode_uuid: String,
    entity_uuids: Vec<String>,
    created_at: String,
    uuid: String,
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
        self.db
            .query("CREATE episode SET content = $content, source = $source, session_id = $session_id, created_at = $created_at, uuid = $uuid")
            .bind(("content", episode.content.clone()))
            .bind(("source", episode.source.clone()))
            .bind(("session_id", episode.session_id.clone()))
            .bind(("created_at", episode.created_at.to_rfc3339()))
            .bind(("uuid", episode.id.to_string()))
            .await?;
        Ok(())
    }

    pub async fn get_episode(&self, id: Uuid) -> Result<Option<Episode>> {
        let mut response = self
            .db
            .query("SELECT * FROM episode WHERE uuid = $uuid LIMIT 1")
            .bind(("uuid", id.to_string()))
            .await?;
        let records: Vec<EpisodeRecord> = response.take(0)?;
        Ok(records.into_iter().next().map(|r| Episode {
            id: Uuid::parse_str(&r.uuid).unwrap_or(id),
            content: r.content,
            source: r.source,
            session_id: r.session_id,
            created_at: DateTime::parse_from_rfc3339(&r.created_at)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        }))
    }

    pub async fn list_recent_episodes(&self, limit: usize) -> Result<Vec<Episode>> {
        let mut response = self
            .db
            .query("SELECT * FROM episode ORDER BY created_at DESC LIMIT $limit")
            .bind(("limit", limit))
            .await?;
        let records: Vec<EpisodeRecord> = response.take(0)?;
        Ok(records
            .into_iter()
            .filter_map(|r| {
                Some(Episode {
                    id: Uuid::parse_str(&r.uuid).ok()?,
                    content: r.content,
                    source: r.source,
                    session_id: r.session_id,
                    created_at: DateTime::parse_from_rfc3339(&r.created_at)
                        .map(|d| d.with_timezone(&Utc))
                        .ok()?,
                })
            })
            .collect())
    }

    // ── Entities ─────────────────────────────────────────────────────

    pub async fn upsert_entity(&self, entity: &Entity) -> Result<()> {
        self.db
            .query("CREATE entity SET name = $name, entity_type = $entity_type, summary = $summary, embedding = $embedding, valid_from = $valid_from, valid_until = $valid_until, uuid = $uuid")
            .bind(("name", entity.name.clone()))
            .bind(("entity_type", entity.entity_type.clone()))
            .bind(("summary", entity.summary.clone()))
            .bind(("embedding", entity.embedding.clone()))
            .bind(("valid_from", entity.valid_from.to_rfc3339()))
            .bind(("valid_until", entity.valid_until.map(|d| d.to_rfc3339())))
            .bind(("uuid", entity.id.to_string()))
            .await?;
        Ok(())
    }

    pub async fn get_entity(&self, id: Uuid) -> Result<Option<Entity>> {
        let mut response = self
            .db
            .query("SELECT * FROM entity WHERE uuid = $uuid LIMIT 1")
            .bind(("uuid", id.to_string()))
            .await?;
        let records: Vec<EntityRecord> = response.take(0)?;
        Ok(records.into_iter().next().and_then(record_to_entity))
    }

    pub async fn find_entities_by_name(&self, name: &str) -> Result<Vec<Entity>> {
        let mut response = self
            .db
            .query("SELECT * FROM entity WHERE name = $name AND valid_until IS NONE")
            .bind(("name", name.to_string()))
            .await?;

        debug!("find_entities_by_name");

        let records: Vec<EntityRecord> = response.take(0)?;
        Ok(records.into_iter().filter_map(record_to_entity).collect())
    }

    pub async fn get_all_active_entities(&self) -> Result<Vec<Entity>> {
        let mut response = self
            .db
            .query("SELECT * FROM entity WHERE valid_until IS NONE")
            .await?;
        
        let records: Vec<EntityRecord> = response.take(0)?;
        Ok(records.into_iter().filter_map(record_to_entity).collect())
    }

    // ── Relations ────────────────────────────────────────────────────

    pub async fn create_relation(&self, relation: &Relation) -> Result<()> {
        self.db
            .query("CREATE relation SET source_entity = $source_entity, target_entity = $target_entity, rel_type = $rel_type, confidence = $confidence, valid_from = $valid_from, valid_until = $valid_until, uuid = $uuid")
            .bind(("source_entity", relation.source_entity_id.to_string()))
            .bind(("target_entity", relation.target_entity_id.to_string()))
            .bind(("rel_type", relation.relation_type.clone()))
            .bind(("confidence", relation.confidence))
            .bind(("valid_from", relation.valid_from.to_rfc3339()))
            .bind(("valid_until", relation.valid_until.map(|d| d.to_rfc3339())))
            .bind(("uuid", relation.id.to_string()))
            .await?;
        Ok(())
    }

    pub async fn invalidate_relation(&self, id: Uuid) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.db
            .query("UPDATE relation SET valid_until = $now WHERE uuid = $uuid")
            .bind(("now", now))
            .bind(("uuid", id.to_string()))
            .await?;
        Ok(())
    }

    pub async fn get_relations_for_entity(&self, entity_id: Uuid) -> Result<Vec<Relation>> {
        let entity_str = entity_id.to_string();
        let mut response = self
            .db
            .query("SELECT * FROM relation WHERE (source_entity = $eid OR target_entity = $eid) AND valid_until IS NONE")
            .bind(("eid", entity_str))
            .await?;
        let records: Vec<RelationRecord> = response.take(0)?;
        Ok(records.into_iter().filter_map(record_to_relation).collect())
    }

    // ── Memories ─────────────────────────────────────────────────────

    pub async fn create_memory(&self, memory: &Memory) -> Result<()> {
        let entity_uuids: Vec<String> = memory.entity_ids.iter().map(|id| id.to_string()).collect();
        self.db
            .query("CREATE memory SET content = $content, embedding = $embedding, source_episode_uuid = $source_episode_uuid, entity_uuids = $entity_uuids, created_at = $created_at, uuid = $uuid")
            .bind(("content", memory.content.clone()))
            .bind(("embedding", memory.embedding.clone()))
            .bind(("source_episode_uuid", memory.source_episode_id.to_string()))
            .bind(("entity_uuids", entity_uuids))
            .bind(("created_at", memory.created_at.to_rfc3339()))
            .bind(("uuid", memory.id.to_string()))
            .await?;
        Ok(())
    }

    pub async fn list_recent_memories(&self, limit: usize) -> Result<Vec<Memory>> {
        let mut response = self
            .db
            .query("SELECT * FROM memory ORDER BY created_at DESC LIMIT $limit")
            .bind(("limit", limit))
            .await?;
        let records: Vec<MemoryRecord> = response.take(0)?;
        Ok(records.into_iter().filter_map(record_to_memory).collect())
    }

    // ── Search ───────────────────────────────────────────────────────

    /// Brute-force cosine similarity search over entity embeddings.
    pub async fn search_entities_by_vector(
        &self,
        query_embedding: &[f64],
        limit: usize,
    ) -> Result<Vec<(Entity, f64)>> {
        let entities = self.get_all_active_entities().await?;
        let mut scored: Vec<(Entity, f64)> = entities
            .into_iter()
            .map(|e| {
                let score = cosine_similarity(query_embedding, &e.embedding);
                (e, score)
            })
            .collect();
        
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);
        
        Ok(scored)
    }

    /// Brute-force cosine similarity search over memory embeddings.
    pub async fn search_memories_by_vector(
        &self,
        query_embedding: &[f64],
        limit: usize,
    ) -> Result<Vec<(Memory, f64)>> {
        let mut response = self.db.query("SELECT * FROM memory").await?;
        let records: Vec<MemoryRecord> = response.take(0)?;
        let memories: Vec<Memory> = records
            .into_iter()
            .filter_map(record_to_memory)
            .collect();

        let mut scored: Vec<(Memory, f64)> = memories
            .into_iter()
            .map(|m| {
                let score = cosine_similarity(query_embedding, &m.embedding);
                (m, score)
            })
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);
        Ok(scored)
    }

    /// Keyword search over entity names (simple CONTAINS match).
    pub async fn search_entities_by_keyword(&self, query: &str) -> Result<Vec<Entity>> {
        let pattern = format!("%{}%", query.to_lowercase());
        let mut response = self
            .db
            .query("SELECT * FROM entity WHERE string::lowercase(name) CONTAINS string::lowercase($q) OR string::lowercase(summary) CONTAINS string::lowercase($q)")
            .bind(("q", query.to_string()))
            .await?;
        
        debug!("search_entities_by_keyword");
        
        let records: Vec<EntityRecord> = response.take(0)?;
        let _ = pattern; // suppress unused
        Ok(records.into_iter().filter_map(record_to_entity).collect())
    }

    /// Get graph neighbors: entities connected to the given entity IDs via relations.
    pub async fn get_graph_neighbors(&self, entity_ids: &[Uuid], depth: usize) -> Result<Vec<Entity>> {
        let mut visited = std::collections::HashSet::new();
        let mut frontier: Vec<Uuid> = entity_ids.to_vec();

        for _ in 0..depth {
            let mut next_frontier = Vec::new();
            for eid in &frontier {
                if !visited.insert(*eid) {
                    continue;
                }
                let relations = self.get_relations_for_entity(*eid).await?;
                for rel in relations {
                    if !visited.contains(&rel.source_entity_id) {
                        next_frontier.push(rel.source_entity_id);
                    }
                    if !visited.contains(&rel.target_entity_id) {
                        next_frontier.push(rel.target_entity_id);
                    }
                }
            }
            frontier = next_frontier;
        }

        let mut entities = Vec::new();
        for eid in &visited {
            if let Some(entity) = self.get_entity(*eid).await? {
                entities.push(entity);
            }
        }
        Ok(entities)
    }

    // ── Temporal queries ─────────────────────────────────────────────

    pub async fn entities_at(&self, at: DateTime<Utc>) -> Result<Vec<Entity>> {
        let at_str = at.to_rfc3339();
        let mut response = self
            .db
            .query("SELECT * FROM entity WHERE valid_from <= $at AND (valid_until IS NONE OR valid_until > $at)")
            .bind(("at", at_str))
            .await?;
        let records: Vec<EntityRecord> = response.take(0)?;
        Ok(records.into_iter().filter_map(record_to_entity).collect())
    }

    pub async fn relations_at(&self, at: DateTime<Utc>) -> Result<Vec<Relation>> {
        let at_str = at.to_rfc3339();
        let mut response = self
            .db
            .query("SELECT * FROM relation WHERE valid_from <= $at AND (valid_until IS NONE OR valid_until > $at)")
            .bind(("at", at_str))
            .await?;
        let records: Vec<RelationRecord> = response.take(0)?;
        Ok(records.into_iter().filter_map(record_to_relation).collect())
    }
}

// ── Conversion helpers ──────────────────────────────────────────────────

fn record_to_entity(r: EntityRecord) -> Option<Entity> {
    Some(Entity {
        id: Uuid::parse_str(&r.uuid).ok()?,
        name: r.name,
        entity_type: r.entity_type,
        summary: r.summary,
        embedding: r.embedding,
        valid_from: DateTime::parse_from_rfc3339(&r.valid_from)
            .map(|d| d.with_timezone(&Utc))
            .ok()?,
        valid_until: r.valid_until.and_then(|s| {
            DateTime::parse_from_rfc3339(&s)
                .map(|d| d.with_timezone(&Utc))
                .ok()
        }),
    })
}

fn record_to_relation(r: RelationRecord) -> Option<Relation> {
    Some(Relation {
        id: Uuid::parse_str(&r.uuid).ok()?,
        source_entity_id: Uuid::parse_str(&r.source_entity).ok()?,
        target_entity_id: Uuid::parse_str(&r.target_entity).ok()?,
        relation_type: r.rel_type,
        confidence: r.confidence,
        valid_from: DateTime::parse_from_rfc3339(&r.valid_from)
            .map(|d| d.with_timezone(&Utc))
            .ok()?,
        valid_until: r.valid_until.and_then(|s| {
            DateTime::parse_from_rfc3339(&s)
                .map(|d| d.with_timezone(&Utc))
                .ok()
        }),
    })
}

fn record_to_memory(r: MemoryRecord) -> Option<Memory> {
    Some(Memory {
        id: Uuid::parse_str(&r.uuid).ok()?,
        content: r.content,
        embedding: r.embedding,
        source_episode_id: Uuid::parse_str(&r.source_episode_uuid).ok()?,
        entity_ids: r
            .entity_uuids
            .iter()
            .filter_map(|s| Uuid::parse_str(s).ok())
            .collect(),
        created_at: DateTime::parse_from_rfc3339(&r.created_at)
            .map(|d| d.with_timezone(&Utc))
            .ok()?,
    })
}

/// Cosine similarity between two vectors.
pub fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let mag_b: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
    if mag_a == 0.0 || mag_b == 0.0 {
        return 0.0;
    }
    dot / (mag_a * mag_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let v = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&v, &v) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        assert!(cosine_similarity(&a, &b).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        assert!((cosine_similarity(&a, &b) + 1.0).abs() < 1e-6);
    }
}
