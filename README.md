# Context Keeper

Temporal knowledge graph memory for AI agents. Give Claude, Cursor, or any MCP-compatible assistant a persistent memory that tracks entities, relationships, and changes over time — no API key required to get started.

## Quickstart: MCP Server (Claude Desktop / Cursor)

**1. Install**

```bash
cargo install context-keeper-mcp
```

**2. Configure your client**

<details>
<summary><b>Claude Desktop</b></summary>

Add to `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "context-keeper": {
      "command": "context-keeper-mcp",
      "args": []
    }
  }
}
```

</details>

<details>
<summary><b>Cursor</b></summary>

Add to `.cursor/mcp.json` in your project (or global settings):

```json
{
  "mcpServers": {
    "context-keeper": {
      "command": "context-keeper-mcp",
      "args": []
    }
  }
}
```

</details>

**3. Try it**

Restart your client, then tell your assistant:

> "Remember that Alice is a software engineer at Acme Corp and Bob is her manager."

Then ask:

> "What do you know about Alice?"

That's it. The MCP server uses mock extraction by default (no API key needed). Entities are extracted from capitalized words, and memories are stored in an in-memory graph. To upgrade to real LLM-powered extraction, see [Using Real LLM Extraction](#using-real-llm-extraction) below.

## Quickstart: CLI

```bash
cargo install context-keeper-cli
```

```bash
# Add a memory (works immediately, no config needed)
context-keeper add --text "Alice is a software engineer at Acme Corp"

# Search
context-keeper search --query "Acme"

# Look up an entity
context-keeper entity --name "Alice"

# List recent memories
context-keeper recent --limit 5
```

Without LLM config, the CLI uses the same mock extraction as the MCP server.

## MCP Tools

| Tool | Description |
|------|-------------|
| `add_memory` | Ingest text into the graph — extracts entities/relations, generates embeddings |
| `search_memory` | Hybrid vector + BM25 keyword search with RRF fusion |
| `expand_search` | Query expansion for improved recall |
| `get_entity` | Fetch entity details with full relationship graph |
| `snapshot` | Point-in-time snapshot of the knowledge graph |
| `list_recent` | List the N most recent memories |

## Using Real LLM Extraction

Set these environment variables (or create a `.env` file — see [`.env.example`](.env.example)):

```bash
export OPENAI_API_URL=https://api.openai.com/v1
export OPENAI_API_KEY=sk-...
export EMBEDDING_MODEL=text-embedding-3-small
export EXTRACTION_MODEL=gpt-4o-mini
```

Any OpenAI-compatible endpoint works. When all four variables are set, both the CLI and MCP server switch from mock extraction to real LLM-powered entity/relation extraction with vector embeddings.

For the MCP server, you can pass these as environment variables in your client config:

```json
{
  "mcpServers": {
    "context-keeper": {
      "command": "context-keeper-mcp",
      "env": {
        "OPENAI_API_URL": "https://api.openai.com/v1",
        "OPENAI_API_KEY": "sk-...",
        "EMBEDDING_MODEL": "text-embedding-3-small",
        "EXTRACTION_MODEL": "gpt-4o-mini"
      }
    }
  }
}
```

## Storage Backends

By default, all Context Keeper binaries use **RocksDB at `~/.context-keeper/data`**. This means every tool (Claude Desktop, Cursor, CLI) shares the same knowledge graph automatically.

```bash
# Default — RocksDB at ~/.context-keeper/data (shared across all tools)
context-keeper add --text "..."

# Custom RocksDB path
context-keeper --storage rocksdb:./my_data add --text "..."

# In-memory (ephemeral — exports to context.sql on exit, reimports on start)
context-keeper --storage memory add --text "..."
```

## How It Works

Context Keeper is a Rust implementation inspired by [Graphiti](https://www.presidio.com/technical-blog/graphiti-giving-ai-a-real-memory-a-story-of-temporal-knowledge-graphs/), replacing Python/Neo4j with Rust, [Rig](https://rig.rs), and [SurrealDB](https://surrealdb.com).

It ingests text episodes, extracts entities and relationships (via LLM or mock heuristics), and stores them in a SurrealDB-backed graph with:

- **HNSW vector search** on entity and memory embeddings
- **BM25 full-text search** across entities, memories, and episodes
- **Hybrid RRF fusion** combining vector similarity and keyword relevance
- **Temporal versioning** — every entity and relation carries `valid_from`/`valid_until` timestamps with point-in-time snapshot queries
- **True UPSERT** — entities are deduplicated by name with summary/embedding merging

## Architecture

```mermaid
graph TD
  subgraph interfaces [Interfaces]
    CLI["CLI (context-keeper-cli)"]
    MCP["MCP Server (context-keeper-mcp)"]
  end

  subgraph core [Core Engine]
    INGESTION["Ingestion Pipeline"]
    TEMPORAL["Temporal Manager"]
    SEARCH["Search with RRF"]
  end

  subgraph rig [Rig Integration]
    LLM["LLM Extraction"]
    EMBED["Embeddings"]
  end

  subgraph surreal [SurrealDB Graph Layer]
    RELATE["RELATE Edges"]
    VECTORS["HNSW Vectors"]
    BM25["BM25 Full-Text"]
  end

  CLI -->|invokes| INGESTION
  MCP -->|invokes| INGESTION
  INGESTION --> TEMPORAL
  INGESTION --> SEARCH
  INGESTION --> LLM
  INGESTION --> EMBED
  LLM --> RELATE
  EMBED --> VECTORS
  SEARCH --> BM25
  TEMPORAL --> RELATE
```

### Workspace Structure

```
context-keeper/
├── crates/
│   ├── context-keeper-core/      # Models, ingestion pipeline, search, temporal manager
│   ├── context-keeper-rig/       # Rig integration: embeddings, entity/relation extraction
│   ├── context-keeper-surreal/   # SurrealDB client, schema, repository, vector store
│   ├── context-keeper-mcp/       # MCP server binary (stdio transport)
│   └── context-keeper-cli/       # CLI binary + examples
├── migrations/                   # Reference SurrealQL schema
└── Cargo.toml                    # Workspace root
```

### Data Model

| Table | Type | Purpose |
|-------|------|---------|
| `episode` | Node | Raw text input with source and timestamp |
| `entity` | Node | Extracted entities with embeddings and temporal bounds |
| `memory` | Node | Searchable memory chunks with embeddings |
| `relates_to` | Edge (entity -> entity) | Typed relationships with confidence scores |
| `sourced_from` | Edge (memory -> episode) | Links memories to source episodes |
| `references` | Edge (memory -> entity) | Links memories to mentioned entities |

## CLI Reference

```
context-keeper [OPTIONS] <COMMAND>

Commands:
  add      Add a memory from text input
  search   Search memories (hybrid vector + keyword)
  entity   Get entity details by name
  recent   List recent memories

Global Options:
  -e, --embedding-model-name   Embedding model name    [env: EMBEDDING_MODEL]
  -d, --embedding-dims         Embedding dimensions    [env: EMBEDDING_DIMS]
  -x, --extraction-model-name  Extraction model name   [env: EXTRACTION_MODEL]
  -u, --api-url                OpenAI-compatible URL   [env: OPENAI_API_URL]
  -k, --api-key                API key                 [env: OPENAI_API_KEY]
  -f, --db-file-path           DB export file path     [env: DB_FILE_PATH]     [default: context.sql]
      --storage                Storage backend         [env: STORAGE_BACKEND]  [default: rocksdb:~/.context-keeper/data]
```

## Docker

```bash
export OPENAI_API_KEY=sk-...
docker compose up --build
```

The MCP server will be available on `http://localhost:3000` with RocksDB persistence via a Docker volume.

## Running Tests

```bash
cargo test --workspace
```

## Key Dependencies

| Crate | Purpose |
|-------|---------|
| `surrealdb` | Graph database with HNSW + BM25 |
| `rig-core` | LLM completions + embeddings |
| `rmcp` | Rust MCP SDK (stdio transport) |
| `tokio` | Async runtime |
| `clap` | CLI argument parsing |

## License

MIT
