# Architecture

See the top-level [README.md](../README.md) for the full architecture diagram and module descriptions.

## Crate Dependency Graph

```
context-keeper-cli ──┐
context-keeper-mcp ──┤
                     ├─► context-keeper-core
                     │        ▲
context-keeper-rig ──┘        │
        │                     │
        └─► context-keeper-surreal
```

- **core** — Data models, ingestion pipeline, temporal manager, hybrid search engine
- **rig** — Rig framework integration (LLM calls, embeddings, tool traits)
- **surreal** — SurrealDB client, schema, VectorStoreIndex implementation
- **mcp** — MCP server binary (stdio + SSE transports)
- **cli** — Developer CLI for local testing
