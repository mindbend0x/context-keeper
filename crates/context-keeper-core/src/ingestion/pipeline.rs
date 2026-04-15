use chrono::Utc;
use serde::Serialize;
use uuid::Uuid;

use crate::error::Result;
use crate::models::{Entity, EntityType, Episode, Memory, Relation};
use crate::traits::{
    Embedder, EntityExtractor, EntityResolver, RelationExtractor, SummarySynthesizer,
};

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
    /// Entity IDs whose active relations should be invalidated (set `valid_until` to now).
    /// Populated when a contradiction invalidates an entity — its stale relations must go too.
    pub entity_ids_to_invalidate_relations: Vec<Uuid>,
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
    "moved to",
    "transferred",
    "switched to",
    "replaced by",
    "no longer at",
    "fired",
    "terminated",
    "retired",
    "dissolved",
];

/// Markers that indicate a state transition (e.g. new employer, new location)
/// rather than an explicit negation of the old state.
const TRANSITION_MARKERS: &[&str] = &[
    "joined",
    "hired by",
    "hired at",
    "now at",
    "now works",
    "now with",
    "started at",
    "started working",
    "began at",
    "began working",
    "appointed",
    "accepted position",
    "promoted to",
    "relocated to",
    "enrolled at",
    "enrolled in",
];

/// Prepositions that introduce a slot value (e.g. "works at Acme").
/// Used to detect conflicting slot-value pairs between summaries.
const SLOT_PREPOSITIONS: &[&str] = &["at", "for", "in", "with"];

/// Heuristic contradiction detection between old and new summaries.
///
/// Returns `Some(reason)` if the new summary contradicts the old one. Signals:
/// 1. Explicit negation markers in the new summary.
/// 2. Transition markers in the new summary (e.g. "joined", "hired at").
/// 3. Conflicting slot-value pairs (e.g. "at Acme" vs "at BigCo").
/// 4. Very low word overlap combined with any negation/transition marker.
fn detect_contradiction(existing_summary: &str, new_summary: &str) -> Option<String> {
    let new_lower = new_summary.to_lowercase();

    for marker in NEGATION_MARKERS {
        if new_lower.contains(marker) {
            return Some(format!("Negation marker '{marker}' found in new summary"));
        }
    }

    for marker in TRANSITION_MARKERS {
        if new_lower.contains(marker) {
            return Some(format!("Transition marker '{marker}' found in new summary"));
        }
    }

    let existing_lower = existing_summary.to_lowercase();

    if let Some(reason) = detect_slot_conflict(&existing_lower, &new_lower) {
        return Some(reason);
    }

    let existing_words: std::collections::HashSet<&str> =
        existing_lower.split_whitespace().collect();
    let new_words: std::collections::HashSet<&str> = new_lower.split_whitespace().collect();
    let overlap: usize = existing_words.intersection(&new_words).count();
    let total = existing_words.len().max(new_words.len());

    if total > 3 && overlap == 0 {
        return Some("Summaries share no common terms".to_string());
    }

    if total > 3 {
        let overlap_ratio = overlap as f64 / total as f64;
        let combined = format!("{} {}", existing_lower, new_lower);
        let has_signal = NEGATION_MARKERS.iter().any(|m| combined.contains(m))
            || TRANSITION_MARKERS.iter().any(|m| combined.contains(m));
        if overlap_ratio < 0.3 && has_signal {
            return Some(format!(
                "Low word overlap ({:.0}%) with negation/transition signal",
                overlap_ratio * 100.0
            ));
        }
    }

    None
}

/// Detect conflicting slot-value pairs between two summaries.
///
/// Looks for patterns like "at Acme" vs "at BigCo" — the same preposition
/// followed by a different capitalized token indicates the entity's state
/// changed to a different target.
fn detect_slot_conflict(existing_lower: &str, new_lower: &str) -> Option<String> {
    let existing_words: Vec<&str> = existing_lower.split_whitespace().collect();
    let new_words: Vec<&str> = new_lower.split_whitespace().collect();

    for prep in SLOT_PREPOSITIONS {
        let existing_slots: Vec<&str> = existing_words
            .windows(2)
            .filter_map(|w| if w[0] == *prep { Some(w[1]) } else { None })
            .collect();

        let new_slots: Vec<&str> = new_words
            .windows(2)
            .filter_map(|w| if w[0] == *prep { Some(w[1]) } else { None })
            .collect();

        for e_slot in &existing_slots {
            for n_slot in &new_slots {
                if e_slot != n_slot {
                    return Some(format!(
                        "Conflicting slot: '{prep} {e_slot}' vs '{prep} {n_slot}'"
                    ));
                }
            }
        }
    }

    None
}

/// Use the LLM synthesizer when available; fall back to heuristic concatenation.
async fn synthesize_or_merge(
    synthesizer: Option<&dyn SummarySynthesizer>,
    existing: &str,
    new: &str,
    entity_name: &str,
) -> String {
    if let Some(synth) = synthesizer {
        match synth.synthesize(existing, new, entity_name).await {
            Ok(merged) => return merged,
            Err(e) => {
                tracing::warn!(
                    entity = entity_name,
                    error = %e,
                    "SummarySynthesizer failed, falling back to heuristic merge"
                );
            }
        }
    }
    merge_summaries(existing, new)
}

/// Merge an existing entity summary with new information.
///
/// If the new summary adds information not present in the old one, the two
/// are concatenated. If they are substantially the same, the new one wins.
fn merge_summaries(existing: &str, new: &str) -> String {
    let existing_lower = existing.to_lowercase();
    let new_lower = new.to_lowercase();

    let existing_words: std::collections::HashSet<&str> =
        existing_lower.split_whitespace().collect();
    let new_words: std::collections::HashSet<&str> = new_lower.split_whitespace().collect();

    let novel_count = new_words.difference(&existing_words).count();
    if novel_count > 0 && !existing.is_empty() && !new.is_empty() {
        format!("{}; {}", existing, new)
    } else {
        new.to_string()
    }
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
    summary_synthesizer: Option<&dyn SummarySynthesizer>,
    min_confidence: Option<u8>,
) -> Result<IngestionResult> {
    tracing::info!(episode_id = %episode.id, "Starting ingestion pipeline");

    let min_conf = min_confidence.unwrap_or(DEFAULT_MIN_CONFIDENCE);
    let mut diff = IngestionDiff::default();

    let ns = episode.namespace.as_deref();
    let agent_id = episode.agent.as_ref().map(|a| a.agent_id.clone());

    // 1. Extract entities
    let extracted = entity_extractor.extract_entities(&episode.content).await?;
    tracing::info!(count = extracted.len(), "Extracted entities");

    // 2. Build Entity models, resolving against existing graph when possible
    let now = Utc::now();
    let mut entities = Vec::with_capacity(extracted.len());

    for ext in &extracted {
        let embedding = embedder.embed(&ext.name).await?;

        if let Some(resolver) = entity_resolver {
            if let Some(existing) = resolver
                .find_existing(&ext.name, &ext.entity_type, ns)
                .await?
            {
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
                    diff.entity_ids_to_invalidate_relations.push(existing.id);
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
                    let merged = synthesize_or_merge(
                        summary_synthesizer,
                        &existing.summary,
                        &ext.summary,
                        &ext.name,
                    )
                    .await;
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
                        namespace: existing
                            .namespace
                            .clone()
                            .or_else(|| episode.namespace.clone()),
                        created_by_agent: agent_id.clone().or(existing.created_by_agent.clone()),
                    });
                }
                continue;
            }

            let similar = resolver
                .find_similar(&ext.name, &embedding, 0.85, ns)
                .await?;
            if let Some(best) = similar.first() {
                let merged = synthesize_or_merge(
                    summary_synthesizer,
                    &best.summary,
                    &ext.summary,
                    &ext.name,
                )
                .await;
                diff.entities_updated.push(EntityUpdate {
                    name: ext.name.clone(),
                    old_summary: best.summary.clone(),
                    new_summary: merged.clone(),
                });
                let resolved_type = if ext.entity_type == EntityType::Other {
                    best.entity_type.clone()
                } else {
                    ext.entity_type.clone()
                };
                entities.push(Entity {
                    id: best.id,
                    name: best.name.clone(),
                    entity_type: resolved_type,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_negation_markers() {
        assert!(detect_contradiction("Works at Acme", "No longer at Acme").is_some());
        assert!(detect_contradiction("Works at Acme", "Left Acme last month").is_some());
        assert!(detect_contradiction("Works at Acme", "Quit Acme in June").is_some());
        assert!(detect_contradiction("Works at Acme", "Retired from Acme").is_some());
    }

    #[test]
    fn transition_markers() {
        assert!(detect_contradiction("Works at Acme", "Joined BigCo as CTO").is_some());
        assert!(detect_contradiction("Works at Acme", "Now works at BigCo").is_some());
        assert!(detect_contradiction("Works at Acme", "Hired at BigCo recently").is_some());
        assert!(detect_contradiction("Lives in Berlin", "Relocated to London").is_some());
        assert!(detect_contradiction("Works at Acme", "Promoted to VP of BigCo").is_some());
    }

    #[test]
    fn slot_conflict_detection() {
        assert!(detect_contradiction("Works at Acme", "Works at BigCo").is_some());
        assert!(detect_contradiction("Lives in Berlin", "Lives in London").is_some());
        assert!(detect_contradiction("Employed at Google", "Employed at Meta").is_some());
    }

    #[test]
    fn no_contradiction_on_additive_info() {
        assert!(detect_contradiction("Works at Acme", "Works at Acme as Director").is_none());
        assert!(
            detect_contradiction("Engineer", "Senior engineer with expertise in Rust").is_none()
        );
    }

    #[test]
    fn no_contradiction_on_same_content() {
        assert!(detect_contradiction("Works at Acme", "Works at Acme").is_none());
    }

    #[test]
    fn zero_overlap_short_summaries_no_false_positive() {
        assert!(detect_contradiction("Hi", "Ok").is_none());
    }

    #[test]
    fn zero_overlap_long_summaries() {
        assert!(detect_contradiction(
            "Software engineer specializing in Rust",
            "Marketing manager for consumer products"
        )
        .is_some());
    }

    #[test]
    fn merge_summaries_concatenates_novel_info() {
        let merged = merge_summaries("Works at Acme", "Leads engineering team");
        assert!(merged.contains("Works at Acme"));
        assert!(merged.contains("Leads engineering team"));
    }

    #[test]
    fn merge_summaries_replaces_when_no_novel_words() {
        let merged = merge_summaries("Works at Acme", "Works at Acme");
        assert_eq!(merged, "Works at Acme");
    }
}
