use std::time::Instant;

use crate::backend::BenchBackend;
use crate::ck_backend::ContextKeeperBackend;
use crate::config::{BehavioralStep, BenchConfig, BenchInput, Operation, ScenarioConfig};
use crate::metrics::{BehavioralResult, IterationMetrics, ScenarioResult, StepVerification};
use crate::quality;

/// Execute all scenarios against all providers, returning collected results.
pub async fn run(config: &BenchConfig) -> Vec<ScenarioResult> {
    let mut results = Vec::new();

    for provider in &config.providers {
        tracing::info!(provider = %provider.name, "Building backend");
        let backend = ContextKeeperBackend::from_provider(provider);

        for scenario in &config.scenarios {
            if scenario.operation == Operation::Behavioral {
                tracing::info!(
                    scenario = %scenario.name,
                    provider = %provider.name,
                    steps = scenario.steps.len(),
                    iterations = scenario.iterations,
                    "Running behavioral scenario"
                );

                let behavioral = run_behavioral(&backend, scenario).await;
                results.push(ScenarioResult::new_behavioral(
                    scenario.name.clone(),
                    provider.name.clone(),
                    behavioral,
                ));
                continue;
            }

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

async fn run_behavioral(backend: &dyn BenchBackend, scenario: &ScenarioConfig) -> BehavioralResult {
    let mut all_verifications = Vec::new();
    let mut total_latency = std::time::Duration::ZERO;
    let mut errors = Vec::new();

    for iter_idx in 0..scenario.iterations {
        if let Err(e) = backend.reset().await {
            errors.push(format!("reset failed on iteration {}: {e}", iter_idx + 1));
            continue;
        }

        let mut iter_verifications = Vec::new();
        for (step_idx, step) in scenario.steps.iter().enumerate() {
            let start = Instant::now();
            match step {
                BehavioralStep::Ingest { text, source } => {
                    let src = source.as_deref().unwrap_or("bench");
                    match backend.ingestion(text, src).await {
                        Ok(_) => {
                            total_latency += start.elapsed();
                            tracing::debug!(
                                iteration = iter_idx + 1,
                                step = step_idx + 1,
                                "Ingested"
                            );
                        }
                        Err(e) => {
                            errors.push(format!(
                                "ingest failed at step {} iter {}: {e}",
                                step_idx + 1,
                                iter_idx + 1
                            ));
                        }
                    }
                }
                BehavioralStep::Search {
                    query,
                    expected_entities,
                    unexpected_entities,
                    gold_answer,
                } => match backend.search_with_text(query).await {
                    Ok((found_names, result_text)) => {
                        total_latency += start.elapsed();
                        let mut pass = true;

                        let missing: Vec<String> = expected_entities
                            .iter()
                            .filter(|e| !found_names.iter().any(|f| f.eq_ignore_ascii_case(e)))
                            .cloned()
                            .collect();

                        let unwanted: Vec<String> = unexpected_entities
                            .iter()
                            .filter(|e| found_names.iter().any(|f| f.eq_ignore_ascii_case(e)))
                            .cloned()
                            .collect();

                        if !missing.is_empty() || !unwanted.is_empty() {
                            pass = false;
                        }

                        let answer_score = gold_answer
                            .as_ref()
                            .map(|gold| crate::quality::score_answer(gold, &result_text));

                        tracing::info!(
                            iteration = iter_idx + 1,
                            step = step_idx + 1,
                            query = query,
                            found = ?found_names,
                            pass = pass,
                            missing = ?missing,
                            unwanted = ?unwanted,
                            answer_f1 = answer_score.as_ref().map(|s| s.f1),
                            "Search verification"
                        );

                        iter_verifications.push(StepVerification {
                            query: query.clone(),
                            found_entities: found_names,
                            missing_expected: missing,
                            found_unexpected: unwanted,
                            pass,
                            answer_score,
                        });
                    }
                    Err(e) => {
                        errors.push(format!(
                            "search failed at step {} iter {}: {e}",
                            step_idx + 1,
                            iter_idx + 1
                        ));
                    }
                },
            }
        }

        all_verifications.push(iter_verifications);
    }

    BehavioralResult {
        iterations: scenario.iterations,
        total_latency,
        verifications: all_verifications,
        errors,
    }
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
            m.extracted_entity_names = Some(out.entities.iter().map(|e| e.name.clone()).collect());

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
            m.extracted_entity_names = Some(out.entities.iter().map(|e| e.name.clone()).collect());

            if let Some(detailed) = input.as_detailed() {
                if !detailed.expected_entities.is_empty() || !detailed.expected_relations.is_empty()
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
        Operation::Search => backend
            .search(text)
            .await
            .map(|_| IterationMetrics::success(start.elapsed())),
        Operation::QueryRewriting => backend
            .query_rewrite(text)
            .await
            .map(|_| IterationMetrics::success(start.elapsed())),
        Operation::Behavioral => {
            unreachable!("behavioral scenarios are handled by run_behavioral()")
        }
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
