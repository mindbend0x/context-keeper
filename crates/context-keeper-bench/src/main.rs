use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
#[command(name = "ck-bench", about = "Benchmark runner for Context Keeper")]
struct Cli {
    /// Path to the benchmark config YAML file.
    #[arg(short, long)]
    config: PathBuf,

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

    tracing::info!(config = %cli.config.display(), "Loading benchmark config");
    let mut config = context_keeper_bench::config::load_config(&cli.config)?;

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

    let results = context_keeper_bench::runner::run(&config).await;

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

    Ok(())
}
