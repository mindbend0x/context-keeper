---
name: search-context
description: Deep multi-query context retrieval from the knowledge graph
---

Search memory for relevant context before starting a task.

1. Call `search_memory` with the user's question or topic.
2. If results are sparse, call `expand_search` with the same query for broader recall.
3. For entity-specific lookups, use `get_entity` with the entity name.
4. Synthesize findings into a concise summary before answering.

Use this skill whenever the user asks about something that may have been discussed in a prior session. Always search before claiming you don't know something.
