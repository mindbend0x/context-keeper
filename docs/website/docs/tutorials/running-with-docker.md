---
sidebar_position: 4
title: Running with Docker
description: Deploy Context Keeper with Docker for persistent, containerized memory.
---

# Running with Docker

Docker is the easiest way to run Context Keeper as a persistent HTTP server. This tutorial covers the full setup from quick start to production configuration.

## Quick start

```bash
git clone https://github.com/0x313/context-keeper.git
cd context-keeper
```

Create a `.env` file:

```bash
# Minimum viable .env for Docker
MCP_AUTH_TOKENS=my-secret-token
```

Start the service:

```bash
docker compose up --build
```

Context Keeper is now running at `http://localhost:3000` in HTTP transport mode.

:::info
The Docker setup always uses HTTP transport. For stdio-based clients (Claude Desktop, Cursor, Claude Code), install the binary locally instead — see [MCP Server Setup](/docs/tutorials/mcp-server-setup).
:::

## docker-compose.yml walkthrough

The project includes a production-ready `docker-compose.yml`:

```yaml
services:
  context-keeper-mcp:
    build:
      context: .
      dockerfile: Dockerfile
    ports:
      - "3000:3000"
    environment:
      # Storage — embedded RocksDB with persistent volume
      STORAGE_BACKEND: "rocksdb:/data"
      # MCP transport
      MCP_TRANSPORT: "http"
      MCP_HTTP_PORT: "3000"
      MCP_HTTP_HOST: "0.0.0.0"
      # Auth — bearer tokens for API access
      MCP_AUTH_TOKENS: "${MCP_AUTH_TOKENS}"
      # LLM configuration (from .env file)
      OPENAI_API_URL: "${OPENAI_API_URL:-}"
      OPENAI_API_KEY: "${OPENAI_API_KEY:-}"
      EMBEDDING_MODEL: "${EMBEDDING_MODEL:-text-embedding-3-small}"
      EMBEDDING_DIMS: "${EMBEDDING_DIMS:-1536}"
      EXTRACTION_MODEL: "${EXTRACTION_MODEL:-gpt-4o-mini}"
    volumes:
      - surrealdb-data:/data
    restart: unless-stopped

volumes:
  surrealdb-data:
```

### Key points

- **Storage**: RocksDB data is stored in a Docker volume (`surrealdb-data`), persisting across container restarts
- **Transport**: Always HTTP in Docker (stdio doesn't work across container boundaries)
- **Auth**: `MCP_AUTH_TOKENS` is required — set it to a comma-separated list of bearer tokens
- **LLM config**: Optional — falls back to mock mode if not set
- **Restart policy**: `unless-stopped` keeps the server running after host reboots

## Environment configuration

### Full .env file for Docker

```bash
# Authentication (required)
MCP_AUTH_TOKENS=token1,token2

# LLM Configuration (optional — omit for mock mode)
OPENAI_API_URL=https://api.openai.com/v1
OPENAI_API_KEY=sk-xxxxx
EMBEDDING_MODEL=text-embedding-3-small
EMBEDDING_DIMS=1536
EXTRACTION_MODEL=gpt-4o-mini
```

### Mock mode

To run without an API key, just set the auth token and leave LLM variables empty:

```bash
MCP_AUTH_TOKENS=dev-token
```

Mock mode uses heuristic extraction — good for testing and development.

## Volume management

### Inspect data

```bash
docker volume inspect context-keeper_surrealdb-data
```

### Backup data

```bash
docker run --rm -v context-keeper_surrealdb-data:/data -v $(pwd):/backup \
  alpine tar czf /backup/context-keeper-backup.tar.gz /data
```

### Restore data

```bash
docker run --rm -v context-keeper_surrealdb-data:/data -v $(pwd):/backup \
  alpine tar xzf /backup/context-keeper-backup.tar.gz -C /
```

### Reset data

```bash
docker compose down -v   # Removes containers AND volumes
docker compose up --build
```

## Connecting MCP clients

Once the Docker server is running, configure your MCP client to use the HTTP endpoint.

### ChatGPT

Point to `http://localhost:3000/mcp` in the MCP server settings.

### Claude Desktop (remote)

You can use an HTTP-based MCP config for Claude Desktop if the server runs on a different machine:

```json
{
  "mcpServers": {
    "context-keeper": {
      "url": "http://your-server:3000/mcp",
      "headers": {
        "Authorization": "Bearer your-token"
      }
    }
  }
}
```

### Any HTTP MCP client

The endpoint is: `http://localhost:3000/mcp`

Include the auth header: `Authorization: Bearer <your-token>`

## Health check

Verify the server is running:

```bash
curl http://localhost:3000/mcp
```

## Viewing logs

```bash
docker compose logs -f context-keeper-mcp
```

## Production considerations

### Resource limits

Add resource constraints in `docker-compose.yml`:

```yaml
services:
  context-keeper-mcp:
    deploy:
      resources:
        limits:
          memory: 512M
          cpus: '1.0'
```

### Reverse proxy with TLS

For production, run behind nginx or Caddy with TLS:

```nginx
server {
    listen 443 ssl;
    server_name memory.yourdomain.com;

    ssl_certificate /etc/letsencrypt/live/memory.yourdomain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/memory.yourdomain.com/privkey.pem;

    location / {
        proxy_pass http://localhost:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

### Logging

Docker logs are available via `docker compose logs`. For structured logging, check the container's stdout output.

---

## Next steps

- [HTTP Transport](/docs/tutorials/http-transport) — Multi-agent setups and security
- [MCP Server Setup](/docs/tutorials/mcp-server-setup) — Connect your AI client
- [Configuration](/docs/configuration) — Full environment variable reference
