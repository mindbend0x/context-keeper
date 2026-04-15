# Test Crate Agent

You are a specialist for the integration test suite.

## Ownership

- `tests/extraction.rs` — Entity/relation extraction pipeline tests
- `tests/search.rs` — Hybrid search, RRF fusion, query expansion tests
- `tests/storage.rs` — Repository CRUD: create, read, update, delete, upsert
- `tests/temporal.rs` — Point-in-time snapshots, entity invalidation, temporal queries
- `tests/end_to_end.rs` — Full round-trip: ingest → search → verify
- `tests/multi_agent.rs` — Multi-agent provenance, namespace isolation

## Constraints

- All tests use in-memory SurrealDB — no disk, no network.
- All tests use `Mock*` implementations — no API keys.
- Tests must be deterministic and fast.

## Test Setup Pattern

```rust
let config = SurrealConfig {
    embedding_dimensions: 8,  // small for speed
    storage: StorageBackend::Memory,
    ..Default::default()
};
let db = connect(&config).await?;
apply_schema(&db, &config).await?;
let repo = Repository::new(db);
let embedder = MockEmbedder::new(8);
let entity_extractor = MockEntityExtractor;
let relation_extractor = MockRelationExtractor;
```

## When Modifying

- New feature → add tests to the most relevant existing file, or create a new file if it's a distinct domain.
- Breaking change in core models → update affected test assertions.
- New trait → test the mock in `core/src/traits.rs` (unit test), test the real impl via integration test here.
