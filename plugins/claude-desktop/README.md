# Context Keeper — Claude Desktop Plugin

Register Context Keeper as an MCP server in [Claude Desktop](https://claude.ai/download), giving Claude access to a persistent temporal knowledge graph with these tools:

| Tool | Description |
|------|-------------|
| `add_memory` | Ingest text → extract entities & relations → store with embeddings |
| `search_memory` | Hybrid vector + BM25 keyword search with RRF fusion |
| `expand_search` | LLM-powered query expansion for improved recall |
| `get_entity` | Look up entity details and relationships |
| `snapshot` | Point-in-time graph state at any timestamp |
| `list_recent` | Browse the most recently added memories |

## Quick Start

### macOS / Linux

```bash
# Minimal — auto-builds if needed, uses mock extractors
./install.sh

# With LLM-powered extraction
./install.sh \
  --api-url https://api.openai.com/v1 \
  --api-key sk-your-key-here

# With RocksDB persistent storage
./install.sh --storage rocksdb:/path/to/data

# HTTP transport (for remote or multi-client setups)
./install.sh --transport http --http-port 3000

# Both transports (stdio for Claude Desktop + HTTP for other clients)
./install.sh --transport both
```

### Windows (PowerShell)

```powershell
# Minimal
.\install.ps1

# With LLM-powered extraction
.\install.ps1 -ApiUrl "https://api.openai.com/v1" -ApiKey "sk-your-key-here"

# With persistent storage
.\install.ps1 -Storage "rocksdb:C:\data\context-keeper"
```

### Manual Setup

If you prefer to configure Claude Desktop manually, copy the relevant template into your config:

**Config file locations:**
- macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
- Linux: `~/.config/Claude/claude_desktop_config.json`
- Windows: `%APPDATA%\Claude\claude_desktop_config.json`

**stdio** (recommended) — see [`config.stdio.json`](config.stdio.json):
```json
{
  "mcpServers": {
    "context-keeper": {
      "command": "/path/to/context-keeper-mcp",
      "args": ["--transport", "stdio"],
      "env": {
        "STORAGE_BACKEND": "memory",
        "DB_FILE_PATH": "context.sql"
      }
    }
  }
}
```

**HTTP** — see [`config.http.json`](config.http.json):
```json
{
  "mcpServers": {
    "context-keeper": {
      "url": "http://localhost:3000/mcp"
    }
  }
}
```

For HTTP, start the server separately:
```bash
context-keeper-mcp --transport http --http-port 3000
```

## Install Options

| Flag (bash) | Flag (PowerShell) | Default | Description |
|---|---|---|---|
| `--transport` | `-Transport` | `stdio` | `stdio`, `http`, or `both` |
| `--http-port` | `-HttpPort` | `3000` | HTTP port |
| `--binary` | `-BinaryPath` | auto-detect | Path to pre-built binary |
| `--storage` | `-Storage` | `memory` | `memory` or `rocksdb:<path>` |
| `--db-file` | `-DbFilePath` | `context.sql` | Persistence file for memory backend |
| `--api-url` | `-ApiUrl` | — | OpenAI-compatible API URL |
| `--api-key` | `-ApiKey` | — | API key |
| `--embedding-model` | `-EmbeddingModel` | `text-embedding-3-small` | Embedding model |
| `--embedding-dims` | `-EmbeddingDims` | `1536` | Embedding dimensions |
| `--extraction-model` | `-ExtractionModel` | `gpt-4o-mini` | Entity/relation extraction model |

## Binary Resolution Order

The installer searches for the `context-keeper-mcp` binary in this order:

1. Explicit `--binary` / `-BinaryPath` flag
2. `context-keeper-mcp` on `$PATH`
3. `target/release/` or `target/debug/` in the repo root
4. **Auto-build** via `cargo build --release` (requires Rust toolchain)

## Uninstall

```bash
# macOS / Linux
./install.sh --uninstall

# Windows
.\install.ps1 -Uninstall
```

This removes the `context-keeper` entry from Claude Desktop's config. The binary itself is not deleted.

## Transport Comparison

| | stdio | HTTP |
|---|---|---|
| Setup | Zero config — Claude Desktop manages the process | Requires running the server separately |
| Best for | Single-user, local use | Multi-client, remote, Docker |
| Latency | Lowest (direct pipe) | Slightly higher (HTTP overhead) |
| Process lifecycle | Managed by Claude Desktop | Self-managed or via systemd/Docker |

## LLM Configuration

Without `--api-url` and `--api-key`, the server falls back to **mock extractors** which use simple heuristics (capitalized words as entities, consecutive pairs as relations). This is fine for testing but won't produce meaningful knowledge graphs.

For production use, provide an OpenAI-compatible endpoint. Any provider that implements the OpenAI API (OpenAI, Azure OpenAI, Ollama, vLLM, etc.) will work.

## Docker

For Docker-based deployments, see the root [`Dockerfile`](../../Dockerfile) and [`docker-compose.yml`](../../docker-compose.yml). Use the HTTP transport config template with Docker's exposed port.
