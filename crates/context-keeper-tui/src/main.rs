//! Context Keeper TUI — local SurrealDB or remote MCP (HTTP).

use std::fs::OpenOptions;
use std::sync::Arc;

use anyhow::Context as _;
use clap::Parser;
use context_keeper_tui::backend::TuiBackend;
use context_keeper_tui::bootstrap::{build_local_backend, default_storage};
use context_keeper_tui::ui::run_tui;
use dotenv::dotenv;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "context-keeper-tui", about = "Terminal UI for Context Keeper")]
struct Cli {
    #[arg(short = 'e', long, env = "EMBEDDING_MODEL", global = true)]
    embedding_model_name: Option<String>,
    #[arg(short = 'd', long, env = "EMBEDDING_DIMS", global = true)]
    embedding_dims: Option<usize>,
    #[arg(short = 'x', long, env = "EXTRACTION_MODEL", global = true)]
    extraction_model_name: Option<String>,
    #[arg(short = 'u', long, env = "OPENAI_API_URL", global = true)]
    api_url: Option<String>,
    #[arg(short = 'k', long, env = "OPENAI_API_KEY", global = true)]
    api_key: Option<String>,

    #[arg(long, env = "EMBEDDING_API_URL", global = true)]
    embedding_api_url: Option<String>,
    #[arg(long, env = "EMBEDDING_API_KEY", global = true)]
    embedding_api_key: Option<String>,

    #[arg(
        short = 'f',
        long,
        env = "DB_FILE_PATH",
        global = true,
        default_value = "context.sql"
    )]
    db_file_path: String,

    #[arg(long, env = "STORAGE_BACKEND", global = true, default_value_t = default_storage())]
    storage: String,

    #[arg(long, env = "CK_NAMESPACE", global = true)]
    namespace: Option<String>,

    #[arg(long, env = "CK_AGENT_ID", global = true)]
    agent_id: Option<String>,

    #[arg(long, env = "SURREAL_USER", global = true)]
    surreal_user: Option<String>,

    #[arg(long, env = "SURREAL_PASS", global = true)]
    surreal_pass: Option<String>,

    /// Show Admin tab (namespaces, agents, cross-search, snapshot, activity).
    #[arg(long, default_value_t = false)]
    admin: bool,

    /// Append tracing logs to this file (stdout logging would corrupt the TUI).
    #[arg(long, env = "CK_TUI_DEBUG_LOG")]
    debug_log: Option<std::path::PathBuf>,

    /// Use remote MCP streamable HTTP instead of local SurrealDB.
    #[arg(long, env = "CK_MCP_URL")]
    mcp_url: Option<String>,

    /// Bearer token for MCP HTTP (`MCP_AUTH_TOKENS` on the server).
    #[arg(long, env = "CK_MCP_TOKEN")]
    mcp_token: Option<String>,
}

fn init_tracing(debug_log: Option<&std::path::Path>) -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("context_keeper_tui=info,context_keeper=warn"));

    if let Some(path) = debug_log {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .with_context(|| format!("open debug log {}", path.display()))?;
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(file)
            .with_ansi(false)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(std::io::sink)
            .init();
    }
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenv();
    let cli = Cli::parse();
    init_tracing(cli.debug_log.as_deref())?;

    let embedding_dims = cli.embedding_dims.unwrap_or(1536);

    let backend: Arc<dyn TuiBackend> = if let Some(url) = cli.mcp_url.clone() {
        #[cfg(feature = "remote-mcp")]
        {
            Arc::new(
                context_keeper_tui::McpHttpBackend::connect(
                    url.as_str(),
                    cli.mcp_token.as_deref(),
                    cli.namespace.clone(),
                    cli.agent_id.clone(),
                )
                .await?,
            )
        }
        #[cfg(not(feature = "remote-mcp"))]
        {
            let _ = (url, embedding_dims);
            anyhow::bail!(
                "Rebuild with `--features remote-mcp` to use --mcp-url, or omit it for local mode."
            );
        }
    } else {
        Arc::new(
            build_local_backend(
                &cli.storage,
                &cli.db_file_path,
                embedding_dims,
                cli.surreal_user,
                cli.surreal_pass,
                cli.namespace,
                cli.agent_id,
                cli.api_url.as_deref(),
                cli.api_key.as_deref(),
                cli.embedding_model_name.as_deref(),
                cli.extraction_model_name.as_deref(),
                cli.embedding_api_url.as_deref(),
                cli.embedding_api_key.as_deref(),
            )
            .await?,
        )
    };

    run_tui(backend, cli.admin).await?;
    Ok(())
}
