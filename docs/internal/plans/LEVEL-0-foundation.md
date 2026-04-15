# Level 0: Foundation Sprint

**Goal:** Address architecture prerequisites from ADR-001 that must land before correctness work begins.
**Timeline:** 1-2 days
**Depends on:** Nothing (start here)
**Unlocks:** Level 1 (Core Correctness)

## Tasks

### 0.1 — Typed Error Hierarchy (ADR-001 R3)

**Linear:** New issue needed
**Files:** `crates/context-keeper-core/src/error.rs` (new), all crates

Introduce `ContextKeeperError` enum in core:
- `LlmUnavailable` — LLM endpoint unreachable or timed out
- `ExtractionFailed { reason: String }` — LLM returned malformed output
- `EntityNotFound { name: String }` — Entity lookup miss
- `StorageError { source: Box<dyn Error> }` — SurrealDB failures
- `ValidationError { field: String, reason: String }` — Bad input data
- `BudgetExceeded` — Future use for FZ-32/FZ-23

Implement `From<ContextKeeperError>` for `McpError` in the MCP crate so errors propagate with meaningful codes.

Replace `anyhow::Result` progressively — start with core and rig crates.

### 0.2 — LLM Extraction Retry + Validation (ADR-001 R2)

**Linear:** New issue needed
**Files:** `crates/context-keeper-rig/src/extraction.rs`

- Add retry-with-backoff (3 attempts, exponential) for transient LLM failures
- Validate extracted entities: reject empty names, empty summaries, unknown types
- Validate extracted relations: reject self-referential, reject empty predicates, reject confidence < 0 or > 100
- Log warnings for rejected extractions (don't silently drop)

### 0.3 — Composite Entity Identity (ADR-001 R1)

**Linear:** New issue needed
**Files:** `crates/context-keeper-surreal/src/schema.rs`, `repository.rs`, `crates/context-keeper-core/src/models.rs`

- Change entity unique index from `name` alone to `(name, entity_type)`
- Update `upsert_entity()` to use composite key
- Update `find_entities_by_name()` to optionally filter by type
- Migration path: re-index existing data (SurrealDB schema update)

**Risk:** This is the most invasive change. All tests that create entities need updating. Do this AFTER 0.1 and 0.2.

## Definition of Done

- [ ] `cargo test` passes with new error types
- [ ] Extraction retry works (test with mock that fails first 2 times)
- [ ] Entity composite key works in tests
- [ ] No more `anyhow::Result` in core or rig crates

## Delete This File When

All three tasks are merged and tests pass on `feat/prototype-v1`.
