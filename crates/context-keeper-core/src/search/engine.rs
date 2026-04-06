use std::collections::HashMap;

use crate::models::{Entity, Memory, SearchResult};

/// Reciprocal Rank Fusion constant. Higher values give more weight to lower-ranked results.
const RRF_K: f32 = 60.0;

/// Compute RRF score for a single item at a given rank (0-indexed).
pub fn rrf_score(rank: usize) -> f32 {
    1.0 / (RRF_K + rank as f32 + 1.0)
}

/// Fuse multiple ranked lists of entity IDs using Reciprocal Rank Fusion.
///
/// Each input is a ranked list of (entity_id, Entity) pairs.
/// Returns entities sorted by combined RRF score.
pub fn fuse_rrf(ranked_lists: Vec<Vec<Entity>>) -> Vec<SearchResult> {
    let mut scores: HashMap<uuid::Uuid, (f32, Entity)> = HashMap::new();

    for list in ranked_lists {
        for (rank, entity) in list.into_iter().enumerate() {
            let entry = scores.entry(entity.id).or_insert((0.0, entity.clone()));
            entry.0 += rrf_score(rank);
        }
    }

    let mut results: Vec<SearchResult> = scores
        .into_values()
        .map(|(score, entity)| SearchResult {
            entity: Some(entity),
            memory: None,
            score,
        })
        .collect();

    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results
}

/// Fuse ranked lists of entities and memories into a unified result set using RRF.
///
/// Uses a discriminated key (`e:<uuid>` / `m:<uuid>`) so that entity and memory
/// UUIDs from different tables never collide in the score map.
pub fn fuse_rrf_mixed(
    entity_lists: Vec<Vec<Entity>>,
    memory_lists: Vec<Vec<Memory>>,
) -> Vec<SearchResult> {
    let mut scores: HashMap<String, (f32, SearchResult)> = HashMap::new();

    for list in entity_lists {
        for (rank, entity) in list.into_iter().enumerate() {
            let key = format!("e:{}", entity.id);
            let entry = scores.entry(key).or_insert_with(|| {
                (
                    0.0,
                    SearchResult {
                        entity: Some(entity),
                        memory: None,
                        score: 0.0,
                    },
                )
            });
            entry.0 += rrf_score(rank);
        }
    }

    for list in memory_lists {
        for (rank, memory) in list.into_iter().enumerate() {
            let key = format!("m:{}", memory.id);
            let entry = scores.entry(key).or_insert_with(|| {
                (
                    0.0,
                    SearchResult {
                        entity: None,
                        memory: Some(memory),
                        score: 0.0,
                    },
                )
            });
            entry.0 += rrf_score(rank);
        }
    }

    let mut results: Vec<SearchResult> = scores
        .into_values()
        .map(|(score, mut sr)| {
            sr.score = score;
            sr
        })
        .collect();

    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn make_entity(name: &str) -> Entity {
        Entity {
            id: Uuid::new_v4(),
            name: name.to_string(),
            entity_type: crate::models::EntityType::Other,
            summary: name.to_string(),
            embedding: vec![],
            valid_from: Utc::now(),
            valid_until: None,
            namespace: None,
            created_by_agent: None,
        }
    }

    fn make_memory(content: &str) -> Memory {
        Memory {
            id: Uuid::new_v4(),
            content: content.to_string(),
            embedding: vec![],
            source_episode_id: Uuid::new_v4(),
            entity_ids: vec![],
            created_at: Utc::now(),
            namespace: None,
            created_by_agent: None,
        }
    }

    #[test]
    fn test_rrf_score_rank_0() {
        let score = rrf_score(0);
        // 1 / (60 + 0 + 1) = 1/61
        assert!((score - 1.0 / 61.0).abs() < 1e-6);
    }

    #[test]
    fn test_rrf_score_decreasing() {
        assert!(rrf_score(0) > rrf_score(1));
        assert!(rrf_score(1) > rrf_score(10));
    }

    #[test]
    fn test_fuse_rrf_single_list() {
        let e1 = make_entity("Alice");
        let e2 = make_entity("Bob");
        let results = fuse_rrf(vec![vec![e1.clone(), e2.clone()]]);
        assert_eq!(results.len(), 2);
        assert!(results[0].score >= results[1].score);
    }

    #[test]
    fn test_fuse_rrf_overlap_boosts() {
        let e1 = make_entity("Alice");
        let e2 = make_entity("Bob");
        let results = fuse_rrf(vec![vec![e1.clone(), e2.clone()], vec![e1.clone()]]);
        let alice = results
            .iter()
            .find(|r| r.entity.as_ref().unwrap().name == "Alice")
            .unwrap();
        let bob = results
            .iter()
            .find(|r| r.entity.as_ref().unwrap().name == "Bob")
            .unwrap();
        assert!(alice.score > bob.score);
    }

    // ── fuse_rrf_mixed tests ─────────────────────────────────────────

    #[test]
    fn test_fuse_rrf_mixed_entities_only() {
        let e1 = make_entity("Alice");
        let e2 = make_entity("Bob");
        let results = fuse_rrf_mixed(vec![vec![e1.clone(), e2.clone()]], vec![]);
        assert_eq!(results.len(), 2);
        assert!(results
            .iter()
            .all(|r| r.entity.is_some() && r.memory.is_none()));
    }

    #[test]
    fn test_fuse_rrf_mixed_memories_only() {
        let m1 = make_memory("Fact about Alice");
        let m2 = make_memory("Fact about Bob");
        let results = fuse_rrf_mixed(vec![], vec![vec![m1, m2]]);
        assert_eq!(results.len(), 2);
        assert!(results
            .iter()
            .all(|r| r.entity.is_none() && r.memory.is_some()));
    }

    #[test]
    fn test_fuse_rrf_mixed_entities_and_memories() {
        let e1 = make_entity("Alice");
        let m1 = make_memory("Alice works at Acme Corp");
        let results = fuse_rrf_mixed(vec![vec![e1]], vec![vec![m1]]);
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|r| r.entity.is_some()));
        assert!(results.iter().any(|r| r.memory.is_some()));
    }

    #[test]
    fn test_fuse_rrf_mixed_memory_boosted_by_multiple_lists() {
        let m1 = make_memory("Important fact");
        let m2 = make_memory("Other fact");
        let results = fuse_rrf_mixed(
            vec![],
            vec![vec![m1.clone(), m2.clone()], vec![m1.clone()]],
        );
        let important = results
            .iter()
            .find(|r| r.memory.as_ref().map(|m| m.content.as_str()) == Some("Important fact"))
            .unwrap();
        let other = results
            .iter()
            .find(|r| r.memory.as_ref().map(|m| m.content.as_str()) == Some("Other fact"))
            .unwrap();
        assert!(important.score > other.score);
    }

    #[test]
    fn test_fuse_rrf_mixed_no_uuid_collisions() {
        let shared_id = Uuid::new_v4();
        let entity = Entity {
            id: shared_id,
            name: "Entity".to_string(),
            entity_type: crate::models::EntityType::Other,
            summary: "An entity".to_string(),
            embedding: vec![],
            valid_from: Utc::now(),
            valid_until: None,
            namespace: None,
            created_by_agent: None,
        };
        let memory = Memory {
            id: shared_id,
            content: "A memory".to_string(),
            embedding: vec![],
            source_episode_id: Uuid::new_v4(),
            entity_ids: vec![],
            created_at: Utc::now(),
            namespace: None,
            created_by_agent: None,
        };
        let results = fuse_rrf_mixed(vec![vec![entity]], vec![vec![memory]]);
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|r| r.entity.is_some()));
        assert!(results.iter().any(|r| r.memory.is_some()));
    }

    #[test]
    fn test_fuse_rrf_mixed_empty_inputs() {
        let results = fuse_rrf_mixed(vec![], vec![]);
        assert!(results.is_empty());
    }

    #[test]
    fn test_fuse_rrf_mixed_preserves_entity_search_behavior() {
        let e1 = make_entity("Alice");
        let e2 = make_entity("Bob");
        let mixed =
            fuse_rrf_mixed(vec![vec![e1.clone(), e2.clone()], vec![e1.clone()]], vec![]);
        let original = fuse_rrf(vec![vec![e1.clone(), e2.clone()], vec![e1.clone()]]);
        assert_eq!(mixed.len(), original.len());
        for (m, o) in mixed.iter().zip(original.iter()) {
            assert_eq!(
                m.entity.as_ref().unwrap().name,
                o.entity.as_ref().unwrap().name
            );
            assert!((m.score - o.score).abs() < 1e-6);
        }
    }
}
