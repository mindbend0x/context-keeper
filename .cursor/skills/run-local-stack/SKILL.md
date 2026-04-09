---
name: run-local-stack
description: >-
  Start the Context Keeper MCP server locally for testing. Use when the user
  wants to run the stack, test MCP tools end-to-end, verify a build, smoke-test
  changes, or needs a live server to interact with.
---

# Run Local Stack

## Quick Start

```bash
make dev
```

Or directly:

```bash
./scripts/dev-server.sh
```

This builds `context-keeper-mcp` in debug mode and starts it with:

| Setting | Value |
|---------|-------|
| Transport | HTTP |
| Port | 3000 |
| Storage | In-memory (clean every run) |
| Auth | Disabled |
| Logging | `context_keeper=debug` |

## Steps

### 1. Start the Dev Server

Run `make dev` in a **background terminal** (`block_until_ms: 0`). The server is ready when you see output containing `Listening` or `StreamableHttp`.

If you need seed data pre-loaded:

```bash
make dev-seed
```

### 2. Verify the Server Is Running

Send an MCP `initialize` request:

```bash
curl -s http://localhost:3000/mcp \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"0.1"}}}'
```

A successful response contains `"result"` with `serverInfo` and `capabilities`.

### 3. Interact via MCP

Call any tool through the MCP JSON-RPC interface. Example — list tools:

```bash
curl -s http://localhost:3000/mcp \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
```

### 4. Stop the Server

Send SIGINT (`Ctrl+C`) to the terminal running the server, or kill the process.

## Customizing the Environment

Override any default by setting the env var before the command:

```bash
# Different port
./scripts/dev-server.sh --port 4000

# Less noisy logs
./scripts/dev-server.sh --log-level info

# Persistent storage instead of memory
STORAGE_BACKEND=rocksdb:./test-data ./scripts/dev-server.sh

# Release build
./scripts/dev-server.sh --release
```

## Troubleshooting

| Problem | Fix |
|---------|-----|
| `Address already in use` | Another process on port 3000. Use `--port 3001` or `lsof -i :3000` to find it. |
| Build fails with clang errors | SurrealDB needs clang: `brew install llvm` (macOS) or `apt install clang` (Linux). |
| Server starts but tools return empty results | Expected with `STORAGE_BACKEND=memory` — no data persists. Use `--seed` or ingest data via the `add_memory` tool first. |
| `MCP_AUTH_TOKENS` errors | The dev script sets `MCP_ALLOW_INSECURE_HTTP=1`. If you override it, you need to provide tokens. |

## Checklist

- [ ] Server builds without errors
- [ ] `Listening` or `StreamableHttp` appears in logs
- [ ] MCP `initialize` returns a valid response
- [ ] Tool calls work (e.g., `tools/list`, `add_memory`, `search_memory`)
