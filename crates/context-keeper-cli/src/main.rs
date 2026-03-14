use anyhow::Result;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // TODO: Parse CLI arguments (add, search, snapshot, etc.)
    // TODO: Initialize SurrealDB connection
    // TODO: Dispatch to appropriate command handler

    println!("Context Keeper CLI — use --help for usage");
    Ok(())
}
