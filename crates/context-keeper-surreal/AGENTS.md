# Surreal Crate Agent

You are a specialist for `context-keeper-surreal`, the SurrealDB storage backend.

## Ownership

- **Client** (`src/client.rs`): `connect()`, `SurrealConfig`, `StorageBackend` enum
- **Schema** (`src/schema.rs`): `apply_schema()` — table definitions, indexes, vector/BM25 config
- **Repository** (`src/repository.rs`): 35+ CRUD methods — the main workhorse (~700 lines)
- **Vector store** (`src/vector_store.rs`): HNSW vector search wrappers

`Repository` also implements `EntityResolver` from core (exact + similarity matching).

## Constraints

- Depends on `context-keeper-core` (models + traits) and `surrealdb`.
- All SurrealQL queries use parameter binding — never string interpolation.
- Temporal queries must filter on `valid_from`/`valid_until` for correctness.
- Storage backends: `Memory` (testing), `RocksDb` (default production), `Remote` (WIP WebSocket).

## Query Patterns

- Active entities: `WHERE valid_until IS NONE`
- Point-in-time: `WHERE valid_from <= $at AND (valid_until IS NONE OR valid_until > $at)`
- Namespace scoping: explicit `namespace = $ns` or `namespace IS NONE` for global scope
- Upsert key: `(name, entity_type, namespace)` — matches the composite identity from ADR-001

## When Modifying

- Adding a new query → add a method to `Repository` in `repository.rs` with parameter binding.
- Schema changes → update `schema.rs` and ensure migration path for existing RocksDB data.
- Adding a new table → define it in `apply_schema()` with appropriate indexes.
- Vector dimension changes → the HNSW index dimension is set from `SurrealConfig::embedding_dimensions`.
