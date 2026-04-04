use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use axum::{extract::Request, middleware, response::Response, routing::{get, post}};
use clap::Parser;
use context_keeper_core::traits::*;
use context_keeper_rig::{
    embeddings::RigEmbedder,
    extraction::{RigEntityExtractor, RigRelationExtractor},
    rewriting::RigQueryRewriter,
};
use context_keeper_surreal::{
    apply_schema, connect, default_storage_string, parse_storage_backend, Repository,
    StorageBackend, SurrealConfig,
};
use dotenv::dotenv;
use rmcp::transport::streamable_http_server::tower::StreamableHttpServerConfig;
use rmcp::transport::{
    streamable_http_server::session::local::{LocalSessionManager, SessionConfig},
    StreamableHttpService,
};
use rmcp::ServiceExt;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::EnvFilter;

mod oauth;
mod tools;

use oauth::{OAuthConfig, OAuthStore};
use tools::ContextKeeperServer;

type LlmServiceStack = (
    Arc<dyn Embedder>,
    Arc<dyn EntityExtractor>,
    Arc<dyn RelationExtractor>,
    Arc<dyn QueryRewriter>,
);

#[derive(Parser)]
#[command(
    name = "context-keeper-mcp",
    about = "MCP server for Context Keeper (CTX.K) temporal knowledge graph"
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

    /// Override API URL for embeddings (falls back to OPENAI_API_URL)
    #[arg(long, env = "EMBEDDING_API_URL", global = true)]
    embedding_api_url: Option<String>,
    /// Override API key for embeddings (falls back to OPENAI_API_KEY)
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

    /// Storage backend: "rocksdb:<path>" (default: ~/.context-keeper/data), "memory", or "remote:<ws_url>"
    #[arg(long, env = "STORAGE_BACKEND", default_value_t = default_storage_string())]
    storage: String,

    /// Comma-separated list of valid bearer tokens for HTTP auth.
    /// Required when MCP_TRANSPORT=http unless MCP_ALLOW_INSECURE_HTTP=1 or MCP_OAUTH_ISSUER is set.
    #[arg(long, env = "MCP_AUTH_TOKENS")]
    auth_tokens: Option<String>,

    /// Bind address for HTTP transport (default: 127.0.0.1).
    /// Set to 0.0.0.0 to listen on all interfaces.
    #[arg(long, env = "MCP_HTTP_HOST", default_value = "127.0.0.1")]
    http_host: String,

    /// Allow running HTTP transport without auth tokens.
    /// Setting this acknowledges the endpoint will be unauthenticated.
    #[arg(long, env = "MCP_ALLOW_INSECURE_HTTP")]
    allow_insecure_http: bool,

    /// Public base URL of this server (e.g., https://mcp.example.com).
    /// Enables OAuth 2.1 authorization flow. When set, the server serves
    /// discovery endpoints and accepts dynamically registered OAuth clients.
    #[arg(long, env = "MCP_OAUTH_ISSUER")]
    oauth_issuer: Option<String>,

    /// SurrealDB root username (for remote connections)
    #[arg(long, env = "SURREAL_USER")]
    surreal_user: Option<String>,

    /// SurrealDB root password (for remote connections)
    #[arg(long, env = "SURREAL_PASS")]
    surreal_pass: Option<String>,
}

async fn bearer_auth(
    valid_tokens: Arc<Vec<String>>,
    req: Request,
    next: middleware::Next,
) -> Response {
    use subtle::ConstantTimeEq;

    if let Some(auth) = req.headers().get("authorization") {
        if let Ok(auth_str) = auth.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                let token_bytes = token.as_bytes();
                if valid_tokens
                    .iter()
                    .any(|t| t.as_bytes().ct_eq(token_bytes).into())
                {
                    return next.run(req).await;
                }
            }
        }
    }

    Response::builder()
        .status(401)
        .header("www-authenticate", "Bearer")
        .body(axum::body::Body::from("Unauthorized"))
        .unwrap()
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("context_keeper=info,warn")),
        )
        .init();

    let _ = dotenv();
    let cli = Cli::parse();

    tracing::info!("Starting Context Keeper MCP server");

    // Initialize SurrealDB
    let embedding_dims = cli.embedding_dims.unwrap_or(1536);
    let config = SurrealConfig {
        embedding_dimensions: embedding_dims,
        storage: parse_storage_backend(&cli.storage),
        username: cli.surreal_user,
        password: cli.surreal_pass,
        ..SurrealConfig::default()
    };

    if let StorageBackend::RocksDb(ref path) = config.storage {
        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::create_dir_all(path).ok();
    }

    let db = connect(&config).await?;
    apply_schema(&db, &config).await?;
    let repo = Repository::new(db);

    if matches!(config.storage, StorageBackend::Memory)
        && std::path::Path::new(&cli.db_file_path).exists()
    {
        repo.import_from_file(&cli.db_file_path).await?;
    }

    tracing::info!("SurrealDB initialized, repository ready");

    // Build LLM services
    let emb_api_url = cli.embedding_api_url.as_deref().or(cli.api_url.as_deref());
    let emb_api_key = cli.embedding_api_key.as_deref().or(cli.api_key.as_deref());

    let (embedder, entity_extractor, relation_extractor, query_rewriter): LlmServiceStack = match (
        cli.api_url.as_deref(),
        cli.api_key.as_deref(),
        cli.embedding_model_name.as_deref(),
        cli.extraction_model_name.as_deref(),
    ) {
        (Some(api_url), Some(api_key), Some(emb_model), Some(ext_model)) => {
            let emb_url = emb_api_url.unwrap_or(api_url);
            let emb_key = emb_api_key.unwrap_or(api_key);
            tracing::info!(
                extraction_url = api_url,
                embedding_url = emb_url,
                "Using LLM-powered extraction"
            );
            (
                Arc::new(RigEmbedder::new(
                    emb_url,
                    emb_key,
                    emb_model,
                    embedding_dims,
                )),
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

    let server = ContextKeeperServer::new(
        repo,
        embedder,
        entity_extractor,
        relation_extractor,
        query_rewriter,
    );

    let valid_tokens: Arc<Vec<String>> = Arc::new(
        cli.auth_tokens
            .as_deref()
            .unwrap_or("")
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
    );

    let oauth_enabled = cli.oauth_issuer.is_some();

    match cli.transport.as_str() {
        "stdio" => {
            tracing::info!("Serving MCP over stdio");
            let service = server.serve(rmcp::transport::stdio()).await?;
            service.waiting().await?;
        }
        "http" => {
            let has_auth =
                !valid_tokens.is_empty() || oauth_enabled || cli.allow_insecure_http;
            if !has_auth {
                anyhow::bail!(
                    "HTTP transport requires auth tokens (MCP_AUTH_TOKENS), \
                     OAuth (MCP_OAUTH_ISSUER), or explicit opt-in to insecure mode \
                     (MCP_ALLOW_INSECURE_HTTP=1). Refusing to start an unauthenticated HTTP endpoint."
                );
            }

            let bind_addr = format!("{}:{}", cli.http_host, cli.http_port);
            tracing::info!(addr = %bind_addr, "Serving MCP over streamable HTTP");

            let mut session_config = SessionConfig::default();
            session_config.keep_alive = Some(Duration::from_secs(300));

            let mut session_manager = LocalSessionManager::default();
            session_manager.session_config = session_config;

            let mut http_config = StreamableHttpServerConfig::default();
            http_config.sse_keep_alive = Some(Duration::from_secs(5));

            let http_service = StreamableHttpService::new(
                move || Ok(server.clone()),
                Arc::new(session_manager),
                http_config,
            );

            let router = if let Some(issuer) = cli.oauth_issuer {
                let issuer = issuer.trim_end_matches('/').to_string();
                let oauth_store = Arc::new(OAuthStore::new());
                let oauth_cfg = OAuthConfig {
                    issuer: issuer.clone(),
                    oauth_store: oauth_store.clone(),
                    static_tokens: valid_tokens.clone(),
                };

                let cors = CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods(Any)
                    .allow_headers(Any);

                let oauth_routes = axum::Router::new()
                    .route(
                        "/.well-known/oauth-protected-resource",
                        get(oauth::protected_resource_metadata),
                    )
                    .route(
                        "/.well-known/oauth-authorization-server",
                        get(oauth::authorization_server_metadata),
                    )
                    .route("/oauth/register", post(oauth::oauth_register))
                    .route("/oauth/token", post(oauth::oauth_token))
                    .layer(cors)
                    .with_state(oauth_cfg.clone());

                let authorize_routes = axum::Router::new()
                    .route("/oauth/authorize", get(oauth::oauth_authorize))
                    .route("/oauth/approve", post(oauth::oauth_approve))
                    .with_state(oauth_cfg.clone());

                let mcp_routes = axum::Router::new()
                    .nest_service("/mcp", http_service)
                    .layer(middleware::from_fn_with_state(
                        oauth_cfg.clone(),
                        oauth::unified_auth_middleware,
                    ));

                tracing::info!(
                    issuer = %issuer,
                    static_tokens = valid_tokens.len(),
                    "OAuth 2.1 + Bearer token auth enabled"
                );

                axum::Router::new()
                    .merge(oauth_routes)
                    .merge(authorize_routes)
                    .merge(mcp_routes)
            } else if valid_tokens.is_empty() {
                tracing::warn!(
                    "MCP_ALLOW_INSECURE_HTTP is set — HTTP endpoint has NO authentication. \
                     Do not expose this to untrusted networks."
                );
                axum::Router::new().nest_service("/mcp", http_service)
            } else {
                tracing::info!(count = valid_tokens.len(), "Bearer token auth enabled");
                let tokens = valid_tokens.clone();
                axum::Router::new()
                    .nest_service("/mcp", http_service)
                    .layer(middleware::from_fn(move |req, next| {
                        bearer_auth(tokens.clone(), req, next)
                    }))
            };

            let listener = tokio::net::TcpListener::bind(&bind_addr).await?;

            tracing::info!("MCP HTTP server ready at http://{}/mcp", bind_addr);

            axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    tokio::signal::ctrl_c().await.ok();
                })
                .await?;
        }
        other => {
            anyhow::bail!("Unknown transport: '{}'. Use 'stdio' or 'http'.", other);
        }
    }

    tracing::info!("Context Keeper MCP server stopped");
    Ok(())
}
