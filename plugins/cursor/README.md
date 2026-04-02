# Context Keeper — Cursor / VS Code Extension

A VS Code/Cursor extension that connects to the Context Keeper knowledge graph via MCP. Spawns `context-keeper-mcp` as a local sidecar process over stdio.

## Features

- **Sidebar panel** — Browse the 20 most recent memories in the activity bar
- **Search command** (`Cmd+Shift+M` / `Ctrl+Shift+M`) — Hybrid search across the knowledge graph with a quick-pick results list
- **Expand search** (`Cmd+Shift+E` / `Ctrl+Shift+E`) — LLM-powered semantic query expansion for broader recall
- **Entity details** — Look up any entity's full details, relationships, and temporal bounds
- **Add from selection** — Select text in any editor and run "Context Keeper: Add Memory from Selection" to ingest it
- **Auto-capture on save** — Optionally record file save events as memories (enable via `contextKeeper.autoCapture`)
- **Status bar** — Connection status indicator with one-click search
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
| `contextKeeper.autoCapture` | `false` | Automatically capture a memory when a file is saved |

## Architecture

The extension uses the official MCP TypeScript SDK (`@modelcontextprotocol/sdk`) to communicate with the Rust MCP server over stdio. On activation it spawns the binary, completes the MCP handshake, and then calls tools (`list_recent`, `search_memory`, `add_memory`) as needed.

Since the MCP server defaults to RocksDB at `~/.context-keeper/data`, memories are shared across all tools — anything you remember in Claude Desktop or the CLI is instantly searchable in Cursor, and vice versa.

## Distributable Rules and Skills

The `dist/` directory contains Cursor-compatible rules and skills that users can copy into their projects to teach the Cursor agent how to use Context Keeper effectively.

```bash
cp -r plugins/cursor/dist/.cursor/ /path/to/your/project/.cursor/
```

| Component | Path | Description |
|-----------|------|-------------|
| Rule: `context-keeper-usage` | `dist/.cursor/rules/` | Reference for all MCP tools — when to search, save, and scope |
| Rule: `memory-aware-development` | `dist/.cursor/rules/` | Workflow: check memory before tasks, save learnings after |
| Skill: `search-context` | `dist/.cursor/skills/` | Deep multi-query context retrieval |
| Skill: `save-session-context` | `dist/.cursor/skills/` | Capture session decisions and trade-offs to memory |
| Skill: `review-with-memory` | `dist/.cursor/skills/` | Memory-augmented code review |

## Future Work

- **`@memory` mention** — In Cursor chat, typing `@memory <query>` triggers `search_memory`
