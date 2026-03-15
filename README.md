# Context Keeper

A high-performance, temporally-aware knowledge graph memory system for AI agents, built entirely in Rust. Context Keeper ingests episodes of text, extracts entities and relationships via LLM calls, and stores them in a SurrealDB-backed graph with full temporal versioning, HNSW vector search, and BM25 full-text search.

Spiritual successor to [Graphiti](https://www.presidio.com/technical-blog/graphiti-giving-ai-a-real-memory-a-story-of-temporal-knowledge-graphs/), replacing Python/Neo4j with Rust, [Rig](https://rig.rs), and [SurrealDB](https://surrealdb.com).

## Architecture

```mermaid
graph TD
  subgraph Interfaces Layer
    CLI["CLI (context-keeper-cli)"]
    MCP["MCP Server (planned)"]
  end

  subgraph Core Engine (context-keeper-core)
    INGESTION["Ingestion Pipeline"]
    TEMPORAL["Temporal Manager"]
    SEARCH["Search (RRF)"]
  end

  subgraph Rig Integration (context-keeper-rig)
    LLM["LLM Extraction"]
    EMBED["Embeddings"]
    TOOL["Tool Definitions"]
  end

  subgraph SurrealDB Graph Layer (context-keeper-surreal)
    RELATE["RELATE Edges"]
    VECTORS["HNSW Vectors"]
    BM25["BM25 Full-Text"]
    ROCKSDB["RocksDB"]
  end

  CLI -->|invokes| INGESTION
  MCP -->|invokes| INGESTION
  INGESTION --> TEMPORAL
  INGESTION --> SEARCH
  INGESTION --> LLM
  INGESTION --> EMBED
  INGESTION --> TOOL
  LLM --> RELATE
  EMBED --> VECTORS
  SEARCH --> BM25
  TEMPORAL --> RELATE
  RELATE --> ROCKSDB
  BM25 --> ROCKSDB
  VECTORS --> ROCKSDB
```

## Features

- **Graph-native storage** — Entities are nodes, relations are `RELATE` edges (`entity->relates_to->entity`), memories link to episodes (`memory->sourced_from->episode`) and entities (`memory->references->entity`) via SurrealDB's native graph engine
- **HNSW vector search** — Configurable-dimension HNSW indexes on entity and memory embeddings with pluggable distance metrics (Cosine, Euclidean, Manhattan, Chebyshev, Hamming, Minkowski)
- **BM25 full-text search** — Snowball-stemmed English analyzer across entity names, entity summaries, memory content, and episode content
- **Hybrid search with RRF** — Reciprocal Rank Fusion combining vector similarity and keyword relevance
- **Temporal awareness** — Every entity and relation carries `valid_from`/`valid_until` timestamps; point-in-time snapshot queries; 30-day changefeeds for auditing
- **True UPSERT** — Entities are upserted by ID with summary/embedding merging
- **Dual storage backends** — In-memory (`kv-mem`) for development and RocksDB (`kv-rocksdb`) for persistent single-node deployments
- **LLM-powered extraction** — Entity and relation extraction via Rig with OpenAI-compatible endpoints
- **Mock pipelines** — Deterministic mock embedder, entity extractor, and relation extractor for testing without API keys

## Workspace Structure

```
context-keeper/
├── crates/
│   ├── context-keeper-core/      # Models, ingestion pipeline, search (RRF), temporal manager
│   ├── context-keeper-rig/       # Rig integration: embeddings, entity/relation extraction
│   ├── context-keeper-surreal/   # SurrealDB client, graph schema, repository, vector store
│   ├── context-keeper-mcp/       # MCP server binary (scaffold)
│   └── context-keeper-cli/       # CLI binary + quickstart/temporal examples
├── migrations/                   # Reference SurrealQL schema
└── Cargo.toml                    # Workspace root
```

## Data Model

SurrealDB serves as a combined graph, vector, and document database. The schema is generated dynamically from `SurrealConfig` (embedding dimensions and distance metric).

**Node tables** (SCHEMAFULL):

| Table | Fields | Indexes |
|-------|--------|---------|
| `episode` | content, source, session_id, created_at | BM25 on content |
| `entity` | name, entity_type, summary, embedding, valid_from, valid_until | HNSW on embedding, BM25 on name + summary, UNIQUE on name |
| `memory` | content, embedding, created_at | HNSW on embedding, BM25 on content |

**Graph edge tables** (TYPE RELATION):

| Edge | Direction | Purpose |
|------|-----------|---------|
| `relates_to` | entity -> entity | Typed relationships with confidence and temporal bounds |
| `sourced_from` | memory -> episode | Links a memory to its source episode |
| `references` | memory -> entity | Links a memory to entities it mentions |

Changefeeds (30-day retention) are enabled on `entity` and `relates_to` for temporal change tracking.

## Quick Start

### With mock LLM services (no API key)

```bash
cargo run --example quickstart
```

### With real LLM extraction

```bash
# Set environment variables
export OPENAI_API_URL=https://api.openai.com/v1
export OPENAI_API_KEY=sk-...
export EMBEDDING_MODEL=text-embedding-3-small
export EMBEDDING_DIMS=1536
export EXTRACTION_MODEL=gpt-4o-mini

# Add a memory
cargo run -p context-keeper-cli -- add --text "Alice is a software engineer at Acme Corp"

# Search
cargo run -p context-keeper-cli -- search --query "Acme"

# Look up an entity
cargo run -p context-keeper-cli -- entity --name "Alice"

# List recent memories
cargo run -p context-keeper-cli -- recent --limit 5
```

### Storage backends

```bash
# In-memory (default, exports to file on exit)
cargo run -p context-keeper-cli -- --storage memory add --text "..."

# RocksDB persistent storage
cargo run -p context-keeper-cli -- --storage rocksdb:./my_data add --text "..."
```

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
      --storage                Storage backend         [env: STORAGE_BACKEND]  [default: memory]
```

## Running Tests

```bash
cargo test --workspace
```

The integration test suite covers episode/entity/memory CRUD, graph edge creation via `RELATE`, HNSW vector search, BM25 full-text search, entity UPSERT deduplication, temporal snapshots, relation invalidation, graph traversal, RRF fusion, and the full ingestion pipeline.

## Key Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `surrealdb` | 3.0.4 | Graph database with HNSW + BM25 (kv-mem, kv-rocksdb) |
| `rig-core` | 0.32.0 | LLM completions + embeddings via Rig framework |
| `tokio` | 1.x | Async runtime |
| `chrono` | 0.4 | Temporal datetime handling |
| `uuid` | 1.x | Entity/relation/memory identifiers |
| `clap` | 4.x | CLI argument parsing |
| `serde` | 1.x | Serialization/deserialization |
| `tracing` | 0.1 | Structured logging |

## License

MIT
