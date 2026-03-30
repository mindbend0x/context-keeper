# Context Keeper — Base Agent Instructions

You are working on **Context Keeper**, a temporal knowledge graph memory layer for AI agents. It gives MCP-compatible assistants persistent memory that tracks entities, relationships, and how they change over time. The stack is Rust, SurrealDB, Rig (AI framework), and rmcp (MCP SDK).

The project is open-source (MIT), hosted at `github.com/0x313/context-keeper`, and tracked in Linear under team **FZ**.

---

## Architecture

The repo is a Cargo workspace with five crates. Understand the dependency flow before making changes — it determines which crate your code belongs in.

```
context-keeper-cli ──┐
context-keeper-mcp ──┤
                     ├─► context-keeper-core (pure logic, traits, models)
                     │        ▲
context-keeper-rig ──┘        │
        │                     │
        └─► context-keeper-surreal (SurrealDB storage)
```

**context-keeper-core** — Data models (`Episode`, `Entity`, `Memory`, `Relation`), ingestion pipeline, hybrid search (RRF fusion), temporal management, and trait definitions (`Embedder`, `EntityExtractor`, `RelationExtractor`, `QueryRewriter`). This crate has zero heavyweight dependencies. If something is pure logic or a trait, it lives here.

**context-keeper-rig** — Implements core's traits using the Rig framework against OpenAI-compatible endpoints. Handles embeddings, LLM-based entity/relation extraction, and query rewriting.

**context-keeper-surreal** — SurrealDB client with 35+ CRUD methods, vector/keyword search, graph traversal, and temporal queries. All SurrealQL is hand-constructed with parameter binding.

**context-keeper-mcp** — MCP server binary exposing 6 tools, resources, and prompts. Supports stdio and streamable HTTP transports.

**context-keeper-cli** — Developer CLI with `add`, `search`, `entity`, and `recent` commands.

### Key design decisions

- **Trait-based decoupling.** Core defines the traits; Rig and Surreal implement them. This is intentional — don't pull LLM or DB dependencies into core. Mock implementations exist so the full test suite runs without API keys or a live database.
- **Hybrid search.** HNSW vector search + BM25 keyword search, fused via Reciprocal Rank Fusion (K=60). Both search paths run in SurrealDB.
- **Temporal graph.** Entities and relations use `valid_from`/`valid_until` for soft deletes. SurrealDB changefeeds provide an audit trail. Point-in-time snapshots are a first-class capability.
- **Storage backends.** RocksDB is the default (data at `~/.context-keeper/data`). In-memory exists for testing. Remote is WIP.
- **Entity identity.** Currently name-only upsert. ADR-001 recommends composite keys (name + type/namespace) — this is an active area of work.

---

## Key Files

When orienting yourself, start here:

| File | What it does |
|------|-------------|
| `crates/context-keeper-core/src/models.rs` | All data models |
| `crates/context-keeper-core/src/traits.rs` | Trait definitions (Embedder, extractors, etc.) |
| `crates/context-keeper-core/src/ingestion/pipeline.rs` | Main ingestion logic |
| `crates/context-keeper-core/src/search/engine.rs` | RRF fusion search |
| `crates/context-keeper-rig/src/extraction.rs` | LLM entity/relation extraction |
| `crates/context-keeper-surreal/src/repository.rs` | All DB operations (~700 lines) |
| `crates/context-keeper-mcp/src/tools.rs` | MCP tool implementations |
| `test/tests/` | Integration test suites |
| `docs/ADR-001-architecture-review.md` | Architecture review with risks and recommendations |
| `docs/plans/OVERVIEW.md` | Current milestone plan with Linear issue links |

---

## Development

### Build and test

```bash
cargo build                        # Full workspace
cargo build -p context-keeper-mcp  # Single crate
cargo test                         # All tests — no API key needed
cargo test -p context-keeper-test  # Integration tests only
```

### Run

```bash
# MCP server (stdio, default)
cargo run -p context-keeper-mcp

# MCP server (HTTP)
MCP_TRANSPORT=http MCP_HTTP_PORT=3000 cargo run -p context-keeper-mcp

# CLI
cargo run -p context-keeper-cli -- add --text "Alice works at Acme" --source "chat"
cargo run -p context-keeper-cli -- search --query "Who works at Acme?"
```

### Environment variables

| Variable | Purpose | Default |
|----------|---------|---------|
| `OPENAI_API_URL` | OpenAI-compatible endpoint | — |
| `OPENAI_API_KEY` | API key (omit for mock extraction) | — |
| `EMBEDDING_MODEL` | e.g., `text-embedding-3-small` | — |
| `EMBEDDING_DIMS` | e.g., `1536` | — |
| `EXTRACTION_MODEL` | e.g., `gpt-4o-mini` | — |
| `STORAGE_BACKEND` | `rocksdb`, `memory`, or `remote:<url>` | `rocksdb` |
| `MCP_TRANSPORT` | `stdio` or `http` | `stdio` |
| `MCP_HTTP_PORT` | HTTP transport port | `3000` |

---

## Code Style and Conventions

Follow these when writing or modifying code:

**Error handling.** Use `thiserror` for error types. The project is migrating away from `anyhow` toward a typed `ContextKeeperError` hierarchy (see ADR-001 R3). New code should define and use typed errors, not `anyhow::Result`.

**Async.** Everything is async on the `tokio` runtime. Don't introduce blocking calls on async paths.

**Crate boundaries.** Traits live in core. Implementations of those traits live in rig (for LLM-related) or surreal (for storage-related). The MCP and CLI crates are thin wiring layers — keep business logic out of them.

**Tests.** Tests must work without API keys. Use the mock implementations (`MockEmbedder`, `MockEntityExtractor`, etc.) for unit and integration tests. If a test needs a real LLM or database, gate it behind a feature flag or environment check, and document why.

**SurrealQL.** Queries are hand-constructed strings with parameter binding (`$param` syntax). Use parameterized queries to avoid injection. There's no query builder — keep queries readable and co-located with the repository method that uses them.

**Naming.** Rust conventions: `snake_case` for functions and variables, `PascalCase` for types and traits, `SCREAMING_SNAKE_CASE` for constants. Crate names use hyphens (`context-keeper-core`), module names use underscores.

---

## Git Workflow

- **Active branch:** `feat/prototype-v1` — all current development happens here or in feature branches off of it.
- **Feature branches:** Named `mindbend0x/fz-XX-description` following Linear convention, branched off `feat/prototype-v1`.
- **Commits:** Write clear commit messages. Reference the Linear issue (e.g., `FZ-59`) when applicable.
- **Before pushing:** Run `cargo test` and `cargo build` to make sure nothing is broken. Don't push code that doesn't compile.

---

## What's Being Built Right Now

The current milestone is **Prototype Public Release** — getting Context Keeper to a publishable v0.1.0 on GitHub, crates.io, and Docker Hub. The work is organized into levels:

- **Level 0 (Foundation):** Typed error hierarchy, LLM extraction retry/validation, composite entity identity.
- **Level 1 (Correctness):** Memory updates/negation detection, entity relationship quality, expanded entity types.
- **Level 2 (Publishing):** Working examples, CI/CD, README, crates.io, Docker, licensing.

See `docs/plans/OVERVIEW.md` for the full plan with Linear issue links. See `docs/ADR-001-architecture-review.md` for the architectural context behind these decisions.

---

## Guiding Principles

These aren't rules for the sake of rules — they reflect what makes this project work well.

**Read before you write.** Before changing a file, understand its role in the architecture. Check the dependency flow diagram above. If you're unsure where code belongs, re-read the crate descriptions.

**Keep core pure.** The `context-keeper-core` crate should never depend on a specific LLM provider, database, or external service. If you're adding a `use rig::*` or `use surrealdb::*` to a file in core, something is wrong.

**Make the tests tell the truth.** If you change behavior, update or add tests. If a test requires an API key or live service, it's not a unit test — gate it accordingly. The default `cargo test` should always pass in a clean environment with no secrets.

**Don't break the public API surface.** The MCP tools and CLI commands are the user-facing interface. Changing their signatures or behavior is a breaking change. Internal refactors are fine and encouraged, but the 6 MCP tools and CLI commands should remain stable unless the task explicitly calls for changing them.

**Ask when uncertain.** If a task is ambiguous — especially around entity identity, temporal semantics, or search ranking — check the ADR and plan docs first. If still unclear, ask rather than guessing. A wrong assumption in the graph model is expensive to unwind.
