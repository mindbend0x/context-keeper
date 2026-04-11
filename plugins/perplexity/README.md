# Context Keeper — Perplexity Plugin

Register Context Keeper as an MCP server in the [Perplexity](https://perplexity.ai) Mac app, giving Perplexity access to a persistent temporal knowledge graph with these tools:

| Tool | Description |
|------|-------------|
| `add_memory` | Ingest text → extract entities & relations → store with embeddings |
| `search_memory` | Hybrid vector + BM25 keyword search with RRF fusion |
| `expand_search` | LLM-powered query expansion for improved recall |
| `get_entity` | Look up entity details and relationships |
| `snapshot` | Point-in-time graph state at any timestamp |
| `list_recent` | Browse the most recently added memories |

## Prerequisites

1. **Perplexity Mac app** — MCP connectors are a macOS-only feature. Download from [perplexity.ai](https://perplexity.ai) or the Mac App Store.

2. **Helper App (PerplexityXPC)** — The Perplexity app is sandboxed; the Helper App bridges it to local MCP servers. You'll be prompted to install it when you first add a connector.

3. **context-keeper-mcp binary** — Build from source:

```bash
git clone https://github.com/mindbend0x/context-keeper.git
cd context-keeper
cargo build --release -p context-keeper-mcp
cp target/release/context-keeper-mcp ~/.cargo/bin/
```

Or download a pre-built binary from [GitHub Releases](https://github.com/mindbend0x/context-keeper/releases) and place it on your `PATH`.

Verify it's on your `PATH`:

```bash
context-keeper-mcp --help
```

## Quick Start

1. Open the Perplexity app
2. Go to **Settings → Connectors**
3. Click **Add Connector**
4. Install the **Helper App** if prompted
5. Enter a server name (e.g. `Context Keeper`)
6. Select **Advanced** and paste the connector JSON below
7. Click **Save**

### stdio (recommended)

Paste the contents of [`config.stdio.json`](config.stdio.json):

```json
{
  "command": "/path/to/context-keeper-mcp",
  "args": ["--transport", "stdio"],
  "env": {
    "STORAGE_BACKEND": "memory",
    "DB_FILE_PATH": "context.sql"
  }
}
```

Replace `/path/to/context-keeper-mcp` with the actual binary path. Find it with:

```bash
which context-keeper-mcp
```

### HTTP

For remote or multi-client setups, paste the contents of [`config.http.json`](config.http.json):

```json
{
  "url": "http://localhost:3000/mcp"
}
```

Start the server separately:

```bash
context-keeper-mcp --transport http --http-port 3000
```

## Environment Variables

When using stdio transport, configure the server through `env` in the connector JSON:

| Variable | Default | Description |
|---|---|---|
| `STORAGE_BACKEND` | `memory` | `memory` or `rocksdb:<path>` |
| `DB_FILE_PATH` | `context.sql` | Persistence file for memory backend |
| `OPENAI_API_URL` | — | OpenAI-compatible API URL |
| `OPENAI_API_KEY` | — | API key for LLM services |
| `EMBEDDING_MODEL` | `text-embedding-3-small` | Embedding model |
| `EMBEDDING_DIMS` | `1536` | Embedding dimensions |
| `EXTRACTION_MODEL` | `gpt-4o-mini` | Entity/relation extraction model |

## Binary Resolution

Point `command` to the binary based on how you installed:

| Location | When |
|---|---|
| Result of `which context-keeper-mcp` | Copied to a directory on your `PATH` |
| `target/release/context-keeper-mcp` | Built with `cargo build --release` |
| `target/debug/context-keeper-mcp` | Built with `cargo build` |

## Transport Comparison

| | stdio | HTTP |
|---|---|---|
| Setup | Paste JSON, Perplexity manages the process | Run server separately, paste URL |
| Best for | Single-user, local use | Multi-client, remote, Docker |
| Latency | Lowest (direct pipe) | Slightly higher (HTTP overhead) |
| Process lifecycle | Managed by Perplexity | Self-managed or via systemd/Docker |

## LLM Configuration

Without `OPENAI_API_URL` and `OPENAI_API_KEY`, the server falls back to **mock extractors** which use simple heuristics (capitalized words as entities, consecutive pairs as relations). This is fine for testing but won't produce meaningful knowledge graphs.

For production use, add your OpenAI-compatible endpoint to the `env` block. Any provider that implements the OpenAI API (OpenAI, Azure OpenAI, Ollama, vLLM, etc.) will work.

## Docker

For Docker-based deployments, see the root [`Dockerfile`](../../Dockerfile) and [`docker-compose.yml`](../../docker-compose.yml). Use the HTTP transport config template with Docker's exposed port.
