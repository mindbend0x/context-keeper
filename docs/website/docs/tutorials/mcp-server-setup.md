---
sidebar_position: 1
title: Adding Context Keeper to Your MCP Client
description: Step-by-step setup for Claude Code, Cursor, Claude Desktop, ChatGPT, Perplexity, and other MCP clients.
---

# Adding Context Keeper to Your MCP Client

This tutorial walks you through adding Context Keeper as an MCP server in each major client. By the end, your AI assistant will have persistent memory across conversations.

## Prerequisites

The easiest way to use the MCP server is via `npx`, which downloads the correct binary for your platform automatically. All you need is **Node.js 18+** installed.

Alternatively, you can build from source:

```bash
git clone https://github.com/mindbend0x/context-keeper.git
cd context-keeper
cargo build --release -p context-keeper-mcp
cp target/release/context-keeper-mcp ~/.cargo/bin/
```

Or download a pre-built binary from [GitHub Releases](https://github.com/mindbend0x/context-keeper/releases) and place it on your PATH.

:::tip
No API key is needed to get started. Context Keeper runs in mock mode by default, using heuristic extraction. Add LLM environment variables later for production-quality extraction.
:::

---

## Claude Code

Claude Code supports MCP servers via its settings file.

### Configuration

Add Context Keeper to your project-level or global settings:

**Project-level** (`.claude/settings.json` in your project root):

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

**Global** (`~/.claude/settings.json`):

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

### Verification

1. Restart Claude Code (or start a new session)
2. You should see Context Keeper tools listed when Claude describes available tools
3. Test by asking: *"Use add_memory to remember that Alice is an engineer at Acme Corp"*
4. Then: *"Search your memory for who works at Acme"*

---

## Cursor

Cursor has built-in MCP support in its settings.

### Configuration

1. Open Cursor Settings (`Cmd/Ctrl + ,`)
2. Search for "MCP" or navigate to **Features > MCP Servers**
3. Click **Add MCP Server** and add this configuration:

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

Alternatively, add it to your project's `.cursor/mcp.json`:

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

### Verification

1. Restart Cursor
2. Open the MCP tools panel — Context Keeper tools should appear
3. In a chat, try: *"Add a memory: The auth service uses JWT tokens with 24h expiry"*
4. Then verify: *"What do you know about the auth service?"*

---

## Claude Desktop

### Configuration

Edit the MCP configuration file for your platform:

| Platform | Config Path |
|----------|------------|
| macOS | `~/Library/Application Support/Claude/claude_desktop_config.json` |
| Windows | `%APPDATA%\Claude\claude_desktop_config.json` |
| Linux | `~/.config/Claude/claude_desktop_config.json` |

Add Context Keeper to the `mcpServers` section:

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

:::info
If the file doesn't exist, create it with the JSON above. Make sure the directory exists first.
:::

### With environment variables

To use LLM-powered extraction, pass environment variables:

```json
{
  "mcpServers": {
    "context-keeper": {
      "command": "npx",
      "args": ["context-keeper-mcp"],
      "env": {
        "OPENAI_API_URL": "https://api.openai.com/v1",
        "OPENAI_API_KEY": "sk-xxxxx",
        "EMBEDDING_MODEL": "text-embedding-3-small",
        "EMBEDDING_DIMS": "1536",
        "EXTRACTION_MODEL": "gpt-4o-mini"
      }
    }
  }
}
```

### Verification

1. Fully quit and reopen Claude Desktop
2. Look for the hammer icon in the input area — it should list Context Keeper tools
3. Try: *"Remember this: our API rate limit is 100 requests per minute per user"*
4. In a new conversation: *"What's our API rate limit?"*

---

## ChatGPT

ChatGPT requires an HTTP-accessible MCP server. You'll need to run Context Keeper in HTTP transport mode.

### Start the HTTP server

```bash
MCP_TRANSPORT=http MCP_HTTP_PORT=3000 npx context-keeper-mcp
```

Or use Docker for a persistent setup:

```bash
docker compose up -d
```

See the [Running with Docker](/docs/tutorials/running-with-docker) tutorial for details.

### Configuration

1. In ChatGPT, go to **Settings > Connected Apps** (or the MCP integration settings)
2. Add a new MCP server with the endpoint: `http://localhost:3000/mcp`
3. If running remotely, use the public URL of your server

:::caution
ChatGPT's MCP support requires the server to be accessible over HTTP. stdio transport is not supported. For production use, deploy behind a reverse proxy with TLS. See [HTTP Transport](/docs/tutorials/http-transport) for security guidance.
:::

### Verification

1. Start a new conversation in ChatGPT
2. Context Keeper tools should appear in the tools menu
3. Test with add_memory and search_memory as described above

---

## Perplexity

Perplexity supports MCP servers for enhanced research workflows.

### Configuration

1. Open Perplexity settings
2. Navigate to the MCP server configuration
3. Add Context Keeper:

**For stdio transport** (local):

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

**For HTTP transport** (remote/Docker):

Point to your running HTTP server endpoint: `http://localhost:3000/mcp`

### Verification

1. Restart Perplexity
2. Context Keeper tools should appear in the available tools
3. Try storing research context and recalling it in later sessions

---

## Troubleshooting

### Binary not found

If you're using `npx` and see an error about the platform package not being found, try:

```bash
npx context-keeper-mcp --help
```

If that fails, ensure Node.js 18+ is installed: `node --version`.

If you installed from source and see `command not found: context-keeper-mcp`:

1. Check that Cargo's bin directory is in your PATH:
   ```bash
   echo $PATH | tr ':' '\n' | grep cargo
   # Should include ~/.cargo/bin
   ```
2. Add it if missing:
   ```bash
   export PATH="$HOME/.cargo/bin:$PATH"
   ```

### No tools appearing

1. Restart the client completely (not just reload)
2. Check the config file for JSON syntax errors
3. Test the server directly: `npx context-keeper-mcp --help` should print usage
4. Check client logs for MCP connection errors

### Mock mode vs LLM mode

- **No API key set**: Context Keeper uses heuristic extraction (capitalized words become entities, simple patterns become relations). Good for testing.
- **API keys configured**: Real LLM extraction with high accuracy. Set `OPENAI_API_URL`, `OPENAI_API_KEY`, `EMBEDDING_MODEL`, and `EXTRACTION_MODEL`.

See [Configuration](/docs/configuration) for the full environment variable reference.

### Data location

By default, data is stored at `~/.context-keeper/data` using RocksDB. To reset:

```bash
rm -rf ~/.context-keeper/data
```

### Connection issues with HTTP transport

- Ensure the server is running: `curl http://localhost:3000/mcp`
- Check firewall rules if connecting remotely
- See the [HTTP Transport](/docs/tutorials/http-transport) tutorial for detailed troubleshooting
