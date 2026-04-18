//! Context Keeper TUI — local SurrealDB or remote MCP (HTTP).
//!
//! Thin shim over [`context_keeper_tui::run`]. The same entrypoint is invoked
//! by `ctxk tui` in the main CLI.

use clap::Parser;
use context_keeper_surreal::default_storage_string;
use context_keeper_tui::{run, TuiConfig};
use dotenv::dotenv;

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

    #[arg(long, env = "STORAGE_BACKEND", global = true, default_value_t = default_storage_string())]
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenv();
    let cli = Cli::parse();

    let cfg = TuiConfig {
        embedding_model_name: cli.embedding_model_name,
        embedding_dims: cli.embedding_dims,
        extraction_model_name: cli.extraction_model_name,
        api_url: cli.api_url,
        api_key: cli.api_key,
        embedding_api_url: cli.embedding_api_url,
        embedding_api_key: cli.embedding_api_key,
        db_file_path: cli.db_file_path,
        storage: cli.storage,
        namespace: cli.namespace,
        agent_id: cli.agent_id,
        surreal_user: cli.surreal_user,
        surreal_pass: cli.surreal_pass,
        admin: cli.admin,
        debug_log: cli.debug_log,
        mcp_url: cli.mcp_url,
        mcp_token: cli.mcp_token,
        init_tracing: true,
    };

    run(cfg).await
}
