use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use context_keeper_core::traits::*;
use context_keeper_rig::{
    embeddings::RigEmbedder,
    extraction::{RigEntityExtractor, RigRelationExtractor},
    rewriting::RigQueryRewriter,
};
use context_keeper_surreal::{apply_schema, connect, Repository, StorageBackend, SurrealConfig};
use dotenv::dotenv;
use rmcp::ServiceExt;
use tracing_subscriber::EnvFilter;

mod tools;
use tools::ContextKeeperServer;

#[derive(Parser)]
#[command(
    name = "context-keeper-mcp",
    about = "MCP server for Context Keeper temporal knowledge graph"
)]
struct Cli {
    /// Transport mode: "stdio" (default) or "http"
    #[arg(long, env = "MCP_TRANSPORT", default_value = "stdio")]
    transport: String,

    /// HTTP port (only used when transport is "http")
    #[arg(long, env = "MCP_HTTP_PORT", default_value = "3000")]
    http_port: u16,

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
    #[arg(short = 'f', long, env = "DB_FILE_PATH", global = true, default_value = "context.sql")]
    db_file_path: String,

    /// Storage backend: "memory" (default), "rocksdb:<path>", or "remote:<ws_url>"
    #[arg(long, env = "STORAGE_BACKEND", default_value = "memory")]
    storage: String,
}

fn parse_storage_backend(s: &str) -> StorageBackend {
    if let Some(path) = s.strip_prefix("rocksdb:") {
        StorageBackend::RocksDb(path.to_string())
    } else if let Some(url) = s.strip_prefix("remote:") {
        StorageBackend::Remote(url.to_string())
    } else {
        StorageBackend::Memory
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let _ = dotenv();
    let cli = Cli::parse();

    tracing::info!("Starting Context Keeper MCP server");

    // Initialize SurrealDB
    let embedding_dims = cli.embedding_dims.unwrap_or(1536);
    let config = SurrealConfig {
        embedding_dimensions: embedding_dims,
        storage: parse_storage_backend(&cli.storage),
        ..SurrealConfig::default()
    };

    let db = connect(&config).await?;
    apply_schema(&db, &config).await?;
    let repo = Repository::new(db);

    // Import existing data for memory backend
    if matches!(config.storage, StorageBackend::Memory) {
        if std::path::Path::new(&cli.db_file_path).exists() {
            repo.import_from_file(&cli.db_file_path).await?;
        }
    }

    tracing::info!("SurrealDB initialized, repository ready");

    // Build LLM services
    let (embedder, entity_extractor, relation_extractor, query_rewriter): (
        Arc<dyn Embedder>,
        Arc<dyn EntityExtractor>,
        Arc<dyn RelationExtractor>,
        Arc<dyn QueryRewriter>,
    ) = match (
        cli.api_url.as_deref(),
        cli.api_key.as_deref(),
        cli.embedding_model_name.as_deref(),
        cli.extraction_model_name.as_deref(),
    ) {
        (Some(api_url), Some(api_key), Some(emb_model), Some(ext_model)) => {
            tracing::info!("Using LLM-powered extraction");
            (
                Arc::new(RigEmbedder::new(api_url, api_key, emb_model, embedding_dims)),
                Arc::new(RigEntityExtractor::new(api_url, api_key, ext_model)),
                Arc::new(RigRelationExtractor::new(api_url, api_key, ext_model)),
                Arc::new(RigQueryRewriter::new(api_url, api_key, ext_model)),
            )
        }
        _ => {
            tracing::warn!("Missing LLM config — falling back to mock implementations");
            (
                Arc::new(MockEmbedder::new(embedding_dims)),
                Arc::new(MockEntityExtractor),
                Arc::new(MockRelationExtractor),
                Arc::new(MockQueryRewriter),
            )
        }
    };

    // Build MCP server
    let server = ContextKeeperServer::new(
        repo,
        embedder,
        entity_extractor,
        relation_extractor,
        query_rewriter,
    );

    match cli.transport.as_str() {
        "stdio" => {
            tracing::info!("Serving MCP over stdio");
            let service = server.serve(rmcp::transport::stdio()).await?;
            service.waiting().await?;
        }
        "http" => {
            tracing::info!(port = cli.http_port, "Serving MCP over streamable HTTP");
            // StreamableHttpService is a Tower service for axum/hyper.
            // For a simple standalone HTTP server, we use stdio for now
            // and recommend using an MCP proxy (rmcp-proxy) for HTTP access.
            tracing::warn!("HTTP transport requires an external HTTP framework integration. \
                           Use 'stdio' transport with rmcp-proxy for HTTP access, or run via Docker.");
            anyhow::bail!("Standalone HTTP transport not yet implemented. Use 'stdio' transport.");
        }
        other => {
            anyhow::bail!("Unknown transport: '{}'. Use 'stdio' or 'http'.", other);
        }
    }

    tracing::info!("Context Keeper MCP server stopped");
    Ok(())
}
