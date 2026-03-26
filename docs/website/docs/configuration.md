---
sidebar_position: 8
title: Configuration
description: Environment variables, CLI flags, and storage backends.
---

# Configuration

## Overview

Context Keeper is configured via CLI flags, environment variables, or a `.env` file. Environment variables take precedence over defaults, and CLI flags override environment variables. This flexibility enables Context Keeper to work in diverse environments—from local development to containerized deployments.

## LLM Settings

Configure the language models used for entity extraction, relation extraction, and embeddings.

| Setting | Env Var | CLI Flag | Default | Description |
|---------|---------|----------|---------|-------------|
| API Endpoint | `OPENAI_API_URL` | `--api-url, -u` | — | OpenAI-compatible API endpoint (e.g., `https://api.openai.com/v1`) |
| API Key | `OPENAI_API_KEY` | `--api-key, -k` | — | API key for authentication |
| Embedding Model | `EMBEDDING_MODEL` | `--embedding-model-name, -e` | — | Model name (e.g., `text-embedding-3-small`) |
| Embedding Dimensions | `EMBEDDING_DIMS` | `--embedding-dims, -d` | 1536 | Vector dimension size for embeddings |
| Extraction Model | `EXTRACTION_MODEL` | `--extraction-model-name, -x` | — | Model for entity and relation extraction (e.g., `gpt-4o-mini`) |

:::info

When all four LLM settings are configured (API URL, API key, embedding model, extraction model), Context Keeper uses real LLM-powered extraction. If any setting is omitted, the system falls back to mock heuristics for testing and development without requiring API keys.

:::

**Example:**

```bash
export OPENAI_API_URL=https://api.openai.com/v1
export OPENAI_API_KEY=sk-xxxxx
export EMBEDDING_MODEL=text-embedding-3-small
export EMBEDDING_DIMS=1536
export EXTRACTION_MODEL=gpt-4o-mini
```

## Storage Settings

Configure where and how Context Keeper persists data.

| Setting | Env Var | CLI Flag | Default | Description |
|---------|---------|----------|---------|-------------|
| Backend | `STORAGE_BACKEND` | `--storage` | `rocksdb:~/.context-keeper/data` | Storage backend (rocksdb, memory, remote) |
| DB File Path | `DB_FILE_PATH` | `--db-file-path, -f` | context.sql | Path to SurrealDB file (RocksDB only) |

### Backend Options

**`rocksdb:<path>`** (Default)
- Persistent key-value store using RocksDB
- Stores data at `~/.context-keeper/data` by default
- Fast, embeds vector indexing, survives process restarts
- Best for production and persistent workflows

**memory**
- Ephemeral in-memory storage
- Useful for testing, development, and CI/CD pipelines
- Exports data to disk on exit (optional)
- Resets on process restart

**`remote:<ws_url>`** (Work in Progress)
- Connect to a remote Context Keeper instance
- Enables distributed queries across a network
- Centralizes knowledge graph for multi-client collaboration
- Coming soon

## Transport Settings

Configure how the MCP server communicates with clients.

| Setting | Env Var | CLI Flag | Default | Description |
|---------|---------|----------|---------|-------------|
| Transport | `MCP_TRANSPORT` | `--transport` | stdio | Protocol (stdio or http) |
| HTTP Port | `MCP_HTTP_PORT` | `--http-port` | 3000 | Port for HTTP transport |

**Transports:**
- **stdio** — Standard input/output. Ideal for direct CLI integration and local testing.
- **http** — HTTP server. Supports remote clients and integration with web frameworks.

## Example .env File

Create a `.env` file in your project root for local development:

```bash
# LLM Configuration
OPENAI_API_URL=https://api.openai.com/v1
OPENAI_API_KEY=sk-xxxxx
EMBEDDING_MODEL=text-embedding-3-small
EMBEDDING_DIMS=1536
EXTRACTION_MODEL=gpt-4o-mini

# Storage Configuration
STORAGE_BACKEND=rocksdb:~/.context-keeper/data
DB_FILE_PATH=context.sql

# MCP Transport
MCP_TRANSPORT=http
MCP_HTTP_PORT=3000
```

## Docker Configuration

When running Context Keeper in Docker, environment variables are passed via `docker-compose.yml` or `docker run -e`.

### docker-compose.yml

```yaml
version: '3.8'
services:
  context-keeper:
    build: .
    ports:
      - "3000:3000"
    environment:
      OPENAI_API_URL: https://api.openai.com/v1
      OPENAI_API_KEY: ${OPENAI_API_KEY}
      EMBEDDING_MODEL: text-embedding-3-small
      EMBEDDING_DIMS: "1536"
      EXTRACTION_MODEL: gpt-4o-mini
      STORAGE_BACKEND: rocksdb:/data
      MCP_TRANSPORT: http
      MCP_HTTP_PORT: "3000"
    volumes:
      - context-keeper-data:/data
    environment_file: .env

volumes:
  context-keeper-data:
```

Start the service:

```bash
docker compose up --build
```

The MCP server will be available at `http://localhost:3000`.

## Development Without API Keys

For local development and testing, omit the LLM environment variables:

```bash
# This will use mock extractors and require no API key
cargo test
cargo run -p context-keeper-cli -- search --query "example"
```

All integration tests work with mock implementations, making development fast and cost-free.
