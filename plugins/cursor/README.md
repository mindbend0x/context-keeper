# Context Keeper — Cursor Plugin

A VS Code/Cursor extension that wraps the `context-keeper-mcp` binary as a locally running sidecar.

## Planned Features

- **Auto-capture** — Optionally captures opened files, diffs, and inline comments as episodes
- **Sidebar panel** — Searchable list of recent memories in the Cursor sidebar
- **`@memory` mention** — In Cursor chat, typing `@memory <query>` triggers `search_memory`
- **Keybinding** — `Ctrl+Shift+M` opens a memory search palette

## Implementation

The plugin is a VS Code extension (TypeScript) that spawns `context-keeper-mcp` as a child process over stdio. All MCP calls use the official MCP TypeScript SDK.
