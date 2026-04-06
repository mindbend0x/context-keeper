# Context Keeper — Benchmark & Demo Strategy

_Last updated: 2026-03-26_

## The Competitive Landscape (March 2026)

The AI memory space has matured rapidly. Here's where the key players stand:

| System | Architecture | Top Benchmark | Stars | Key Weakness |
|--------|-------------|---------------|-------|-------------|
| **Graphiti/Zep** | Python/Neo4j temporal graph | LongMemEval 71.2%, LoCoMo 75.1% | ~20K | Few integrations, Python-only |
| **Mem0** | Vector + optional graph (Pro) | LOCOMO 67.1%, p95 latency 200ms | ~41K | Graph gated behind $249/mo paywall |
| **Letta/MemGPT** | OS-inspired memory hierarchy | #1 Terminal-Bench | ~15K | Less suited for stateless memory |
| **LangMem** | Lightweight Python library | LoCoMo 78.1% | ~5K | Minimal benchmark story |
| **Memori Labs** | Structured data + efficiency | LoCoMo 81.95% at 4.98% cost | New | Limited ecosystem maturity |
| **Observational Memory (Mastra)** | Passive observation + reflection | LongMemEval 94.9% | New | Very recent (Feb 2026) |

### Where Context Keeper Fits

Context Keeper's differentiation is at the intersection of three things no competitor offers together:

1. **Rust-native performance** — No Python/Neo4j dependency, embedded SurrealDB
2. **Local-first architecture** — In-memory or RocksDB, no cloud required
3. **MCP-native distribution** — First-class MCP server, not a wrapper

The gap: we need **numbers** to prove it.

---

## Part 1: Benchmarking Strategy

### Standard Benchmarks to Adopt

The field uses three primary evaluation frameworks. We should run all three to avoid the "cherry-picked benchmark" criticism that plagues competitors (Zep's LoCoMo scores were publicly disputed).

| Benchmark | What It Tests | Scale | Why It Matters |
|-----------|--------------|-------|---------------|
| **LongMemEval** | Temporal reasoning, multi-session QA, fact updates | 500 Qs, 115K-1.5M tokens | Graphiti's flagship; temporal is our strength too |
| **LoCoMo** | Long conversation memory | 81 Q&A pairs, 35 sessions | Most-used comparison benchmark |
| **DMR** | Deep memory retrieval | 500 conversations, 60 msgs each | Tests consistency at scale |

### Three Measurement Axes (Prioritized)

**Axis 1: Correctness (F1 / Recall) — P0**

This is table stakes. Without published accuracy numbers, Context Keeper is "why not just use Graphiti?"

What we already have:
- `test/src/metrics.rs` — Precision, recall, F1, MRR, recall@k, NDCG@k
- `test/src/fixtures.rs` — 5 parameterized scenarios with groundtruth
- `test/tests/end_to_end.rs` — Aggregate metrics report (F1 ≥ 0.5, MRR ≥ 0.5)

What we need to build:
- **Adapter to run LongMemEval/LoCoMo/DMR datasets** through CK's pipeline
- **LLM-as-Judge scoring** (Mem0's approach) alongside exact-match F1
- **Temporal reasoning subset** — our expected differentiator vs. Mem0 (they score 21.71% on temporal, Graphiti scores ~54%)

Target numbers to be competitive:
- LoCoMo: ≥75% (matches Graphiti, beats Mem0's 67%)
- LongMemEval temporal: ≥60% (beats Graphiti's 71% on temporal subset if possible)
- DMR: ≥90% (baseline territory)

**Axis 2: Speed / Latency — P1**

This is the Rust story. Every competitor is Python. We should be measuring:

| Operation | Metric | Expected Advantage |
|-----------|--------|-------------------|
| Single episode ingestion | ms/episode (extract + embed + store) | 2-5x vs Python/Neo4j |
| Hybrid search (RRF) | ms/query (vector + BM25 + fusion) | 3-10x vs Neo4j |
| Batch ingestion (1K episodes) | episodes/sec throughput | Memory-bound, not GIL-bound |
| Cold start to first query | seconds | Embedded DB vs. Neo4j startup |
| Temporal snapshot | ms/snapshot | SurrealDB vs. Neo4j Cypher |

What we already have:
- In-memory SurrealDB in TestEnv (pure speed, no network)
- Feature showcase example with wall-clock timing

What we need:
- `criterion` benchmarks for each operation
- Comparative harness that runs the same dataset through CK and Graphiti
- Latency percentiles (p50, p95, p99) not just averages

**Axis 3: Cost Efficiency — P2**

Mem0's strongest pitch is token efficiency (91% lower latency, 90%+ token savings). We need our own numbers.

Metrics:
- Tokens per ingestion (extraction prompt size)
- Tokens per search (query rewriting + expansion)
- Total cost per 1K memories (at OpenAI pricing)
- Cost comparison: accuracy mode vs. balanced vs. economic (ties to FZ-23)

### Implementation: `crates/context-keeper-bench/`

New crate in the workspace:

```
crates/context-keeper-bench/
├── Cargo.toml          # criterion, serde_json
├── benches/
│   ├── ingestion.rs    # Episode ingestion throughput
│   ├── search.rs       # Hybrid search latency
│   ├── temporal.rs     # Snapshot performance
│   └── e2e.rs          # Full pipeline benchmarks
├── datasets/
│   ├── longmemeval/    # LongMemEval adapter
│   ├── locomo/         # LoCoMo adapter
│   └── dmr/            # DMR adapter
├── src/
│   ├── runner.rs       # Benchmark orchestrator
│   ├── report.rs       # HTML report generator
│   ├── compare.rs      # Cross-system comparison
│   └── cost.rs         # Token counting & cost calc
└── reports/            # Generated HTML dashboards
```

### CI Integration

Run benchmarks on every PR to `main`:
- `cargo bench` for latency regression detection
- Accuracy suite on the 5 existing fixtures (fast, mock LLM)
- Weekly full benchmark run against LongMemEval/LoCoMo with real LLM
- Store results in `reports/` with git-tracked history for trend analysis

---

## Part 2: Demo Strategy

### The Demo Must Show Three Things

1. **"It works"** — Accurate memory extraction, search, and retrieval
2. **"It's fast"** — Visibly fast responses, no Python startup lag
3. **"It's everywhere"** — Works in Claude, Cursor, CLI, and any MCP client

### Demo 1: Interactive CLI Recording (asciinema) — Ship First

**Audience:** Developers, GitHub visitors, README hero section

**Script:**
```
1. `cargo run --example quickstart` — 3 episodes, instant results
2. Feed a conversation about a project team (Alice, Bob, Acme Corp)
3. Ask: "Who works at Acme?" → instant entity retrieval
4. Add: "Alice left Acme and joined BigCo" → temporal update
5. Ask again: "Who works at Acme?" → only Bob (temporal correctness!)
6. Show: `snapshot --at 2024-01-01` → Alice was still at Acme then
7. Show: latency numbers in the terminal output
```

**Why it works:** This is the exact scenario that breaks most competitors. Mem0 scores 21.71% on temporal reasoning. If CK gets this right in a 30-second recording, it's immediately compelling.

**Tools:** `asciinema rec` → `asciinema upload` or embed SVG in README

### Demo 2: Benchmark Dashboard (HTML) — Ship with Benchmarks

**Audience:** Everyone evaluating CK vs. alternatives

Auto-generated HTML report with:
- Accuracy comparison table (CK vs. Graphiti vs. Mem0 vs. LangMem)
- Latency waterfall chart (ingestion, search, snapshot per system)
- Cost comparison (tokens per operation)
- Temporal reasoning spotlight (where CK should shine)
- Generated from `cargo bench` output — reproducible, not hand-crafted

Host at: `context-keeper.dev/benchmarks` or as a GitHub Pages artifact

### Demo 3: Live Web App — Ship with Cursor Plugin

**Audience:** AI tool power users, investors

React app that talks to CK over MCP (HTTP transport):
- Chat-style interface: type natural language, see entities extracted in real-time
- Knowledge graph visualization (force-directed graph of entities + relations)
- Timeline view showing temporal evolution of entities
- Side-by-side: "what CK remembers" vs. "raw conversation"

This doubles as the foundation for the Cursor plugin sidebar (FZ-17).

### Demo 4: Video Walkthrough — Ship Last

**Audience:** Broader audience, social media, investors

3-minute narrated screen recording:
1. Start with the problem: "AI tools forget everything between sessions"
2. Show CK running locally (no cloud, no API keys for storage)
3. Demo the CLI + MCP in Claude Desktop
4. Flash the benchmark numbers
5. End with: "Open source, Rust-native, local-first"

---

## Part 3: Execution Order

### Phase 1: Prove It (Weeks 1-2)

| Task | Depends On | Ties to Linear |
|------|-----------|---------------|
| Create `context-keeper-bench` crate | — | New issue |
| Implement criterion benchmarks (ingestion, search, temporal) | bench crate | New issue |
| Adapt LongMemEval dataset | bench crate | FZ-22 |
| Adapt LoCoMo dataset | bench crate | FZ-22 |
| Run first benchmark pass, publish numbers | adapters | FZ-22 |
| Record asciinema CLI demo | temporal demo example | New issue |

### Phase 2: Show It (Weeks 3-4)

| Task | Depends On | Ties to Linear |
|------|-----------|---------------|
| Build HTML benchmark dashboard generator | benchmark results | New issue |
| Complete memory updates (negation/dedup) | — | FZ-12 |
| Improve entity relationship management | FZ-12 | FZ-13 |
| Update README with benchmark results + asciinema | benchmarks + recording | New issue |

### Phase 3: Distribute It (Weeks 5-6)

| Task | Depends On | Ties to Linear |
|------|-----------|---------------|
| Build live web demo (React + MCP) | FZ-12 done | New issue |
| Ship Cursor plugin MVP | web demo foundation | FZ-17 |
| Open waitlist, track day-3 retention | Cursor plugin | FZ-33 |
| Record video walkthrough | all demos ready | New issue |

---

## Appendix: What We Already Have

### Test Infrastructure (1,901 lines)
- **TestEnv harness** — In-memory SurrealDB + mock LLM services
- **5 parameterized fixtures** with groundtruth entities/relations
- **9 IR metrics** — precision, recall, F1, MRR, recall@k, NDCG@k
- **39 test cases** across extraction, search, temporal, storage, E2E
- **Aggregate metrics report** with formatted output

### Demo Infrastructure
- **4 runnable examples** (quickstart, feature_showcase, temporal_demo, feature_showcase_llm)
- **6 MCP tools** (add_memory, search_memory, expand_search, get_entity, snapshot, list_recent)
- **Docker one-command setup** (`docker compose up`)
- **CLI binary** with add, search, entity, recent commands

### Key Metrics Already Tracked
- Extraction: precision, recall, F1 per scenario
- Search: MRR, recall@5 per query
- Temporal: staleness scores, snapshot correctness
- Storage: embedding preservation (10^-10 tolerance), roundtrip fidelity
