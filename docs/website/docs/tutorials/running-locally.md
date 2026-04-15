---
sidebar_position: 3
title: Running Locally
description: Build from source, configure your environment, and run Context Keeper locally.
---

# Running Locally

This tutorial covers building Context Keeper from source, configuring your environment, and running both the MCP server and CLI for local development.

## Prerequisites

- **Rust toolchain** (stable, 1.70+): Install via [rustup](https://rustup.rs/)
- **Git**: For cloning the repository
- **Optional**: An OpenAI-compatible API key for LLM-powered extraction

## Clone and build

```bash
git clone https://github.com/mindbend0x/context-keeper.git
cd context-keeper
cargo build --release
```

This builds the entire workspace. Binaries are in `target/release/`:

| Binary | Description |
|--------|-------------|
| `context-keeper-mcp` | MCP server (stdio or HTTP) |
| `context-keeper` | CLI tool |

To build a specific crate:

```bash
cargo build --release -p context-keeper-mcp   # MCP server only
cargo build --release -p context-keeper-cli    # CLI only
```

## Workspace structure

```
context-keeper/
├── crates/
│   ├── context-keeper-core/     # Pure logic: models, pipeline, search, traits
│   ├── context-keeper-rig/      # LLM integrations: embeddings, extraction
│   ├── context-keeper-surreal/  # SurrealDB: repository, indexes, queries
│   ├── context-keeper-mcp/      # MCP server binary
│   └── context-keeper-cli/      # CLI binary
├── test/                        # Integration tests
├── docs/                        # Documentation
├── docker-compose.yml           # Docker deployment
├── Cargo.toml                   # Workspace root
└── .env.example                 # Environment variable template
```

## Environment configuration

### Create a .env file

Copy the example and configure:

```bash
cp .env.example .env
```

### Mock mode (no API key needed)

Leave the LLM variables empty or unset. Context Keeper uses heuristic extraction:

```bash
# .env — mock mode
STORAGE_BACKEND=rocksdb:~/.context-keeper/data
```

### LLM mode (production-quality extraction)

```bash
# .env — LLM mode
OPENAI_API_URL=https://api.openai.com/v1
OPENAI_API_KEY=sk-xxxxx
EMBEDDING_MODEL=text-embedding-3-small
EMBEDDING_DIMS=1536
EXTRACTION_MODEL=gpt-4o-mini
STORAGE_BACKEND=rocksdb:~/.context-keeper/data
```

:::info
Any OpenAI-compatible endpoint works. This includes local models via Ollama, LM Studio, vLLM, or any provider with an OpenAI-compatible API.
:::

### Full environment variable reference

| Variable | Default | Description |
|----------|---------|-------------|
| `OPENAI_API_URL` | — | API endpoint (e.g., `https://api.openai.com/v1`) |
| `OPENAI_API_KEY` | — | API key |
| `EMBEDDING_MODEL` | — | Embedding model (e.g., `text-embedding-3-small`) |
| `EMBEDDING_DIMS` | `1536` | Vector dimensions |
| `EXTRACTION_MODEL` | — | Entity/relation extraction model (e.g., `gpt-4o-mini`) |
| `STORAGE_BACKEND` | `rocksdb:~/.context-keeper/data` | Storage backend |
| `DB_FILE_PATH` | `context.sql` | SurrealDB file name |
| `MCP_TRANSPORT` | `stdio` | MCP transport (`stdio` or `http`) |
| `MCP_HTTP_PORT` | `3000` | HTTP server port |

## Running the MCP server

### stdio transport (default)

```bash
cargo run --release -p context-keeper-mcp
```

The server reads from stdin and writes to stdout. This is the mode used by Claude Desktop, Cursor, and Claude Code.

### HTTP transport

```bash
MCP_TRANSPORT=http MCP_HTTP_PORT=3000 cargo run --release -p context-keeper-mcp
```

The server listens on `http://localhost:3000`. Use this for:
- Multi-agent setups
- Remote clients
- ChatGPT (which requires HTTP)
- Docker deployments

See the [HTTP Transport](/docs/tutorials/http-transport) tutorial for more details.

## Running the CLI

```bash
# From the project directory
cargo run -p context-keeper-cli -- add --text "Hello from local dev" --source test
cargo run -p context-keeper-cli -- search --query "Hello"
```

Or if installed via `cargo install`:

```bash
context-keeper add --text "Hello from local dev" --source test
context-keeper search --query "Hello"
```

## Running tests

The full test suite runs without API keys:

```bash
cargo test                          # All workspace tests
cargo test -p context-keeper-core   # Core logic tests
cargo test -p context-keeper-test   # Integration tests
```

Integration tests use mock extractors and in-memory storage, so they're fast and free.

## Resetting data

To start with a clean knowledge graph:

```bash
rm -rf ~/.context-keeper/data
```

The next run will reinitialize the database automatically.

## IDE setup

### VS Code / Cursor

Install the **rust-analyzer** extension for:
- Inline type hints
- Go-to-definition across crates
- Cargo check on save

Recommended `settings.json`:

```json
{
  "rust-analyzer.cargo.features": "all",
  "rust-analyzer.check.command": "clippy"
}
```

### IntelliJ / CLion

Install the **Rust plugin**. The workspace `Cargo.toml` will be detected automatically.

---

## Next steps

- [Installing the CLI](/docs/tutorials/cli-installation) — CLI first-use walkthrough
- [Running with Docker](/docs/tutorials/running-with-docker) — Containerized deployment
- [MCP Server Setup](/docs/tutorials/mcp-server-setup) — Connect to your AI client
- [Configuration](/docs/configuration) — Full configuration reference
