---
name: test-runner
description: >-
  Run the full Context Keeper test suite and diagnose failures.
  Use when the user asks to run tests, check if tests pass, verify
  changes didn't break anything, or troubleshoot test failures.
---

# Test Runner

## Run the Full Suite

```bash
cargo test --workspace 2>&1
```

Or equivalently: `make test`

This runs every unit test (in-crate `#[cfg(test)]` blocks) **and** every integration test in the `test/` crate.

## Workspace Test Layout

| Location | What it covers |
|---|---|
| `crates/context-keeper-core/src/**` | Unit tests for models, traits, ingestion, search, temporal |
| `crates/context-keeper-surreal/tests/` | SurrealDB integration tests |
| `crates/context-keeper-rig/src/**` | LLM adapter unit tests |
| `crates/context-keeper-bench/src/**` | Benchmark config/dataset parsing tests |
| `test/tests/extraction.rs` | Entity/relation extraction pipeline |
| `test/tests/search.rs` | Hybrid search, RRF fusion, query expansion |
| `test/tests/storage.rs` | Repository CRUD operations |
| `test/tests/temporal.rs` | Point-in-time snapshots, entity invalidation |
| `test/tests/end_to_end.rs` | Full ingest-to-search round trips |
| `test/tests/multi_agent.rs` | Multi-agent provenance, namespace isolation |

## Running Specific Subsets

When diagnosing a failure, narrow scope to save time:

```bash
# Single crate
cargo test -p context-keeper-core

# Single integration test file
cargo test -p context-keeper-test --test search

# Single test function
cargo test -p context-keeper-test --test end_to_end test_conversation_memory

# Show stdout/stderr (useful for metrics reports)
cargo test --workspace -- --nocapture
```

## Diagnosing Failures

After running the suite, follow this process:

### 1. Categorise the failure

Read the compiler/test output and classify:

| Category | Signals | Likely cause |
|---|---|---|
| **Compile error** | `error[E...]`, no tests run | Changed a trait signature, model struct, or import without updating dependents |
| **Assertion failure** | `assertion ... failed`, `left != right` | Logic change that shifted expected outputs, or a broken invariant |
| **Panic / unwrap** | `thread panicked`, `called unwrap() on None` | Missing data setup, incorrect mock, or new code path hitting None |
| **Timeout** | test hangs, no output | Deadlock or unbounded loop, usually async-related |
| **Metrics threshold** | `below threshold` in assertion msg | Search quality regression (MRR, F1, precision, recall) |

### 2. Trace to root cause

- **Compile errors**: Read the full error chain. Cargo prints the originating crate first. Check if a trait, struct, or function signature changed in `context-keeper-core` — downstream crates (`rig`, `surreal`, `test`) must match.
- **Assertion failures**: Look at `left` vs `right` values. If it's a count mismatch, check whether new ingestion logic creates more/fewer entities. If it's a name mismatch, check `MockEntityExtractor` output.
- **Integration test failures**: The `test/` crate uses `TestEnv` (in-memory SurrealDB + mocks). If `TestEnv::new()` fails, check `connect_memory` or `apply_schema` in `context-keeper-surreal`.
- **Metrics regressions**: These tests (`test_full_pipeline_metrics_report`, `test_search_after_bulk_ingest`) assert aggregate MRR/F1 >= 0.5. A regression usually means the mock embedder or search fusion logic changed.

### 3. Respond with a summary

After running and analysing, provide:

1. **Pass/fail count** — e.g. "47 passed, 2 failed"
2. **Failed test names** — full `crate::module::test_name`
3. **Root cause per failure** — one sentence each
4. **Fix recommendations** — concrete next steps (file + what to change)

## Key Constraints

- All integration tests use **in-memory SurrealDB** — no disk, no network, no `.env` required.
- All tests use **Mock\*** implementations — no API keys needed.
- Tests must be **deterministic and fast**. If a test is flaky, that's a bug.
- CI runs `cargo test --workspace` on Ubuntu. If tests pass locally on macOS but fail in CI, check for platform-specific path handling or RocksDB feature gates.
