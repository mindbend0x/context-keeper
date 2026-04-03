---
sidebar_position: 1
title: Getting Started
description: Install Context Keeper and start building your agent's memory in minutes.
---

import DemoVideo from '@site/src/components/DemoVideo';

# Getting Started

Welcome to Context Keeper! This guide will help you install and run the system in minutes.

## Prerequisites

Before you begin, ensure you have:

- **Rust toolchain** — Install via [rustup](https://rustup.rs/) (latest stable)
- **Optional: Docker** — For containerized setup
- **Optional: OpenAI-compatible API key** — Only needed for LLM-powered extraction. Mock mode works without it.

## Installation

### Via Cargo

Install the MCP server and CLI directly:

```bash
cargo install context-keeper-mcp
cargo install context-keeper-cli
```

### From Source

Clone the repository and build:

```bash
git clone https://github.com/0x313/context-keeper.git
cd context-keeper
cargo build --release
```

Binaries will be in `target/release/`.

## Quick Start with CLI

The easiest way to try Context Keeper is via the CLI. Add some memories and search them:

<DemoVideo
  caption="Add a memory, search it, and inspect the extracted entity — all from the terminal."
  alt="Terminal recording of Context Keeper CLI usage"
/>

```bash
# Add memories
context-keeper add --text "Alice is a senior engineer at Acme Corp" --source chat
context-keeper add --text "Bob manages the infrastructure team at Acme" --source chat

# Search for relationships
context-keeper search --query "Who works at Acme?"

# Look up an entity
context-keeper entity --name "Alice"
```

:::tip
On your first run, Context Keeper initializes a RocksDB store at `~/.context-keeper/data`. No external database required.
:::

## MCP Server Setup

To use Context Keeper as an MCP server with Claude Desktop or Cursor, add it to your client config.

### Claude Desktop

Edit `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS) or the Windows equivalent:

```json
{
  "mcpServers": {
    "context-keeper": {
      "command": "context-keeper-mcp"
    }
  }
}
```

Restart Claude Desktop, and Context Keeper tools will be available.

### Cursor

Add the same config to your Cursor MCP settings, or use HTTP transport (see below).

### HTTP Transport

For remote or containerized setups, run the server in HTTP mode:

```bash
MCP_TRANSPORT=http MCP_HTTP_PORT=3000 context-keeper-mcp
```

Then configure your client to point to `http://localhost:3000/mcp`.

:::info
HTTP mode is useful for Docker deployments or when running the server on a separate machine.
:::

## Docker

For a complete containerized setup with persistence:

```bash
docker compose up --build
```

This starts the MCP server in HTTP mode on port 3000. Configure environment variables in a `.env` file:

```env
OPENAI_API_URL=https://api.openai.com/v1
OPENAI_API_KEY=sk-...
EMBEDDING_MODEL=text-embedding-3-small
EMBEDDING_DIMS=1536
EXTRACTION_MODEL=gpt-4o-mini
STORAGE_BACKEND=rocksdb
```

Data persists in a Docker volume.

## Mock Mode vs LLM Mode

Context Keeper works in two modes:

**LLM Mode** — When you set `OPENAI_API_URL`, `OPENAI_API_KEY`, `EMBEDDING_MODEL`, and `EXTRACTION_MODEL`, the system uses real LLM calls for entity and relation extraction. This is the most accurate but requires an API key.

**Mock Mode** — When any of those environment variables are missing, Context Keeper falls back to heuristic extraction: capitalized words are treated as entities, and simple patterns are used to infer relations. This is perfect for testing and development without API costs.

:::tip
Start with mock mode to explore the API. Upgrade to LLM mode once you're ready for production accuracy.
:::

## Next Steps

- **Architecture** — Learn how the ingestion pipeline and hybrid search work: [How It Works](/docs/how-it-works)
- **MCP Tools** — Explore all 10 tools available via MCP: [MCP Reference](/docs/mcp-tools)
- **Configuration** — Advanced setup and environment variables: [Configuration](/docs/configuration)
