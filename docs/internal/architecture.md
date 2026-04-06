# Architecture

See the top-level [README.md](../README.md) for the high-level architecture diagram.

## Crate Dependency Graph

```
context-keeper-cli ──┐
context-keeper-mcp ──┤
                     ├─► context-keeper-core
                     │        ▲
context-keeper-rig ──┘        │
        │                     │
        └─► context-keeper-surreal
```

### Crate Roles

- **core** — Data models (`Episode`, `Entity`, `Memory`, `Relation`), ingestion pipeline, temporal manager, hybrid search engine with RRF fusion, query expansion, and trait definitions (`Embedder`, `EntityExtractor`, `RelationExtractor`, `QueryRewriter`).
- **rig** — [Rig](https://rig.rs) framework integration: `RigEmbedder` for vector embeddings, `RigEntityExtractor` / `RigRelationExtractor` for LLM-powered graph extraction, and `RigQueryRewriter` for search query expansion via structured LLM output.
- **surreal** — SurrealDB client, schema management, `Repository` (CRUD + search), storage backend abstraction (`Memory`, `RocksDb`, `Remote`).
- **mcp** — MCP server binary. Implements the full MCP capabilities surface (tools, resources, prompts) over stdio and streamable HTTP transports using `rmcp` + `axum`.
- **cli** — Developer CLI for local testing and scripting (`add`, `search`, `entity`, `recent` commands).

## MCP Server Architecture

The MCP server (`crates/context-keeper-mcp/`) is the primary integration point for AI assistants.

### Capabilities

The server advertises three MCP capability types:

**Tools** — Six callable operations:
`add_memory`, `search_memory`, `expand_search`, `get_entity`, `snapshot`, `list_recent`.
See [docs/mcp.md](mcp.md) for the full reference.

**Resources** — Browsable data exposed to clients:
- `memory://recent` — Static resource listing the 20 most recent memories.
- `memory://entity/{name}` — Dynamic per-entity resources auto-generated from all active entities. Clients with resource support can browse the knowledge graph without tool calls.
- A URI template (`memory://entity/{name}`) is registered so clients can resolve arbitrary entity lookups.

**Prompts** — Three prompt templates (`summarize-topic`, `what-changed`, `add-context`) that compose tool calls into guided workflows.

### Transport Layer

The server supports two transport modes, selected at startup via `--transport`:

1. **stdio** (default) — JSON-RPC over stdin/stdout. The MCP client (Claude Desktop, Cursor) spawns the binary and manages its lifecycle. Uses `rmcp::transport::stdio()`.
2. **Streamable HTTP** — An `axum` HTTP server at `/mcp` using `rmcp::transport::StreamableHttpService` with session management. Suitable for Docker, remote, or multi-client setups.

### Initialization Flow

1. Parse CLI args / environment (via `clap` + `dotenv`)
2. Connect to SurrealDB (RocksDB, in-memory, or remote)
3. Apply schema and optionally import persisted data
4. Build LLM services (real via Rig, or mock fallbacks)
5. Construct `ContextKeeperServer` with `Repository` + trait objects
6. Serve over selected transport

### Request Flow (tool call)

```
MCP Client → transport (stdio/HTTP) → rmcp ToolRouter → ContextKeeperServer method
  → core ingestion / search / temporal logic
    → rig (embeddings, extraction, rewriting)
    → surreal Repository (SurrealDB queries)
  → JSON response → transport → MCP Client
```

## Data Flow

### Ingestion (`add_memory`)

1. Create an `Episode` from input text
2. Extract entities via `EntityExtractor` (LLM or mock)
3. Extract relations via `RelationExtractor`
4. Generate embeddings via `Embedder`
5. Persist episode, entities (UPSERT by name), relations, and memories to SurrealDB

### Search (`search_memory` / `expand_search`)

1. Embed the query via `Embedder`
2. Run HNSW vector search on entity embeddings
3. Run BM25 full-text search on entity names/summaries
4. Fuse results with Reciprocal Rank Fusion (RRF)
5. For `expand_search`: use `QueryRewriter` to generate semantic variants, search each, then fuse all ranked lists

### Temporal Queries (`snapshot`)

1. Parse the ISO 8601 timestamp
2. Query entities where `valid_from <= timestamp` and (`valid_until IS NULL` or `valid_until > timestamp`)
3. Query relations with the same temporal filter
4. Return the point-in-time graph state
