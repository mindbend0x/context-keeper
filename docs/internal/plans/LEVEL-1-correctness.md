# Level 1: Core Correctness

**Goal:** Fix the two critical correctness issues that would embarrass a public demo — memory updates (FZ-12) and entity relationship noise (FZ-13).
**Timeline:** 3-5 days
**Depends on:** Level 0 (Foundation Sprint)
**Unlocks:** Level 2 (Publishing)

## Tasks

### 1.1 — Memory Updates / Negation Detection (FZ-12)

**Linear:** [FZ-12](https://linear.app/0x319/issue/FZ-12/memory-updates)
**Branch:** `mindbend0x/fz-12-memory-updates`
**Files:** `crates/context-keeper-core/src/ingestion/pipeline.rs`, `crates/context-keeper-rig/src/extraction.rs`, `crates/context-keeper-surreal/src/repository.rs`

Current state: Basic contradiction detection exists using heuristic markers ("no longer", "former", "ex-"). This needs to become robust.

Sub-tasks:
- [ ] **Semantic contradiction detection** — Use LLM to compare new text against existing entity summaries. If contradicts, mark old entity version `valid_until = now`.
- [ ] **Entity re-identification** — "Alice at Acme" and "Alice (software engineer)" should resolve to same node. Use embedding similarity + LLM confirmation for ambiguous cases.
- [ ] **Update diffs in ingestion response** — `IngestionDiff` should clearly report: entities created, entities updated, entities invalidated, relations invalidated.
- [ ] **Temporal invalidation cascade** — When an entity is invalidated, also invalidate its relations.

### 1.2 — Entity Relationship Quality (FZ-13)

**Linear:** [FZ-13](https://linear.app/0x319/issue/FZ-13/improve-entity-relationship-management)
**Branch:** `mindbend0x/fz-13-improve-entity-relationship-management`
**Files:** `crates/context-keeper-core/src/models.rs`, `crates/context-keeper-rig/src/extraction.rs`, `crates/context-keeper-surreal/src/repository.rs`

Current state: Extraction produces noisy relations. Too many generic "related_to" edges. No deduplication.

Sub-tasks:
- [ ] **Constrain relation types** — The `RelationType` enum already exists (WorksAt, LocatedIn, PartOf, Uses, CreatedBy, Knows, DependsOn, RelatedTo). Tighten extraction prompts to use these canonical types. Reject or remap unknown predicates.
- [ ] **Relation deduplication** — During ingestion, check if a semantically equivalent relation already exists (same subject + object + similar predicate). Merge instead of creating duplicates.
- [ ] **Confidence-based pruning** — Current threshold is 50. Evaluate whether this is right by testing against the existing fixtures. Consider per-type thresholds.
- [ ] **Bidirectional relation normalization** — "Alice knows Bob" and "Bob knows Alice" should be one edge, not two.

### 1.3 — Expand Entity Types (FZ-14) — Light Pass

**Linear:** [FZ-14](https://linear.app/0x319/issue/FZ-14/expand-entity-types)
**Files:** `crates/context-keeper-core/src/models.rs`, `crates/context-keeper-rig/src/extraction.rs`

Current state: EntityType enum already has Person, Organization, Location, Event, Product, Service, Concept, File, Other. Extraction prompts need updating to use the full taxonomy.

Sub-tasks:
- [ ] **Update extraction prompts** — Instruct the LLM to classify entities using the existing enum.
- [ ] **Type-filtered search** — Add optional `entity_type` filter to search queries in repository.
- [ ] **Surface entity type in MCP tools** — `get_entity` and `search_memory` should return entity types.

## Definition of Done

- [ ] Demo scenario works: ingest "Alice works at Acme", then "Alice left Acme for BigCo" → search "who works at Acme?" returns only non-Alice results
- [ ] Relation noise reduced: same fixtures produce fewer generic "related_to" edges
- [ ] Entity types populated correctly from LLM extraction
- [ ] All existing tests still pass
- [ ] New tests cover negation, re-identification, relation dedup

## Delete This File When

FZ-12, FZ-13, and FZ-14 are all merged and the demo scenario passes.
