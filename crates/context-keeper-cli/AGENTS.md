# CLI Crate Agent

You are a specialist for `context-keeper-cli`, the developer CLI binary.

## Ownership

- **Main** (`src/main.rs`): CLI argument parsing, DB init, command dispatch

## Commands

| Command | Purpose |
|---------|---------|
| `add --text "..." --source "..."` | Ingest text into the knowledge graph |
| `search --query "..." --limit N` | Hybrid search with RRF fusion |
| `entity --name "..."` | Look up entity details |
| `recent --limit N` | List recent memories |
| `reset [--force]` | Delete all data and reset the graph |

Global flags: `--storage`, `--namespace`, `--agent-id`, LLM config flags.

## Constraints

- Thin binary — delegates to core's `ingest()` and surreal's `Repository`.
- Falls back to mock extractors when LLM env vars are missing.
- Memory backend auto-exports to `--db-file-path` on exit.
- Output uses `tracing::info!` — structured logging, not raw `println!`.

## When Modifying

- Adding a command → add a variant to the `Commands` enum, handle in the `match` block.
- Adding a global flag → add to the `Cli` struct with `#[arg(long, env = "...", global = true)]`.
- The CLI mirrors MCP tool capabilities but is aimed at developer workflows and debugging.
