---
sidebar_position: 5
title: MCP Tools Reference
description: Complete reference for all 6 MCP tools, resources, and prompts exposed by Context Keeper.
---

## Overview

Context Keeper implements the MCP protocol (version 2024-11-05) to integrate persistent memory with any AI agent. The server exposes:

- **10 tools** for ingesting, querying, and managing the knowledge graph
- **Browsable resources** including recent memories and per-entity snapshots
- **3 prompt templates** for common memory tasks

All tools support optional `namespace` scoping and `agent_id`/`agent_name` tracking, enabling multi-tenant and multi-agent deployments.

## Tools

### add_memory

Ingest text content into the knowledge graph. The system automatically extracts entities and relationships, stores them with timestamps, and deduplicates based on entity identity (name, optional type and namespace).

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| text | string | yes | Text content to ingest and extract entities/relations from |
| source | string | no | Source label for lineage (e.g., "slack", "email", "chat"). Defaults to "mcp". |
| namespace | string | no | Scope this memory to a namespace (e.g., "project-alpha", "acme-corp"). Omit for global scope. |
| agent_id | string | no | Identifier of the agent adding this memory (e.g., UUID). Used for audit trails. |
| agent_name | string | no | Human-readable name of the agent (e.g., "ResearchBot"). Paired with agent_id for logging. |

**Returns:** JSON object containing:
- `entities_created` (integer) — New entities added to the graph
- `entities_updated` (integer) — Existing entities with updated summaries
- `entities_invalidated` (integer) — Entities marked invalid due to contradictions
- `relations_created` (integer) — New relationships created
- `relations_merged` (integer) — Duplicate relationships merged
- `relations_pruned` (integer) — Relationships pruned due to temporal conflicts
- `memories_created` (integer) — Raw memory chunks stored
- `entity_names` (array) — Names of all entities (new and updated) from this ingestion
- `updates` (array) — Details of entity updates with old/new summaries
- `invalidations` (array) — Details of entities marked invalid with reasons

### search_memory

Hybrid vector + keyword search with Reciprocal Rank Fusion (RRF, K=60). Combines dense semantic search with sparse keyword matching for better recall.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| query | string | yes | Search query (natural language or keywords) |
| limit | integer | no | Maximum results to return. Defaults to 5. |
| entity_type | string | no | Filter results by entity type (e.g., "person", "organization", "codebase"). Omit to search all types. |
| namespace | string | no | Namespace to search within. Omit to search globally across all namespaces. |

**Returns:** JSON array of results. Each result contains:
- `name` (string) — Entity name
- `entity_type` (string) — Type classification
- `summary` (string) — Current summary of the entity
- `score` (float) — RRF fusion score (0-1, higher is better)

### expand_search

Query expansion powered by an LLM. Generates search variants and queries the graph with multiple formulations, then fuses results to improve recall on complex questions.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| query | string | yes | Query to expand and search |
| limit | integer | no | Maximum results to return. Defaults to 10. |
| entity_type | string | no | Filter results by entity type. Omit to search all types. |
| namespace | string | no | Namespace to search within. Omit for global search. |

**Returns:** JSON array of expanded search results with name, entity_type, summary, and score. Typically higher recall than `search_memory` for complex questions.

### get_entity

Look up an entity by exact name. Returns the full current state including relationships and temporal validity.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| name | string | yes | Exact entity name to retrieve |
| namespace | string | no | Namespace to look up in. Omit to search globally. |

**Returns:** JSON object containing:
- `name` (string) — Entity name
- `entity_type` (string) — Type classification
- `summary` (string) — Current summary
- `valid_from` (ISO 8601 timestamp) — When this entity became valid
- `valid_until` (ISO 8601 timestamp or null) — When this entity was invalidated (null = still valid)
- `relations` (array) — Related entities with relationship type and targets

### snapshot

Retrieve the point-in-time state of the knowledge graph as of a given timestamp. Useful for auditing how facts changed and comparing past vs. present.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| timestamp | string | yes | ISO 8601 timestamp (e.g., "2026-01-15T14:30:00Z") |

**Returns:** JSON object containing:
- `timestamp` (ISO 8601 string) — The snapshot timestamp
- `entity_count` (integer) — Number of valid entities at that time
- `relation_count` (integer) — Number of valid relationships at that time
- `entities` (array) — Full list of entities with name, type, summary, and temporal boundaries

### list_recent

Retrieve the most recent memories added to the graph, with optional limit.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| limit | integer | no | Maximum memories to return. Defaults to 10. |

**Returns:** JSON array of recent memory chunks with content, ingestion timestamp, and source.

### list_agents

List all AI agents that have contributed to the knowledge graph, including their namespaces and episode counts. Useful for multi-agent setups to audit which agents have been writing memories.

No parameters required.

**Returns:** JSON array of agent records with agent_id, agent_name, namespace, and episode count. Returns a descriptive message if no agents have contributed yet.

### list_namespaces

List all namespaces in the knowledge graph with entity counts. Namespaces partition the graph so different projects or teams can have isolated memory spaces.

No parameters required.

**Returns:** JSON array of namespace records with name and entity count. Returns a descriptive message if no namespaces exist (all data is in the global namespace).

### agent_activity

Show recent episodes ingested by a specific agent. Useful for auditing what a particular agent has been contributing.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| agent_id | string | yes | The agent_id to look up activity for |
| limit | integer | no | Maximum number of recent episodes to return. Defaults to 20. |

**Returns:** JSON array of episodes with content, source, namespace, and created_at timestamp. Returns a message if no activity is found for the given agent_id.

### cross_namespace_search

Search the entire knowledge graph across all namespaces using hybrid vector + keyword search. Unlike `search_memory`, this always searches globally, ignoring namespace scoping.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| query | string | yes | The search query string |
| limit | integer | no | Maximum number of results to return. Defaults to 10. |
| entity_type | string | no | Filter by entity type (person, organization, location, event, product, service, concept, file, other). |

**Returns:** JSON array of results with name, entity_type, summary, and score. Searches across every namespace in a single query.

## Resources

### Static Resources

**`memory://recent`** — Returns the 20 most recently added memories across all namespaces, formatted as text for easy browsing.

### Dynamic Resources

**`memory://entity/{name}`** — Per-entity resource. Automatically created for each active entity in the graph. Returns the entity's current summary, type, and related entities.

### Resource Templates

Clients can subscribe to the template `memory://entity/{name}` to receive updates whenever an entity matching the pattern is modified.

## Prompts

### summarize-topic

Searches the graph for entities related to a topic, then generates a summary.

**Arguments:**
- `topic` (string) — Topic to summarize (e.g., "architecture decisions", "customer feedback")

**Behavior:** Expands the topic query, retrieves matching entities, and returns a structured summary with key facts and relationships.

### what-changed

Compares the graph at a past timestamp against the current state. Returns what entities were added, updated, or invalidated.

**Arguments:**
- `since` (ISO 8601 timestamp) — Comparison base time (e.g., "2026-01-01T00:00:00Z")

**Behavior:** Retrieves a snapshot at the given time, compares against current entities, and summarizes changes.

### add-context

Ingests raw conversation or document context into the graph. Useful for feeding an agent's chat history or external documents into the knowledge base.

**Arguments:**
- `context` (string) — Text content to ingest (e.g., conversation excerpt, document chunk)

**Behavior:** Calls `add_memory` internally with source "prompt:add-context" and returns extraction details.

## Transports

### Stdio (Default)

The MCP server runs as a subprocess communicating over stdin/stdout. Use this for local desktop agents (Claude Desktop, Cursor IDE).

```bash
cargo run -p context-keeper-mcp
```

### Streamable HTTP

For multi-agent scenarios, run the server on HTTP with streaming support.

```bash
MCP_TRANSPORT=http MCP_HTTP_PORT=3000 cargo run -p context-keeper-mcp
```

Clients connect to `http://localhost:3000/mcp` and use the same tool/resource/prompt schema as stdio.

## Error Handling

All tools return a JSON response. On success, the top-level `success` field is `true` and result data is in the response. On error, `success` is `false` and `error` contains a descriptive message.

Example error response:
```json
{
  "success": false,
  "error": "Entity not found: Alice"
}
```

## Rate Limiting and Defaults

- `search_memory` and `expand_search` default to 5–10 results; requesting higher limits incurs higher compute cost.
- `list_recent` caps at 100 memories per call.
- `snapshot` queries are expensive; consider caching for bulk historical analysis.
- Namespace filtering is free; it simply narrows the search space without additional cost.
