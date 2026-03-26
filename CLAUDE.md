# Context Keeper — Project Instructions

## Identity

Context Keeper is a temporal knowledge graph memory layer for AI agents. It gives MCP-compatible assistants persistent memory that tracks entities, relationships, and changes over time. Built in Rust with SurrealDB, Rig, and rmcp.

## Architecture

Five-crate workspace:

- **context-keeper-core** — Pure logic: data models, ingestion pipeline, hybrid search (RRF), temporal management, trait definitions. Zero heavyweight deps.
- **context-keeper-rig** — Rig framework integration: embeddings, LLM extraction, query rewriting. OpenAI-compatible endpoints.
- **context-keeper-surreal** — SurrealDB client: Repository with 35+ CRUD methods, vector/keyword search, graph traversal, temporal queries.
- **context-keeper-mcp** — MCP server binary: 6 tools, resources, prompts. Supports stdio + streamable HTTP transports.
- **context-keeper-cli** — Developer CLI: add, search, entity, recent commands.

Dependency flow: `cli/mcp → core ← rig → surreal`

## Key Technical Decisions

- **Trait-based decoupling**: Core defines `Embedder`, `EntityExtractor`, `RelationExtractor`, `QueryRewriter` traits. Rig implements them. Mock implementations exist for zero-config testing.
- **Hybrid search**: HNSW vector + BM25 keyword, fused via Reciprocal Rank Fusion (K=60).
- **Temporal graph**: Soft deletes via `valid_from`/`valid_until`. SurrealDB changefeeds for audit.
- **Storage backends**: RocksDB (default, `~/.context-keeper/data`), in-memory, remote (WIP).
- **Entity identity**: Currently name-only upsert key. ADR-001 recommends composite key (name + type/namespace) — implement this during FZ-12/FZ-13 work.

## Branch Strategy

- `feat/prototype-v1` — Current active development branch
- Feature branches off of `feat/prototype-v1` for individual issues

## Current Milestone: Prototype Public Release

See `docs/plans/` for detailed multi-level plans. The goal is a publishable GitHub release with:

1. Core correctness fixes (memory updates, entity dedup)
2. Architecture hardening (typed errors, retry logic)
3. Working examples and polished README
4. CI/CD pipeline
5. crates.io + Docker Hub publishing

## Development Workflow

### Building

```bash
cargo build                        # Full workspace
cargo build -p context-keeper-mcp  # MCP server only
cargo build -p context-keeper-cli  # CLI only
```

### Testing

```bash
cargo test                          # All tests (uses mock extractors, no API key needed)
cargo test -p context-keeper-test   # Integration tests only
```

### Running

```bash
# MCP server (stdio)
cargo run -p context-keeper-mcp

# MCP server (HTTP)
MCP_TRANSPORT=http MCP_HTTP_PORT=3000 cargo run -p context-keeper-mcp

# CLI
cargo run -p context-keeper-cli -- add --text "Alice works at Acme" --source "chat"
cargo run -p context-keeper-cli -- search --query "Who works at Acme?"

# Docker
docker compose up
```

### Environment Variables

```
OPENAI_API_URL     — OpenAI-compatible endpoint
OPENAI_API_KEY     — API key (omit for mock extraction)
EMBEDDING_MODEL    — e.g., text-embedding-3-small
EMBEDDING_DIMS     — e.g., 1536
EXTRACTION_MODEL   — e.g., gpt-4o-mini
STORAGE_BACKEND    — rocksdb (default), memory, remote:<url>
MCP_TRANSPORT      — stdio (default), http
MCP_HTTP_PORT      — default 3000
```

## Code Style

- Use `thiserror` for error types (migrating from `anyhow` — see ADR-001 R3)
- Async everywhere (`tokio` runtime)
- Traits in core, implementations in rig/surreal
- Tests should work without API keys using mock implementations
- SurrealQL queries are hand-constructed with parameter binding

## Key Files

- `crates/context-keeper-core/src/ingestion/pipeline.rs` — Main ingestion logic
- `crates/context-keeper-core/src/search/engine.rs` — RRF fusion search
- `crates/context-keeper-rig/src/extraction.rs` — LLM entity/relation extraction
- `crates/context-keeper-surreal/src/repository.rs` — All DB operations (~700 lines)
- `crates/context-keeper-mcp/src/tools.rs` — 6 MCP tool implementations
- `test/tests/` — 5 integration test suites

## Linear Project

Team: FZ | Project: Context Keeper | Key: FZ-*
Milestones: Base lib (done) → Efficacy & Correctness (active) → Plugins and Connectors → Privacy, Security and Package



## Other Considerations

### Releases and Open-Source

The main aspects of the project that are meant to be released and open-sourced are the crates in [/crates](./crates) and documentation in [/docs](./docs).

Items such as the website are not mentioned here yet.

