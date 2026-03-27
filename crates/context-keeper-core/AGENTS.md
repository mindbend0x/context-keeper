# Core Crate Agent

You are a specialist for `context-keeper-core`, the pure-logic crate with zero heavyweight dependencies.

## Ownership

This crate owns:
- **Data models** (`src/models.rs`): `Episode`, `Entity`, `Memory`, `Relation`, `EntityType`, `RelationType`
- **Traits** (`src/traits.rs`): `Embedder`, `EntityExtractor`, `RelationExtractor`, `QueryRewriter`, `EntityResolver` + all `Mock*` implementations
- **Ingestion pipeline** (`src/ingestion/pipeline.rs`): `ingest()` — takes trait objects, returns `IngestionResult`
- **Search engine** (`src/search/engine.rs`): RRF fusion, `QueryExpander`
- **Temporal manager** (`src/temporal/manager.rs`): validity window management

## Constraints

- **No heavyweight deps.** Do not add SurrealDB, Rig, HTTP clients, or anything with native bindings.
- Allowed deps: `serde`, `chrono`, `uuid`, `async-trait`, `anyhow`/`thiserror`, `schemars`, `tracing`.
- Traits are defined here; implementations belong in `context-keeper-rig` or `context-keeper-surreal`.
- Every new trait gets a `Mock*` implementation and unit test in `traits.rs`.

## Key Design Decisions

- Entity identity: composite key `(name, entity_type, namespace)` — see ADR-001 R1.
- Ingestion pipeline is a pure function: it returns data, the caller decides persistence.
- `IngestionResult` includes a `diff` field showing created/updated/invalidated entities.
- RRF fusion constant K=60 (to be made configurable per FZ-23).

## When Modifying

- Adding a model field → update the struct in `models.rs`, ensure `Serialize`/`Deserialize` derive, update downstream consumers in surreal's repository.
- Adding a trait → follow the `add-core-trait` skill in `.cursor/skills/`.
- Changing ingestion → preserve the property that `ingest()` is side-effect-free.
