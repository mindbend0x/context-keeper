use std::collections::{HashMap, HashSet};

use context_keeper_core::traits::{ExtractedEntity, ExtractedRelation};
use serde::Serialize;

use crate::config::{DetailedInput, ExpectedRelation};

/// Quality metrics for a single benchmark iteration.
#[derive(Debug, Clone, Serialize)]
pub struct QualityMetrics {
    pub entity_precision: f64,
    pub entity_recall: f64,
    pub entity_f1: f64,
    pub entity_type_accuracy: Option<f64>,
    pub relation_precision: Option<f64>,
    pub relation_recall: Option<f64>,
    pub relation_f1: Option<f64>,
}

// ── Primitive scoring functions ─────────────────────────────────────────

pub fn precision(retrieved: &[String], relevant: &HashSet<String>) -> f64 {
    if retrieved.is_empty() {
        return 0.0;
    }
    let hits = retrieved.iter().filter(|r| relevant.contains(*r)).count();
    hits as f64 / retrieved.len() as f64
}

pub fn recall(retrieved: &[String], relevant: &HashSet<String>) -> f64 {
    if relevant.is_empty() {
        return 1.0;
    }
    let hits = retrieved.iter().filter(|r| relevant.contains(*r)).count();
    hits as f64 / relevant.len() as f64
}

pub fn f1(precision: f64, recall: f64) -> f64 {
    if precision + recall == 0.0 {
        return 0.0;
    }
    2.0 * precision * recall / (precision + recall)
}

/// Fraction of extracted entities whose type matches the expected type.
/// Only scores entities that appear in both extracted and expected.
pub fn entity_type_accuracy(
    extracted: &[ExtractedEntity],
    expected_types: &HashMap<String, String>,
) -> Option<f64> {
    if expected_types.is_empty() {
        return None;
    }
    let mut matched = 0;
    let mut total = 0;
    for entity in extracted {
        if let Some(expected_type) = expected_types.get(&entity.name) {
            total += 1;
            if entity
                .entity_type
                .to_string()
                .eq_ignore_ascii_case(expected_type)
            {
                matched += 1;
            }
        }
    }
    if total == 0 {
        return None;
    }
    Some(matched as f64 / total as f64)
}

/// Relation quality: match on (subject, predicate, object) tuples.
/// Predicates are compared case-insensitively with underscore normalization.
fn relation_tuple(r: &ExpectedRelation) -> (String, String, String) {
    (
        r.subject.to_lowercase(),
        normalize_predicate(&r.predicate),
        r.object.to_lowercase(),
    )
}

fn extracted_relation_tuple(r: &ExtractedRelation) -> (String, String, String) {
    (
        r.subject.to_lowercase(),
        normalize_predicate(&r.predicate),
        r.object.to_lowercase(),
    )
}

fn normalize_predicate(p: &str) -> String {
    p.to_lowercase().replace(' ', "_")
}

pub fn relation_precision(
    extracted: &[ExtractedRelation],
    expected: &[ExpectedRelation],
) -> Option<f64> {
    if expected.is_empty() {
        return None;
    }
    if extracted.is_empty() {
        return Some(0.0);
    }
    let expected_set: HashSet<_> = expected.iter().map(relation_tuple).collect();
    let hits = extracted
        .iter()
        .filter(|r| expected_set.contains(&extracted_relation_tuple(r)))
        .count();
    Some(hits as f64 / extracted.len() as f64)
}

pub fn relation_recall(
    extracted: &[ExtractedRelation],
    expected: &[ExpectedRelation],
) -> Option<f64> {
    if expected.is_empty() {
        return None;
    }
    let extracted_set: HashSet<_> = extracted.iter().map(extracted_relation_tuple).collect();
    let hits = expected
        .iter()
        .filter(|r| extracted_set.contains(&relation_tuple(r)))
        .count();
    Some(hits as f64 / expected.len() as f64)
}

// ── Composite scoring ───────────────────────────────────────────────────

/// Score entity extraction quality against ground truth.
pub fn score_entities(extracted: &[ExtractedEntity], expected: &DetailedInput) -> QualityMetrics {
    let extracted_names: Vec<String> = extracted.iter().map(|e| e.name.clone()).collect();
    let expected_set: HashSet<String> = expected.expected_entities.iter().cloned().collect();

    let p = precision(&extracted_names, &expected_set);
    let r = recall(&extracted_names, &expected_set);
    let f = f1(p, r);
    let type_acc = entity_type_accuracy(extracted, &expected.expected_entity_types);

    QualityMetrics {
        entity_precision: p,
        entity_recall: r,
        entity_f1: f,
        entity_type_accuracy: type_acc,
        relation_precision: None,
        relation_recall: None,
        relation_f1: None,
    }
}

/// Score entity + relation extraction quality against ground truth.
pub fn score_extraction(
    extracted_entities: &[ExtractedEntity],
    extracted_relations: &[ExtractedRelation],
    expected: &DetailedInput,
) -> QualityMetrics {
    let mut qm = score_entities(extracted_entities, expected);

    let rp = relation_precision(extracted_relations, &expected.expected_relations);
    let rr = relation_recall(extracted_relations, &expected.expected_relations);

    qm.relation_precision = rp;
    qm.relation_recall = rr;
    qm.relation_f1 = match (rp, rr) {
        (Some(p), Some(r)) => Some(f1(p, r)),
        _ => None,
    };

    qm
}

// ── Consistency ─────────────────────────────────────────────────────────

/// Jaccard similarity between two string sets.
pub fn jaccard_similarity(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let intersection = a.intersection(b).count();
    let union = a.union(b).count();
    if union == 0 {
        return 1.0;
    }
    intersection as f64 / union as f64
}

/// Average pairwise Jaccard similarity across a set of entity-name sets.
/// Used to measure how consistently a model produces the same entities across iterations.
pub fn consistency_score(entity_sets: &[HashSet<String>]) -> Option<f64> {
    if entity_sets.len() < 2 {
        return None;
    }
    let mut total = 0.0;
    let mut count = 0;
    for i in 0..entity_sets.len() {
        for j in (i + 1)..entity_sets.len() {
            total += jaccard_similarity(&entity_sets[i], &entity_sets[j]);
            count += 1;
        }
    }
    if count == 0 {
        return None;
    }
    Some(total / count as f64)
}

// ── Answer-level scoring (for LoCoMo / LongMemEval benchmarks) ──────────

/// Tokenize text into lowercase words, stripping punctuation.
fn answer_tokens(text: &str) -> HashSet<String> {
    text.split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

/// Exact match after normalization (lowercase, strip whitespace).
pub fn answer_exact_match(gold: &str, predicted: &str) -> bool {
    let norm = |s: &str| {
        s.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    };
    norm(gold) == norm(predicted)
}

/// Token-level F1 between a gold answer and a predicted answer.
pub fn answer_f1(gold: &str, predicted: &str) -> f64 {
    let gold_tokens = answer_tokens(gold);
    let pred_tokens = answer_tokens(predicted);
    if gold_tokens.is_empty() || pred_tokens.is_empty() {
        return 0.0;
    }
    let overlap = gold_tokens.intersection(&pred_tokens).count() as f64;
    let p = overlap / pred_tokens.len() as f64;
    let r = overlap / gold_tokens.len() as f64;
    if p + r == 0.0 {
        0.0
    } else {
        2.0 * p * r / (p + r)
    }
}

/// Answer-level scoring result.
#[derive(Debug, Clone, Serialize)]
pub struct AnswerScore {
    pub exact_match: bool,
    pub f1: f64,
}

/// Compute answer score from a gold answer and predicted text.
pub fn score_answer(gold: &str, predicted: &str) -> AnswerScore {
    AnswerScore {
        exact_match: answer_exact_match(gold, predicted),
        f1: answer_f1(gold, predicted),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_keeper_core::models::EntityType;

    #[test]
    fn precision_all_relevant() {
        let retrieved = vec!["Alice".into(), "Bob".into()];
        let relevant: HashSet<String> = ["Alice".into(), "Bob".into()].into();
        assert!((precision(&retrieved, &relevant) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn precision_none_relevant() {
        let retrieved = vec!["X".into(), "Y".into()];
        let relevant: HashSet<String> = ["Alice".into()].into();
        assert!(precision(&retrieved, &relevant).abs() < 1e-9);
    }

    #[test]
    fn recall_all_found() {
        let retrieved = vec!["Alice".into(), "Bob".into(), "Extra".into()];
        let relevant: HashSet<String> = ["Alice".into(), "Bob".into()].into();
        assert!((recall(&retrieved, &relevant) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn f1_perfect() {
        assert!((f1(1.0, 1.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn f1_zero() {
        assert!(f1(0.0, 0.0).abs() < 1e-9);
    }

    #[test]
    fn entity_type_accuracy_all_correct() {
        let extracted = vec![
            ExtractedEntity {
                name: "Alice".into(),
                entity_type: EntityType::Person,
                summary: "".into(),
            },
            ExtractedEntity {
                name: "Acme".into(),
                entity_type: EntityType::Organization,
                summary: "".into(),
            },
        ];
        let mut expected = HashMap::new();
        expected.insert("Alice".into(), "person".into());
        expected.insert("Acme".into(), "organization".into());

        let acc = entity_type_accuracy(&extracted, &expected).unwrap();
        assert!((acc - 1.0).abs() < 1e-9);
    }

    #[test]
    fn entity_type_accuracy_half_correct() {
        let extracted = vec![
            ExtractedEntity {
                name: "Alice".into(),
                entity_type: EntityType::Person,
                summary: "".into(),
            },
            ExtractedEntity {
                name: "Acme".into(),
                entity_type: EntityType::Person, // wrong
                summary: "".into(),
            },
        ];
        let mut expected = HashMap::new();
        expected.insert("Alice".into(), "person".into());
        expected.insert("Acme".into(), "organization".into());

        let acc = entity_type_accuracy(&extracted, &expected).unwrap();
        assert!((acc - 0.5).abs() < 1e-9);
    }

    #[test]
    fn entity_type_accuracy_empty_expected() {
        let extracted = vec![ExtractedEntity {
            name: "Alice".into(),
            entity_type: EntityType::Person,
            summary: "".into(),
        }];
        assert!(entity_type_accuracy(&extracted, &HashMap::new()).is_none());
    }

    #[test]
    fn relation_scoring() {
        let extracted = vec![
            ExtractedRelation {
                subject: "Alice".into(),
                predicate: "works_at".into(),
                object: "Acme".into(),
                confidence: 90,
            },
            ExtractedRelation {
                subject: "Alice".into(),
                predicate: "knows".into(),
                object: "Bob".into(),
                confidence: 80,
            },
        ];
        let expected = vec![ExpectedRelation {
            subject: "Alice".into(),
            predicate: "works_at".into(),
            object: "Acme".into(),
        }];

        let p = relation_precision(&extracted, &expected).unwrap();
        assert!((p - 0.5).abs() < 1e-9); // 1 of 2 extracted is relevant

        let r = relation_recall(&extracted, &expected).unwrap();
        assert!((r - 1.0).abs() < 1e-9); // 1 of 1 expected is found
    }

    #[test]
    fn jaccard_identical_sets() {
        let a: HashSet<String> = ["A".into(), "B".into()].into();
        let b: HashSet<String> = ["A".into(), "B".into()].into();
        assert!((jaccard_similarity(&a, &b) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn jaccard_disjoint_sets() {
        let a: HashSet<String> = ["A".into()].into();
        let b: HashSet<String> = ["B".into()].into();
        assert!(jaccard_similarity(&a, &b).abs() < 1e-9);
    }

    #[test]
    fn jaccard_empty_sets() {
        let a: HashSet<String> = HashSet::new();
        let b: HashSet<String> = HashSet::new();
        assert!((jaccard_similarity(&a, &b) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn consistency_single_set_returns_none() {
        let sets = vec![["A".into()].into()];
        assert!(consistency_score(&sets).is_none());
    }

    #[test]
    fn consistency_identical_sets() {
        let a: HashSet<String> = ["A".into(), "B".into()].into();
        let sets = vec![a.clone(), a.clone(), a];
        let score = consistency_score(&sets).unwrap();
        assert!((score - 1.0).abs() < 1e-9);
    }

    #[test]
    fn consistency_partial_overlap() {
        let a: HashSet<String> = ["A".into(), "B".into()].into();
        let b: HashSet<String> = ["A".into(), "C".into()].into();
        let score = consistency_score(&[a, b]).unwrap();
        // Jaccard = 1/3
        assert!((score - 1.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn answer_exact_match_basic() {
        assert!(answer_exact_match("Acme Corp", "Acme Corp"));
        assert!(answer_exact_match("Acme Corp", "acme corp"));
        assert!(answer_exact_match(" Acme  Corp ", "acme corp"));
        assert!(!answer_exact_match("Acme Corp", "BigCo"));
    }

    #[test]
    fn answer_f1_perfect() {
        let score = answer_f1("Alice works at Acme", "Alice works at Acme");
        assert!((score - 1.0).abs() < 1e-9);
    }

    #[test]
    fn answer_f1_partial() {
        let score = answer_f1("Alice works at Acme", "Alice works at BigCo");
        // 3/4 overlap (alice, works, at) P=3/4 R=3/4 F1=3/4
        assert!((score - 0.75).abs() < 1e-9);
    }

    #[test]
    fn answer_f1_no_overlap() {
        let score = answer_f1("Alice", "Bob");
        assert!((score - 0.0).abs() < 1e-9);
    }
}
