use std::collections::HashMap;

use crate::models::{Entity, SearchResult};

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
            let entry = scores
                .entry(entity.id)
                .or_insert((0.0, entity.clone()));
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

    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
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
            entity_type: "test".to_string(),
            summary: name.to_string(),
            embedding: vec![],
            valid_from: Utc::now(),
            valid_until: None,
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
        // Alice appears first in both lists → should get higher combined score
        let results = fuse_rrf(vec![
            vec![e1.clone(), e2.clone()],
            vec![e1.clone()],
        ]);
        let alice = results.iter().find(|r| r.entity.as_ref().unwrap().name == "Alice").unwrap();
        let bob = results.iter().find(|r| r.entity.as_ref().unwrap().name == "Bob").unwrap();
        assert!(alice.score > bob.score);
    }
}
