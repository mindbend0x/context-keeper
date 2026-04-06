# MCP Server Reference

Context Keeper implements the [Model Context Protocol](https://modelcontextprotocol.io/) (MCP) specification, exposing a temporal knowledge graph as tools, resources, and prompts to any MCP-compatible client.

**Protocol version:** `2024-11-05`

## Tools

### `add_memory`

Ingest text into the knowledge graph. Extracts entities and relations (via LLM or mock heuristics), generates embeddings, and stores everything for later retrieval.

**Parameters:**

| Name | Type | Required | Description |
|---|---|---|---|
| `text` | string | yes | The text content to ingest as a memory |
| `source` | string | no | Source label for the episode (default: `"mcp"`) |

**Returns:** JSON with `entities_created`, `relations_created`, `memories_created`, and `entity_names`.

**Example:**
```json
{ "text": "Alice is a software engineer at Acme Corp", "source": "chat" }
```

---

### `search_memory`

Search memories and entities using hybrid vector similarity and BM25 keyword search, fused with Reciprocal Rank Fusion (RRF).

**Parameters:**

| Name | Type | Required | Description |
|---|---|---|---|
| `query` | string | yes | The search query string |
| `limit` | integer | no | Maximum results to return (default: 5) |

**Returns:** JSON array of `{ name, entity_type, summary, score }`.

---

### `expand_search`

Expand a query into semantic variants using LLM-powered rewriting, then search each variant and merge results with RRF for improved recall. Useful when `search_memory` returns sparse results.

**Parameters:**

| Name | Type | Required | Description |
|---|---|---|---|
| `query` | string | yes | The search query to expand |
| `limit` | integer | no | Maximum results to return (default: 10) |

**Returns:** Same format as `search_memory`.

---

### `get_entity`

Fetch detailed information about a named entity, including its type, summary, temporal bounds, and all active relationships.

**Parameters:**

| Name | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes | The exact name of the entity to look up |

**Returns:** JSON array of entity details with nested `relations` array containing `relation_type`, `from_entity_id`, `to_entity_id`, and `confidence`.

---

### `snapshot`

Get a point-in-time snapshot of the knowledge graph at a specific timestamp, showing all entities and relations that were active at that moment.

**Parameters:**

| Name | Type | Required | Description |
|---|---|---|---|
| `timestamp` | string | yes | ISO 8601 timestamp (e.g. `"2025-01-15T12:00:00Z"`) |

**Returns:** JSON with `timestamp`, `entity_count`, `relation_count`, and `entities` array.

---

### `list_recent`

List the N most recently added memories, ordered by creation time.

**Parameters:**

| Name | Type | Required | Description |
|---|---|---|---|
| `limit` | integer | no | Maximum memories to return (default: 10) |

**Returns:** JSON array of `{ content, created_at }`.

---

## Resources

The server exposes browsable MCP resources that clients can list and read without calling tools.

### Static resources

| URI | Name | Description |
|---|---|---|
| `memory://recent` | `recent-memories` | The 20 most recently added memories (JSON) |

### Dynamic resources

Every active entity in the knowledge graph is listed as a resource:

| URI pattern | Name | Description |
|---|---|---|
| `memory://entity/{name}` | Entity name | Entity detail with type, summary, temporal bounds, and relationships |

### Resource templates

| URI template | Name | Description |
|---|---|---|
| `memory://entity/{name}` | `entity-detail` | Look up any entity by name |

Clients that support resource browsing (e.g. Claude Desktop) will show all active entities in a browsable list.

---

## Prompts

Pre-built prompt templates that guide the assistant through multi-step workflows.

### `summarize-topic`

Searches the knowledge graph for everything related to a topic and produces a comprehensive summary.

| Argument | Required | Description |
|---|---|---|
| `topic` | yes | The topic to summarize |

### `what-changed`

Compares a point-in-time snapshot with the current graph state to describe what entities, relationships, or memories have been added or changed.

| Argument | Required | Description |
|---|---|---|
| `since` | yes | ISO 8601 date/time to look back from |

### `add-context`

Ingests conversation context into the knowledge graph and confirms what was extracted.

| Argument | Required | Description |
|---|---|---|
| `context` | yes | The conversation context or notes to remember |

---

## Transports

### stdio (default)

The server reads/writes MCP JSON-RPC messages over stdin/stdout. This is the standard transport for local MCP clients like Claude Desktop and Cursor.

```bash
context-keeper-mcp
# or explicitly:
context-keeper-mcp --transport stdio
```

### Streamable HTTP

The server exposes an HTTP endpoint at `/mcp` using the MCP streamable HTTP transport. Suitable for remote, multi-client, or Docker deployments.

```bash
context-keeper-mcp --transport http --http-port 3000
```

The endpoint is then available at `http://localhost:3000/mcp`.

**Client config for HTTP:**
```json
{
  "mcpServers": {
    "context-keeper": {
      "url": "http://localhost:3000/mcp"
    }
  }
}
```

### Environment variables

| Variable | CLI flag | Default | Description |
|---|---|---|---|
| `MCP_TRANSPORT` | `--transport` | `stdio` | `stdio` or `http` |
| `MCP_HTTP_PORT` | `--http-port` | `3000` | HTTP port (only for HTTP transport) |

---

## Configuration

All configuration can be provided via CLI flags, environment variables, or a `.env` file.

### LLM settings

| Variable | CLI flag | Default | Description |
|---|---|---|---|
| `OPENAI_API_URL` | `-u, --api-url` | — | OpenAI-compatible API base URL |
| `OPENAI_API_KEY` | `-k, --api-key` | — | API key |
| `EMBEDDING_MODEL` | `-e, --embedding-model-name` | — | Embedding model (e.g. `text-embedding-3-small`) |
| `EMBEDDING_DIMS` | `-d, --embedding-dims` | `1536` | Embedding vector dimensions |
| `EXTRACTION_MODEL` | `-x, --extraction-model-name` | — | Model for entity/relation extraction |

When all four LLM variables are set, the server uses real LLM-powered extraction. Otherwise it falls back to mock heuristics (capitalized words → entities, consecutive pairs → relations).

### Storage settings

| Variable | CLI flag | Default | Description |
|---|---|---|---|
| `STORAGE_BACKEND` | `--storage` | `rocksdb:~/.context-keeper/data` | `memory`, `rocksdb:<path>`, or `remote:<ws_url>` |
| `DB_FILE_PATH` | `-f, --db-file-path` | `context.sql` | Export/import path for memory backend |

### Docker

```bash
docker compose up --build
```

The `docker-compose.yml` runs the MCP server in HTTP mode on port 3000 with RocksDB persistence via a Docker volume. Set `OPENAI_API_KEY` and `OPENAI_API_URL` in your environment or `.env` file.
