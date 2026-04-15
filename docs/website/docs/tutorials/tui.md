---
sidebar_position: 7
title: TUI
description: Run the Context Keeper terminal UI for local or remote knowledge graph exploration.
---

# Terminal UI (TUI)

Context Keeper ships a full-featured terminal interface built with [Ratatui](https://ratatui.rs). It connects either directly to a local SurrealDB instance or to a running MCP HTTP server.

## Screens

| Tab | What it shows |
|-----|---------------|
| **Dashboard** | Recent memories, entity counts, activity summary |
| **Search** | Hybrid vector + keyword search with results |
| **Entity** | Entity detail view with relations and episodes |
| **Ingest** | Add new memories from the terminal |
| **Admin** | Namespaces, agents, cross-search, snapshot, activity (requires `--admin`) |

## Local mode

Runs the TUI directly against a local SurrealDB instance. No separate server needed.

```bash
make tui
```

This starts with in-memory storage — data does not persist between runs. To use a persistent file:

```bash
STORAGE_BACKEND=rocksdb:./my-context.db cargo run -p context-keeper-tui -- --admin
```

:::note
Ingesting memories in local mode requires `OPENAI_API_KEY` (or equivalent) for entity extraction and embedding. Browsing existing data works without API keys.
:::

## Remote mode (MCP HTTP server)

Connect to a running Context Keeper MCP HTTP server. Requires building with the `remote-mcp` feature flag.

### Against the local dev server (no auth)

```bash
# Terminal 1 — start the dev server
make dev

# Terminal 2 — connect TUI
CK_MCP_URL=http://localhost:3000/mcp cargo run -p context-keeper-tui --features remote-mcp -- --admin
```

`make dev` runs with `MCP_ALLOW_INSECURE_HTTP=true` so no token is needed.

### Against a server with bearer auth

```bash
CK_MCP_URL=https://mcp.yourdomain.com/mcp \
CK_MCP_TOKEN=your-bearer-token \
cargo run -p context-keeper-tui --features remote-mcp -- --admin
```

Or with CLI flags directly:

```bash
cargo run -p context-keeper-tui --features remote-mcp -- \
  --mcp-url https://mcp.yourdomain.com/mcp \
  --mcp-token your-bearer-token \
  --admin
```

The token must match one of the values in `MCP_AUTH_TOKENS` on the server. See [Authorization](./authorization#mode-2--static-bearer-tokens).

## All options

| Flag | Env var | Default | Description |
|------|---------|---------|-------------|
| `--mcp-url` | `CK_MCP_URL` | — | Remote MCP HTTP URL. Requires `--features remote-mcp`. |
| `--mcp-token` | `CK_MCP_TOKEN` | — | Bearer token for MCP HTTP auth. |
| `--storage` | `STORAGE_BACKEND` | `rocksdb:./context.db` | Local storage: `memory`, `rocksdb:<path>` |
| `--db-file-path` | `DB_FILE_PATH` | `context.sql` | Seed file path for `rocksdb` storage |
| `--namespace` | `CK_NAMESPACE` | — | Scope memories to a namespace |
| `--agent-id` | `CK_AGENT_ID` | — | Agent identifier for attribution |
| `--admin` | — | off | Enable the Admin tab |
| `--debug-log` | `CK_TUI_DEBUG_LOG` | — | Append tracing logs to a file (stdout logging corrupts the TUI display) |
| `--api-url` | `OPENAI_API_URL` | — | LLM API base URL (local mode) |
| `--api-key` | `OPENAI_API_KEY` | — | LLM API key (local mode) |
| `--embedding-api-url` | `EMBEDDING_API_URL` | — | Separate embedding API URL if different from extraction |
| `--embedding-api-key` | `EMBEDDING_API_KEY` | — | Separate embedding API key |
| `--embedding-model-name` | `EMBEDDING_MODEL` | — | Embedding model override |
| `--extraction-model-name` | `EXTRACTION_MODEL` | — | Extraction model override |

## Keybindings

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Switch between tabs |
| `q` | Quit (from Dashboard) |
| `Ctrl+C` | Quit from any screen |

## Related

- [Authorization](./authorization) — configuring bearer tokens and OAuth
- [Using HTTP Transport](./http-transport) — running the MCP server
- [Running Locally](./running-locally) — local SurrealDB setup
