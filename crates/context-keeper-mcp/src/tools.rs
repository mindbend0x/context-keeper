//! MCP tool handler definitions for the Context Keeper server.
//!
//! Each tool corresponds to a capability exposed to MCP clients
//! (Claude Desktop, Cursor, etc.).

// TODO: Implement MCP tool handlers using the official rust-sdk:
//
// | Tool Name       | Description                                      |
// |-----------------|--------------------------------------------------|
// | add_memory      | Ingest a raw text episode into the graph          |
// | search_memory   | Hybrid search over entities and memories          |
// | expand_search   | Run query expansion + widened search              |
// | get_entity      | Fetch an entity with its full timeline            |
// | snapshot        | Query graph state at a specific timestamp         |
// | list_recent     | Return N most recently added memories             |
