---
sidebar_position: 7
title: CLI Reference
description: Command reference for the Context Keeper CLI tool.
---

# CLI Reference

## Overview

The Context Keeper CLI provides a command-line interface for adding and searching memories, querying entities, and viewing recent activity. It uses the same core ingestion and search pipeline as the MCP server, enabling seamless integration with scripts and workflows.

## Installation

Install the CLI directly from source:

```bash
cargo install --path crates/context-keeper-cli
```

Or build and run locally:

```bash
cargo run -p context-keeper-cli -- [COMMAND] [OPTIONS]
```

## Commands

### add

Add a new memory to the knowledge graph.

```bash
context-keeper add --text "Alice works at Acme" --source chat
```

**Flags:**
- `--text, -t` (required) — The memory text to add.
- `--source, -s` (default: "cli") — Source identifier for the memory.
- `--namespace` (optional) — Namespace to organize memories.

**Example:**

```bash
context-keeper add \
  --text "Alice works at Acme and leads the engineering team" \
  --source chat \
  --namespace "team-context"
```

### search

Search memories using hybrid keyword and vector search.

```bash
context-keeper search --query "Who works at Acme?" --limit 10
```

**Flags:**
- `--query, -q` (required) — Search query.
- `--limit, -l` (default: 5) — Maximum number of results.
- `--namespace` (optional) — Filter results to a specific namespace.

**Example:**

```bash
context-keeper search \
  --query "engineering team members" \
  --limit 20 \
  --namespace "team-context"
```

### entity

Get detailed information about a specific entity.

```bash
context-keeper entity --name "Alice"
```

**Flags:**
- `--name, -n` (required) — Entity name to look up.
- `--namespace` (optional) — Namespace containing the entity.

**Example:**

```bash
context-keeper entity --name "Alice" --namespace "team-context"
```

### recent

List recently added or updated memories.

```bash
context-keeper recent --limit 5
```

**Flags:**
- `--limit, -l` (default: 10) — Number of recent memories to display.

## Global Flags

These flags apply to all commands and can be set via environment variables or CLI arguments.

| Flag | Env Var | Default | Description |
|------|---------|---------|-------------|
| `--embedding-model-name, -e` | `EMBEDDING_MODEL` | — | Embedding model name (e.g., `text-embedding-3-small`) |
| `--embedding-dims, -d` | `EMBEDDING_DIMS` | 1536 | Embedding vector dimensions |
| `--extraction-model-name, -x` | `EXTRACTION_MODEL` | — | Extraction model name (e.g., `gpt-4o-mini`) |
| `--api-url, -u` | `OPENAI_API_URL` | — | OpenAI-compatible API endpoint |
| `--api-key, -k` | `OPENAI_API_KEY` | — | API key for the LLM provider |
| `--db-file-path, -f` | `DB_FILE_PATH` | context.sql | Path to SurrealDB file |
| `--storage` | `STORAGE_BACKEND` | rocksdb:~/.context-keeper/data | Storage backend configuration |

**Example with global flags:**

```bash
context-keeper \
  --api-url https://api.openai.com/v1 \
  --api-key sk-xxxxx \
  --embedding-model-name text-embedding-3-small \
  --embedding-dims 1536 \
  search --query "Alice"
```

## Storage Backends

The CLI supports multiple storage backends for flexibility:

### RocksDB (Default)

Persistent, fast key-value storage using RocksDB.

```bash
--storage rocksdb:~/.context-keeper/data
```

**Features:** Persistent across runs, embeds LevelDB compaction, efficient vector indexing.

### Memory

Ephemeral in-memory storage. Useful for testing and development.

```bash
--storage memory
```

**Features:** Fast, no disk I/O, exports data to disk on exit (optional), resets on process restart.

### Remote (WIP)

Connect to a remote Context Keeper instance (coming soon).

```bash
--storage remote:ws://localhost:9000
```

**Features:** Distributed queries, centralized knowledge graph, multi-client collaboration.
