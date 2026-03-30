# MCP Crate Agent

You are a specialist for `context-keeper-mcp`, the MCP server binary.

## Ownership

- **Main** (`src/main.rs`): CLI args, SurrealDB init, LLM service wiring, transport selection (stdio/HTTP)
- **Tools** (`src/tools.rs`): `ContextKeeperServer` with all MCP tools, resources, and prompts

## Architecture

`ContextKeeperServer` holds:
- `repo: Repository` — SurrealDB operations
- `embedder: Arc<dyn Embedder>` — embedding generation
- `entity_extractor: Arc<dyn EntityExtractor>` — entity extraction
- `relation_extractor: Arc<dyn RelationExtractor>` — relation extraction
- `query_rewriter: Arc<dyn QueryRewriter>` — query expansion

Tools are registered via rmcp's `#[tool_router]` / `#[tool]` macros. The `#[tool_handler]` macro generates the `ServerHandler` plumbing.

## Current Tools

| Tool | Purpose |
|------|---------|
| `add_memory` | Ingest text → extract entities/relations → persist |
| `search_memory` | Hybrid vector+keyword search with RRF |
| `expand_search` | LLM query expansion → multi-variant search → RRF |
| `get_entity` | Entity detail lookup with relations |
| `snapshot` | Point-in-time graph state |
| `list_recent` | Recent memories |
| `list_agents` | Contributing agents |
| `list_namespaces` | Available namespaces |
| `agent_activity` | Agent-specific episode history |
| `cross_namespace_search` | Global search across all namespaces |

## Transports

- **stdio**: default, for local MCP clients (Claude Desktop, Cursor)
- **HTTP**: streamable HTTP at `/mcp`, optional bearer token auth

## When Modifying

- Adding a tool → follow the `add-mcp-tool` skill in `.cursor/skills/`.
- Adding a resource → implement in `list_resources`/`read_resource`, use `memory://` URI scheme.
- Adding a prompt → implement in `list_prompts`/`get_prompt`.
- Changing server capabilities → update `get_info()` and the `instructions` string.
