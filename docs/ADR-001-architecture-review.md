# ADR-001: Architecture Review — Context Keeper

**Status:** Proposed
**Date:** 2026-03-22
**Deciders:** mindbend0x

## Context

Context Keeper has completed its first milestone ("Base lib for context insertion and search") — the prototype, Rig integration, and external LLM integrations are done. Before diving into the Efficacy & Correctness milestone (FZ-12 through FZ-32), this is a good checkpoint to evaluate the architecture: what's working well, where the risks are, and what decisions should be revisited or made explicit before the codebase grows.

This review covers the five-crate workspace as of 2026-03-22.

## Current Architecture

```
context-keeper-cli ──┐
context-keeper-mcp ──┤
                     ├─► context-keeper-core (pure logic, trait definitions)
                     │        ▲
context-keeper-rig ──┘        │
        │                     │
        └─► context-keeper-surreal (SurrealDB storage)
```

**Core** owns data models (`Episode`, `Entity`, `Memory`, `Relation`), the ingestion pipeline, hybrid search (RRF fusion), temporal management, and trait definitions (`Embedder`, `EntityExtractor`, `RelationExtractor`, `QueryRewriter`).

**Rig** implements those traits against the Rig framework (OpenAI-compatible endpoints). **Surreal** provides the `Repository` (CRUD, vector/keyword search, graph traversal, temporal queries). **MCP** and **CLI** are thin binaries that wire everything together.

---

## Strengths

### 1. Clean trait-based decoupling

The core crate has zero heavyweight dependencies — it defines traits and pure functions. LLM providers and storage backends are swappable at the binary level. This is already paying off: the mock implementations let the entire test suite run without API keys or a database process.

### 2. Hybrid search with principled fusion

The combination of HNSW vector search + BM25 keyword search, fused via Reciprocal Rank Fusion (K=60), is a well-understood IR technique. The query expander (LLM-generated semantic variants → parallel search → RRF merge) adds a meaningful recall boost. This is a solid foundation for the upcoming "modes" work (FZ-23).

### 3. Temporal graph as a first-class concept

Soft deletes via `valid_from` / `valid_until`, point-in-time snapshots, and SurrealDB changefeeds (30-day retention) give the system a built-in audit trail and the ability to answer "what did the graph look like at time T?" Few knowledge graph systems get this right from the start.

### 4. SurrealDB as a single-engine solution

Using SurrealDB for document storage, graph edges (TYPE RELATION), vector indexes (HNSW), and full-text search (BM25) avoids the operational complexity of stitching together separate systems. For a project at this stage, that's a strong trade-off.

### 5. Ingestion pipeline as a pure function

`ingest()` takes trait objects and returns an `IngestionResult` — the caller decides how to persist it. This makes the pipeline testable, composable, and easy to extend (e.g., adding a validation step before persistence).

### 6. Multi-transport MCP server

Supporting both stdio (local clients like Claude Desktop / Cursor) and streamable HTTP (Docker, remote) covers the two main deployment scenarios cleanly.

---

## Risks & Concerns

### R1: Entity name as the global upsert key

**What:** The unique index on `entity.name` means entity identity is determined solely by name. "Alice" from a work conversation and "Alice" from a podcast will silently merge into the same node.

**Impact:** As the system handles more diverse content (FZ-14 entity type expansion, multi-session usage), name collisions will corrupt the graph.

**Recommendation:** Consider a composite key — e.g., `(name, entity_type)` or `(name, namespace)` — or introduce an entity resolution step that's smarter than exact name match. This is closely related to FZ-12 (memory updates) and FZ-13 (entity relationship management), so it should be addressed as part of that work rather than deferred.

### R2: No retry or validation on LLM extraction

**What:** The Rig extractors call the LLM once and expect well-formed JSON. If the model returns malformed output, the extraction silently returns an empty result (or the Rig framework errors out with an `anyhow` error that gets stringified).

**Impact:** At the core of the Efficacy & Correctness milestone — if extraction is unreliable, everything downstream suffers. The upcoming benchmarking work (FZ-22) will expose this, but the fix should be in the extraction layer, not in the test harness.

**Recommendation:** Add retry-with-backoff for transient LLM failures, schema validation on the parsed output (reject entities with empty names, relations referencing non-existent entities), and structured error types so callers can distinguish "LLM unavailable" from "LLM returned garbage."

### R3: `anyhow::Result` everywhere — no typed error hierarchy

**What:** Every function returns `anyhow::Result`. The `thiserror` dependency exists in the workspace but isn't used.

**Impact:** Callers can't pattern-match on failure modes. The MCP server maps all errors to `McpError::internal_error(msg)` — the client sees a generic 500 regardless of cause. As the system adds modes (FZ-23) and token budgets (FZ-32), distinguishing "over budget" from "LLM down" from "entity not found" becomes important.

**Recommendation:** Introduce a `ContextKeeperError` enum in core with variants like `LlmUnavailable`, `ExtractionFailed`, `EntityNotFound`, `StorageError`, `BudgetExceeded`. Implement `From<ContextKeeperError>` for `McpError` so the MCP layer can return meaningful error codes.

### R4: Graph traversal depth is hardcoded to 1-hop

**What:** `get_graph_neighbors()` accepts a `_depth` parameter but ignores it — always doing a single-hop traversal.

**Impact:** Limits the value of the knowledge graph for multi-hop reasoning. SurrealDB supports arbitrary-depth traversal natively, so this is a feature gap, not a technology limitation.

**Recommendation:** Expose configurable depth (capped, e.g., 1–3). This ties directly into the "Accuracy" mode from FZ-23, where deeper graph walks would improve context quality at the cost of more tokens.

### R5: RRF constant (K=60) is not tunable

**What:** The Reciprocal Rank Fusion constant is hardcoded. The relative weight between vector similarity and keyword relevance can't be adjusted.

**Impact:** Different use cases (short factual queries vs. long narrative searches) may benefit from different fusion parameters. The "modes" feature (FZ-23) would be a natural place to vary this.

**Recommendation:** Make K configurable per-mode or per-search-call. Consider also allowing explicit vector/keyword weight blending as an alternative to pure RRF.

### R6: No Repository trait — tight coupling to SurrealDB

**What:** The `Repository` struct is a concrete type, not a trait. Core doesn't depend on it directly (the ingestion pipeline returns data, doesn't persist it), but the MCP server and CLI are coupled to the SurrealDB-specific Repository.

**Impact:** If you ever need a different storage backend (e.g., SQLite for the local app in FZ-25/FZ-26, or Postgres for a hosted version), you'd need to duplicate the Repository implementation or refactor under pressure.

**Recommendation:** This is fine for now — YAGNI applies. But when the "Local DB" work (FZ-25) starts, extracting a `RepositoryTrait` in core and making `SurrealRepository` one implementation of it would be the right move. Flag it for the Privacy/Security milestone, not now.

### R7: Manual SQL construction in Repository

**What:** All SurrealQL queries are hand-constructed strings with parameter binding. There's no query builder or abstraction layer.

**Impact:** Moderate maintenance risk. SurrealDB is pre-1.0 and its query syntax has changed between versions. A schema or query language change could require updates across 600+ lines of repository code.

**Recommendation:** Acceptable at this scale. If it becomes painful, a thin query builder (internal to the surreal crate) could help. The bigger mitigation is good test coverage — the test suite already exercises most query paths.

### R8: In-memory backend has no automatic persistence

**What:** The `Memory` storage backend loses all data on shutdown unless explicitly exported via `export()`. The import path exists but is manual.

**Impact:** Fine for testing. Risky if anyone accidentally runs a long session with the memory backend and loses work.

**Recommendation:** Add a warning log on startup when using the memory backend. Consider auto-export on graceful shutdown (the HTTP transport already has `ctrl_c()` handling — could trigger export there).

---

## Coupling Analysis

| Boundary | Coupling | Notes |
|----------|----------|-------|
| Core → LLM provider | **Loose** | Trait-based. Any `Embedder`/`Extractor` works. |
| Core → Storage | **Loose** | Pipeline returns data; caller persists. |
| Rig → OpenAI | **Tight** | Rig's `openai::Client` is the only provider. Adding Claude/Gemini means new provider impls. |
| Surreal → SurrealDB | **Tight** | Expected — it's the storage crate. |
| MCP/CLI → Rig + Surreal | **Moderate** | Binaries wire concrete types. Refactoring to a factory pattern would reduce this. |
| MCP → rmcp | **Moderate** | Macro-based tool registration. Switching MCP frameworks would be a rewrite of tools.rs. |

The tightest coupling that matters for the roadmap is **Rig → OpenAI**. The upcoming work on local models (FZ-24) and multiple provider plugins (FZ-19 through FZ-21) will stress this. Rig itself supports multiple providers, but the current code only uses `openai::Client`. The fix is straightforward: parameterize the provider in the Rig crate constructors rather than hardcoding OpenAI.

---

## Recommendations Summary

| # | Recommendation | Priority | Tied to |
|---|---------------|----------|---------|
| 1 | Composite entity identity (not just name) | **High** | FZ-12, FZ-13 |
| 2 | LLM extraction retry + output validation | **High** | FZ-22 (benchmarking) |
| 3 | Typed error enum in core | **Medium** | FZ-23 (modes need error discrimination) |
| 4 | Configurable graph traversal depth | **Medium** | FZ-23 (Accuracy mode) |
| 5 | Tunable RRF constant / fusion weights | **Medium** | FZ-23 (modes) |
| 6 | Extract Repository trait (when needed) | **Low** | FZ-25 (Local DB) |
| 7 | Parameterize LLM provider in Rig crate | **Medium** | FZ-24 (local models) |
| 8 | Warn / auto-export on memory backend | **Low** | General robustness |

Items 1–2 should be addressed as part of the current milestone. Items 3–5 are natural companions to FZ-23. Items 6–8 can wait for their respective milestones.

---

## Consequences

If these recommendations are adopted:

- **Easier:** Adding new entity types, swapping models, diagnosing failures, tuning search quality per mode.
- **Harder:** Slightly more boilerplate for error handling and entity resolution. The composite identity change will require a schema migration.
- **Revisit later:** The Repository trait extraction (R6) should be re-evaluated when the Local DB milestone begins. The MCP framework coupling (rmcp) is fine unless the MCP spec changes significantly.

---

## Action Items

1. [ ] Decide on entity identity strategy (composite key vs. resolution step) before FZ-12/FZ-13 work lands
2. [ ] Add retry + validation to Rig extractors as part of FZ-22 benchmarking setup
3. [ ] Introduce `ContextKeeperError` enum when starting FZ-23
4. [ ] Make RRF K and graph depth configurable as part of FZ-23
5. [ ] Parameterize Rig provider when FZ-24 begins
