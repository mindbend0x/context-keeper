---
sidebar_position: 2
title: How It Works
description: Understand the ingestion pipeline, hybrid search, and temporal tracking inside Context Keeper.
---

# How It Works

Context Keeper turns unstructured text into a temporal knowledge graph. This guide explains the core mechanisms: the ingestion pipeline, hybrid search, and temporal tracking.

## Overview

Every piece of information you add to Context Keeper flows through an **ingestion pipeline**, then becomes queryable via **hybrid search**:

```mermaid
graph LR
    A["📝 Raw Text"] --> B["🔍 Extract<br/>Entities & Relations"]
    B --> C["🧬 Embed<br/>Vector Embeddings"]
    C --> D["💾 Store<br/>Temporal Graph"]
    D --> E["🔎 Search<br/>Hybrid RRF"]

    style A fill:#ea580c,stroke:#c2410c,color:#fff
    style B fill:#f97316,stroke:#ea580c,color:#fff
    style C fill:#fb923c,stroke:#f97316,color:#000
    style D fill:#eab308,stroke:#ca8a04,color:#000
    style E fill:#fbbf24,stroke:#eab308,color:#000
```

The result is a living, time-aware knowledge base that your agent can query to recall and reason about past interactions.

## Ingestion Pipeline

When you call `add` or `search` (which also ingests context), here's what happens:

### Step 1: Episode Creation

An **episode** is the base unit of input:
- Raw text content
- Source identifier (e.g., "chat", "email", "document")
- Timestamp (defaults to now)

### Step 2: Entity Extraction

The system identifies key entities (people, organizations, concepts) in the text.

**In LLM mode:** A language model reads the text and outputs structured JSON with entities (name, type, summary).

**In mock mode:** The system uses simple heuristics—capitalized words are marked as entities, their type is inferred from context, and summaries are extracted.

Example:
```
Input: "Alice and Bob founded TechCorp in 2020."
Extracted entities:
  - Alice (type: person)
  - Bob (type: person)
  - TechCorp (type: organization)
```

### Step 3: Relation Extraction

After entities are identified, the system finds relationships between them.

**In LLM mode:** The model outputs structured JSON with relations (from entity, to entity, relation type, confidence).

**In mock mode:** Simple patterns like "X founded Y" or "X works at Y" are recognized.

Example:
```
Relations:
  - Alice --founded--> TechCorp
  - Bob --founded--> TechCorp
```

### Step 4: Embedding Generation

Each entity and the full episode text are embedded using a semantic embedding model (e.g., `text-embedding-3-small`). These embeddings enable vector similarity search.

#### What gets embedded

- **Entity summaries** — The rich text description of each entity (not just the name)
- **Full episode text** — The raw memory chunk, enabling semantic search over original content

Relation types (e.g., "works_at") are not independently embedded — they're found via graph traversal from matching entities.

#### Dimension tradeoffs

| Dimensions | Model Example | Tradeoff |
|-----------|---------------|----------|
| 256 | text-embedding-3-small (truncated) | Fastest, lowest storage, slightly lower recall |
| 1536 | text-embedding-3-small (default) | Good balance of speed and accuracy |
| 3072 | text-embedding-3-large | Highest accuracy, more storage and compute |

Configure via `EMBEDDING_DIMS`. The HNSW index is rebuilt to match.

#### Mock embedder

In mock mode, the `MockEmbedder` generates deterministic vectors by hashing the input text. This ensures reproducible test results without network calls. The vectors have the configured dimensionality but aren't semantically meaningful — they're purely for testing pipeline correctness.

### Step 5: Persistence & Entity Upsert

Entities are stored with deduplication:
- If an entity with the same name already exists, its summary is **merged** with the new information
- If new information contradicts old data, the old entity is **invalidated** (soft delete via `valid_until`)
- Relations between entities are created or updated
- The raw memory chunk is linked to the episode

### Step 6: Diff Returned

The pipeline returns a diff showing:
- Entities created, updated, or invalidated
- Relations added or changed
- Conflicts or contradictions detected

## Hybrid Search

Context Keeper uses a dual-search strategy to balance semantic and lexical relevance:

```mermaid
graph TD
    Q["User Query"] --> VS["Vector Search<br/>(HNSW)"]
    Q --> KS["Keyword Search<br/>(BM25)"]
    VS --> RRF["Reciprocal Rank Fusion<br/>(K=60)"]
    KS --> RRF
    RRF --> R["Ranked Results"]

    style Q fill:#ea580c,stroke:#c2410c,color:#fff
    style VS fill:#f97316,stroke:#ea580c,color:#fff
    style KS fill:#fb923c,stroke:#f97316,color:#000
    style RRF fill:#eab308,stroke:#ca8a04,color:#000
    style R fill:#fbbf24,stroke:#eab308,color:#000
```

### Vector Search (HNSW)

Embeddings are indexed in an HNSW (Hierarchical Navigable Small World) vector index. When you query:

1. Your query is embedded using the same model as the corpus
2. Nearest neighbors are retrieved via approximate nearest neighbor search
3. Results are ranked by cosine similarity

This captures semantic meaning: "CEO" is similar to "chief executive".

### Keyword Search (BM25)

In parallel, the full episode text and entity summaries are indexed with BM25 (probabilistic relevance). This captures exact term matching: "CEO" must literally appear.

### Reciprocal Rank Fusion (K=60)

The two ranked lists are merged via RRF. For each document `d`, the fused score is:

```
RRF(d) = Σ  1 / (K + rank_r(d))
         r
```

Where `r` ranges over the rankers (vector and keyword), and `rank_r(d)` is the position of document `d` in ranker `r`'s output (1-indexed). Documents not present in a ranker's list receive no contribution from that ranker.

#### Why K=60?

The constant K (set to 60, from the [Cormack et al. 2009 paper](https://dl.acm.org/doi/10.1145/1571941.1572114)) controls how much weight the top-ranked results receive relative to lower-ranked ones:

- **Lower K (e.g., 1)**: Top results dominate. A rank-1 result gets score `1/2 = 0.5`, rank-10 gets `1/11 = 0.09` — a 5.5x difference.
- **Higher K (e.g., 60)**: Rankings are flattened. Rank-1 gets `1/61 ≈ 0.016`, rank-10 gets `1/70 ≈ 0.014` — only a 1.1x difference.

K=60 is the sweet spot: it prevents any single ranker from dominating while still rewarding higher ranks. This makes RRF robust across domains without requiring tuned weights.

#### Worked example

```
Query: "Who leads engineering?"

Vector search results:          Keyword search results:
  1. Alice (0.95 sim)             1. "leads engineering team"
  2. "leads" relation             2. Engineering Team entity
  3. Engineering Team             3. Alice
  4. Bob                          4. Bob

RRF scores:
  Alice:           1/(60+1) + 1/(60+3) = 0.01639 + 0.01587 = 0.03226  ← rank 1
  "leads" relation: 1/(60+2) + 0       = 0.01613                       ← rank 3
  Engineering Team: 1/(60+3) + 1/(60+2) = 0.01587 + 0.01613 = 0.03200  ← rank 2
  Bob:             1/(60+4) + 1/(60+4) = 0.01563 + 0.01563 = 0.03125  ← rank 4

Final ranking: Alice → Engineering Team → Bob → "leads" relation
```

Notice how Alice rises to #1 because she appears in both lists (ranks 1 and 3), even though she's only rank 3 in keyword search. This is the key property of RRF: **presence in multiple lists is rewarded**.

### Query Expansion

Before searching, the system expands your query into 3 semantic variants using an LLM (or heuristics). Each variant is searched independently, and all results are fused. This improves recall.

Example:
```
Input: "Who leads engineering?"
Expanded variants:
  - "engineering leadership"
  - "head of engineering"
  - "chief engineer"

All three are searched, results combined via RRF.
```

## Temporal Tracking

Context Keeper never forgets or truly deletes. Every fact carries a **valid time window**:

- `valid_from` — when the fact became true
- `valid_until` — when it stopped being true (null = still valid)

### Soft Deletes

When you invalidate an entity (e.g., because new information contradicts it), the system sets `valid_until = now`. The old record remains in the database for audit purposes. Point-in-time queries can reconstruct history.

### Point-in-Time Snapshots

Use the `snapshot` tool to query the state of the graph at any past time:
```bash
context-keeper snapshot --timestamp "2024-01-15T10:00:00Z"
```

This returns only entities and relations that were valid at that moment.

### Changefeeds & Audit

SurrealDB changefeeds track all mutations. You can subscribe to changes in real-time, useful for audit trails and downstream systems.

## Entity Resolution

The system maintains a single canonical entity per composite key of **(name, type, namespace)**. When a new entity is extracted from text, the resolution process determines whether to create, merge, or invalidate:

```mermaid
flowchart TD
    A["New entity extracted"] --> B{"Composite key exists?<br/>(name + type + namespace)"}
    B -->|No| C["Create new entity"]
    B -->|Yes| D{"Summaries contradict?"}
    D -->|No| E["Merge: append new info<br/>to existing summary"]
    D -->|Yes| F["Invalidate old entity<br/>(set valid_until = now)"]
    F --> G["Create new entity<br/>(valid_from = now)"]

    style A fill:#ea580c,stroke:#c2410c,color:#fff
    style C fill:#22c55e,stroke:#16a34a,color:#fff
    style E fill:#eab308,stroke:#ca8a04,color:#000
    style F fill:#ef4444,stroke:#dc2626,color:#fff
    style G fill:#22c55e,stroke:#16a34a,color:#fff
```

### Summary merging

When the same entity appears in multiple inputs without contradiction, summaries are enriched:

```
Existing: "Alice is a senior engineer at Acme Corp"
New input: "Alice leads the platform team and is based in SF"
Merged:    "Alice is a senior engineer at Acme Corp. Leads the platform team, based in SF."
```

### Contradiction detection

In LLM mode, the extractor detects when new information contradicts the existing summary. When this happens:

1. The old entity is **invalidated** by setting `valid_until = now`
2. A new entity is created with `valid_from = now` and the updated information
3. The old entity remains in the database for historical queries

Example:
```
Existing: "Alice works at Acme Corp" (valid_from: Jan 1)
New input: "Alice joined TechCorp last month"
→ Old entity invalidated: valid_until = Mar 15
→ New entity created: "Alice works at TechCorp" (valid_from: Mar 15)
```

### Relation confidence

When duplicate relations are found between the same entity pair, confidences are merged using the maximum value. This reflects that repeated observation of a relationship increases certainty.

:::tip
Composite keys were introduced in [ADR-001 R3](/docs/adr-001) to prevent false merges when different entities share the same name. Use `namespace` to further isolate entities across projects or tenants.
:::

## Data Model

### Episode

The raw input unit:
- `id` — unique identifier
- `content` — raw text
- `source` — origin (e.g., "chat", "email")
- `timestamp` — when the episode was ingested

### Entity

A distinct concept or actor:
- `id` — unique identifier
- `name` — human-readable name (dedup key)
- `type` — category (person, organization, concept, etc.)
- `summary` — rich description
- `embedding` — semantic vector
- `valid_from` — start of validity
- `valid_until` — end of validity (null = still valid)

### Relation

A typed edge between two entities:
- `id` — unique identifier
- `from_entity_id` — source entity
- `to_entity_id` — target entity
- `relation_type` — type (e.g., "works_at", "manages", "founded")
- `confidence` — strength of the relation (0.0–1.0)
- `valid_from` — start of validity
- `valid_until` — end of validity (null = still valid)

### Memory

A chunk of text tied to an episode and entities:
- `id` — unique identifier
- `episode_id` — source episode
- `content` — text snippet
- `entity_ids` — linked entities
- `embedding` — semantic vector

## Summary

Context Keeper's strength is in its **temporal, hybrid-search knowledge graph**. Agents can ingest conversational context, structured data, or documents, and then query for semantically similar facts, exact keywords, or historical snapshots—all with full provenance and soft-delete audit trails.
