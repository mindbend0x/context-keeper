# Context Keeper — Claude Code Plugin

Persistent temporal knowledge graph memory for AI agents. Gives Claude Code cross-session, cross-tool memory with automatic entity extraction, hybrid search, and temporal reasoning.

## What's Included

| Component | Path | Description |
|-----------|------|-------------|
| MCP Server | `.mcp.json` | Registers `context-keeper-mcp` as a stdio MCP server |
| Skills | `skills/` | Reusable prompt fragments for common memory workflows |
| Agents | `agents/` | Autonomous agent definitions (⚠️ planned) |
| Hooks | `hooks/` | Event-driven hooks for automatic memory capture (⚠️ planned) |

## MCP Tools

The plugin exposes six MCP tools via the `context-keeper` server:

| Tool | Description |
|------|-------------|
| `add_memory` | Ingest text, extract entities and relations, store with embeddings |
| `search_memory` | Hybrid vector + BM25 keyword search with Reciprocal Rank Fusion |
| `expand_search` | LLM-rewritten semantic query expansion for improved recall |
| `get_entity` | Detailed entity lookup with type, summary, temporal bounds, and relations |
| `snapshot` | Point-in-time view of the knowledge graph at any timestamp |
| `list_recent` | Retrieve the N most recently added memories |

Additional tools are available for multi-agent workflows: `list_agents`, `list_namespaces`, `agent_activity`, and `cross_namespace_search`.

## Installation

### Claude Code (plugin system)

```bash
claude plugin add context-keeper
```

Or clone and install locally:

```bash
git clone https://github.com/0x313/context-keeper
cd context-keeper
claude plugin add ./plugins/claude
```

### Manual Setup

Copy `.mcp.json` into your project root or `~/.claude/` directory:

```bash
cp plugins/claude/.mcp.json ~/.claude/.mcp.json
```

Ensure `context-keeper-mcp` is in your `PATH`, or edit `.mcp.json` to use an absolute path:

```json
{
  "mcpServers": {
    "context-keeper": {
      "command": "/absolute/path/to/context-keeper-mcp",
      "args": ["--transport", "stdio"],
      "env": {
        "STORAGE_BACKEND": "memory",
        "DB_FILE_PATH": "context.sql"
      }
    }
  }
}
```

### Claude Desktop (legacy)

Use the installer scripts in `scripts/`:

```bash
# macOS / Linux
./plugins/claude/scripts/install.sh

# Windows (PowerShell)
.\plugins\claude\scripts\install.ps1
```

See `scripts/config.stdio.json` and `scripts/config.http.json` for reference configurations.

## Quick Start

Once installed, Claude Code automatically has access to the memory tools. Try:

```
Remember that our API uses JWT tokens with RS256 signing and tokens expire after 1 hour.
```

Claude will call `add_memory` to store this. Later, in any session:

```
What do we know about our authentication setup?
```

Claude will call `search_memory` to retrieve the stored context.

## Configuration

Environment variables in `.mcp.json`:

| Variable | Default | Description |
|----------|---------|-------------|
| `STORAGE_BACKEND` | `memory` | Storage backend (`memory`, `rocksdb:<path>`) |
| `DB_FILE_PATH` | `context.sql` | Path to SQLite persistence file |
| `OPENAI_API_URL` | — | OpenAI-compatible API URL (enables real entity extraction) |
| `OPENAI_API_KEY` | — | API key for LLM services |
| `EMBEDDING_MODEL` | `text-embedding-3-small` | Model for vector embeddings |
| `EMBEDDING_DIMS` | `1536` | Embedding vector dimensions |
| `EXTRACTION_MODEL` | `gpt-4o-mini` | Model for entity/relation extraction |

Without LLM credentials, the server runs in mock mode with keyword-only search (no embeddings or LLM extraction).

## Skills

| Skill | Description |
|-------|-------------|
| `search-context` | Deep multi-query context retrieval from the knowledge graph |
| `save-session-context` | Capture session decisions, learnings, and trade-offs to memory |
| `review-with-memory` | Memory-augmented code review: pre-search context, then save findings |

## Agents

| Agent | Model | Description |
|-------|-------|-------------|
| `memory-curator` | Haiku | Background agent that reviews and deduplicates stored memories |
| `context-searcher` | Sonnet | Read-only research agent for deep multi-angle knowledge graph search |
| `session-recorder` | Haiku | Summarizes session decisions and persists them to memory |

## Hooks

| Event | Trigger | Description |
|-------|---------|-------------|
| `SessionStart` | Session begins | Injects a context reminder to check memory before starting work |
| `Stop` | Claude finishes | Logs session end timestamp to `~/.context-keeper/session-log.jsonl` |
| `PostToolUse` | Write or Edit | Logs file write/edit events to the session log |

## License

MIT — see repository root for full license text.
