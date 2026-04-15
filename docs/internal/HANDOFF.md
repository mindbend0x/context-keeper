# Context Keeper — Agent Handoff

**Branch:** `feat/prototype-v1` (active development)
**Date:** 2026-03-27
**Test suite:** 118 tests, all passing

---

## What Was Done This Session

Eight Linear issues were implemented and marked Done across parallel feature branches, then merged into `feat/prototype-v1`. A merge conflict resolution commit (`cd88b8d`) finalised the state.

### Completed Issues

| Issue | What landed |
|-------|------------|
| **FZ-59** | `ContextKeeperError` typed error enum in `crates/context-keeper-core/src/error.rs`. All crates now use `context_keeper_core::error::Result` instead of `anyhow::Result`. Surreal wraps DB errors as `StorageError`. Rig wraps LLM failures as `ExtractionFailed`/`EmbeddingFailed`. MCP maps errors via `to_mcp()`. |
| **FZ-57** | LLM extraction retry with exponential backoff (3 attempts: 100ms / 400ms / 1600ms). Entity validation rejects empty name/summary. Relation validation rejects self-referential, empty predicate, confidence > 100, dangling entity refs. 7 new unit tests. |
| **FZ-58** | Entity identity is now `(name, entity_type)` — composite index `entity_identity_idx` on `(name, entity_type, namespace)`. `find_entities_by_name_and_type()` added to repository. `find_existing()` in `EntityResolver` is **namespace-agnostic**: the same real-world entity (Alice Person) is not duplicated per namespace. |
| **FZ-12** | Memory updates (negation & deduplication). 33 negation markers. Summary merging for non-contradictory updates (word-novelty based). Cascade relation invalidation on entity contradiction via `invalidate_relations_for_entity()` (single UPDATE query). `IngestionDiff` now carries `entity_ids_to_invalidate_relations: Vec<Uuid>` and `EntityInvalidation.invalidated_id: Uuid`. |
| **FZ-13** | Entity/relation type management. 50+ `EntityType::from()` aliases. 60+ `RelationType::canonicalize()` aliases. Stop-word filter on `MockEntityExtractor` to reduce noise. Tighter LLM extraction prompts (proper nouns only, stricter relation types). |
| **FZ-61** | CI/CD — `.github/workflows/ci.yml` (build matrix Ubuntu+macOS, test, clippy, fmt) and `release.yml` (multi-arch binary release on tag push). |
| **FZ-60** | Runnable examples in `crates/context-keeper-cli/examples/`: `quickstart.rs`, `temporal_demo.rs`, `feature_showcase.rs`, `feature_showcase_llm.rs`. All work with mock extractors (no API key). |
| **FZ-53** | `context-keeper-bench` crate — **NOT yet merged** into `feat/prototype-v1`. Lives on `feat/fz-53-bench-crate`. Criterion benchmarks for ingestion, search, temporal, e2e. |

---

## Current State of `feat/prototype-v1`

### Workspace Members (Cargo.toml)

```
crates/context-keeper-core
crates/context-keeper-rig
crates/context-keeper-surreal
crates/context-keeper-mcp
crates/context-keeper-cli
test
```

`context-keeper-bench` is **missing** from `feat/prototype-v1` — it needs to be cherry-picked or merged from `feat/fz-53-bench-crate`.

### Key Architecture Decisions Made

1. **`find_existing` is namespace-agnostic** (resolves across all namespaces by `(name, entity_type)`). The namespace on an episode/memory records provenance; entity identity is global. This was the HEAD intent confirmed by the updated `test_namespace_scoped_name_search` test.

2. **`entity_identity_idx` is NOT UNIQUE** in the schema. SurrealDB `UNIQUE` indexes treat `NONE` values as distinct, which broke multi-namespace tests. Uniqueness is enforced at the application level by `EntityResolver`.

3. **Relation invalidation uses `invalidate_relations_for_entity(id)`** (single `UPDATE relates_to SET valid_until = now WHERE in/out = entity AND valid_until IS NONE`) rather than fetching then individually invalidating. This is the preferred pattern.

4. **`IngestionDiff.entity_ids_to_invalidate_relations`** — the pipeline populates this list; the caller (MCP/CLI) drives the relation invalidation. The pipeline itself remains side-effect-free.

5. **`anyhow::Result` is still used at binary boundaries** (CLI `main`, MCP `main`). Internal crate boundaries use `context_keeper_core::error::Result<T>`.

---

## Key Files Changed This Session

| File | What changed |
|------|-------------|
| `crates/context-keeper-core/src/error.rs` | **NEW** — `ContextKeeperError` enum |
| `crates/context-keeper-core/src/lib.rs` | Exports `error` module and `ContextKeeperError` |
| `crates/context-keeper-core/src/traits.rs` | Uses `error::Result`; `MockEntityExtractor` has stop-word filter |
| `crates/context-keeper-core/src/ingestion/pipeline.rs` | Enhanced negation detection, summary merging, `entity_ids_to_invalidate_relations`, `EntityInvalidation.invalidated_id` |
| `crates/context-keeper-core/src/models.rs` | 50+ `EntityType` aliases, 60+ `RelationType::canonicalize` aliases |
| `crates/context-keeper-core/src/search/expander.rs` | Uses `error::Result` |
| `crates/context-keeper-rig/src/extraction.rs` | Retry logic, entity/relation validation, improved prompts |
| `crates/context-keeper-rig/src/embeddings.rs` | Uses `error::Result`, maps to `EmbeddingFailed` |
| `crates/context-keeper-rig/src/rewriting.rs` | Uses `error::Result`, maps to `ExtractionFailed` |
| `crates/context-keeper-surreal/src/client.rs` | Uses `error::Result`, maps to `StorageError` |
| `crates/context-keeper-surreal/src/schema.rs` | `entity_identity_idx` on `(name, entity_type, namespace)` (non-unique) |
| `crates/context-keeper-surreal/src/repository.rs` | `find_existing` namespace-agnostic; `find_entities_by_name_and_type()`; `invalidate_relations_for_entity()` added |
| `crates/context-keeper-surreal/src/vector_store.rs` | Uses `error::Result` |
| `crates/context-keeper-mcp/src/tools.rs` | `to_mcp()` helper; uses `invalidate_relations_for_entity()`; `relations_invalidated` in response |
| `crates/context-keeper-cli/src/main.rs` | Uses `invalidate_relations_for_entity()` on contradiction |
| `.github/workflows/ci.yml` | **NEW** |
| `.github/workflows/release.yml` | **NEW** |
| `crates/context-keeper-cli/examples/` | 4 example binaries |

---

## Pending / Not Yet Done

### Branches not merged into `feat/prototype-v1`

- **`feat/fz-53-bench-crate`** — `context-keeper-bench` crate with criterion benchmarks. Needs to be merged. Adds `criterion` workspace dep, `crates/context-keeper-bench/` with `benches/{ingestion,search,temporal,e2e}.rs`.

### Linear Issues Still Open (Todo/Backlog)

Remaining work in the **Efficacy & Correctness** milestone (use Linear MCP to verify current state):

| Issue | Title | Priority | Notes |
|-------|-------|----------|-------|
| FZ-22 | Create efficacy tests / Graphiti comparison | High | Blocked by FZ-57 (done) |
| FZ-54 | Adapt LongMemEval + LoCoMo datasets | High | |
| FZ-23 | Create operational modes (accuracy/balanced/economic) | Medium | |
| FZ-14 | Expand entity types | Low | Blocked by FZ-58 (done) |
| FZ-32 | Token counter extension | Low | |

Publishing pipeline (Backlog):

| Issue | Title | Priority |
|-------|-------|----------|
| FZ-62 | README polish for public release | Medium |
| FZ-64 | crates.io publishing prep | Medium |
| FZ-63 | Docker Hub multi-arch publishing | Medium |
| FZ-65 | License file + CONTRIBUTING.md | Low |

### Homebrew Tap

Homebrew distribution is live. `release.yml` has an `update-tap` job that renders `Formula/context-keeper.rb.template` with release checksums and pushes to [`mindbend0x/homebrew-context-keeper`](https://github.com/mindbend0x/homebrew-context-keeper). The `TAP_REPO_TOKEN` secret is set on the main repo for cross-repo push. On each `v*` tag, the formula is automatically updated with the new version and SHA256 hashes.

---

## Development Workflow Reminders

### Building & Testing

```bash
cargo build                          # full workspace
cargo test                           # all tests (118 passing, no API key needed)
cargo test -p context-keeper-test    # integration tests only
cargo test -p context-keeper-core    # unit tests only
```

### Running

```bash
# MCP server (stdio)
cargo run -p context-keeper-mcp

# MCP server (HTTP)
MCP_TRANSPORT=http MCP_HTTP_PORT=3000 cargo run -p context-keeper-mcp

# CLI
cargo run -p context-keeper-cli -- add --text "Alice works at Acme" --source "chat"
cargo run -p context-keeper-cli -- search --query "Who works at Acme?"

# Examples (no API key)
cargo run -p context-keeper-cli --example quickstart
cargo run -p context-keeper-cli --example temporal_demo
```

### Feature Branch Convention

```bash
git checkout feat/prototype-v1
git pull origin feat/prototype-v1
git checkout -b feat/fz-XX-short-description
# ... implement ...
git cherry-pick feat/fz-XX-short-description   # or PR/merge
```

### Linear

- Team: **FZ** | Project: **Context Keeper**
- Use the Linear MCP (`plugin-linear-linear`) — call `mcp_auth` first if tools aren't available
- Mark issues Done with `save_issue(id: "FZ-XX", state: "Done")`

---

## Known Quirks & Gotchas

- **`find_existing` ignores namespace**: intentional. The `_namespace` param is accepted (trait signature) but not used in the query. Entity dedup is global by `(name, entity_type)`.
- **`entity_identity_idx` is not a UNIQUE index**: SurrealDB treats `NONE` namespace as a unique value per row, which would prevent the same entity from being created in two different sessions when namespace is omitted. Uniqueness is application-level only.
- **`IngestionDiff` has two entity-invalidation lists**: `entities_invalidated` (carries name + reason + id for the response), and `entity_ids_to_invalidate_relations` (just UUIDs, for the repo call). Both are populated together on contradiction.
- **`merge_summaries` uses word-novelty**: if the new summary adds words not in the old one, they're concatenated with `"; "`. If no new words, new summary wins outright.
- **Stop words in `MockEntityExtractor`**: ~60 common English words filtered to reduce false entity extraction (e.g., sentence-initial capitals like "The", "This", etc.).
- **Bench crate not in workspace**: `context-keeper-bench` exists on `feat/fz-53-bench-crate` but hasn't been merged. `cargo bench` will fail on `feat/prototype-v1`.

---

## Docs Update: Private Homebrew Releases (2026-04-11)

### What Changed

All installation documentation was updated to reflect the actual release channels. The crates are not published to crates.io (`publish = false`), so every `cargo install context-keeper-mcp` / `cargo install context-keeper-cli` reference was replaced with working alternatives.

### Decisions Made

1. **Removed all `cargo install` references** — crates have `publish = false` and are not on crates.io. Publishing is future work (FZ-64).
2. **Homebrew = CLI only** — the formula (`Formula/context-keeper.rb.template`) only packages the CLI binary. Docs now explicitly state this and point users to from-source or GitHub Releases for the MCP server.
3. **Canonical GitHub org is `mindbend0x`** — the release workflow, Homebrew formula, and tap all use `mindbend0x/context-keeper`. All docs, `Cargo.toml`, and config files were updated from `0x313/context-keeper` to `mindbend0x/context-keeper` for consistency.

### Install Channels After This Change

| Binary | Homebrew | From Source | GitHub Release |
|--------|----------|-------------|----------------|
| CLI (`context-keeper`) | Yes (primary) | Yes | Yes |
| MCP server (`context-keeper-mcp`) | No | Yes | Yes |

### Files Changed

| File | What changed |
|------|-------------|
| `README.md` | MCP quickstart: from-source + Releases link. CLI quickstart: Homebrew + from-source (no `cargo install`). |
| `docs/website/docs/getting-started.md` | Removed "Via Cargo" section. Added binary table and tip about Homebrew = CLI only. |
| `docs/website/docs/tutorials/cli-installation.md` | Removed "Via Cargo" subsection, kept Homebrew + from-source. |
| `docs/website/docs/cli-reference.md` | Replaced `cargo install --path` with `git clone` + `cargo build` from-source. |
| `docs/website/docs/tutorials/mcp-server-setup.md` | Replaced `cargo install context-keeper-mcp` prereq with from-source build + Releases download. |
| `Cargo.toml` | `[workspace.package].repository` URL: `0x313` → `mindbend0x`. |
| `CONTRIBUTING.md` | Clone URL and issues link: `0x313` → `mindbend0x`. |
| `docs/website/docs/contributing.md` | Clone URL and issues link: `0x313` → `mindbend0x`. |
| `docs/website/docs/tutorials/running-locally.md` | Clone URL: `0x313` → `mindbend0x`. |
| `docs/website/docs/tutorials/running-with-docker.md` | Clone URL: `0x313` → `mindbend0x`. |
| `docs/website/src/pages/index.tsx` | GitHub links: `0x313` → `mindbend0x`. |
| `docs/website/docusaurus.config.ts` | Edit URL + navbar/footer links: `0x313` → `mindbend0x`. |
| `docs/internal/AGENT_PROMPT.md` | Repo reference: `0x313` → `mindbend0x`. |
| `plugins/cursor/package.json` | Repository URL: `0x313` → `mindbend0x`. |
| `docs/website/landing-preview.html` | Replaced `cargo install` with `brew install`, updated quickstart code block, fixed GitHub URLs. |
| `plugins/cursor/README.md` | Replaced `cargo install context-keeper-mcp` prereq with from-source build + Releases download. |
| `plugins/perplexity/README.md` | Replaced `cargo install context-keeper-mcp` prereq with from-source build + Releases download. Updated binary resolution table. |

### Remaining Gaps

- **MCP server not in Homebrew**: adding it to the formula requires a release infra change (update `Formula/context-keeper.rb.template` to also install the MCP binary).
- **crates.io publishing** (FZ-64): once published, `cargo install` references can be restored.
- **`docs/website/build-preview/`**: static build output is stale — needs regeneration after these doc changes.
