# Level 3: Post-Launch (Next Phase)

**Goal:** After the public release, build the evidence and distribution layers. This is what turns "a published repo" into "a project with traction."
**Timeline:** 2-4 weeks after launch
**Depends on:** Level 2 (Published release)
**Note:** This file is for reference. Don't start this until Levels 0-2 are complete.

## Workstreams

### 3A — Benchmarking (FZ-22, FZ-53, FZ-54, FZ-56)

Already planned in Linear. Summary:
- Create `context-keeper-bench` crate with criterion benchmarks
- Adapt LongMemEval + LoCoMo datasets
- Run first benchmark pass, publish numbers
- Build HTML benchmark dashboard

Target numbers: LoCoMo >= 75%, LongMemEval temporal >= 60%.

### 3B — Demo Assets (FZ-55)

- Record asciinema CLI demo (temporal reasoning showcase)
- Write the "Building Context Keeper" technical blog post
- Create 2-minute video walkthrough

### 3C — Operational Modes (FZ-23)

- Accuracy / Balanced / Economic modes
- Per-mode: model selection, prompt complexity, search strategy, RRF tuning
- Configurable graph traversal depth (ADR-001 R4)
- Tunable RRF constant (ADR-001 R5)

### 3D — Plugin Ecosystem (FZ-15, FZ-17)

- Polish Claude Desktop plugin (already functional via MCP)
- Ship Cursor plugin MVP (sidebar + search command)
- Open waitlist for niche validation (FZ-33)

### 3E — Local Models (FZ-24)

- Ollama/llama.cpp integration for 3B models
- Fully offline operation path
- Benchmark quality at 3B scale

### 3F — Token Counting (FZ-32)

- Hook into Rig calls for input/output token counts
- Aggregate per-operation and per-session
- Expose via MCP resource

## This File Is Reference Only

Do not create Linear issues or branches for Level 3 until Levels 0-2 are shipped. Priorities may shift based on community feedback after launch.
