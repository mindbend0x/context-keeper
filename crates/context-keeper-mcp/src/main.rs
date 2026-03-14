use anyhow::Result;
use tracing_subscriber::EnvFilter;

mod tools;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    tracing::info!("Starting Context Keeper MCP server");

    // TODO: Initialize SurrealDB client
    // TODO: Build MCP server with registered tool handlers:
    //   - add_memory
    //   - search_memory
    //   - expand_search
    //   - get_entity
    //   - snapshot
    //   - list_recent
    // TODO: Start stdio transport

    tracing::info!("Context Keeper MCP server stopped");
    Ok(())
}
