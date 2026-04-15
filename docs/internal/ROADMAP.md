# Roadmap — Efficacy & Correctness

Current milestone for Context Keeper. The base library (prototype, Rig integration, external LLM integrations) is complete. This phase focuses on making extraction and memory management accurate, measurable, and tunable.

## Status overview

| Issue | Title | Status | Priority |
|-------|-------|--------|----------|
| FZ-12 | Memory updates | Todo | — |
| FZ-13 | Improve Entity Relationship management | Todo | — |
| FZ-14 | Expand Entity types | Todo | — |
| FZ-22 | Create efficacy tests / benchmarking | Todo | — |
| FZ-23 | Create "modes" | Todo | — |
| FZ-24 | Local / lightweight models (3B) | Backlog | — |
| FZ-32 | Token counter extension (hooks?) | Todo | — |

## Task details

### FZ-12 — Memory updates

Handle mutation of previously ingested knowledge: negation of prior info, re-identification of the same entities across episodes, and temporal invalidation of stale facts.

Key areas:

- Detect when new text contradicts an existing entity summary or relation and update `valid_until` accordingly.
- Improve entity matching so that "Alice at Acme" and "Alice (software engineer)" resolve to the same node.
- Surface update diffs (what changed, what was invalidated) in the ingestion response.

Touches: `context-keeper-core` (ingestion pipeline, temporal manager), `context-keeper-rig` (extraction prompts), `context-keeper-surreal` (UPSERT logic).

### FZ-13 — Improve Entity Relationship management

The variety of extracted entities is currently too large — relationships are noisy and under-constrained. This task tightens the extraction schema and deduplicates relations.

Key areas:

- Constrain the set of allowed relation types (or introduce a canonical mapping).
- Merge duplicate / near-duplicate relations during ingestion.
- Add confidence-based pruning so low-quality edges don't pollute search.

Touches: `context-keeper-core` (models, ingestion), `context-keeper-rig` (relation extraction prompts).

### FZ-14 — Expand Entity types

Introduce a richer entity type taxonomy beyond the current generic extraction.

Key areas:

- Define a configurable entity type enum (Person, Organization, Location, Concept, Event, etc.).
- Update extraction prompts to classify entities by type.
- Enable type-filtered search (e.g. "show me all Person entities").

Touches: `context-keeper-core` (models), `context-keeper-rig` (extraction), `context-keeper-surreal` (schema, queries).

### FZ-22 — Create efficacy tests / benchmarking

Build a test harness that measures extraction and retrieval quality, using Graphiti as a baseline.

Key areas:

- Curate a golden dataset of input episodes → expected entities, relations, and search results.
- Implement precision / recall / F1 metrics for entity extraction and relation extraction.
- Benchmark retrieval relevance (MRR, NDCG) across search modes.
- Make it runnable via `cargo test` or a dedicated binary so different models can be compared.

Touches: `test/` or a new `crates/context-keeper-bench/`, plus `context-keeper-core` and `context-keeper-rig`.

### FZ-23 — Create "modes"

Expose operational modes that trade token usage for accuracy:

- **Accuracy** — expanded queries, multi-pass extraction, higher-capability models.
- **Balanced** — default behavior, single-pass extraction.
- **Economic** — minimal prompts, smaller models, skip query expansion.

Key areas:

- Define a `Mode` enum threaded through the ingestion and search pipelines.
- Per-mode configuration of model selection, prompt complexity, and search strategy.
- Surface the mode as a CLI flag, MCP server config, and potentially runtime-switchable.

Touches: `context-keeper-core` (pipeline config), `context-keeper-rig` (model selection), `context-keeper-mcp` and `context-keeper-cli` (flag / config).

### FZ-32 — Token counter extension (hooks?)

Track token usage across LLM calls to enable cost monitoring and mode-aware budgeting.

Key areas:

- Hook into Rig's completion / embedding calls to capture input + output token counts.
- Aggregate per-operation (ingestion, search, expansion) and per-session.
- Expose via MCP resource or CLI command.

Touches: `context-keeper-rig`, `context-keeper-core` (metrics), `context-keeper-mcp` (optional resource).

### FZ-24 — Local / lightweight models (3B)

Currently in Backlog. Enable running extraction and embedding with small local models (e.g. Phi-3, Llama 3B) for fully offline / privacy-first usage.

Key areas:

- Integration with a local inference runtime (llama.cpp, Ollama, or similar).
- Evaluate extraction quality at 3B scale (ties into FZ-22 benchmarking).
- May depend on FZ-23 modes (Economic mode as the natural fit for small models).

Touches: `context-keeper-rig` (new provider), `context-keeper-core` (model config).

## Suggested sequencing

```
                    ┌─────────┐
              ┌────►│  FZ-22  │ benchmarking (enables measuring everything else)
              │     └────┬────┘
              │          │ validates
┌─────────┐   │     ┌────▼────┐     ┌─────────┐
│  FZ-13  ├───┤────►│  FZ-14  │────►│  FZ-23  │ modes
└─────────┘   │     └─────────┘     └────┬────┘
  relations   │                          │
              │     ┌─────────┐     ┌────▼────┐
              └────►│  FZ-12  │     │  FZ-32  │ token counter
                    └─────────┘     └────┬────┘
                      memory             │
                      updates       ┌────▼────┐
                                    │  FZ-24  │ local models
                                    └─────────┘
```

Recommended order:

1. **FZ-22** first — you can't improve what you can't measure.
2. **FZ-13 + FZ-12** in parallel — tighten relations and fix memory updates. These are the core quality issues.
3. **FZ-14** after FZ-13 — expanding entity types is easier once the relation schema is clean.
4. **FZ-23** once extraction quality is solid — modes are a knob on top of a working pipeline.
5. **FZ-32** alongside or after FZ-23 — token counting informs mode budgeting.
6. **FZ-24** last — local models are a deployment concern that benefits from all the above.

## What's after this milestone

For reference, the project has two more milestones in the backlog:

- **Plugins and Connectors** — MCP, Cursor, ChatGPT, Claude, Gemini, Perplexity integrations (FZ-15 through FZ-21).
- **Privacy, Security and Package** — Local DB, local app (TUI / Sniffnet-style), purchasable release (FZ-25 through FZ-27).
