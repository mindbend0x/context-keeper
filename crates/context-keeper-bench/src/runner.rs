use std::time::Instant;

use crate::backend::BenchBackend;
use crate::ck_backend::ContextKeeperBackend;
use crate::config::{BenchConfig, BenchInput, Operation, ScenarioConfig};
use crate::metrics::{IterationMetrics, ScenarioResult};
use crate::quality;

/// Execute all scenarios against all providers, returning collected results.
pub async fn run(config: &BenchConfig) -> Vec<ScenarioResult> {
    let mut results = Vec::new();

    for provider in &config.providers {
        tracing::info!(provider = %provider.name, "Building backend");
        let backend = ContextKeeperBackend::from_provider(provider);

        for scenario in &config.scenarios {
            tracing::info!(
                scenario = %scenario.name,
                provider = %provider.name,
                operation = %scenario.operation,
                iterations = scenario.iterations,
                inputs = scenario.inputs.len(),
                "Running scenario"
            );

            let warmup = config.settings.warmup_iterations;
            if warmup > 0 {
                tracing::debug!(warmup, "Running warmup iterations");
                for _ in 0..warmup {
                    for input in &scenario.inputs {
                        let _ = run_single(&backend, scenario.operation, input).await;
                    }
                }
            }

            let mut iterations = Vec::with_capacity(scenario.iterations * scenario.inputs.len());
            for iter_idx in 0..scenario.iterations {
                for (input_idx, input) in scenario.inputs.iter().enumerate() {
                    tracing::debug!(
                        iteration = iter_idx + 1,
                        input = input_idx + 1,
                        "Running iteration"
                    );
                    let metrics = run_single(&backend, scenario.operation, input).await;
                    log_iteration(&metrics, iter_idx, input_idx);
                    iterations.push(metrics);
                }
            }

            results.push(ScenarioResult::new(
                scenario.name.clone(),
                provider.name.clone(),
                scenario.operation,
                scenario.inputs.len(),
                iterations,
            ));
        }
    }

    results
}

async fn run_single(
    backend: &dyn BenchBackend,
    operation: Operation,
    input: &BenchInput,
) -> IterationMetrics {
    let text = input.text();
    let start = Instant::now();

    let result = match operation {
        Operation::EntityExtraction => backend.entity_extraction(text).await.map(|out| {
            let mut m = IterationMetrics::success(start.elapsed());
            m.entity_count = Some(out.entities.len());
            m.extracted_entity_names =
                Some(out.entities.iter().map(|e| e.name.clone()).collect());

            if let Some(detailed) = input.as_detailed() {
                if !detailed.expected_entities.is_empty() {
                    m.quality = Some(quality::score_entities(&out.entities, detailed));
                }
            }
            m
        }),
        Operation::RelationExtraction => backend.relation_extraction(text).await.map(|out| {
            let mut m = IterationMetrics::success(start.elapsed());
            m.entity_count = Some(out.entities.len());
            m.relation_count = Some(out.relations.len());
            m.extracted_entity_names =
                Some(out.entities.iter().map(|e| e.name.clone()).collect());

            if let Some(detailed) = input.as_detailed() {
                if !detailed.expected_entities.is_empty()
                    || !detailed.expected_relations.is_empty()
                {
                    m.quality = Some(quality::score_extraction(
                        &out.entities,
                        &out.relations,
                        detailed,
                    ));
                }
            }
            m
        }),
        Operation::Ingestion => backend.ingestion(text, "bench").await.map(|out| {
            let mut m = IterationMetrics::success(start.elapsed());
            m.entity_count = Some(out.entity_count);
            m.relation_count = Some(out.relation_count);
            m.memory_count = Some(out.memory_count);
            m
        }),
        Operation::Search => backend.search(text).await.map(|_| {
            IterationMetrics::success(start.elapsed())
        }),
        Operation::QueryRewriting => backend.query_rewrite(text).await.map(|_| {
            IterationMetrics::success(start.elapsed())
        }),
    };

    match result {
        Ok(metrics) => metrics,
        Err(e) => IterationMetrics::failure(start.elapsed(), e.to_string()),
    }
}

fn log_iteration(metrics: &IterationMetrics, iter_idx: usize, input_idx: usize) {
    if metrics.is_success() {
        tracing::info!(
            iteration = iter_idx + 1,
            input = input_idx + 1,
            latency_ms = metrics.latency.as_millis(),
            entities = ?metrics.entity_count,
            relations = ?metrics.relation_count,
            entity_f1 = ?metrics.quality.as_ref().map(|q| q.entity_f1),
            "Iteration complete"
        );
    } else {
        tracing::warn!(
            iteration = iter_idx + 1,
            input = input_idx + 1,
            latency_ms = metrics.latency.as_millis(),
            error = metrics.error.as_deref().unwrap_or("unknown"),
            "Iteration failed"
        );
    }
}

/// Run a single scenario with a pre-built backend (useful for programmatic use).
pub async fn run_scenario(
    backend: &dyn BenchBackend,
    scenario: &ScenarioConfig,
    warmup: usize,
) -> ScenarioResult {
    for _ in 0..warmup {
        for input in &scenario.inputs {
            let _ = run_single(backend, scenario.operation, input).await;
        }
    }

    let mut iterations = Vec::with_capacity(scenario.iterations * scenario.inputs.len());
    for iter_idx in 0..scenario.iterations {
        for (input_idx, input) in scenario.inputs.iter().enumerate() {
            let metrics = run_single(backend, scenario.operation, input).await;
            log_iteration(&metrics, iter_idx, input_idx);
            iterations.push(metrics);
        }
    }

    ScenarioResult::new(
        scenario.name.clone(),
        backend.name().to_string(),
        scenario.operation,
        scenario.inputs.len(),
        iterations,
    )
}
