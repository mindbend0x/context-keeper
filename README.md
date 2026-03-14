Here is a comprehensive project plan for **Context Keeper**, a Rust-native temporal knowledge graph memory tool built as a spiritual successor to Graphiti on a completely different stack.

***

# Context Keeper — Full Project Plan

## Overview

Context Keeper is a high-performance, temporally-aware memory and knowledge graph tool for AI agents, built entirely in Rust. It replicates and extends the core capabilities of Graphiti  — real-time memory ingestion, entity/relationship extraction, temporal fact management, and hybrid search — but replaces Python/Neo4j/FalkorDB with Rust, Rig, and SurrealDB. It exposes its capabilities via an MCP server  and a Cursor IDE plugin. [presidio](https://www.presidio.com/technical-blog/graphiti-giving-ai-a-real-memory-a-story-of-temporal-knowledge-graphs/)

***

## Goals & Non-Goals

**Goals:**
- Store, update, and search episodic memories with full temporal awareness
- Extract entities and relationships from natural language using Rig-powered LLM calls [docs](https://docs.rs/rig-core)
- Perform hybrid search (vector + BM25 full-text + graph traversal) on a SurrealDB backend [surrealdb](https://surrealdb.com/docs/surrealdb/models/vector)
- Expand search queries automatically to improve recall
- Expose all features via MCP (for Claude, Cursor, etc.)  and as a Cursor plugin [github](https://github.com/conikeec/mcpr)

**Non-Goals:**
- Replacing a general-purpose database or message broker
- Supporting Python or non-Rust runtimes natively
- Providing a GUI dashboard (CLI + API only in v1)

***

## Architecture

The system is split into four logical layers:

```
┌─────────────────────────────────────────────────────┐
│               Interfaces Layer                      │
│    MCP Server (stdio/SSE)  │  Cursor Plugin (LSP)   │
└─────────────────┬───────────────────────────────────┘
                  │
┌─────────────────▼───────────────────────────────────┐
│             Core Engine (context-keeper-core)        │
│  Memory Ingestion │ Entity Extraction │ Search       │
│  Temporal Manager │ Query Expander    │ Graph Linker │
└─────────────────┬───────────────────────────────────┘
                  │
┌─────────────────▼───────────────────────────────────┐
│              Rig Integration Layer                   │
│   LLM Completions │ Embeddings │ Tool Definitions   │
└─────────────────┬───────────────────────────────────┘
                  │
┌─────────────────▼───────────────────────────────────┐
│              SurrealDB Storage Layer                 │
│  Nodes (Entities) │ Edges (Relations) │ Embeddings  │
│  Full-text Index  │ HNSW Vector Index │ Timestamps  │
└─────────────────────────────────────────────────────┘
```

***

## Data Model (SurrealDB)

SurrealDB serves as a combined graph, vector, and document database, eliminating the need for separate stores. [reddit](https://www.reddit.com/r/rust/comments/wt3ygg/surrealdb_a_new_scalable_documentgraph_database/)

**Tables:**
- `episode` — Raw input units (text, source, timestamp, session_id)
- `entity` — Extracted named entities with embedding vectors and `valid_from` / `valid_until` timestamps
- `relation` — Graph edges between entities with `relation_type`, confidence score, and temporal range
- `memory` — Distilled, searchable fact units with BM25 + HNSW vector indexes

**SurrealQL schema excerpt:**
```sql
DEFINE TABLE entity SCHEMAFULL;
DEFINE FIELD name        ON entity TYPE string;
DEFINE FIELD summary     ON entity TYPE string;
DEFINE FIELD embedding   ON entity TYPE array<float>;
DEFINE FIELD valid_from  ON entity TYPE datetime;
DEFINE FIELD valid_until ON entity TYPE option<datetime>;

DEFINE INDEX idx_entity_vec ON TABLE entity
  FIELDS embedding HNSW DIMENSION 1536 DIST COSINE;

DEFINE INDEX idx_entity_ft ON TABLE entity
  FIELDS name, summary FULLTEXT ANALYZER simple BM25;

DEFINE TABLE relation SCHEMAFULL;
DEFINE FIELD in          ON relation TYPE record<entity>;
DEFINE FIELD out         ON relation TYPE record<entity>;
DEFINE FIELD rel_type    ON relation TYPE string;
DEFINE FIELD valid_from  ON relation TYPE datetime;
DEFINE FIELD valid_until ON relation TYPE option<datetime>;
```

Temporal awareness is achieved natively: every fact carries `valid_from`/`valid_until` fields. When a conflicting fact arrives, the old edge's `valid_until` is set rather than deleted, preserving full history just as Graphiti does. [arxiv](https://arxiv.org/html/2501.13956v1)

***

## Core Engine Modules

### 1. Ingestion Pipeline (`ingestion/`)

Responsible for processing raw episodes into the graph. Steps:

1. **Chunk & normalize** the input text
2. **Entity extraction** — call Rig's `agent().prompt()` with a structured JSON schema output to extract `(entity, type)` pairs [docs](https://docs.rs/rig-core)
3. **Relation extraction** — a second LLM call extracts `(subject, predicate, object, confidence)` triplets
4. **Deduplication** — query SurrealDB for existing entities by name similarity; merge or create new nodes
5. **Temporal resolution** — compare new facts to existing edges; invalidate superseded edges by setting `valid_until = now()`
6. **Embedding** — call Rig's embedding pipeline to generate vectors for each entity and memory node [docs.rig](https://docs.rig.rs/docs/quickstart/embeddings)
7. **Persist** — write nodes, edges, and embeddings to SurrealDB in a single transaction

### 2. Temporal Manager (`temporal/`)

- Maintains a `valid_from`/`valid_until` timeline per entity and relation
- Provides a `snapshot(at: DateTime)` query mode — returns the graph state as it existed at a given time [surrealdb](https://surrealdb.com/static/whitepaper.pdf)
- On conflicting fact ingestion, auto-invalidates the stale edge and chains the new one
- Exposes a "fact staleness score" based on how long ago a memory was last confirmed

### 3. Search Engine (`search/`)

Implements a three-tier hybrid search — matching Graphiti's dual-path design but with SurrealDB's native `search::rrf()` fusion: [surrealdb](https://surrealdb.com/docs/surrealdb/models/vector)

- **Vector search** — HNSW cosine similarity on `embedding` field
- **Full-text search** — BM25 keyword match on entity name and summary
- **Graph traversal** — depth-first or BFS over relation edges from seed entities found in tier 1/2 [youtube](https://www.youtube.com/watch?v=n3SjFz6tFes)

Results are fused using Reciprocal Rank Fusion (RRF) natively in SurrealQL. [surrealdb](https://surrealdb.com/docs/surrealdb/models/vector)

### 4. Query Expander (`search/expander.rs`)

A key feature for ensuring good recall. When a raw query returns few results (below a configurable threshold):

1. LLM call via Rig rewrites the query into 3–5 semantic variants
2. All variants are embedded and searched in parallel (Tokio async)
3. Results are merged and re-ranked via RRF
4. If still insufficient, the expander widens to 2-hop graph neighbors of matched nodes

This mirrors Graphiti's "episode search expansion" but is fully composable via Rust traits.

***

## Rig Integration Layer

Rig  replaces LangChain and provides: [rig](https://rig.rs/index.html)

| Rig Feature | Context Keeper Usage |
|---|---|
| `agent().prompt()` with JSON schema | Entity + relation extraction |
| `EmbeddingsBuilder` | Vectorizing entities and memories  [docs.rig](https://docs.rig.rs/docs/quickstart/embeddings) |
| `Tool` + `ToolEmbedding` traits | Exposing `add_memory`, `search`, `expand_search` as agent-callable tools  [docs.rig](https://docs.rig.rs/docs/concepts/tools) |
| `VectorStoreIndex` trait | Wrapping SurrealDB's HNSW index for RAG-style retrieval  [docs](https://docs.rs/rig-core) |
| Streaming completions | Progressive memory summarization |

The `SurrealVectorStore` struct will implement Rig's `VectorStoreIndex` trait, registering it as a first-class Rig vector backend — similar to how `LanceDbVectorStore` is implemented. [dev](https://dev.to/0thtachi/build-a-fast-and-lightweight-rust-vector-search-app-with-rig-lancedb-57h2)

***

## MCP Server (`context-keeper-mcp/`)

The MCP server exposes Context Keeper's API as tool calls consumable by any MCP client (Claude Desktop, Cursor, etc.). [github](https://github.com/modelcontextprotocol/rust-sdk)

**Transport:** stdio (primary) + SSE (for remote/HTTP use)

**Exposed MCP Tools:**

| Tool Name | Description |
|---|---|
| `add_memory` | Ingest a raw text episode into the graph |
| `search_memory` | Hybrid search over entities and memories |
| `expand_search` | Run query expansion + widened search |
| `get_entity` | Fetch an entity with its full timeline |
| `snapshot` | Query graph state at a specific timestamp |
| `list_recent` | Return N most recently added memories |

The server is built using the official `modelcontextprotocol/rust-sdk` crate  and exposes a `ServerConfig` with all tools registered via `server.register_tool_handler(...)`. [github](https://github.com/conikeec/mcpr)

**Server startup:**
```rust
let transport = StdioTransport::new();
let mut server = Server::new(
    ServerConfig::new()
        .with_name("context-keeper")
        .with_version("0.1.0")
        .with_tool(add_memory_tool)
        .with_tool(search_memory_tool)
        // ...
);
server.start(transport)?;
```

***

## Cursor Plugin (`context-keeper-cursor/`)

The Cursor plugin wraps the MCP server as a locally running sidecar and adds IDE-specific UX:

- **Auto-capture** — optionally captures opened files, diffs, and inline comments as episodes
- **Sidebar panel** — shows a searchable list of recent memories in the Cursor sidebar
- **`@memory` mention** — in Cursor chat, typing `@memory <query>` triggers `search_memory` and injects results as context
- **Keybinding** — `Ctrl+Shift+M` opens a memory search palette

**Implementation approach:** The plugin is a VS Code extension (TypeScript) that spawns the `context-keeper-mcp` binary as a child process over stdio — no separate server process required. All MCP calls are made via the official MCP TypeScript SDK against the local binary.

***

## Project Structure

```
context-keeper/
├── crates/
│   ├── context-keeper-core/      # Ingestion, temporal, search, expander
│   ├── context-keeper-rig/       # Rig integration: embeddings, LLM calls, tools
│   ├── context-keeper-surreal/   # SurrealDB client, schema, VectorStoreIndex impl
│   ├── context-keeper-mcp/       # MCP server binary (stdio + SSE)
│   └── context-keeper-cli/       # Optional CLI for local dev/testing
├── plugins/
│   └── cursor/                   # VS Code/Cursor extension (TypeScript)
├── migrations/                   # SurrealDB schema migrations
├── examples/                     # Usage examples
├── docs/                         # Architecture docs
└── Cargo.toml                    # Workspace
```

***

## Key Dependencies

```toml
[dependencies]
rig-core       = "0.x"          # LLM + embeddings (Rig framework)
surrealdb      = "2.x"          # Database client
mcp-sdk        = { git = "https://github.com/modelcontextprotocol/rust-sdk" }
tokio          = { features = ["full"] }
serde          = { features = ["derive"] }
serde_json     = "1"
tracing        = "0.x"          # Structured logging
tracing-subscriber = "0.x"
chrono         = "0.x"          # Datetime handling for temporal fields
uuid           = { features = ["v4"] }
```

***

## Advantages Over Graphiti

| Dimension | Graphiti (Python) | Context Keeper (Rust) |
|---|---|---|
| Runtime | CPython, GIL-constrained | Tokio async, zero-cost concurrency |
| Graph DB | FalkorDB / Neo4j | SurrealDB (graph + vector + doc in one)  [reddit](https://www.reddit.com/r/rust/comments/wt3ygg/surrealdb_a_new_scalable_documentgraph_database/) |
| LLM Framework | LangChain | Rig — type-safe, compile-time correctness  [docs](https://docs.rs/rig-core) |
| Memory Safety | Runtime errors | Rust ownership model, no GC pauses |
| Deployment | Docker + separate DB | Single binary + embedded SurrealDB option |
| MCP Support | Via wrapper | Native first-class MCP server  [github](https://github.com/modelcontextprotocol/rust-sdk) |
