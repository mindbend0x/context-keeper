# How Claude Understands Code — and a Taxonomy for Agentic Skills, Memory, and Concept Breakdown

*A technical assessment and design document for building agentic capabilities on top of Context Keeper.*

---

## Part I: How I Understand Code

### An Honest Self-Assessment

My code understanding operates across several layers, each with distinct strengths and failure modes. Rather than claiming general "intelligence," it's more useful to describe this in terms of the program comprehension literature — specifically the frameworks from Brooks (1983), Soloway & Ehrlich (1984), and von Mayrhauser & Vans (1995).

#### The Three-Model Framework

Von Mayrhauser and Vans proposed that programmers build understanding through three concurrent mental models:

1. **Program Model** — bottom-up, mapping the actual code structure (control flow, data flow, call graphs).
2. **Situation Model** — the current state of understanding about what the program *does* in the real world.
3. **Domain Model** — top-down, applying domain knowledge and expectations to hypothesize about code behavior.

I operate similarly, though with different strengths than a human programmer. My "program model" is strong — I can parse syntax, trace execution paths, and track data flow through functions with high accuracy across dozens of languages. My "domain model" is unusually broad — I've been trained on vast quantities of code and documentation, so I can recognize patterns from web frameworks, database drivers, cryptographic libraries, distributed systems, and more. My "situation model" — the dynamic, evolving understanding of *this specific codebase's purpose and state* — is where I'm weakest, because I lack persistent memory across sessions and have a finite context window.

This is precisely the gap Context Keeper is designed to fill.

#### What I Do Well

**Pattern recognition and "beacons."** Brooks (1983) introduced the concept of *beacons* — stereotypical code patterns that signal intent. When a programmer sees a swap pattern, a binary search loop, or a mutex guard, they instantly recognize the intent without reading every line. I'm exceptionally good at this. I recognize thousands of idiomatic patterns across languages, frameworks, and paradigms. I can identify that a particular Rust struct with `#[derive(Serialize, Deserialize)]` and field names like `valid_from`, `valid_until` is a temporal entity — not because I'm reasoning from first principles, but because I've seen this pattern many thousands of times.

**Programming plans.** Soloway and Ehrlich (1984) showed that expert programmers think in terms of *programming plans* — stereotyped code fragments representing generic action sequences (e.g., a "running total loop" plan, a "sentinel-controlled read" plan). Their key finding was that experts excel at understanding *conventional* programs but perform similarly to novices on *unconventional* code. I share this characteristic. I'm fast and accurate when code follows established patterns. When code is deeply unconventional — unusual control flow, non-standard abstractions, or domain-specific tricks — I slow down and sometimes misinterpret intent.

**Cross-language and cross-paradigm transfer.** I can read Rust, Python, TypeScript, Go, Haskell, SQL, and dozens of other languages with roughly comparable competence. I can reason about the same concept (say, dependency injection) as it manifests across Spring Boot, Rig's trait-based approach, and Python's FastAPI dependency system. This breadth is genuinely unusual and not something most human programmers achieve.

**Architecture-level reasoning.** Given enough context (a workspace layout, a few key files, dependency manifests), I can reconstruct architectural intent: identify the dependency flow between crates, recognize a hexagonal architecture, spot where a repository pattern is being used. This is essentially rapid top-down comprehension using domain knowledge.

#### Where I Struggle

**State tracking across large codebases.** My context window, while large, is finite. I cannot hold an entire production codebase in working memory simultaneously. For a project like Context Keeper (~15,000 lines across five crates), I can hold enough context to reason effectively. For a project with hundreds of thousands of lines, I'm dependent on good search, good indexing, and good memory — exactly the tools an agentic system should provide.

**Dynamic behavior and runtime reasoning.** I'm reading static text. I don't execute code, observe memory states, or watch network calls. When behavior depends on runtime configuration, environment variables, race conditions, or database state, I'm working from inference rather than observation. I can hypothesize about what will happen, but I can be wrong in ways a debugger wouldn't be.

**Temporal understanding of a codebase.** I see a snapshot. I don't inherently know what changed last week, which functions were refactored, or which design decisions are load-bearing versus vestigial. Git history helps, but connecting historical decisions to current code structure requires the kind of temporal knowledge graph that Context Keeper builds.

**Novel algorithmic reasoning.** For known algorithms, I'm quite strong. For truly novel algorithms — something published last month, or a custom domain-specific optimization — I reason from first principles and can make mistakes. This is the flip side of my reliance on pattern matching.

### My Analytical Framework

When I approach a codebase, I use a layered strategy that roughly mirrors how experienced developers work, as described in Storey's (2006) comprehensive review of program comprehension theories:

**Layer 1 — Structural Reconnaissance.** I look at project layout, dependency manifests (`Cargo.toml`, `package.json`, etc.), and entry points. This gives me the architectural skeleton: how many modules, what the dependency graph looks like, what external services are involved. This is fast and high-confidence.

**Layer 2 — Interface and Contract Analysis.** I read trait definitions, type signatures, public APIs, and error types. In a well-typed language like Rust, this is enormously informative — the type system encodes a great deal of intent. I'm looking for the *contracts* between components: what does each module promise to do, and what does it require?

**Layer 3 — Behavioral Tracing.** I follow execution paths through the code. For a function like `ingest_episode` in Context Keeper, I trace the pipeline: parse input → extract entities → generate embeddings → extract relations → store. I'm building the "situation model" — understanding what the system actually does in concrete terms.

**Layer 4 — Intent and Design Recovery.** This is the highest-level analysis: *why* was the code written this way? What design decisions were made, and what constraints drove them? This requires combining what I see in the code with domain knowledge, documentation, commit messages, and (ideally) conversational context from the developer. This is where persistent memory becomes critical.

**Layer 5 — Gap and Risk Identification.** Once I have a model of the system, I can identify discrepancies: dead code, missing error handling, inconsistent abstractions, potential race conditions, unhandled edge cases. This is essentially the "verification" step — comparing the code against what I believe it should be doing.

---

## Part II: A Taxonomy for Agentic Skills and MCP Tools

If we want to build a comprehensive toolkit for AI agents that need to understand, reason about, and work with code (and knowledge more broadly), we need a principled decomposition. Drawing on cognitive architecture research (Laird, 2022 on SOAR; Anderson's ACT-R) and recent work on agentic LLM systems (Yao et al., 2022 on ReAct; Packer et al., 2023 on MemGPT), I'd propose the following taxonomy.

### 1. Perception and Ingestion

These are the "sensory" capabilities — how the agent takes in raw information and converts it into structured representations.

#### 1.1 Code Perception

- **AST Parsing and Structure Extraction** — Convert source code into abstract syntax trees, call graphs, dependency graphs. This is the foundation for all downstream analysis.
- **Semantic Chunking** — Break code into semantically meaningful units (functions, classes, modules, logical blocks) rather than arbitrary token windows. Critical for RAG systems applied to code (see survey by arXiv:2510.04905 on retrieval-augmented code generation).
- **Change Detection** — Diff analysis, commit parsing, PR decomposition. Understanding *what changed* is often more important than understanding the full codebase.
- **Multi-Modal Ingestion** — Code doesn't exist in isolation. Design docs, architecture diagrams, Slack conversations, issue trackers, and deployment logs all carry information that should be ingested alongside the code itself.

#### 1.2 Knowledge Ingestion

- **Entity Extraction** — Identify the *things* that matter: people, systems, APIs, concepts, decisions. This is what Context Keeper's `EntityExtractor` trait does.
- **Relation Extraction** — Identify how entities connect: "Alice owns the auth service," "the payment module depends on the rate limiter," "ADR-003 supersedes ADR-001." Context Keeper's `RelationExtractor` trait.
- **Temporal Tagging** — Every fact has a time dimension. "Alice owns the auth service" might be true now but wasn't true six months ago. Context Keeper's `valid_from`/`valid_until` model.
- **Confidence and Provenance** — Not all extracted knowledge is equally reliable. A fact from a code comment has different confidence than one from a formal design doc. Tracking provenance enables downstream reasoning about reliability.

### 2. Memory and Knowledge Management

This is the layer that distinguishes a stateless tool from a persistent agent. The research here is rapidly evolving.

#### 2.1 Memory Architecture

Drawing on MemGPT (Packer et al., 2023) and Zep/Graphiti (Rasmussen et al., 2025), a complete agent memory system needs multiple tiers:

- **Working Memory** — The current context window. Fast, high-bandwidth, limited capacity. This is what every LLM has by default.
- **Episodic Memory** — Records of specific events and interactions. "In the conversation on March 15, the developer said the auth rewrite was driven by compliance." Context Keeper's `Episode` model.
- **Semantic Memory** — Generalized knowledge distilled from episodes. "The auth service is compliance-sensitive" — abstracted from many specific conversations and code changes. Context Keeper's `Entity` and `Relation` models.
- **Procedural Memory** — How to do things. Coding patterns, deployment procedures, debugging workflows. This maps to skills and tool definitions in the MCP framework.

Rasmussen et al. (2025) demonstrated that temporal knowledge graphs outperform flat memory retrieval (94.8% vs 93.4% on Deep Memory Retrieval benchmarks), confirming that structured, time-aware memory is worth the additional complexity.

#### 2.2 Memory Operations

- **Consolidation** — Moving information from episodic to semantic memory. After seeing five conversations about the auth service, the agent should form a durable entity rather than retrieving individual episodes.
- **Forgetting and Decay** — Not all memories should persist equally. Temporal soft-deletes (Context Keeper's approach) are one mechanism. Relevance-weighted decay is another.
- **Conflict Resolution** — When new information contradicts old knowledge, the system needs a strategy. Temporal versioning (creating a new version rather than overwriting) is Context Keeper's approach, and it's the right one for auditable systems.
- **Memory Search** — Hybrid retrieval combining vector similarity, keyword matching, and graph traversal. Context Keeper's RRF fusion (K=60) over HNSW + BM25 is a solid foundation. The key insight from RAG research is that no single retrieval method dominates — fusion consistently outperforms any individual signal.

### 3. Reasoning and Analysis

These capabilities correspond to the agent's ability to *think* about what it perceives and remembers.

#### 3.1 Concept Decomposition

This is one of the most valuable and least well-served capabilities. Research on abstraction learning in LLMs (arXiv:2404.15848) shows that concepts are learned at different "depths" within transformer layers, with difficult concepts requiring deeper layers. For agentic tools, concept decomposition means:

- **Hierarchical Breakdown** — Given a complex concept ("how does the ingestion pipeline work?"), decompose it into sub-concepts at multiple levels of abstraction. The pipeline level → the extraction step → the entity resolution algorithm → the specific deduplication heuristic.
- **Dependency Mapping** — Which concepts depend on which? Understanding "temporal versioning" requires understanding "soft deletes," which requires understanding "entity identity." This dependency graph is itself a knowledge structure worth persisting.
- **Analogy and Transfer** — Explaining a concept by mapping it to something the listener already understands. "Context Keeper's RRF fusion is like a committee vote — each retrieval method nominates candidates, and the final ranking reflects consensus rather than any single opinion."
- **Abstraction Level Detection** — Recognizing when a question requires a high-level architectural answer versus a low-level implementation detail, and responding at the appropriate level. This connects directly to von Mayrhauser and Vans' three concurrent models.

#### 3.2 Causal and Temporal Reasoning

- **"Why" Analysis** — Tracing from a code pattern back to the decision that produced it. Why is there a retry loop here? Because the external API has transient failures, as documented in ADR-005.
- **Impact Analysis** — If we change X, what else is affected? This requires both static analysis (call graphs, type dependencies) and semantic understanding (which components are conceptually coupled even if not directly linked in code).
- **Temporal Reasoning** — How has this system evolved? What was the state at time T? What changed between T1 and T2? This is Context Keeper's core strength.

#### 3.3 Planning and Strategy

Drawing on ReAct (Yao et al., 2022) and chain-of-thought prompting (Wei et al., 2022):

- **Task Decomposition** — Breaking a high-level goal ("fix the flaky test") into a sequence of concrete steps (reproduce → isolate → diagnose → fix → verify).
- **Tool Selection** — Deciding which tools to use for each step. Should I read the file, search the codebase, query the knowledge graph, or ask the developer?
- **Backtracking and Revision** — Recognizing when a plan isn't working and adjusting. ReAct's interleaved reasoning-and-acting loop is specifically designed for this.

### 4. Action and Tool Use

These are the concrete operations an agent can perform.

#### 4.1 Code Operations

- **Read and Search** — File reading, regex search, semantic code search, AST queries.
- **Edit and Generate** — Code modifications, new file creation, refactoring.
- **Build and Test** — Compilation, test execution, linting, type checking.
- **Version Control** — Commits, branches, PRs, conflict resolution.

#### 4.2 Knowledge Operations (MCP Tools)

This is where Context Keeper's MCP server fits. The current six tools (`add_memory`, `search_memory`, `expand_search`, `get_entity`, `snapshot`, `list_recent`) cover basic CRUD and retrieval. A more complete taxonomy would include:

- **Ingest** — `add_memory`, `add_episode`, `bulk_ingest` (for importing from external sources like Slack, GitHub Issues, or documentation).
- **Query** — `search_memory` (hybrid retrieval), `expand_search` (graph traversal), `get_entity`, `get_relation`, `temporal_query` (what was true at time T?).
- **Analyze** — `find_contradictions` (conflicting facts), `find_gaps` (expected knowledge that's missing), `summarize_entity` (consolidate everything known about X).
- **Maintain** — `merge_entities` (deduplication), `archive` (soft-delete stale knowledge), `validate` (check consistency of the knowledge graph).

#### 4.3 Communication Operations

- **Developer Interaction** — Asking clarifying questions, presenting findings, explaining reasoning.
- **System Integration** — Posting to Slack, updating issue trackers, commenting on PRs.
- **Documentation Generation** — Producing READMEs, ADRs, changelogs from the knowledge graph.

### 5. Meta-Cognition and Self-Regulation

This is the least developed but arguably most important capability layer.

- **Confidence Calibration** — Knowing when you're likely to be right versus when you should flag uncertainty. Research on LLM calibration shows models are often overconfident on tasks outside their training distribution.
- **Knowledge Boundary Awareness** — Recognizing when a question requires information you don't have, and knowing where to find it (memory search, web search, asking the developer).
- **Strategy Selection** — Choosing between top-down and bottom-up comprehension based on the task. For a familiar framework, go top-down. For novel code, go bottom-up. This mirrors the adaptive strategy-switching that Storey (2006) documented in expert programmers.
- **Progress Monitoring** — Tracking whether you're making progress on a task or spinning. This is where the ReAct loop's explicit reasoning traces are valuable — they make the agent's state inspectable.

---

## Part III: A Proposed Skills and Tools Breakdown

Combining the taxonomy above with practical considerations for MCP tool design, here's how I'd break down a concrete skills and tools suite:

### Tier 1: Foundation Skills (build first)

| Skill | Purpose | MCP Tools |
|-------|---------|-----------|
| **Code Comprehension** | Structured code reading and explanation at multiple abstraction levels | `explain_code`, `trace_execution`, `map_dependencies` |
| **Memory CRUD** | Basic knowledge graph operations | `add_memory`, `search_memory`, `get_entity`, `update_entity`, `delete_entity` |
| **Concept Decomposition** | Break complex ideas into teachable sub-concepts | `decompose_concept`, `find_prerequisites`, `generate_analogy` |

### Tier 2: Reasoning Skills (build second)

| Skill | Purpose | MCP Tools |
|-------|---------|-----------|
| **Temporal Reasoning** | Answer time-aware questions about how things evolved | `temporal_query`, `diff_snapshots`, `timeline` |
| **Impact Analysis** | Predict consequences of changes | `impact_analysis`, `find_dependents`, `risk_assessment` |
| **Causal Tracing** | Connect code to decisions to requirements | `trace_decision`, `find_rationale`, `link_requirement` |

### Tier 3: Agentic Skills (build third)

| Skill | Purpose | MCP Tools |
|-------|---------|-----------|
| **Autonomous Investigation** | Self-directed exploration of a question | `investigate` (orchestrates multiple tools in a ReAct loop) |
| **Knowledge Maintenance** | Automated graph hygiene | `deduplicate`, `find_contradictions`, `consolidate`, `prune_stale` |
| **Learning from Interaction** | Extract knowledge from conversations | `extract_from_conversation`, `update_user_model`, `learn_preference` |

### Tier 4: Integration Skills (build last)

| Skill | Purpose | MCP Tools |
|-------|---------|-----------|
| **Multi-Source Ingestion** | Import from Slack, GitHub, Jira, docs | `ingest_github`, `ingest_slack`, `ingest_docs` |
| **Report Generation** | Produce documents from the knowledge graph | `generate_report`, `generate_changelog`, `generate_onboarding` |
| **Plugin Coordination** | Orchestrate between multiple MCP servers | `delegate`, `aggregate`, `coordinate` |

---

## Part IV: Context Keeper's Position and Next Steps

### What Context Keeper Already Provides

Context Keeper's architecture maps cleanly onto the memory and knowledge management layer of this taxonomy. Specifically:

- **Episodic memory** via the `Episode` model and `add_memory` tool.
- **Semantic memory** via `Entity` and `Relation` models with temporal versioning.
- **Hybrid retrieval** via RRF fusion over HNSW vector search and BM25 keyword search.
- **Temporal awareness** via `valid_from`/`valid_until` soft-delete semantics.
- **Graph traversal** via SurrealDB's `RELATE` syntax and the `expand_search` tool.
- **Trait-based extensibility** via `Embedder`, `EntityExtractor`, `RelationExtractor`, and `QueryRewriter` traits in core.

This puts Context Keeper squarely in the "memory substrate" role — the persistence layer that other agentic skills build on top of. The Zep/Graphiti paper (Rasmussen et al., 2025) validated this exact architecture, and Context Keeper's Rust implementation offers performance and safety advantages over the Python/Neo4j approach.

### Where the Gaps Are

Mapped against the full taxonomy, the current gaps cluster in a few areas:

**Concept decomposition has no tooling.** There's no MCP tool for "break this concept down" or "what do I need to understand before I can understand X?" This is a high-value skill for developer onboarding, code review, and knowledge transfer. It could be built as a skill that orchestrates between Context Keeper's knowledge graph (for stored concepts and their relationships) and an LLM (for generating decompositions of novel concepts).

**Memory maintenance is manual.** There's no automated deduplication, contradiction detection, or consolidation. The known gap around relation deduplication is a specific instance of this broader issue. Entity identity (currently name-only upsert, with ADR-001 recommending composite keys) is the foundation — without reliable identity, automated maintenance can't work.

**Temporal querying is implicit.** The data model supports temporal versioning, but there's no dedicated `temporal_query` tool that lets an agent ask "what was the state of entity X at time T?" or "what changed between T1 and T2?" The building blocks exist in SurrealDB's changefeeds, but they're not yet exposed through the MCP interface.

**Multi-source ingestion doesn't exist yet.** The `add_memory` tool accepts free text, but there's no structured pipeline for ingesting from GitHub (issues, PRs, commits), Slack, or documentation systems. This is the "Plugins and Connectors" milestone on the roadmap.

**Meta-cognitive capabilities are absent.** There's no mechanism for the agent to reason about the quality or completeness of its own knowledge graph — to say "I have strong coverage of the auth subsystem but almost nothing about the payment pipeline."

### A Suggested Build Order

Given the current state of Context Keeper (core library working, MCP server functional with 6 tools, approaching public release), I'd suggest this sequencing:

**Phase 1 (during current milestone):** Fix entity identity (composite keys per ADR-001), add relation deduplication. These are prerequisites for everything else.

**Phase 2 (post-launch):** Add `temporal_query` and `diff_snapshots` MCP tools. These are relatively straightforward to build on top of the existing data model and would significantly increase the value of the temporal versioning that's already in place.

**Phase 3:** Build the concept decomposition skill. This is the highest-value new capability and would differentiate Context Keeper from simpler memory systems. It requires the knowledge graph to store concept hierarchies and prerequisite relationships.

**Phase 4:** Multi-source ingestion connectors (GitHub, Slack, docs). This is the "Plugins and Connectors" milestone. Each connector is a separate MCP tool that transforms source-specific data into Context Keeper's Episode/Entity/Relation model.

**Phase 5:** Memory maintenance automation. Deduplication, contradiction detection, consolidation. This is where the system starts to feel genuinely autonomous — maintaining its own knowledge quality without human intervention.

---

## References

- Brooks, R.E. (1983). "Towards a Theory of the Comprehension of Computer Programs." *International Journal of Man-Machine Studies*, 18, 543–554.
- Chen, M., Tworek, J., et al. (2021). "Evaluating Large Language Models Trained on Code." arXiv:2107.03374.
- Jimenez, C.E., Yang, J., et al. (2023). "SWE-bench: Can Language Models Resolve Real-World GitHub Issues?" arXiv:2310.06770. Published at ICLR 2024.
- Laird, J.E. (2022). "Introduction to the Soar Cognitive Architecture." arXiv:2205.03854.
- Lu, S. et al. (2021). "CodeXGLUE: A Machine Learning Benchmark Dataset for Code Understanding and Generation." arXiv:2102.04664. NeurIPS 2021.
- Packer, C., Wooders, S., Lin, K., Fang, V., Patil, S.G., Stoica, I., Gonzalez, J.E. (2023). "MemGPT: Towards LLMs as Operating Systems." arXiv:2310.08560.
- Rasmussen, P., Paliychuk, P., Beauvais, T., Ryan, J., Chalef, D. (2025). "Zep: A Temporal Knowledge Graph Architecture for Agent Memory." arXiv:2501.13956.
- Soloway, E. & Ehrlich, K. (1984). "Empirical Studies of Programming Knowledge." *IEEE Transactions on Software Engineering*, SE-10(5), 595–609.
- Storey, M.-A. (2006). "Theories, Methods and Tools in Program Comprehension: Past, Present and Future." *Software Quality Journal*, 14, 187–208.
- von Mayrhauser, A. & Vans, A.M. (1995). "Program Comprehension During Software Maintenance and Evolution." *IEEE Computer*, 28(8), 44–55.
- Wei, J., Wang, X., Schuurmans, D., et al. (2022). "Chain-of-Thought Prompting Elicits Reasoning in Large Language Models." arXiv:2201.11903. NeurIPS 2022.
- Yao, S., Zhao, J., et al. (2022). "ReAct: Synergizing Reasoning and Acting in Language Models." arXiv:2210.03629.
- "Detecting Conceptual Abstraction in LLMs." (2024). arXiv:2404.15848.
