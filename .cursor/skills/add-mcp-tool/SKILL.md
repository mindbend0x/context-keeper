---
name: add-mcp-tool
description: >-
  Add a new MCP tool to the Context Keeper server. Use when adding a tool,
  implementing a new MCP capability, or extending the server's tool surface.
---

# Add MCP Tool

## Steps

### 1. Define the Input Schema

In `crates/context-keeper-mcp/src/tools.rs`, add a new input struct near the other `*Input` types:

```rust
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MyToolInput {
    #[schemars(description = "Description shown to MCP clients")]
    pub field: String,
    #[schemars(description = "Optional field with default behavior")]
    pub limit: Option<usize>,
}
```

Every field needs a `#[schemars(description)]` — this is what the MCP client sees.

### 2. Define the Response Type (if needed)

Add a `#[derive(Debug, Serialize)]` struct if the response has structure beyond a simple string.

### 3. Implement the Tool Method

Add the method inside the `#[tool_router] impl ContextKeeperServer` block:

```rust
#[tool(description = "One-sentence description for the MCP tool listing")]
async fn my_tool(
    &self,
    Parameters(input): Parameters<MyToolInput>,
) -> Result<String, McpError> {
    // Use self.repo, self.embedder, etc.
    // Map errors with .map_err(|e| McpError::internal_error(format!("...: {e}"), None))
    // Return JSON string
    serde_json::to_string_pretty(&result)
        .map_err(|e| McpError::internal_error(format!("Serialization failed: {e}"), None))
}
```

### 4. Update Server Instructions (if significant)

If the tool adds a new capability category, update the `instructions` string in `get_info()` at the bottom of `tools.rs`.

### 5. Test

```bash
cargo build -p context-keeper-mcp
cargo test -p context-keeper-mcp
```

Then test with an MCP client (Claude Desktop, Cursor, or `mcp-cli`).

## Checklist

- [ ] Input struct with `schemars::JsonSchema` + descriptions on all fields
- [ ] `#[tool(description = "...")]` annotation with clear description
- [ ] Errors mapped to `McpError` variants (not raw `anyhow`)
- [ ] Response is serialized JSON
- [ ] Server instructions updated if tool adds new capability
