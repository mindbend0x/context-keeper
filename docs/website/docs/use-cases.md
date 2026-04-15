---
sidebar_position: 6
title: Use Cases
description: Concrete scenarios for using Context Keeper with AI agents, RAG pipelines, and developer tools.
---

import DemoVideo from '@site/src/components/DemoVideo';

## Conversational Memory for Chat Agents

Give long-lived chat agents like Claude Desktop or Cursor persistent memory across sessions. Instead of re-explaining the same facts in every conversation, the agent recalls relevant past conversations automatically.

<DemoVideo
  caption="Claude remembers preferences across conversations — no prompt engineering required."
  alt="Demo of conversational memory with Claude Desktop"
/>

### How It Works

1. **Agent logs conversation context** — After each multi-turn conversation, the agent calls `add_memory` with the conversation summary and `agent_name` to identify itself.

2. **On next session, agent searches its memory** — The agent calls `search_memory` with user context (e.g., "projects I'm working on") to recall past discussions before responding.

3. **Namespace per user or project** — Use `namespace` to isolate memory by user ID or project folder, ensuring privacy and relevance.

**Example Flow:**
```
Session 1: User asks "How do I configure async/await in this codebase?"
Agent: Answers the question, then calls:
  add_memory(text="User asked about async/await configuration in project-alpha",
             agent_name="Claude", namespace="user-123")

Session 2 (next day): Same user asks "What did we talk about yesterday?"
Agent: Calls search_memory(query="async/await", namespace="user-123")
       Recalls the previous session and responds "We discussed async/await config..."
```

## Knowledge Base for RAG

Augment your RAG pipeline with Context Keeper's hybrid search and entity relationship awareness. Instead of retrieving document chunks in isolation, the system understands semantic relationships and returns richer context.

### How It Works

1. **Ingest documents** — Call `add_memory` for each document section (e.g., design docs, API specs, architecture decisions).

2. **Hybrid search for better recall** — When answering a user query, call `search_memory` or `expand_search` instead of vector-only retrieval. The RRF fusion often surfaces relevant entities that pure semantic search would miss.

3. **Follow relationships** — When retrieving an entity like "UserService", call `get_entity` to see related components and decisions. This adds structure that chunk-based RAG lacks.

**Example Flow:**
```
Ingestion:
  add_memory(text="UserService is responsible for authentication and account management.
                   Depends on PostgreSQL and Redis.", source="architecture")
  add_memory(text="Redis is used for session caching to reduce database load.",
             source="architecture")

Query time:
  search_memory(query="how does session management work?")
  Returns: [UserService (score=0.92), Redis (score=0.85)]
  Call get_entity("UserService") to see that it depends on Redis
  LLM returns a coherent answer that includes both components.
```

## Codebase Intelligence

Equip your code agent with a persistent understanding of architectural decisions, ownership, dependencies, and historical changes. Useful for onboarding, code review, refactoring, and impact analysis.

<DemoVideo
  caption="An agent in Cursor stores project context and recalls it across sessions."
  alt="Demo of codebase intelligence with Cursor"
/>

### How It Works

1. **Feed architectural context** — Ingest architecture decision records (ADRs), design docs, and dependency maps into the graph with a `namespace` like "project-backend".

2. **Agent enriches with code observations** — When the agent analyzes code, it calls `add_memory` with findings: "File X is owned by team-backend", "Migration Y was reverted due to performance", etc.

3. **Query before making changes** — When planning a refactor, the agent calls `expand_search` on "components that depend on UserService" to assess impact.

4. **Use the "add-context" prompt** — The agent can use this template to ingest conversation context (e.g., code review feedback) into the graph.

**Example Flow:**
```
Initialization:
  add_memory(text="UserService is owned by @alice and @bob.
                   It provides user authentication and profile management.",
             source="codebase", namespace="project-backend")

During code review:
  Agent: "I see a change to UserService. Let me check who owns it."
  get_entity("UserService", namespace="project-backend")
  Returns: owners = [@alice, @bob]
  Agent routes review to the right team.
```

## Temporal Audit Trail

Track how facts change over time. Use snapshots to compare graph states and answer questions like "When did we decide to use Kafka?" or "What was the org structure before the merger?"

### How It Works

1. **Memories are timestamped** — Every call to `add_memory` records the ingestion time and temporal validity (`valid_from`, `valid_until`).

2. **Take snapshots at key points** — After significant updates, call `snapshot` with the current timestamp to create a checkpoint.

3. **Compare past vs. present** — Use the `what-changed` prompt to compare a snapshot from month ago against today, or call `snapshot` with a historical timestamp.

**Example Flow:**
```
2026-01-15:
  add_memory(text="We use PostgreSQL 12 with replication",
             source="infrastructure")
  snapshot(timestamp="2026-01-15T18:00:00Z")

2026-03-15:
  add_memory(text="We migrated to PostgreSQL 15 for performance",
             source="infrastructure")

Later:
  what_changed(since="2026-01-15T18:00:00Z")
  Returns: "PostgreSQL version was updated from 12 to 15 due to performance improvements"
```

## Multi-Agent Shared Memory

Run Context Keeper over HTTP so multiple agents can share one knowledge graph. One agent researches a topic, another summarizes findings, a third validates answers — all pulling from the same persistent memory.

<DemoVideo
  caption="Two agents writing and reading from the same knowledge graph via HTTP."
  alt="Demo of multi-agent shared memory over HTTP"
/>

### How It Works

1. **Launch on HTTP** — Start the server with `MCP_TRANSPORT=http MCP_HTTP_PORT=3000`.

2. **Agents identify themselves** — Each agent includes `agent_id` (UUID or hostname) and `agent_name` (e.g., "ResearchBot", "SummaryBot") in `add_memory` calls.

3. **Namespace per project or domain** — Use `namespace` to partition the graph. For example, agents working on "project-alpha" all use `namespace="project-alpha"`.

4. **Shared discovery** — All agents call `search_memory` with the same namespace, so findings from one agent become immediately available to others.

**Example Flow:**
```
ResearchBot (agent_id=uuid-1, agent_name="ResearchBot"):
  add_memory(text="Paper X demonstrates that technique Y improves latency by 30%",
             agent_id="uuid-1", agent_name="ResearchBot",
             namespace="project-alpha")

SummaryBot (agent_id=uuid-2, agent_name="SummaryBot"):
  search_memory(query="latency improvements", namespace="project-alpha")
  Finds: "Paper X demonstrates that technique Y improves latency by 30%"
  Generates summary with the research.
```

## Personal Knowledge Graph

Use the CLI and memory tools to build a personal knowledge graph of notes, contacts, projects, and ideas. Search by meaning, not just keywords. Great for maintaining a second brain or building conceptual maps.

### How It Works

1. **Add notes and facts via CLI** — Use `cargo run -p context-keeper-cli -- add` to quickly log observations.

2. **Link entities across contexts** — The system automatically extracts relationships. "Alice works at Acme" creates a person entity, an organization entity, and a "works-at" relationship.

3. **Search by concept** — Call `expand_search` with natural language queries like "What are my projects with machine learning?" or "Who have I met from AWS?"

4. **Export snapshots** — Take periodic snapshots and export them for backup or analysis.

**Example Flow:**
```
Day 1:
  CLI: add --text "Alice is VP of Engineering at Acme Corp" --source "meeting"
  CLI: add --text "Acme Corp is building an ML recommendation system" --source "meeting"

Day 30:
  CLI: search --query "Who have I met from companies building recommendation systems?"
  Returns: Alice (score=0.89)

  CLI: entity --name "Acme Corp"
  Returns: Acme Corp (type=organization) -> Alice (type=person, role=VP)
                                          -> Recommendation system (type=project)
```

---

**Next:** Explore the [MCP Tools Reference](./mcp-tools.md) for complete API documentation, or see [Getting Started](./getting-started.md) for setup instructions.
