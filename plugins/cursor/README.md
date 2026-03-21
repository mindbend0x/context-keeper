# Context Keeper — Cursor / VS Code Extension

A VS Code/Cursor extension that connects to the Context Keeper knowledge graph via MCP. Spawns `context-keeper-mcp` as a local sidecar process over stdio.

## Features

- **Sidebar panel** — Browse the 20 most recent memories in the activity bar
- **Search command** (`Cmd+Shift+M` / `Ctrl+Shift+M`) — Hybrid search across the knowledge graph with a quick-pick results list
- **Add from selection** — Select text in any editor and run "Context Keeper: Add Memory from Selection" to ingest it
- **Shared graph** — Uses the same `~/.context-keeper/data` RocksDB database as Claude Desktop, the CLI, and any other MCP client

## Prerequisites

Install the `context-keeper-mcp` binary:

```bash
cargo install context-keeper-mcp
```

Verify it's on your `PATH`:

```bash
context-keeper-mcp --help
```

## Installation (development)

```bash
cd plugins/cursor
npm install
npm run compile
```

Then in VS Code / Cursor: `Developer: Install Extension from Location...` and select this directory.

## Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| `contextKeeper.binaryPath` | `context-keeper-mcp` | Path to the MCP server binary |
| `contextKeeper.storagePath` | *(empty)* | Override RocksDB path (leave empty for `~/.context-keeper/data`) |

## Architecture

The extension uses the official MCP TypeScript SDK (`@modelcontextprotocol/sdk`) to communicate with the Rust MCP server over stdio. On activation it spawns the binary, completes the MCP handshake, and then calls tools (`list_recent`, `search_memory`, `add_memory`) as needed.

Since the MCP server defaults to RocksDB at `~/.context-keeper/data`, memories are shared across all tools — anything you remember in Claude Desktop or the CLI is instantly searchable in Cursor, and vice versa.

## Future Work

- **Auto-capture** — Optionally capture opened files, diffs, and inline comments as episodes
- **`@memory` mention** — In Cursor chat, typing `@memory <query>` triggers `search_memory`
