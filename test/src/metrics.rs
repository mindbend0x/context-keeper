use std::collections::HashSet;

/// Fraction of retrieved items that are relevant.
pub fn precision(retrieved: &[String], relevant: &HashSet<String>) -> f64 {
    if retrieved.is_empty() {
        return 0.0;
    }
    let hits = retrieved.iter().filter(|r| relevant.contains(*r)).count();
    hits as f64 / retrieved.len() as f64
}

/// Fraction of relevant items that were retrieved.
pub fn recall(retrieved: &[String], relevant: &HashSet<String>) -> f64 {
    if relevant.is_empty() {
        return 1.0;
    }
    let hits = retrieved.iter().filter(|r| relevant.contains(*r)).count();
    hits as f64 / relevant.len() as f64
}

/// Harmonic mean of precision and recall.
pub fn f1(precision: f64, recall: f64) -> f64 {
    if precision + recall == 0.0 {
        return 0.0;
    }
    2.0 * precision * recall / (precision + recall)
}

/// Mean Reciprocal Rank: 1/rank of the first relevant result (1-indexed).
pub fn mrr(ranked: &[String], relevant: &HashSet<String>) -> f64 {
    for (i, item) in ranked.iter().enumerate() {
        if relevant.contains(item) {
            return 1.0 / (i as f64 + 1.0);
        }
    }
    0.0
}

/// Fraction of relevant items appearing in the top-k results.
pub fn recall_at_k(ranked: &[String], relevant: &HashSet<String>, k: usize) -> f64 {
    if relevant.is_empty() {
        return 1.0;
    }
    let top_k: HashSet<&String> = ranked.iter().take(k).collect();
    let hits = relevant.iter().filter(|r| top_k.contains(r)).count();
    hits as f64 / relevant.len() as f64
}

/// Normalized Discounted Cumulative Gain at position k.
///
/// `relevance_map` provides the relevance score for each item by name.
/// Items not in the map are treated as relevance 0.
pub fn ndcg_at_k(
    ranked: &[String],
    relevance_map: &std::collections::HashMap<String, f64>,
    k: usize,
) -> f64 {
    let dcg = ranked
        .iter()
        .take(k)
        .enumerate()
        .map(|(i, item)| {
            let rel = relevance_map.get(item).copied().unwrap_or(0.0);
            rel / (i as f64 + 2.0).log2()
        })
        .sum::<f64>();

    let mut ideal_rels: Vec<f64> = relevance_map.values().copied().collect();
    ideal_rels.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    let idcg: f64 = ideal_rels
        .iter()
        .take(k)
        .enumerate()
        .map(|(i, &rel)| rel / (i as f64 + 2.0).log2())
        .sum();

    if idcg == 0.0 {
        return 0.0;
    }
    dcg / idcg
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_precision_all_relevant() {
        let retrieved = vec!["a".into(), "b".into()];
        let relevant: HashSet<String> = ["a".into(), "b".into()].into();
        assert!((precision(&retrieved, &relevant) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_precision_none_relevant() {
        let retrieved = vec!["x".into(), "y".into()];
        let relevant: HashSet<String> = ["a".into(), "b".into()].into();
        assert!((precision(&retrieved, &relevant)).abs() < 1e-9);
    }

    #[test]
    fn test_recall_all_found() {
        let retrieved = vec!["a".into(), "b".into(), "c".into()];
        let relevant: HashSet<String> = ["a".into(), "b".into()].into();
        assert!((recall(&retrieved, &relevant) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_f1_balanced() {
        let score = f1(1.0, 1.0);
        assert!((score - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_mrr_first_relevant() {
        let ranked = vec!["x".into(), "a".into(), "b".into()];
        let relevant: HashSet<String> = ["a".into()].into();
        assert!((mrr(&ranked, &relevant) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_recall_at_k_partial() {
        let ranked = vec!["a".into(), "x".into(), "b".into()];
        let relevant: HashSet<String> = ["a".into(), "b".into()].into();
        assert!((recall_at_k(&ranked, &relevant, 2) - 0.5).abs() < 1e-9);
        assert!((recall_at_k(&ranked, &relevant, 3) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_ndcg_perfect_ranking() {
        let ranked = vec!["a".into(), "b".into()];
        let mut rel: HashMap<String, f64> = HashMap::new();
        rel.insert("a".into(), 2.0);
        rel.insert("b".into(), 1.0);
        let score = ndcg_at_k(&ranked, &rel, 2);
        assert!((score - 1.0).abs() < 1e-9);
    }
}
