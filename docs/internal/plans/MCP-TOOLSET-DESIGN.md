# Context Keeper MCP Toolset Design: The Memory Layer Agents Actually Need

*A comprehensive design for transforming Context Keeper from a knowledge graph memory server into the definitive agentic memory infrastructure — specifically targeting what Claude Code, Claude Cowork, and comparable agents are missing today.*

---

## The Problem This Solves

Every agentic coding tool — Claude Code, Cursor, Windsurf, Cline, Aider — suffers from the same fundamental limitation: **session amnesia**. Developers report spending 10–30 minutes at the start of each session re-explaining project structure. Over a week, this compounds into hours of lost productivity. The root cause isn't context window size — it's that agents have no structured, persistent, temporally-aware memory system.

Claude Code's current solutions are pragmatic but limited. `CLAUDE.md` files are static text committed to version control. The auto-memory system (`/memory`) writes notes to flat files that compete for context window space. Neither system understands *relationships* between facts, *when* facts became true or false, or *how confident* the agent should be in its knowledge.

Anthropic's official Memory MCP server offers entities, relations, and observations — but no temporal versioning, no hybrid search, no contradiction detection, and no multi-agent provenance. Community alternatives (mem0, mcp-memory-service) are similarly limited to flat key-value or simple graph stores.

Context Keeper already has the core architecture to solve this properly: temporal knowledge graphs, RRF hybrid search, entity resolution, and multi-agent provenance tracking. What it needs is the right MCP tool surface to make that architecture *usable* by agents like Claude Cowork in real workflows.

---

## Design Principles

These principles are derived from MCP best practices, the program comprehension literature, and practical observation of how agents actually use tools.

**1. Tools should map to agent intentions, not database operations.** An agent doesn't think "I need to UPSERT an entity." It thinks "I learned something new and need to remember it" or "I need to recall what I know about X." Tool names and descriptions should match the agent's mental model.

**2. Every tool must earn its context budget.** Each tool definition consumes tokens in the agent's context window. A tool that's rarely used or duplicates another tool's functionality is actively harmful — it wastes context and creates decision fatigue. Prefer fewer, well-composed tools over many narrow ones.

**3. Read-heavy, write-light.** Agents search and retrieve far more often than they write. The read path should be fast, flexible, and richly informative. The write path should be simple and forgiving — the system should handle deduplication, resolution, and conflict detection internally rather than requiring the agent to manage it.

**4. Temporal awareness should be ambient, not opt-in.** Every response should naturally include temporal context ("this entity was last updated 3 days ago", "this relation was invalidated on March 15"). Agents shouldn't have to call a separate tool to understand when something was true.

**5. Actionable errors over diagnostic errors.** "No entity found with name 'Alice'" is diagnostic. "No entity found with name 'Alice'. Did you mean 'Alice Chen' (person, updated 2 days ago)? Use search_memory for fuzzy matching." is actionable.

---

## Current State: 10 Tools, 3 Prompts, 2 Resources

The existing MCP server exposes these tools:

| Tool | Annotation | Purpose |
|------|-----------|---------|
| `add_memory` | write | Ingest text → extract entities/relations → store |
| `search_memory` | read | Hybrid vector+BM25 search with RRF fusion |
| `expand_search` | read | LLM query expansion → multi-variant search → RRF |
| `get_entity` | read | Look up entity by exact name |
| `snapshot` | read | Point-in-time graph state |
| `list_recent` | read | N most recent memories |
| `list_agents` | read | All contributing agents |
| `list_namespaces` | read | All namespaces with counts |
| `agent_activity` | read | Episodes from a specific agent |
| `cross_namespace_search` | read | Global search ignoring namespace scoping |

**Prompts:** `summarize-topic`, `what-changed`, `add-context`

**Resources:** `memory://recent`, `memory://entity/{name}`

### Honest Assessment

The current toolset covers basic CRUD and retrieval well. What's missing falls into five categories:

1. **Temporal power tools** — The data model supports rich temporal queries, but only `snapshot` exposes this. No way to ask "what changed?" or "what was true about X at time T?"
2. **Graph intelligence** — SurrealDB's graph traversal capabilities are barely exposed. `get_entity` returns relations but there's no way to walk the graph, find paths between entities, or discover clusters.
3. **Memory maintenance** — No tools for the agent to maintain graph quality: merge duplicates, resolve contradictions, prune stale data. The agent can only add, never curate.
4. **Meta-knowledge** — No tool for the agent to assess its own knowledge: "what do I know well? where are my blind spots? how stale is my understanding?"
5. **Structured ingestion** — `add_memory` accepts raw text only. No way to directly assert entities or relations with high confidence when the agent already knows the structured facts.

---

## Proposed Toolset: 16 Tools Organized by Intent

The redesigned toolset is organized into four intent groups. Each tool has a clear name, MCP annotations, and a description written for the agent (not the developer).

### Group 1: Remember (Write Path)

These tools store new knowledge. The agent uses them when it learns something worth persisting.

#### `ck_remember`

Replaces and extends `add_memory`. The primary ingestion tool.

```
name: ck_remember
annotations:
  readOnlyHint: false
  destructiveHint: false
  idempotentHint: false
  openWorldHint: false

inputs:
  text: string (required)
    "The information to remember. Can be a fact, observation, conversation
     excerpt, or any text. Entities and relations will be automatically
     extracted."
  source: string (optional, default: "agent")
    "Where this information came from: 'conversation', 'code', 'document',
     'observation', 'git', etc."
  namespace: string (optional)
    "Scope this memory to a specific namespace (e.g., 'project-alpha').
     Omit for the default namespace."
  confidence: string (optional, default: "normal")
    "How confident you are: 'high' (verified fact), 'normal' (reasonable
     inference), 'low' (uncertain, worth noting). Affects entity/relation
     confidence scores."

output:
  JSON object with:
    - summary: human-readable description of what was stored
    - entities: list of {name, type, action} where action is "created",
      "updated", or "invalidated"
    - relations: list of {from, relation, to, action}
    - contradictions: list of any detected conflicts with existing knowledge
```

**Why this design:** Agents need exactly one "save this" tool that handles everything. The `confidence` parameter lets the agent express certainty without managing numeric scores. The response is structured as a *diff* so the agent knows exactly what changed.

#### `ck_assert`

New tool for when the agent already knows structured facts and wants to bypass LLM extraction.

```
name: ck_assert
annotations:
  readOnlyHint: false
  destructiveHint: false
  idempotentHint: true
  openWorldHint: false

inputs:
  entities: array of {name, type, summary} (optional)
    "Entities to create or update directly. Types: person, organization,
     location, event, product, service, concept, file."
  relations: array of {from, relation, to} (optional)
    "Relations to assert. Relations: works_at, located_in, part_of,
     member_of, uses, created_by, knows, depends_on, related_to."
  namespace: string (optional)
  source: string (optional, default: "agent-assertion")

output:
  JSON diff of what was created/updated/merged
```

**Why this design:** During a coding session, an agent discovers "module A depends on module B" by reading code. It knows the structured fact — forcing it through free-text extraction adds latency and extraction errors. `ck_assert` is the "I already know the answer, just store it" path. Marked `idempotentHint: true` because asserting the same fact twice is a no-op (upsert semantics).

#### `ck_retract`

New tool for when the agent learns something is no longer true.

```
name: ck_retract
annotations:
  readOnlyHint: false
  destructiveHint: false  # soft-delete, not hard delete
  idempotentHint: true
  openWorldHint: false

inputs:
  entity_name: string (optional)
    "Name of entity to invalidate (soft-delete). The entity's history
     is preserved."
  relation: {from, relation, to} (optional)
    "A specific relation to invalidate."
  reason: string (required)
    "Why this is being retracted. Stored for audit trail."
  namespace: string (optional)

output:
  Confirmation of what was invalidated, with timestamp
```

**Why this design:** Agents currently have no way to say "this is no longer true." The `reason` field matters — it's stored in the temporal record and explains *why* the knowledge changed. This is critical for the "what changed" temporal query. Marked non-destructive because it's a soft-delete.

### Group 2: Recall (Read Path)

These tools retrieve knowledge. This is where agents spend most of their time.

#### `ck_recall`

The primary search tool. Replaces `search_memory`, `expand_search`, and `cross_namespace_search` with a single, adaptive tool.

```
name: ck_recall
annotations:
  readOnlyHint: true
  destructiveHint: false
  idempotentHint: true
  openWorldHint: false

inputs:
  query: string (required)
    "What you want to recall. Can be a question, keywords, or a
     description of what you're looking for."
  scope: string (optional, default: "namespace")
    "Search scope: 'namespace' (current namespace only), 'global'
     (all namespaces), 'deep' (auto-expands query variants for
     better recall)."
  filter_type: string (optional)
    "Filter by entity type: person, organization, concept, file, etc."
  limit: integer (optional, default: 10)
  include_history: boolean (optional, default: false)
    "If true, includes invalidated entities/relations to show how
     knowledge evolved."

output:
  JSON with:
    - results: list of {name, type, summary, score, last_updated, namespace}
    - graph_context: for top results, their immediate relations
    - coverage_note: "Found 3 strong matches. Knowledge was last updated
      5 days ago." or "Sparse results — consider using scope:'deep' for
      expanded search."
```

**Why this design:** Three separate search tools (`search_memory`, `expand_search`, `cross_namespace_search`) force the agent to choose a search strategy before it knows what the results look like. In practice, agents should try fast search first and escalate to expanded search if results are sparse. The `scope` parameter handles this: `namespace` is fast and focused, `global` crosses boundaries, `deep` triggers query expansion. The `coverage_note` in the response gives the agent a signal about whether it should search again differently.

Including `graph_context` (immediate relations) in search results is critical — it answers the "and what's connected to this?" question that agents almost always ask as a follow-up.

#### `ck_inspect`

Replaces `get_entity` with a richer entity detail view.

```
name: ck_inspect
annotations:
  readOnlyHint: true
  destructiveHint: false
  idempotentHint: true
  openWorldHint: false

inputs:
  name: string (required)
    "The entity to inspect. Fuzzy matching is applied — you don't need
     the exact name."
  depth: integer (optional, default: 1)
    "How many hops of relations to include. 1 = direct relations only,
     2 = relations of relations."
  include_history: boolean (optional, default: false)
    "If true, includes previous versions and invalidated relations."
  namespace: string (optional)

output:
  JSON with:
    - entity: {name, type, summary, valid_from, valid_until, namespace,
      created_by_agent, staleness_days}
    - relations: list of {direction, relation_type, other_entity_name,
      other_entity_type, confidence, valid_from}
    - history: (if include_history) list of previous summaries with
      timestamps and reasons for changes
    - related_memories: recent memories mentioning this entity
    - suggestions: "This entity has 3 relations but 0 outgoing. You may
      want to add what Alice works on."
```

**Why this design:** `get_entity` currently requires exact name matching and only returns raw relation IDs (not entity names). Agents need fuzzy matching (the entity might be "Alice Chen" or "Alice" or "A. Chen"), resolved relation endpoints (not UUIDs), and temporal context. The `suggestions` field helps the agent identify knowledge gaps proactively.

#### `ck_timeline`

New tool. The temporal query the agents have been missing.

```
name: ck_timeline
annotations:
  readOnlyHint: true
  destructiveHint: false
  idempotentHint: true
  openWorldHint: false

inputs:
  subject: string (optional)
    "An entity name or topic to scope the timeline to. Omit for a
     global timeline."
  since: string (optional)
    "ISO 8601 timestamp or relative duration ('7d', '30d', '1h').
     How far back to look."
  until: string (optional)
    "ISO 8601 timestamp. Defaults to now."
  event_types: array of string (optional)
    "Filter to specific event types: 'entity_created', 'entity_updated',
     'entity_invalidated', 'relation_created', 'relation_invalidated',
     'memory_added'."
  namespace: string (optional)

output:
  JSON with:
    - period: {from, to}
    - events: chronologically ordered list of:
      {timestamp, event_type, description, entities_involved, agent}
    - summary: "In the last 7 days: 5 entities created, 2 updated,
      1 invalidated. Most active topic: auth service (3 events)."
```

**Why this design:** This is the tool that makes temporal knowledge graphs *useful* to agents. Without it, the temporal data is just audit metadata. With it, an agent can answer "what changed since I last worked on this?" or "what happened to the auth service this week?" — questions that developers ask constantly and that current tools cannot answer. The `since` parameter accepts relative durations because agents rarely know the exact timestamp.

#### `ck_graph_walk`

New tool. Exposes SurrealDB's graph traversal capabilities.

```
name: ck_graph_walk
annotations:
  readOnlyHint: true
  destructiveHint: false
  idempotentHint: true
  openWorldHint: false

inputs:
  start: string (required)
    "Entity name to start from."
  direction: string (optional, default: "both")
    "Traversal direction: 'outgoing', 'incoming', 'both'."
  relation_types: array of string (optional)
    "Filter to specific relation types. Omit to follow all relations."
  max_depth: integer (optional, default: 2)
    "Maximum hops. Keep low (1-3) to avoid overwhelming results."
  namespace: string (optional)

output:
  JSON with:
    - root: {name, type, summary}
    - paths: list of discovered paths, each as a chain of
      {entity} --[relation]--> {entity}
    - clusters: groups of tightly connected entities
    - isolated: entities reachable only through a single path
      (potential knowledge gaps)
```

**Why this design:** Graph traversal is what makes a knowledge *graph* different from a knowledge *base*. "What depends on the auth service?" requires following `depends_on` edges outward. "How are Alice and the payment team connected?" requires pathfinding. Current tools can't do this. The `clusters` and `isolated` fields help agents understand the graph's structure and identify blind spots.

### Group 3: Reflect (Meta-Knowledge)

These tools help the agent reason about its own knowledge — what it knows, what it doesn't, and how reliable its knowledge is.

#### `ck_status`

New tool. The agent's self-assessment of its knowledge state.

```
name: ck_status
annotations:
  readOnlyHint: true
  destructiveHint: false
  idempotentHint: true
  openWorldHint: false

inputs:
  namespace: string (optional)
    "Scope to a specific namespace. Omit for global status."
  focus: string (optional)
    "A topic or entity type to focus the assessment on."

output:
  JSON with:
    - overview: {total_entities, total_relations, total_memories,
      total_episodes, namespaces}
    - freshness:
      - recently_updated: entities changed in last 24h
      - stale: entities not updated in 30+ days
      - average_staleness_days: float
    - coverage:
      - entity_types: count by type
      - relation_density: avg relations per entity
      - orphan_entities: entities with 0 relations
      - singleton_clusters: entities connected to nothing else
    - quality:
      - low_confidence_relations: count below 50
      - potential_duplicates: entity pairs with similar names/embeddings
      - contradictions_detected: count of conflicting facts
    - agents:
      - contributing_agents: list with episode counts
      - most_recent_contribution: timestamp
    - recommendation: "Your knowledge of 'auth service' is strong
      (8 entities, updated 2 days ago) but 'payment pipeline' has
      only 1 stale entity. Consider adding memory about payment."
```

**Why this design:** This is the meta-cognitive capability identified in the taxonomy document. Without it, an agent has no way to know whether its knowledge is comprehensive or dangerously incomplete. The `recommendation` field is the actionable output — it tells the agent what to do next. A Claude Cowork agent could call this at the start of every session to orient itself.

#### `ck_diff`

New tool. Compare knowledge states across time.

```
name: ck_diff
annotations:
  readOnlyHint: true
  destructiveHint: false
  idempotentHint: true
  openWorldHint: false

inputs:
  since: string (required)
    "ISO 8601 timestamp or relative duration ('24h', '7d'). The
     'before' point."
  until: string (optional, default: "now")
    "The 'after' point."
  namespace: string (optional)
  entity_name: string (optional)
    "Diff a specific entity's evolution."

output:
  JSON with:
    - period: {from, to}
    - entities_added: list of {name, type, summary}
    - entities_updated: list of {name, old_summary, new_summary,
      change_description}
    - entities_removed: list of {name, reason, removed_at}
    - relations_added: list of {from, relation, to}
    - relations_removed: list of {from, relation, to, reason}
    - net_change: "+12 entities, +8 relations, -3 invalidated"
```

**Why this design:** `snapshot` shows a static state. `ck_diff` shows the *delta* — which is almost always what agents actually want. "What changed since yesterday?" is answered by this tool. It's the temporal equivalent of `git diff`.

### Group 4: Maintain (Graph Hygiene)

These tools let the agent curate and improve the knowledge graph's quality. This is the capability category that no existing MCP memory server offers.

#### `ck_merge`

New tool. Combine duplicate entities.

```
name: ck_merge
annotations:
  readOnlyHint: false
  destructiveHint: false  # merges, doesn't delete
  idempotentHint: true
  openWorldHint: false

inputs:
  entities: array of string (required, min: 2)
    "Entity names to merge. The first name becomes the canonical name."
  reason: string (optional)
    "Why these are duplicates. Stored for audit."
  namespace: string (optional)

output:
  JSON with:
    - merged_into: canonical entity name and summary
    - absorbed: list of entities that were merged in
    - relations_transferred: count
    - summary_updated: boolean (if summaries were combined)
```

**Why this design:** Entity deduplication is a known gap. Agents encounter duplicates naturally: "Alice" and "Alice Chen" might be the same person. Rather than building a fully automated dedup system (which risks false merges), this tool lets the agent make the call with human-in-the-loop reasoning. It's `idempotentHint: true` because merging already-merged entities is a no-op.

#### `ck_validate`

New tool. Check knowledge graph consistency.

```
name: ck_validate
annotations:
  readOnlyHint: true
  destructiveHint: false
  idempotentHint: true
  openWorldHint: false

inputs:
  scope: string (optional, default: "all")
    "What to validate: 'all', 'entities', 'relations', 'duplicates',
     'contradictions', 'orphans'."
  namespace: string (optional)
  limit: integer (optional, default: 20)

output:
  JSON with:
    - issues: list of:
      {type, severity, description, entities_involved, suggestion}
    - health_score: 0-100
    - summary: "Found 3 potential duplicates, 2 orphan entities, and
      1 stale relation cluster. Health: 78/100."
```

**Why this design:** This is the "lint" for the knowledge graph. An agent can run it periodically and then use `ck_merge`, `ck_retract`, or `ck_remember` to fix issues. The `health_score` gives a single number the agent can track over time. The `suggestion` field in each issue tells the agent exactly how to fix it.

#### `ck_consolidate`

New tool. Distill episodic memory into semantic memory.

```
name: ck_consolidate
annotations:
  readOnlyHint: false
  destructiveHint: false
  idempotentHint: false
  openWorldHint: false

inputs:
  topic: string (optional)
    "A topic to consolidate knowledge around. Omit for general
     consolidation."
  namespace: string (optional)
  dry_run: boolean (optional, default: true)
    "If true, shows what would change without making changes."

output:
  JSON with:
    - consolidated: list of entities whose summaries were enriched
    - merged: list of duplicate entities that were combined
    - pruned: list of low-value relations that were removed
    - summary: "Consolidated 12 episodes about 'auth service' into
      3 entities with updated summaries. Merged 2 duplicates."
```

**Why this design:** This addresses the episodic-to-semantic memory consolidation gap. Over time, an agent accumulates many episodes about the same topics. Consolidation distills them into cleaner entity summaries — analogous to how human memory consolidates during sleep. The `dry_run` default protects against unintended changes. This is the tool that would power the "Auto Dream" capability that Claude Code's memory system attempts with flat files.

---

## What This Unlocks for Claude Cowork Specifically

With this toolset, a Claude Cowork agent's session lifecycle would look fundamentally different:

**Session Start:** Agent calls `ck_status` to assess its knowledge state, then `ck_diff(since: "24h")` to catch up on what changed since the last session. Instead of the developer spending 10–30 minutes re-explaining context, the agent *already knows* and can say: "Since we last spoke, the auth service entity was updated with new compliance requirements, and 2 new dependencies were added to the payment module."

**During Work:** Agent calls `ck_recall` with natural questions as it works. When it reads code and discovers relationships, it calls `ck_assert` to store structured facts directly. When it notices contradictions ("wait, this config says the DB is Postgres but the entity says MySQL"), it calls `ck_retract` on the stale fact.

**Deep Investigation:** Agent calls `ck_graph_walk` to understand how systems connect. "What depends on the rate limiter?" becomes a single tool call that returns the full dependency subgraph. Agent calls `ck_timeline(subject: "rate-limiter")` to understand how it evolved.

**Session End:** Agent calls `ck_consolidate(dry_run: false)` to clean up the day's episodic memories into clean semantic knowledge. Calls `ck_validate` to flag any issues for next session.

**Multi-Agent Scenarios:** When multiple Claude Code instances are working on different parts of the same project, they share the same Context Keeper instance, scoped by namespace. `ck_status` shows each agent what the others have contributed. `ck_recall(scope: "global")` lets them search across all namespaces. This is native multi-agent collaboration without explicit coordination.

---

## Areas to Investigate for Next-Level Differentiation

Beyond the tool redesign, these are the research and engineering areas that would make Context Keeper genuinely category-defining.

### 1. Community Detection and Knowledge Clustering

**What:** Apply community detection algorithms (Louvain, Label Propagation) to the knowledge graph to automatically identify clusters of related entities.

**Why it matters for agents:** Instead of searching for individual entities, an agent could ask "show me the clusters" and get back: "I see 4 main knowledge domains: auth/identity (12 entities), payment processing (8 entities), deployment infrastructure (6 entities), and user management (5 entities). Auth and payment share 3 cross-cutting relations." This is the *map* of the agent's knowledge — invaluable for orientation.

**How to build:** SurrealDB's graph traversal is the foundation. Implement Louvain community detection as a periodic background task or on-demand via a `ck_map` tool. Store cluster assignments as metadata on entities.

**Research:** Rasmussen et al. (2025) demonstrate community subgraphs in Zep/Graphiti. Their approach generates "community summaries" that capture the essence of each cluster. This is directly applicable.

### 2. Contradiction Resolution via Temporal Provenance

**What:** When contradictions are detected during ingestion, use the provenance chain (which agent said what, when, based on what source) to automatically determine which fact is more likely to be correct.

**Why it matters for agents:** Currently, contradiction detection is heuristic (negation markers + word overlap). A richer system would weigh: source reliability (code > conversation > inference), recency (newer facts generally win), agent confidence scores, and corroboration (facts confirmed by multiple agents/sources are stronger).

**How to build:** Extend the `Entity` model with a `provenance_chain: Vec<ProvenanceEntry>` where each entry records `{source, agent, timestamp, confidence, corroborated_by}`. During ingestion, when a contradiction is detected, compute a resolution score and either auto-resolve or flag for agent review.

**Research:** This connects to the "confidence and provenance" capability in the taxonomy. The Semantic Web community has extensive work on provenance-aware knowledge graphs (W3C PROV model).

### 3. Incremental Embedding Updates

**What:** Instead of re-embedding entire entity summaries on every update, compute incremental embedding updates that blend old and new information.

**Why it matters:** Embedding generation is the primary latency bottleneck in `add_memory`. For entity updates (which are common), re-embedding the entire summary is wasteful when only a few words changed. Incremental updates could reduce p95 latency by 40–60%.

**How to build:** Maintain a running weighted average of embeddings. When an entity summary is updated, embed only the *delta* (new information) and blend it with the existing embedding using an exponential moving average. The blend weight should favor recency.

**Research:** This is related to "continual learning" in embedding models. The key insight from RAG literature is that embedding drift is usually small for minor updates — a blended approach preserves retrieval quality while dramatically reducing compute.

### 4. Agent Preference Learning

**What:** Track how each agent interacts with the knowledge graph — what it searches for, what it finds useful, what it ignores — and use this to personalize results.

**Why it matters for Cowork:** A Claude Cowork agent helping with code review searches differently than one helping with documentation. Over time, Context Keeper could learn: "This agent mostly queries for architectural concepts and dependency relationships, so boost those in search results." This is the "procedural memory" layer from the taxonomy.

**How to build:** Log every tool call with `{agent_id, tool, query, result_count, follow_up_action}`. Compute per-agent topic distributions and entity type preferences. Use these as soft boosts in search ranking.

### 5. Schema-Aware Code Ingestion

**What:** Instead of treating code as free text, parse it into structural elements (functions, types, imports, comments) and create entities/relations that mirror code structure.

**Why it matters:** When an agent ingests "Alice refactored the auth module," `add_memory` extracts Alice (person) and auth module (concept). But when the agent reads actual code, it should extract `AuthService` (file), `validate_token` (concept), `depends_on: jwt_library` (relation) with much higher fidelity than free-text extraction provides.

**How to build:** Add a `source_type` parameter to `ck_remember` that activates specialized extraction. For `source_type: "rust"`, use tree-sitter to parse the AST and extract struct names, function signatures, import relationships, and trait implementations as entities and relations. This doesn't require LLM extraction — it's deterministic and fast.

**Research:** CodeXGLUE (Lu et al., 2021) provides a taxonomy of code understanding tasks. Tree-sitter based extraction is well-established in tools like GitHub's semantic search.

### 6. Conversation-Aware Ingestion

**What:** Instead of the agent manually calling `ck_remember` after each important exchange, provide a mode where Context Keeper can ingest an entire conversation transcript and extract knowledge automatically.

**Why it matters for Cowork:** Claude Cowork already has access to conversation transcripts. A `ck_ingest_conversation` tool could process an entire session's worth of dialogue, extracting decisions, facts, preferences, and action items. This bridges the gap between episodic memory (what happened in the conversation) and semantic memory (what facts were established).

**How to build:** Accept a conversation transcript (array of `{role, content, timestamp}` messages). Use LLM extraction with a conversation-aware prompt that distinguishes between: factual claims, decisions, preferences, questions (which indicate knowledge gaps), and corrections (which indicate contradictions with prior knowledge).

### 7. Subscription and Notification Model

**What:** Allow agents to subscribe to changes on specific entities or topics, and be notified when relevant knowledge changes.

**Why it matters for multi-agent:** In a multi-agent setup, Agent A working on the auth service should know immediately when Agent B adds a memory about a security vulnerability. Currently, agents would only discover this on their next `ck_recall`. Subscriptions enable reactive knowledge sharing.

**How to build:** Leverage SurrealDB's changefeed capability (already partially implemented via `entity_changes_since` and `relation_changes_since`). Add a `ck_subscribe` tool that registers interest in entities/topics, and return pending notifications in the response of every subsequent tool call (piggybacked, not polling).

### 8. Knowledge Graph Embeddings (TransE/TransR)

**What:** Beyond embedding individual entities, learn embeddings for the *relations* themselves and the *graph structure* using knowledge graph embedding methods like TransE or TransR.

**Why it matters:** With graph-aware embeddings, search can answer structural queries: "What entities play a similar role to the rate limiter?" (entities with similar graph neighborhoods). This is fundamentally different from text similarity — it captures *functional role* rather than *description similarity*.

**How to build:** Periodically train lightweight TransE/TransR embeddings on the knowledge graph. Store these as additional embedding vectors on entities. Blend graph embeddings with text embeddings in the RRF fusion pipeline.

**Research:** TransE (Bordes et al., 2013) represents relations as translations in embedding space. TransR (Lin et al., 2015) extends this with relation-specific projection spaces. Both are well-established and have efficient implementations.

---

## Proposed Implementation Phases

### Phase 1: Core Tool Redesign (Current Milestone)

Implement the 16-tool surface described above, refactoring the existing 10 tools. This can be done incrementally — the underlying Repository methods already support most of these operations.

**Priority order within Phase 1:**
1. `ck_recall` (consolidate three search tools)
2. `ck_inspect` (replace `get_entity` with fuzzy matching and richer output)
3. `ck_timeline` and `ck_diff` (expose temporal power)
4. `ck_assert` and `ck_retract` (structured write path)
5. `ck_status` (meta-knowledge)
6. `ck_validate`, `ck_merge`, `ck_consolidate` (maintenance)
7. `ck_graph_walk` (graph traversal)
8. `ck_remember` (replace `add_memory` with improved version)

### Phase 2: Intelligence Layer (Post-Launch)

Build the features that make Context Keeper smarter than a database:
- Community detection and automatic clustering
- Contradiction resolution with provenance scoring
- Agent preference learning
- Conversation-aware ingestion

### Phase 3: Integration Layer (Plugins & Connectors Milestone)

Build connectors for the tools agents already use:
- GitHub (issues, PRs, commits, code review comments)
- Linear/Jira (tickets, sprints, project states)
- Slack (conversations, decisions, announcements)
- Documentation systems (Notion, Confluence, Markdown repos)

Each connector is a thin transform layer that converts source-specific data into Context Keeper's Episode/Entity/Relation model.

### Phase 4: Advanced Graph Intelligence

Implement the research-stage capabilities:
- Knowledge graph embeddings (TransE/TransR)
- Schema-aware code ingestion (tree-sitter)
- Subscription and notification model
- Incremental embedding updates

---

## How This Compares to the Ecosystem

| Capability | Anthropic Memory MCP | Zep/Graphiti | mem0 | **Context Keeper (Proposed)** |
|---|---|---|---|---|
| Entity/Relation storage | Yes | Yes | Flat KV | Yes |
| Temporal versioning | No | Yes | No | **Yes (valid_from/valid_until)** |
| Hybrid search (vector+keyword) | No (entity-only) | Yes | Vector only | **Yes (RRF fusion)** |
| Graph traversal | No | Yes (Neo4j) | No | **Yes (SurrealDB)** |
| Contradiction detection | No | No | No | **Yes (heuristic + provenance)** |
| Multi-agent provenance | No | No | No | **Yes (agent_id tracking)** |
| Meta-knowledge tools | No | No | No | **Yes (ck_status, ck_validate)** |
| Memory consolidation | No | Community summaries | No | **Yes (ck_consolidate)** |
| Knowledge maintenance tools | No | No | No | **Yes (ck_merge, ck_validate)** |
| Community detection | No | Yes | No | **Planned (Phase 2)** |
| Code-aware ingestion | No | No | No | **Planned (Phase 4)** |
| Rust performance | No (TypeScript) | No (Python) | No (Python) | **Yes** |

Context Keeper's unique positioning is the intersection of: temporal awareness + graph intelligence + maintenance tooling + Rust performance + MCP-native design. No existing system offers all five.

---

## References

- Bordes, A., Usunier, N., Garcia-Duran, A., Weston, J., Yakhnenko, O. (2013). "Translating Embeddings for Modeling Multi-relational Data." NeurIPS 2013.
- Lin, Y., Liu, Z., Sun, M., Liu, Y., Zhu, X. (2015). "Learning Entity and Relation Embeddings for Knowledge Graph Completion." AAAI 2015.
- Lu, S. et al. (2021). "CodeXGLUE: A Machine Learning Benchmark Dataset for Code Understanding and Generation." NeurIPS 2021.
- Packer, C. et al. (2023). "MemGPT: Towards LLMs as Operating Systems." arXiv:2310.08560.
- Rasmussen, P. et al. (2025). "Zep: A Temporal Knowledge Graph Architecture for Agent Memory." arXiv:2501.13956.
- W3C PROV Working Group. "PROV-DM: The PROV Data Model." W3C Recommendation, April 2013.
