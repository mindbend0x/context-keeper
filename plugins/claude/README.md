# Context Keeper — Claude Code Plugin

Persistent temporal knowledge graph memory for AI agents. Gives Claude Code cross-session, cross-tool memory with automatic entity extraction, hybrid search, and temporal reasoning.

## What's Included

| Component | Path | Description |
|-----------|------|-------------|
| MCP Server | `.mcp.json` | Registers `context-keeper-mcp` as a stdio MCP server |
| Skills | `skills/` | Reusable prompt fragments for common memory workflows |
| Install Script | `scripts/install.sh` | Builds the binary and writes config |

## MCP Tools

The plugin exposes these MCP tools via the `context-keeper` server:

| Tool | Description |
|------|-------------|
| `add_memory` | Ingest text, extract entities and relations, store with embeddings |
| `search_memory` | Hybrid vector + BM25 keyword search with Reciprocal Rank Fusion |
| `expand_search` | LLM-rewritten semantic query expansion for improved recall |
| `get_entity` | Detailed entity lookup with type, summary, temporal bounds, and relations |
| `snapshot` | Point-in-time view of the knowledge graph at any timestamp |
| `list_recent` | Retrieve the N most recently added memories |

Additional tools for multi-agent workflows: `list_agents`, `list_namespaces`, `agent_activity`, `cross_namespace_search`.

## Installation

### Quick (Claude Code plugin system)

```bash
claude plugin add ./plugins/claude
```

### Script

```bash
# Local project install
./plugins/claude/scripts/install.sh

# Global install
./plugins/claude/scripts/install.sh --global

# With LLM extraction
./plugins/claude/scripts/install.sh --api-url https://api.openai.com/v1 --api-key sk-...
```

### Manual

Copy `.mcp.json` into your project root or `~/.claude/`:

```bash
cp plugins/claude/.mcp.json .mcp.json
```

Ensure `context-keeper-mcp` is in your `PATH`, or edit `.mcp.json` to use an absolute path.

### Claude Desktop

Use the installer in `plugins/claude-desktop/`:

```bash
./plugins/claude-desktop/install.sh
```

## Configuration

Environment variables in `.mcp.json`:

| Variable | Default | Description |
|----------|---------|-------------|
| `STORAGE_BACKEND` | `memory` | Storage backend (`memory`, `rocksdb:<path>`) |
| `DB_FILE_PATH` | `context.sql` | Path to SQLite persistence file |
| `OPENAI_API_URL` | — | OpenAI-compatible API URL (enables real entity extraction) |
| `OPENAI_API_KEY` | — | API key for LLM services |
| `EMBEDDING_MODEL` | `text-embedding-3-small` | Model for vector embeddings |
| `EXTRACTION_MODEL` | `gpt-4o-mini` | Model for entity/relation extraction |

Without LLM credentials, the server runs in mock mode with keyword-only search.

## Skills

| Skill | Description |
|-------|-------------|
| `search-context` | Deep multi-query context retrieval from the knowledge graph |
| `save-session-context` | Capture session decisions, learnings, and trade-offs to memory |
| `review-with-memory` | Memory-augmented code review: pre-search context, then save findings |

## License

MIT — see repository root for full license text.
