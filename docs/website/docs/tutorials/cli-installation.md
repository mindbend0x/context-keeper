---
sidebar_position: 2
title: Installing the CLI
description: Install the Context Keeper CLI and start adding memories from your terminal.
---

# Installing the CLI

The Context Keeper CLI lets you add memories, search the knowledge graph, and inspect entities directly from your terminal. It's ideal for scripting, testing, and quick interactions.

## Installation

### Via Homebrew (macOS / Linux)

```bash
brew install mindbend0x/context-keeper/context-keeper
```

### From source

```bash
git clone https://github.com/mindbend0x/context-keeper.git
cd context-keeper
cargo build --release -p context-keeper-cli
```

The binary will be at `target/release/context-keeper`.

### Verify installation

```bash
context-keeper --help
```

Expected output:

```
Context Keeper CLI - Persistent memory for AI agents

Usage: context-keeper [OPTIONS] <COMMAND>

Commands:
  add     Add a new memory to the knowledge graph
  search  Search memories using hybrid search
  entity  Get detailed information about an entity
  recent  List recent memories
  help    Print help

Options:
  -u, --api-url <URL>              OpenAI-compatible API endpoint
  -k, --api-key <KEY>              API key for authentication
  -e, --embedding-model-name <M>   Embedding model name
  -d, --embedding-dims <N>         Embedding vector dimensions [default: 1536]
  -x, --extraction-model-name <M>  Extraction model name
  -f, --db-file-path <PATH>        SurrealDB file path [default: context.sql]
      --storage <BACKEND>          Storage backend [default: rocksdb:~/.context-keeper/data]
  -h, --help                       Print help
  -V, --version                    Print version
```

---

## First-use walkthrough

### 1. Add your first memory

```bash
context-keeper add --text "Alice is a senior engineer at Acme Corp" --source chat
```

On first run, Context Keeper initializes a RocksDB store at `~/.context-keeper/data`. You'll see output showing the extracted entities and relations:

```
Episode created: ep_abc123
Entities extracted: 2
  - Alice (person): Senior engineer at Acme Corp
  - Acme Corp (organization): Technology company
Relations extracted: 1
  - Alice --[works_at]--> Acme Corp (confidence: 0.95)
```

:::tip
In mock mode (no API key), entity extraction uses simple heuristics. Capitalized words become entities, and common patterns like "works at" become relations. The output format may differ slightly.
:::

### 2. Add more context

```bash
context-keeper add --text "Bob manages the infrastructure team at Acme" --source chat
context-keeper add --text "Alice and Bob are working on Project Phoenix" --source meeting-notes
```

### 3. Search your memories

```bash
context-keeper search --query "Who works at Acme?"
```

Output:

```
Results (3 matches):
  1. [0.92] Alice is a senior engineer at Acme Corp (source: chat)
  2. [0.87] Bob manages the infrastructure team at Acme (source: chat)
  3. [0.71] Alice and Bob are working on Project Phoenix (source: meeting-notes)
```

### 4. Inspect an entity

```bash
context-keeper entity --name "Alice"
```

Output:

```
Entity: Alice
  Type: person
  Summary: Senior engineer at Acme Corp, working on Project Phoenix
  Relations:
    - works_at → Acme Corp (confidence: 0.95)
    - collaborates_with → Bob (confidence: 0.80)
    - works_on → Project Phoenix (confidence: 0.85)
  Valid from: 2025-01-15T10:30:00Z
  Valid until: (active)
```

### 5. View recent memories

```bash
context-keeper recent --limit 5
```

---

## Using namespaces

Namespaces let you organize memories into separate scopes:

```bash
# Add memories to different namespaces
context-keeper add --text "Sprint 14 ends Friday" --source standup --namespace work
context-keeper add --text "Dentist appointment on Thursday" --source calendar --namespace personal

# Search within a namespace
context-keeper search --query "what's happening this week?" --namespace work
```

---

## Mock mode vs LLM mode

**Mock mode** (default, no API key):
- Uses heuristic extraction
- Capitalized words become entities
- Simple pattern matching for relations
- Instant, no network calls, free

**LLM mode** (with API key):
- Accurate entity and relation extraction
- Semantic understanding of text
- Requires an OpenAI-compatible API key

To enable LLM mode:

```bash
export OPENAI_API_URL=https://api.openai.com/v1
export OPENAI_API_KEY=sk-xxxxx
export EMBEDDING_MODEL=text-embedding-3-small
export EXTRACTION_MODEL=gpt-4o-mini
```

Or pass flags directly:

```bash
context-keeper \
  --api-url https://api.openai.com/v1 \
  --api-key sk-xxxxx \
  --embedding-model-name text-embedding-3-small \
  --extraction-model-name gpt-4o-mini \
  add --text "Alice joined the platform team"
```

---

## Data management

### Data location

Default: `~/.context-keeper/data` (RocksDB)

Override with:
```bash
context-keeper --storage rocksdb:/path/to/custom/data add --text "..."
```

### Using in-memory storage

For testing without persisting data:
```bash
context-keeper --storage memory add --text "Temporary test data"
```

### Resetting data

To start fresh, remove the data directory:

```bash
rm -rf ~/.context-keeper/data
```

---

## Next steps

- [MCP Server Setup](/docs/tutorials/mcp-server-setup) — Connect Context Keeper to your AI assistant
- [Configuration](/docs/configuration) — Full reference for all environment variables and flags
- [CLI Reference](/docs/cli-reference) — Complete command documentation
