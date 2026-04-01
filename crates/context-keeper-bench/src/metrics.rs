use std::collections::HashSet;
use std::time::Duration;

use serde::Serialize;

use crate::config::Operation;
use crate::quality::{self, QualityMetrics};

/// Metrics for a single iteration of a benchmark scenario.
#[derive(Debug, Clone, Serialize)]
pub struct IterationMetrics {
    pub latency: Duration,
    pub entity_count: Option<usize>,
    pub relation_count: Option<usize>,
    pub memory_count: Option<usize>,
    pub error: Option<String>,
    pub quality: Option<QualityMetrics>,
    /// Entity names extracted in this iteration (used for consistency scoring).
    #[serde(skip)]
    pub extracted_entity_names: Option<Vec<String>>,
}

impl IterationMetrics {
    pub fn success(latency: Duration) -> Self {
        Self {
            latency,
            entity_count: None,
            relation_count: None,
            memory_count: None,
            error: None,
            quality: None,
            extracted_entity_names: None,
        }
    }

    pub fn failure(latency: Duration, error: String) -> Self {
        Self {
            latency,
            entity_count: None,
            relation_count: None,
            memory_count: None,
            error: Some(error),
            quality: None,
            extracted_entity_names: None,
        }
    }

    pub fn is_success(&self) -> bool {
        self.error.is_none()
    }
}

/// Aggregated results for one (scenario, provider) pair.
#[derive(Debug, Clone, Serialize)]
pub struct ScenarioResult {
    pub scenario_name: String,
    pub provider_name: String,
    pub operation: Operation,
    pub input_count: usize,
    pub iterations: Vec<IterationMetrics>,
    #[serde(skip)]
    aggregated: Option<AggregatedMetrics>,
}

impl ScenarioResult {
    pub fn new(
        scenario_name: String,
        provider_name: String,
        operation: Operation,
        input_count: usize,
        iterations: Vec<IterationMetrics>,
    ) -> Self {
        let mut result = Self {
            scenario_name,
            provider_name,
            operation,
            input_count,
            iterations,
            aggregated: None,
        };
        result.aggregated = Some(result.compute_aggregates());
        result
    }

    pub fn aggregated(&self) -> AggregatedMetrics {
        self.aggregated
            .clone()
            .unwrap_or_else(|| self.compute_aggregates())
    }

    fn compute_aggregates(&self) -> AggregatedMetrics {
        let successful: Vec<&IterationMetrics> =
            self.iterations.iter().filter(|i| i.is_success()).collect();

        let total = self.iterations.len();
        let success_count = successful.len();

        if successful.is_empty() {
            return AggregatedMetrics {
                total_iterations: total,
                successful_iterations: 0,
                success_rate: 0.0,
                mean_latency: Duration::ZERO,
                median_latency: Duration::ZERO,
                p95_latency: Duration::ZERO,
                min_latency: Duration::ZERO,
                max_latency: Duration::ZERO,
                avg_entity_count: None,
                avg_relation_count: None,
                avg_entity_f1: None,
                avg_entity_type_accuracy: None,
                avg_relation_f1: None,
                consistency: None,
            };
        }

        let mut latencies: Vec<Duration> = successful.iter().map(|i| i.latency).collect();
        latencies.sort();

        let total_latency: Duration = latencies.iter().sum();
        let mean = total_latency / success_count as u32;
        let median = latencies[success_count / 2];
        let p95_idx = ((success_count as f64) * 0.95).ceil() as usize;
        let p95 = latencies[p95_idx.min(success_count - 1)];

        let avg_entity = avg_optional_usize(successful.iter().filter_map(|i| i.entity_count));
        let avg_relation = avg_optional_usize(successful.iter().filter_map(|i| i.relation_count));

        let avg_entity_f1 =
            avg_optional_f64(successful.iter().filter_map(|i| i.quality.as_ref().map(|q| q.entity_f1)));
        let avg_entity_type_accuracy = avg_optional_f64(
            successful
                .iter()
                .filter_map(|i| i.quality.as_ref().and_then(|q| q.entity_type_accuracy)),
        );
        let avg_relation_f1 = avg_optional_f64(
            successful
                .iter()
                .filter_map(|i| i.quality.as_ref().and_then(|q| q.relation_f1)),
        );

        let consistency = self.compute_consistency();

        AggregatedMetrics {
            total_iterations: total,
            successful_iterations: success_count,
            success_rate: success_count as f64 / total as f64,
            mean_latency: mean,
            median_latency: median,
            p95_latency: p95,
            min_latency: latencies[0],
            max_latency: latencies[success_count - 1],
            avg_entity_count: avg_entity,
            avg_relation_count: avg_relation,
            avg_entity_f1,
            avg_entity_type_accuracy,
            avg_relation_f1,
            consistency,
        }
    }

    /// Group successful iterations by input index, compute pairwise Jaccard,
    /// then average across all inputs.
    fn compute_consistency(&self) -> Option<f64> {
        if self.input_count == 0 {
            return None;
        }

        let successful: Vec<(usize, &IterationMetrics)> = self
            .iterations
            .iter()
            .enumerate()
            .filter(|(_, m)| m.is_success() && m.extracted_entity_names.is_some())
            .map(|(idx, m)| (idx % self.input_count, m))
            .collect();

        if successful.is_empty() {
            return None;
        }

        let mut per_input_scores = Vec::new();
        for input_idx in 0..self.input_count {
            let sets: Vec<HashSet<String>> = successful
                .iter()
                .filter(|(idx, _)| *idx == input_idx)
                .map(|(_, m)| {
                    m.extracted_entity_names
                        .as_ref()
                        .unwrap()
                        .iter()
                        .cloned()
                        .collect()
                })
                .collect();

            if let Some(score) = quality::consistency_score(&sets) {
                per_input_scores.push(score);
            }
        }

        avg_optional_f64(per_input_scores.into_iter())
    }
}

/// Pre-computed summary statistics.
#[derive(Debug, Clone, Serialize)]
pub struct AggregatedMetrics {
    pub total_iterations: usize,
    pub successful_iterations: usize,
    pub success_rate: f64,
    pub mean_latency: Duration,
    pub median_latency: Duration,
    pub p95_latency: Duration,
    pub min_latency: Duration,
    pub max_latency: Duration,
    pub avg_entity_count: Option<f64>,
    pub avg_relation_count: Option<f64>,
    pub avg_entity_f1: Option<f64>,
    pub avg_entity_type_accuracy: Option<f64>,
    pub avg_relation_f1: Option<f64>,
    pub consistency: Option<f64>,
}

fn avg_optional_usize(iter: impl Iterator<Item = usize>) -> Option<f64> {
    let vals: Vec<usize> = iter.collect();
    if vals.is_empty() {
        return None;
    }
    let sum: usize = vals.iter().sum();
    Some(sum as f64 / vals.len() as f64)
}

fn avg_optional_f64(iter: impl Iterator<Item = f64>) -> Option<f64> {
    let vals: Vec<f64> = iter.collect();
    if vals.is_empty() {
        return None;
    }
    let sum: f64 = vals.iter().sum();
    Some(sum / vals.len() as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregation_with_all_success() {
        let iters = vec![
            IterationMetrics {
                latency: Duration::from_millis(100),
                entity_count: Some(3),
                relation_count: Some(2),
                memory_count: Some(1),
                error: None,
                quality: None,
                extracted_entity_names: None,
            },
            IterationMetrics {
                latency: Duration::from_millis(200),
                entity_count: Some(5),
                relation_count: Some(4),
                memory_count: Some(1),
                error: None,
                quality: None,
                extracted_entity_names: None,
            },
        ];

        let result = ScenarioResult::new(
            "test".into(),
            "provider".into(),
            Operation::EntityExtraction,
            1,
            iters,
        );
        let agg = result.aggregated();
        assert_eq!(agg.total_iterations, 2);
        assert_eq!(agg.successful_iterations, 2);
        assert!((agg.success_rate - 1.0).abs() < f64::EPSILON);
        assert_eq!(agg.mean_latency, Duration::from_millis(150));
        assert_eq!(agg.min_latency, Duration::from_millis(100));
        assert_eq!(agg.max_latency, Duration::from_millis(200));
        assert!((agg.avg_entity_count.unwrap() - 4.0).abs() < f64::EPSILON);
    }

    #[test]
    fn aggregation_with_failures() {
        let iters = vec![
            IterationMetrics::success(Duration::from_millis(50)),
            IterationMetrics::failure(Duration::from_millis(10), "boom".into()),
        ];
        let result = ScenarioResult::new(
            "test".into(),
            "prov".into(),
            Operation::Ingestion,
            1,
            iters,
        );
        let agg = result.aggregated();
        assert_eq!(agg.successful_iterations, 1);
        assert!((agg.success_rate - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn aggregation_all_failures() {
        let iters = vec![IterationMetrics::failure(
            Duration::from_millis(5),
            "err".into(),
        )];
        let result = ScenarioResult::new(
            "test".into(),
            "prov".into(),
            Operation::Search,
            1,
            iters,
        );
        let agg = result.aggregated();
        assert_eq!(agg.successful_iterations, 0);
        assert!(agg.mean_latency.is_zero());
    }

    #[test]
    fn aggregation_with_quality_metrics() {
        let make_iter = |f1_val: f64, names: Vec<String>| -> IterationMetrics {
            IterationMetrics {
                latency: Duration::from_millis(100),
                entity_count: Some(2),
                relation_count: None,
                memory_count: None,
                error: None,
                quality: Some(QualityMetrics {
                    entity_precision: 1.0,
                    entity_recall: f1_val,
                    entity_f1: f1_val,
                    entity_type_accuracy: Some(0.8),
                    relation_precision: None,
                    relation_recall: None,
                    relation_f1: None,
                }),
                extracted_entity_names: Some(names),
            }
        };

        let iters = vec![
            make_iter(0.8, vec!["Alice".into(), "Bob".into()]),
            make_iter(1.0, vec!["Alice".into(), "Bob".into()]),
        ];

        let result = ScenarioResult::new(
            "test".into(),
            "prov".into(),
            Operation::EntityExtraction,
            1,
            iters,
        );
        let agg = result.aggregated();

        assert!((agg.avg_entity_f1.unwrap() - 0.9).abs() < 1e-9);
        assert!((agg.avg_entity_type_accuracy.unwrap() - 0.8).abs() < 1e-9);
        assert!((agg.consistency.unwrap() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn consistency_across_different_entity_sets() {
        let make_iter = |names: Vec<String>| -> IterationMetrics {
            IterationMetrics {
                latency: Duration::from_millis(100),
                entity_count: Some(names.len()),
                relation_count: None,
                memory_count: None,
                error: None,
                quality: None,
                extracted_entity_names: Some(names),
            }
        };

        let iters = vec![
            make_iter(vec!["Alice".into(), "Bob".into()]),
            make_iter(vec!["Alice".into(), "Charlie".into()]),
        ];

        let result = ScenarioResult::new(
            "test".into(),
            "prov".into(),
            Operation::EntityExtraction,
            1,
            iters,
        );
        let agg = result.aggregated();
        // Jaccard({Alice,Bob}, {Alice,Charlie}) = 1/3
        assert!((agg.consistency.unwrap() - 1.0 / 3.0).abs() < 1e-9);
    }
}
