use anyhow::Result;
use context_keeper_surreal::{connect_memory, apply_schema, Repository, SurrealConfig};
use tracing_subscriber::EnvFilter;

mod tools;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    tracing::info!("Starting Context Keeper MCP server");

    // Initialize embedded in-memory SurrealDB
    let config = SurrealConfig::default();
    let db = connect_memory(&config).await?;
    apply_schema(&db, &config).await?;
    let _repo = Repository::new(db);

    tracing::info!("SurrealDB initialized, repository ready");

    // TODO: Build MCP server with registered tool handlers using
    // the official modelcontextprotocol/rust-sdk crate:
    //   - add_memory
    //   - search_memory
    //   - expand_search
    //   - get_entity
    //   - snapshot
    //   - list_recent
    // Each handler delegates to the Repository + core engine functions.
    //
    // Example (once mcp-sdk is integrated):
    // let transport = StdioTransport::new();
    // let server = Server::new(ServerConfig::new()
    //     .with_name("context-keeper")
    //     .with_version("0.1.0")
    //     .with_tool(add_memory_tool)
    //     .with_tool(search_memory_tool)
    //     ...);
    // server.start(transport)?;

    tracing::info!("Context Keeper MCP server stopped");
    Ok(())
}
