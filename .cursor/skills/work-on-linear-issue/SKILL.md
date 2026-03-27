---
name: work-on-linear-issue
description: >-
  Workflow for picking up and implementing a Linear issue for Context Keeper.
  Use when starting work on an FZ-* issue, creating a feature branch, or
  following the project's branching and issue workflow.
---

# Work on Linear Issue

## Context

- Team: **FZ** | Project: **Context Keeper** | Issue key format: `FZ-*`
- Active branch: `feat/prototype-v1`
- Feature branches off `feat/prototype-v1`

## Workflow

### 1. Understand the Issue

Use the Linear MCP tools to fetch issue details:

```
linear: get_issue(issueId: "FZ-XX")
```

Read the issue description, acceptance criteria, and linked issues. Cross-reference with:
- `docs/ROADMAP.md` for sequencing context
- `docs/plans/` for detailed level plans
- `docs/ADR-001-architecture-review.md` for architectural guidance

### 2. Create a Feature Branch

```bash
git checkout feat/prototype-v1
git pull origin feat/prototype-v1
git checkout -b feat/fz-XX-short-description
```

Branch naming: `feat/fz-XX-short-description` (e.g., `feat/fz-12-memory-updates`).

### 3. Identify Affected Crates

Check the roadmap's "Touches" section for each issue. Common patterns:

| Issue area | Crates typically affected |
|-----------|--------------------------|
| Extraction quality | core (pipeline), rig (prompts/extraction) |
| Entity management | core (models), surreal (repository), rig (extraction) |
| Search improvements | core (search/engine), surreal (vector_store) |
| New MCP capability | mcp (tools), possibly core |
| Temporal features | core (temporal), surreal (repository) |

### 4. Implement

Follow the crate conventions from the project rules. Key files:
- Pipeline: `crates/context-keeper-core/src/ingestion/pipeline.rs`
- Search: `crates/context-keeper-core/src/search/engine.rs`
- Extraction: `crates/context-keeper-rig/src/extraction.rs`
- Repository: `crates/context-keeper-surreal/src/repository.rs`
- MCP tools: `crates/context-keeper-mcp/src/tools.rs`

### 5. Test

```bash
cargo test                          # Full suite
cargo test -p context-keeper-test   # Integration tests
```

### 6. PR and Linear Update

- Open PR against `feat/prototype-v1`
- Update Linear issue status via MCP tools
- Reference the Linear issue in the PR description: `Closes FZ-XX`
