use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
#[command(name = "ck-bench", about = "Benchmark runner for Context Keeper")]
struct Cli {
    /// Path to the benchmark config YAML file.
    /// Not required when --from-json is used.
    #[arg(short, long, required_unless_present = "from_json")]
    config: Option<PathBuf>,

    /// Load results from a prior JSON output file instead of running a benchmark.
    /// Use with --html to regenerate a dashboard without making any LLM calls.
    ///
    /// Example: ck-bench --from-json results.json --html report.html
    #[arg(long)]
    from_json: Option<PathBuf>,

    /// Load an external dataset. Format: locomo:/path/to/file.json or longmemeval:/path/to/file.json
    #[arg(short, long)]
    dataset: Option<String>,

    /// When using --dataset longmemeval, only load temporal reasoning questions.
    #[arg(long, default_value_t = false)]
    temporal_only: bool,

    /// Write JSON results to this file.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Suppress the ASCII table and only write JSON output.
    #[arg(long, default_value_t = false)]
    json_only: bool,

    /// Skip LLM-as-Judge answer evaluation (faster, no extra LLM calls for scoring).
    #[arg(long, default_value_t = false)]
    no_judge: bool,

    /// Generate a self-contained HTML dashboard at this path.
    #[arg(long)]
    html: Option<PathBuf>,

    /// Previous JSON result files for the trend chart (repeatable).
    #[arg(long = "history")]
    history: Vec<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    let results = if let Some(from_json) = &cli.from_json {
        tracing::info!(path = %from_json.display(), "Loading results from JSON (skipping benchmark run)");
        let raw = std::fs::read_to_string(from_json)
            .map_err(|e| anyhow::anyhow!("Failed to read --from-json file: {e}"))?;
        serde_json::from_str::<Vec<context_keeper_bench::metrics::ScenarioResult>>(&raw)
            .map_err(|e| anyhow::anyhow!("Failed to parse --from-json file: {e}"))?
    } else {
        let config_path = cli.config.as_ref().unwrap();
        tracing::info!(config = %config_path.display(), "Loading benchmark config");
        let mut config = context_keeper_bench::config::load_config(config_path)?;

        if let Some(dataset_spec) = &cli.dataset {
            let (dtype, path) = dataset_spec.split_once(':').ok_or_else(|| {
                anyhow::anyhow!("--dataset must be type:path (e.g. locomo:data.json)")
            })?;

            let path = std::path::Path::new(path);
            let scenarios = match dtype {
                "locomo" => context_keeper_bench::datasets::locomo::load(path)?,
                "longmemeval" if cli.temporal_only => {
                    context_keeper_bench::datasets::longmemeval::load_temporal_subset(path)?
                }
                "longmemeval" => context_keeper_bench::datasets::longmemeval::load(path)?,
                other => {
                    anyhow::bail!("Unknown dataset type '{other}'. Use 'locomo' or 'longmemeval'.")
                }
            };

            tracing::info!(
                dataset = dtype,
                scenarios = scenarios.len(),
                "Loaded external dataset"
            );
            config.scenarios.extend(scenarios);
        }

        tracing::info!(
            providers = config.providers.len(),
            scenarios = config.scenarios.len(),
            "Starting benchmark run"
        );

        context_keeper_bench::runner::run(&config, !cli.no_judge).await
    };

    if !cli.json_only {
        context_keeper_bench::report::print_report(&results);
    }

    if let Some(output_path) = &cli.output {
        let json = context_keeper_bench::report::to_json(&results)?;
        std::fs::write(output_path, &json)?;
        tracing::info!(path = %output_path.display(), "JSON report written");
    } else if cli.json_only {
        let json = context_keeper_bench::report::to_json(&results)?;
        println!("{json}");
    }

    if let Some(html_path) = &cli.html {
        let history_refs: Vec<&std::path::Path> = cli.history.iter().map(|p| p.as_path()).collect();
        let html = context_keeper_bench::report::to_html(&results, &history_refs)?;
        std::fs::write(html_path, &html)?;
        tracing::info!(path = %html_path.display(), "HTML dashboard written");
    }

    Ok(())
}
