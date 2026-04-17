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

- **Node.js 18+** — Required for `npx` to install the MCP server ([download](https://nodejs.org/))
- **Optional: Rust toolchain** — Only needed to build from source ([rustup](https://rustup.rs/))
- **Optional: Docker** — For containerized setup
- **Optional: OpenAI-compatible API key** — Only needed for LLM-powered extraction. Mock mode works without it.

## Installation

### CLI via Homebrew (macOS / Linux)

The fastest way to install the CLI:

```bash
brew install mindbend0x/context-keeper/context-keeper
```

### MCP Server via npx

The MCP server requires no separate install — use `npx` directly in your client config (see [MCP Server Setup](#mcp-server-setup) below). This downloads the correct binary for your platform automatically.

### From Source

Clone the repository and build:

```bash
git clone https://github.com/mindbend0x/context-keeper.git
cd context-keeper
cargo build --release
```

Binaries will be in `target/release/`:

| Binary | Description |
|--------|-------------|
| `context-keeper` | CLI tool |
| `context-keeper-mcp` | MCP server (stdio or HTTP) |

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

Context Keeper works with any MCP-compatible client. Here's a quick setup for each:

### Claude Code

Add to `.claude/settings.json` in your project root (or `~/.claude/settings.json` for global):

```json
{
  "mcpServers": {
    "context-keeper": {
      "command": "npx",
      "args": ["context-keeper-mcp"]
    }
  }
}
```

### Claude Desktop

Edit the config file for your platform:

| Platform | Path |
|----------|------|
| macOS | `~/Library/Application Support/Claude/claude_desktop_config.json` |
| Windows | `%APPDATA%\Claude\claude_desktop_config.json` |
| Linux | `~/.config/Claude/claude_desktop_config.json` |

```json
{
  "mcpServers": {
    "context-keeper": {
      "command": "npx",
      "args": ["context-keeper-mcp"]
    }
  }
}
```

Restart Claude Desktop, and Context Keeper tools will appear in the hammer icon.

### Cursor

Add to your Cursor MCP settings or `.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "context-keeper": {
      "command": "npx",
      "args": ["context-keeper-mcp"]
    }
  }
}
```

### ChatGPT & Perplexity

These clients require HTTP transport. Start the server first:

```bash
MCP_TRANSPORT=http MCP_HTTP_PORT=3000 npx context-keeper-mcp
```

Then point the client to `http://localhost:3000/mcp`.

### HTTP Transport

For remote, containerized, or multi-agent setups, run in HTTP mode:

```bash
MCP_TRANSPORT=http MCP_HTTP_PORT=3000 npx context-keeper-mcp
```

:::info
See the [detailed MCP server setup tutorial](/docs/tutorials/mcp-server-setup) for platform-specific paths, verification steps, and troubleshooting.
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

- **Tutorials** — Step-by-step guides for each integration path: [MCP Server Setup](/docs/tutorials/mcp-server-setup) · [CLI Installation](/docs/tutorials/cli-installation) · [Docker](/docs/tutorials/running-with-docker)
- **How It Works** — Understand the ingestion pipeline and hybrid search: [How It Works](/docs/how-it-works)
- **MCP Tools** — Explore all 10 tools available via MCP: [MCP Reference](/docs/mcp-tools)
- **Configuration** — Advanced setup and environment variables: [Configuration](/docs/configuration)
