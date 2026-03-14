use anyhow::Result;
use chrono::{DateTime, Utc};
use context_keeper_core::models::*;
use serde::{Deserialize, Serialize};
use surrealdb::engine::local::Db;
use surrealdb::RecordId;
use surrealdb::Surreal;
use uuid::Uuid;

/// Typed repository for all Context Keeper data operations against SurrealDB.
#[derive(Clone)]
pub struct Repository {
    db: Surreal<Db>,
}

// ── Internal record types for SurrealDB serialization ───────────────────

#[derive(Debug, Serialize, Deserialize)]
struct EpisodeRecord {
    content: String,
    source: String,
    session_id: Option<String>,
    created_at: String,
    uuid: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct EntityRecord {
    name: String,
    entity_type: String,
    summary: String,
    embedding: Vec<f32>,
    valid_from: String,
    valid_until: Option<String>,
    uuid: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct RelationRecord {
    source_entity: String,
    target_entity: String,
    rel_type: String,
    confidence: f32,
    valid_from: String,
    valid_until: Option<String>,
    uuid: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct MemoryRecord {
    content: String,
    embedding: Vec<f32>,
    source_episode_uuid: String,
    entity_uuids: Vec<String>,
    created_at: String,
    uuid: String,
}

#[derive(Debug, Deserialize)]
struct DbRecord<T> {
    id: RecordId,
    #[serde(flatten)]
    data: T,
}

impl Repository {
    pub fn new(db: Surreal<Db>) -> Self {
        Self { db }
    }

    // ── Episodes ─────────────────────────────────────────────────────

    pub async fn create_episode(&self, episode: &Episode) -> Result<()> {
        let record = EpisodeRecord {
            content: episode.content.clone(),
            source: episode.source.clone(),
            session_id: episode.session_id.clone(),
            created_at: episode.created_at.to_rfc3339(),
            uuid: episode.id.to_string(),
        };
        let _: Option<DbRecord<EpisodeRecord>> = self.db.create(("episode", episode.id.to_string())).content(record).await?;
        Ok(())
    }

    pub async fn get_episode(&self, id: Uuid) -> Result<Option<Episode>> {
        let record: Option<DbRecord<EpisodeRecord>> = self.db.select(("episode", id.to_string())).await?;
        Ok(record.map(|r| Episode {
            id: Uuid::parse_str(&r.data.uuid).unwrap_or(id),
            content: r.data.content,
            source: r.data.source,
            session_id: r.data.session_id,
            created_at: DateTime::parse_from_rfc3339(&r.data.created_at)
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
        let records: Vec<DbRecord<EpisodeRecord>> = response.take(0)?;
        Ok(records
            .into_iter()
            .filter_map(|r| {
                Some(Episode {
                    id: Uuid::parse_str(&r.data.uuid).ok()?,
                    content: r.data.content,
                    source: r.data.source,
                    session_id: r.data.session_id,
                    created_at: DateTime::parse_from_rfc3339(&r.data.created_at)
                        .map(|d| d.with_timezone(&Utc))
                        .ok()?,
                })
            })
            .collect())
    }

    // ── Entities ─────────────────────────────────────────────────────

    pub async fn upsert_entity(&self, entity: &Entity) -> Result<()> {
        let record = EntityRecord {
            name: entity.name.clone(),
            entity_type: entity.entity_type.clone(),
            summary: entity.summary.clone(),
            embedding: entity.embedding.clone(),
            valid_from: entity.valid_from.to_rfc3339(),
            valid_until: entity.valid_until.map(|d| d.to_rfc3339()),
            uuid: entity.id.to_string(),
        };
        let _: Option<DbRecord<EntityRecord>> = self.db.update(("entity", entity.id.to_string())).content(record).await?;
        Ok(())
    }

    pub async fn get_entity(&self, id: Uuid) -> Result<Option<Entity>> {
        let record: Option<DbRecord<EntityRecord>> = self.db.select(("entity", id.to_string())).await?;
        Ok(record.and_then(|r| record_to_entity(r.data)))
    }

    pub async fn find_entities_by_name(&self, name: &str) -> Result<Vec<Entity>> {
        let mut response = self
            .db
            .query("SELECT * FROM entity WHERE name = $name AND valid_until IS NONE")
            .bind(("name", name.to_string()))
            .await?;
        let records: Vec<DbRecord<EntityRecord>> = response.take(0)?;
        Ok(records.into_iter().filter_map(|r| record_to_entity(r.data)).collect())
    }

    pub async fn get_all_active_entities(&self) -> Result<Vec<Entity>> {
        let mut response = self
            .db
            .query("SELECT * FROM entity WHERE valid_until IS NONE")
            .await?;
        let records: Vec<DbRecord<EntityRecord>> = response.take(0)?;
        Ok(records.into_iter().filter_map(|r| record_to_entity(r.data)).collect())
    }

    // ── Relations ────────────────────────────────────────────────────

    pub async fn create_relation(&self, relation: &Relation) -> Result<()> {
        let record = RelationRecord {
            source_entity: relation.source_entity_id.to_string(),
            target_entity: relation.target_entity_id.to_string(),
            rel_type: relation.relation_type.clone(),
            confidence: relation.confidence,
            valid_from: relation.valid_from.to_rfc3339(),
            valid_until: relation.valid_until.map(|d| d.to_rfc3339()),
            uuid: relation.id.to_string(),
        };
        let _: Option<DbRecord<RelationRecord>> = self.db.create(("relation", relation.id.to_string())).content(record).await?;
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
        let records: Vec<DbRecord<RelationRecord>> = response.take(0)?;
        Ok(records.into_iter().filter_map(|r| record_to_relation(r.data)).collect())
    }

    // ── Memories ─────────────────────────────────────────────────────

    pub async fn create_memory(&self, memory: &Memory) -> Result<()> {
        let record = MemoryRecord {
            content: memory.content.clone(),
            embedding: memory.embedding.clone(),
            source_episode_uuid: memory.source_episode_id.to_string(),
            entity_uuids: memory.entity_ids.iter().map(|id| id.to_string()).collect(),
            created_at: memory.created_at.to_rfc3339(),
            uuid: memory.id.to_string(),
        };
        let _: Option<DbRecord<MemoryRecord>> = self.db.create(("memory", memory.id.to_string())).content(record).await?;
        Ok(())
    }

    pub async fn list_recent_memories(&self, limit: usize) -> Result<Vec<Memory>> {
        let mut response = self
            .db
            .query("SELECT * FROM memory ORDER BY created_at DESC LIMIT $limit")
            .bind(("limit", limit))
            .await?;
        let records: Vec<DbRecord<MemoryRecord>> = response.take(0)?;
        Ok(records.into_iter().filter_map(|r| record_to_memory(r.data)).collect())
    }

    // ── Search ───────────────────────────────────────────────────────

    /// Brute-force cosine similarity search over entity embeddings.
    pub async fn search_entities_by_vector(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<(Entity, f32)>> {
        let entities = self.get_all_active_entities().await?;
        let mut scored: Vec<(Entity, f32)> = entities
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
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<(Memory, f32)>> {
        let mut response = self.db.query("SELECT * FROM memory").await?;
        let records: Vec<DbRecord<MemoryRecord>> = response.take(0)?;
        let memories: Vec<Memory> = records
            .into_iter()
            .filter_map(|r| record_to_memory(r.data))
            .collect();

        let mut scored: Vec<(Memory, f32)> = memories
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
        let records: Vec<DbRecord<EntityRecord>> = response.take(0)?;
        let _ = pattern; // suppress unused
        Ok(records.into_iter().filter_map(|r| record_to_entity(r.data)).collect())
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
        let records: Vec<DbRecord<EntityRecord>> = response.take(0)?;
        Ok(records.into_iter().filter_map(|r| record_to_entity(r.data)).collect())
    }

    pub async fn relations_at(&self, at: DateTime<Utc>) -> Result<Vec<Relation>> {
        let at_str = at.to_rfc3339();
        let mut response = self
            .db
            .query("SELECT * FROM relation WHERE valid_from <= $at AND (valid_until IS NONE OR valid_until > $at)")
            .bind(("at", at_str))
            .await?;
        let records: Vec<DbRecord<RelationRecord>> = response.take(0)?;
        Ok(records.into_iter().filter_map(|r| record_to_relation(r.data)).collect())
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
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
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
