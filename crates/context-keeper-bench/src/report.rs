use std::collections::HashMap;
use std::time::Duration;

use comfy_table::{modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL, Cell, Color, Table};

use crate::metrics::ScenarioResult;

/// Render results as a JSON string.
pub fn to_json(results: &[ScenarioResult]) -> serde_json::Result<String> {
    serde_json::to_string_pretty(results)
}

/// Print all report tables to stdout.
pub fn print_report(results: &[ScenarioResult]) {
    let regular: Vec<&ScenarioResult> = results.iter().filter(|r| r.behavioral.is_none()).collect();
    let behavioral: Vec<&ScenarioResult> = results.iter().filter(|r| r.behavioral.is_some()).collect();

    if !regular.is_empty() {
        let owned: Vec<ScenarioResult> = regular.into_iter().cloned().collect();
        print_table(&owned);
        print_comparison(&owned);
    }

    if !behavioral.is_empty() {
        print_behavioral(&behavioral);
    }
}

/// Print a human-readable summary table to stdout.
pub fn print_table(results: &[ScenarioResult]) {
    if results.is_empty() {
        println!("No benchmark results to display.");
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS);

    table.set_header(vec![
        "Scenario",
        "Provider",
        "Operation",
        "Inputs",
        "Iters",
        "OK",
        "Mean",
        "Median",
        "p95",
        "Min",
        "Max",
        "Avg Ent.",
        "Avg Rel.",
        "Ent F1",
        "Type Acc",
        "Rel F1",
        "Consist.",
    ]);

    for r in results {
        let agg = r.aggregated();

        let success_cell = if agg.success_rate >= 1.0 {
            Cell::new(format!(
                "{}/{}",
                agg.successful_iterations, agg.total_iterations
            ))
            .fg(Color::Green)
        } else if agg.success_rate > 0.0 {
            Cell::new(format!(
                "{}/{}",
                agg.successful_iterations, agg.total_iterations
            ))
            .fg(Color::Yellow)
        } else {
            Cell::new(format!(
                "{}/{}",
                agg.successful_iterations, agg.total_iterations
            ))
            .fg(Color::Red)
        };

        table.add_row(vec![
            Cell::new(&r.scenario_name),
            Cell::new(&r.provider_name),
            Cell::new(r.operation.to_string()),
            Cell::new(r.input_count),
            Cell::new(agg.total_iterations),
            success_cell,
            Cell::new(fmt_duration(agg.mean_latency)),
            Cell::new(fmt_duration(agg.median_latency)),
            Cell::new(fmt_duration(agg.p95_latency)),
            Cell::new(fmt_duration(agg.min_latency)),
            Cell::new(fmt_duration(agg.max_latency)),
            Cell::new(fmt_opt_f64(agg.avg_entity_count)),
            Cell::new(fmt_opt_f64(agg.avg_relation_count)),
            Cell::new(fmt_opt_pct(agg.avg_entity_f1)),
            Cell::new(fmt_opt_pct(agg.avg_entity_type_accuracy)),
            Cell::new(fmt_opt_pct(agg.avg_relation_f1)),
            Cell::new(fmt_opt_pct(agg.consistency)),
        ]);
    }

    println!("\n{table}\n");
}

/// Print a comparison table when multiple providers ran the same scenario.
///
/// The first provider for each scenario is treated as the baseline.
/// Deltas show latency as percentage change and F1 as absolute difference.
pub fn print_comparison(results: &[ScenarioResult]) {
    let mut by_scenario: HashMap<&str, Vec<&ScenarioResult>> = HashMap::new();
    for r in results {
        by_scenario.entry(&r.scenario_name).or_default().push(r);
    }

    let multi_provider: Vec<_> = by_scenario
        .into_values()
        .filter(|group| group.len() >= 2)
        .collect();

    if multi_provider.is_empty() {
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS);

    table.set_header(vec![
        "Scenario",
        "Provider",
        "vs Baseline",
        "Latency Delta",
        "Ent F1 Delta",
        "Rel F1 Delta",
    ]);

    for group in &multi_provider {
        let baseline = &group[0];
        let base_agg = baseline.aggregated();

        table.add_row(vec![
            Cell::new(&baseline.scenario_name),
            Cell::new(&baseline.provider_name),
            Cell::new("(baseline)").fg(Color::DarkCyan),
            Cell::new(fmt_duration(base_agg.mean_latency)),
            Cell::new(fmt_opt_pct(base_agg.avg_entity_f1)),
            Cell::new(fmt_opt_pct(base_agg.avg_relation_f1)),
        ]);

        for challenger in &group[1..] {
            let ch_agg = challenger.aggregated();

            let latency_delta = if base_agg.mean_latency.as_nanos() > 0 {
                let base_ms = base_agg.mean_latency.as_secs_f64() * 1000.0;
                let ch_ms = ch_agg.mean_latency.as_secs_f64() * 1000.0;
                let pct = ((ch_ms - base_ms) / base_ms) * 100.0;
                fmt_delta_pct(pct)
            } else {
                "-".to_string()
            };

            let ent_f1_delta = match (base_agg.avg_entity_f1, ch_agg.avg_entity_f1) {
                (Some(b), Some(c)) => fmt_delta_abs(c - b),
                _ => "-".to_string(),
            };

            let rel_f1_delta = match (base_agg.avg_relation_f1, ch_agg.avg_relation_f1) {
                (Some(b), Some(c)) => fmt_delta_abs(c - b),
                _ => "-".to_string(),
            };

            let latency_cell = if latency_delta.starts_with('-') {
                Cell::new(&latency_delta).fg(Color::Green)
            } else if latency_delta.starts_with('+') {
                Cell::new(&latency_delta).fg(Color::Red)
            } else {
                Cell::new(&latency_delta)
            };

            let ent_cell = color_delta_cell(&ent_f1_delta, true);
            let rel_cell = color_delta_cell(&rel_f1_delta, true);

            table.add_row(vec![
                Cell::new(&challenger.scenario_name),
                Cell::new(&challenger.provider_name),
                Cell::new(""),
                latency_cell,
                ent_cell,
                rel_cell,
            ]);
        }
    }

    println!("Comparison (vs first provider per scenario):\n{table}\n");
}

/// Print a summary of behavioral scenario results.
pub fn print_behavioral(results: &[&ScenarioResult]) {
    if results.is_empty() {
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS);

    table.set_header(vec![
        "Scenario",
        "Provider",
        "Iters",
        "Checks",
        "Passed",
        "Pass Rate",
        "Total Time",
        "Errors",
    ]);

    for r in results {
        if let Some(beh) = &r.behavioral {
            let pass_rate = beh.pass_rate();
            let rate_cell = if pass_rate >= 1.0 {
                Cell::new(format!("{:.0}%", pass_rate * 100.0)).fg(Color::Green)
            } else if pass_rate > 0.5 {
                Cell::new(format!("{:.0}%", pass_rate * 100.0)).fg(Color::Yellow)
            } else {
                Cell::new(format!("{:.0}%", pass_rate * 100.0)).fg(Color::Red)
            };

            table.add_row(vec![
                Cell::new(&r.scenario_name),
                Cell::new(&r.provider_name),
                Cell::new(beh.iterations),
                Cell::new(beh.total_checks()),
                Cell::new(beh.passed_checks()),
                rate_cell,
                Cell::new(fmt_duration(beh.total_latency)),
                Cell::new(beh.errors.len()),
            ]);
        }
    }

    println!("\nBehavioral Scenarios:\n{table}\n");

    for r in results {
        if let Some(beh) = &r.behavioral {
            let failures: Vec<_> = beh
                .verifications
                .iter()
                .enumerate()
                .flat_map(|(iter_idx, checks)| {
                    checks
                        .iter()
                        .filter(|v| !v.pass)
                        .map(move |v| (iter_idx, v))
                })
                .collect();

            if !failures.is_empty() {
                println!("  {} — {} failures:", r.scenario_name, failures.len());
                for (iter_idx, v) in &failures {
                    println!(
                        "    iter {}: query={:?} missing={:?} unwanted={:?}",
                        iter_idx + 1,
                        v.query,
                        v.missing_expected,
                        v.found_unexpected
                    );
                }
                println!();
            }
        }
    }
}

fn color_delta_cell(text: &str, higher_is_better: bool) -> Cell {
    if text == "-" {
        return Cell::new(text);
    }
    let positive = text.starts_with('+');
    let negative = text.starts_with('-');
    if (higher_is_better && positive) || (!higher_is_better && negative) {
        Cell::new(text).fg(Color::Green)
    } else if (higher_is_better && negative) || (!higher_is_better && positive) {
        Cell::new(text).fg(Color::Red)
    } else {
        Cell::new(text)
    }
}

fn fmt_duration(d: Duration) -> String {
    let ms = d.as_millis();
    if ms == 0 {
        let us = d.as_micros();
        if us == 0 {
            return "0".to_string();
        }
        return format!("{us}us");
    }
    if ms < 1000 {
        return format!("{ms}ms");
    }
    let secs = d.as_secs_f64();
    format!("{secs:.2}s")
}

fn fmt_opt_f64(v: Option<f64>) -> String {
    match v {
        Some(val) => format!("{val:.1}"),
        None => "-".to_string(),
    }
}

fn fmt_opt_pct(v: Option<f64>) -> String {
    match v {
        Some(val) => format!("{:.0}%", val * 100.0),
        None => "-".to_string(),
    }
}

fn fmt_delta_pct(pct: f64) -> String {
    if pct >= 0.0 {
        format!("+{pct:.0}%")
    } else {
        format!("{pct:.0}%")
    }
}

fn fmt_delta_abs(delta: f64) -> String {
    if delta >= 0.0 {
        format!("+{:.2}", delta)
    } else {
        format!("{:.2}", delta)
    }
}
