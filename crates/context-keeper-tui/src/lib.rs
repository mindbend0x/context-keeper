//! Terminal UI and backends for Context Keeper.

pub mod backend;
pub mod bootstrap;
pub mod error;
pub mod types;
pub mod ui;

use std::fs::OpenOptions;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context as _;
use tracing_subscriber::EnvFilter;

#[cfg(feature = "remote-mcp")]
pub use backend::McpHttpBackend;
pub use backend::{LocalBackend, TuiBackend};

/// Configuration for launching the TUI event loop.
///
/// All fields mirror the historical standalone `context-keeper-tui` CLI flags
/// so that the `ctxk tui` subcommand and the standalone binary share a single
/// entrypoint.
#[derive(Debug, Default, Clone)]
pub struct TuiConfig {
    pub embedding_model_name: Option<String>,
    pub embedding_dims: Option<usize>,
    pub extraction_model_name: Option<String>,
    pub api_url: Option<String>,
    pub api_key: Option<String>,
    pub embedding_api_url: Option<String>,
    pub embedding_api_key: Option<String>,

    pub db_file_path: String,
    pub storage: String,

    pub namespace: Option<String>,
    pub agent_id: Option<String>,

    pub surreal_user: Option<String>,
    pub surreal_pass: Option<String>,

    /// Show Admin tab (namespaces, agents, cross-search, snapshot, activity).
    pub admin: bool,

    /// Append tracing logs to this file. The TUI always redirects tracing off
    /// stdout to avoid corrupting the alt-screen; `None` sends logs to a sink.
    pub debug_log: Option<PathBuf>,

    /// Use remote MCP streamable HTTP instead of local SurrealDB.
    pub mcp_url: Option<String>,

    /// Bearer token for MCP HTTP (`MCP_AUTH_TOKENS` on the server).
    pub mcp_token: Option<String>,

    /// If true, initialize a tracing subscriber inside [`run`]. Set to `false`
    /// when the caller (e.g. `ctxk`) already installed a global subscriber.
    pub init_tracing: bool,
}

/// Initialize tracing for the TUI. Always redirects off stdout.
pub fn init_tui_tracing(debug_log: Option<&std::path::Path>) -> anyhow::Result<()> {
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
            .try_init()
            .ok();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(std::io::sink)
            .try_init()
            .ok();
    }
    Ok(())
}

/// Build the appropriate backend (local SurrealDB or remote MCP HTTP) and run
/// the TUI event loop until the user quits.
pub async fn run(config: TuiConfig) -> anyhow::Result<()> {
    if config.init_tracing {
        init_tui_tracing(config.debug_log.as_deref())?;
    }

    let embedding_dims = config.embedding_dims.unwrap_or(1536);

    let backend: Arc<dyn TuiBackend> = if let Some(url) = config.mcp_url.clone() {
        #[cfg(feature = "remote-mcp")]
        {
            Arc::new(
                McpHttpBackend::connect(
                    url.as_str(),
                    config.mcp_token.as_deref(),
                    config.namespace.clone(),
                    config.agent_id.clone(),
                )
                .await?,
            )
        }
        #[cfg(not(feature = "remote-mcp"))]
        {
            let _ = (url, embedding_dims);
            anyhow::bail!(
                "Rebuild with `--features remote-mcp` to use --remote-mcp, or omit it for local mode."
            );
        }
    } else {
        Arc::new(
            bootstrap::build_local_backend(
                &config.storage,
                &config.db_file_path,
                embedding_dims,
                config.surreal_user.clone(),
                config.surreal_pass.clone(),
                config.namespace.clone(),
                config.agent_id.clone(),
                config.api_url.as_deref(),
                config.api_key.as_deref(),
                config.embedding_model_name.as_deref(),
                config.extraction_model_name.as_deref(),
                config.embedding_api_url.as_deref(),
                config.embedding_api_key.as_deref(),
            )
            .await?,
        )
    };

    ui::run_tui(backend, config.admin).await?;
    Ok(())
}
