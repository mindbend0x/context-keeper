use chrono::Utc;
use serde::Serialize;
use uuid::Uuid;

use crate::error::Result;
use crate::models::{Entity, Episode, Memory, Relation};
use crate::traits::{Embedder, EntityExtractor, EntityResolver, RelationExtractor};

/// The output of a successful ingestion run.
#[derive(Debug)]
pub struct IngestionResult {
    pub entities: Vec<Entity>,
    pub relations: Vec<Relation>,
    pub memories: Vec<Memory>,
    pub diff: IngestionDiff,
}

/// Summary of what changed during ingestion relative to the existing graph.
#[derive(Debug, Default, Serialize)]
pub struct IngestionDiff {
    pub entities_created: Vec<String>,
    pub entities_updated: Vec<EntityUpdate>,
    pub entities_invalidated: Vec<EntityInvalidation>,
    pub relations_created: usize,
    pub relations_merged: usize,
    pub relations_pruned: usize,
}

#[derive(Debug, Serialize)]
pub struct EntityUpdate {
    pub name: String,
    pub old_summary: String,
    pub new_summary: String,
}

#[derive(Debug, Serialize)]
pub struct EntityInvalidation {
    pub name: String,
    pub entity_type: String,
    pub reason: String,
    pub invalidated_id: Uuid,
}

const DEFAULT_MIN_CONFIDENCE: u8 = 50;

const NEGATION_MARKERS: &[&str] = &[
    "no longer",
    "not anymore",
    "no more",
    "former",
    "formerly",
    "previously",
    "used to",
    "left",
    "quit",
    "resigned",
    "departed",
    "was replaced",
    "was fired",
    "was terminated",
    "retired from",
    "moved away",
    "moved from",
    "switched from",
    "changed from",
    "stopped",
    "ended",
    "cancelled",
    "divorced",
    "separated from",
    "ex-",
];

/// Heuristic contradiction detection between old and new summaries.
///
/// Returns `Some(reason)` when the new summary appears to negate or supersede
/// the existing one. Checks for explicit negation markers first, then falls
/// back to a word-overlap heuristic for complete rewrites.
fn detect_contradiction(existing_summary: &str, new_summary: &str) -> Option<String> {
    let new_lower = new_summary.to_lowercase();

    for marker in NEGATION_MARKERS {
        if new_lower.contains(marker) {
            return Some(format!("Negation marker '{marker}' found in new summary"));
        }
    }

    let existing_lower = existing_summary.to_lowercase();
    let existing_words: std::collections::HashSet<&str> =
        existing_lower.split_whitespace().collect();
    let new_words: std::collections::HashSet<&str> = new_lower.split_whitespace().collect();
    let overlap: usize = existing_words.intersection(&new_words).count();
    let total = existing_words.len().max(new_words.len());

    if total > 3 && overlap == 0 {
        return Some("Summaries share no common terms".to_string());
    }

    None
}

/// Merge two summaries, preferring the new information while preserving context.
fn merge_summaries(existing: &str, new: &str) -> String {
    if existing.is_empty() {
        return new.to_string();
    }
    if new.is_empty() {
        return existing.to_string();
    }
    if existing == new {
        return new.to_string();
    }
    format!("{} | Updated: {}", existing, new)
}

/// Process an episode through the ingestion pipeline.
///
/// When `entity_resolver` is provided, new entities are matched against the
/// existing graph: exact name matches are updated in-place, contradictions
/// trigger `valid_until` invalidation, and aliases are resolved via vector
/// similarity.
///
/// Relations with `confidence < min_confidence` are pruned before output.
pub async fn ingest(
    episode: &Episode,
    embedder: &dyn Embedder,
    entity_extractor: &dyn EntityExtractor,
    relation_extractor: &dyn RelationExtractor,
    entity_resolver: Option<&dyn EntityResolver>,
    min_confidence: Option<u8>,
) -> Result<IngestionResult> {
    tracing::info!(episode_id = %episode.id, "Starting ingestion pipeline");

    let min_conf = min_confidence.unwrap_or(DEFAULT_MIN_CONFIDENCE);
    let mut diff = IngestionDiff::default();

    let ns = episode.namespace.as_deref();
    let agent_id = episode.agent.as_ref().map(|a| a.agent_id.clone());

    // 1. Extract entities
    let extracted = entity_extractor
        .extract_entities(&episode.content)
        .await?;
    tracing::info!(count = extracted.len(), "Extracted entities");

    // 2. Build Entity models, resolving against existing graph when possible
    let now = Utc::now();
    let mut entities = Vec::with_capacity(extracted.len());

    for ext in &extracted {
        let embedding = embedder.embed(&ext.name).await?;

        if let Some(resolver) = entity_resolver {
            if let Some(existing) = resolver.find_existing(&ext.name, &ext.entity_type, ns).await? {
                if let Some(reason) = detect_contradiction(&existing.summary, &ext.summary) {
                    tracing::info!(
                        entity = %ext.name,
                        reason = %reason,
                        "Contradiction detected — invalidating old entity"
                    );
                    diff.entities_invalidated.push(EntityInvalidation {
                        name: ext.name.clone(),
                        entity_type: ext.entity_type.to_string(),
                        reason: reason.clone(),
                        invalidated_id: existing.id,
                    });
                    entities.push(Entity {
                        id: Uuid::new_v4(),
                        name: ext.name.clone(),
                        entity_type: ext.entity_type.clone(),
                        summary: ext.summary.clone(),
                        embedding,
                        valid_from: now,
                        valid_until: None,
                        namespace: episode.namespace.clone(),
                        created_by_agent: agent_id.clone(),
                    });
                    diff.entities_created.push(ext.name.clone());
                } else {
                    let merged = merge_summaries(&existing.summary, &ext.summary);
                    diff.entities_updated.push(EntityUpdate {
                        name: ext.name.clone(),
                        old_summary: existing.summary.clone(),
                        new_summary: merged.clone(),
                    });
                    entities.push(Entity {
                        id: existing.id,
                        name: ext.name.clone(),
                        entity_type: ext.entity_type.clone(),
                        summary: merged,
                        embedding,
                        valid_from: existing.valid_from,
                        valid_until: None,
                        namespace: existing.namespace.clone().or_else(|| episode.namespace.clone()),
                        created_by_agent: agent_id.clone().or(existing.created_by_agent.clone()),
                    });
                }
                continue;
            }

            let similar = resolver.find_similar(&ext.name, &embedding, 0.85, ns).await?;
            if let Some(best) = similar.first() {
                let merged = merge_summaries(&best.summary, &ext.summary);
                diff.entities_updated.push(EntityUpdate {
                    name: ext.name.clone(),
                    old_summary: best.summary.clone(),
                    new_summary: merged.clone(),
                });
                entities.push(Entity {
                    id: best.id,
                    name: best.name.clone(),
                    entity_type: ext.entity_type.clone(),
                    summary: merged,
                    embedding,
                    valid_from: best.valid_from,
                    valid_until: None,
                    namespace: best.namespace.clone().or_else(|| episode.namespace.clone()),
                    created_by_agent: agent_id.clone().or(best.created_by_agent.clone()),
                });
                continue;
            }
        }

        // No existing match — create new entity
        diff.entities_created.push(ext.name.clone());
        entities.push(Entity {
            id: Uuid::new_v4(),
            name: ext.name.clone(),
            entity_type: ext.entity_type.clone(),
            summary: ext.summary.clone(),
            embedding,
            valid_from: now,
            valid_until: None,
            namespace: episode.namespace.clone(),
            created_by_agent: agent_id.clone(),
        });
    }

    // 3. Extract relations
    let extracted_rels = relation_extractor
        .extract_relations(&episode.content, &extracted)
        .await?;
    tracing::info!(count = extracted_rels.len(), "Extracted relations");

    // 4. Build Relation models with canonical types and confidence pruning
    let mut relations = Vec::with_capacity(extracted_rels.len());
    for ext_rel in &extracted_rels {
        if ext_rel.confidence < min_conf {
            diff.relations_pruned += 1;
            continue;
        }

        let source = entities.iter().find(|e| e.name == ext_rel.subject);
        let target = entities.iter().find(|e| e.name == ext_rel.object);
        if let (Some(src), Some(tgt)) = (source, target) {
            let canonical_type = ext_rel.canonical_type();
            relations.push(Relation {
                id: Uuid::new_v4(),
                from_entity_id: src.id,
                to_entity_id: tgt.id,
                relation_type: canonical_type,
                confidence: ext_rel.confidence,
                valid_from: now,
                valid_until: None,
            });
            diff.relations_created += 1;
        }
    }

    // 5. Create a memory for this episode
    let memory_embedding = embedder.embed(&episode.content).await?;
    let memory = Memory {
        id: Uuid::new_v4(),
        content: episode.content.clone(),
        embedding: memory_embedding,
        source_episode_id: episode.id,
        entity_ids: entities.iter().map(|e| e.id).collect(),
        created_at: now,
        namespace: episode.namespace.clone(),
        created_by_agent: agent_id,
    };

    tracing::info!(
        entities = entities.len(),
        relations = relations.len(),
        created = diff.entities_created.len(),
        updated = diff.entities_updated.len(),
        invalidated = diff.entities_invalidated.len(),
        pruned = diff.relations_pruned,
        "Ingestion complete"
    );

    Ok(IngestionResult {
        entities,
        relations,
        memories: vec![memory],
        diff,
    })
}
