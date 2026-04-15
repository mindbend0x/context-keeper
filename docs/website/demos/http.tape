# Context Keeper HTTP Multi-Agent Demo
# Generates: static/demos/http-multi-agent.gif
# Run: vhs demos/http.tape
# Requires: context-keeper-mcp running with MCP_TRANSPORT=http MCP_HTTP_PORT=3000

Output docs/website/static/demos/http-multi-agent.gif
Output docs/website/static/demos/http-multi-agent.mp4

Set Shell "zsh"
Set FontFamily "JetBrains Mono"
Set FontSize 14
Set Width 1000
Set Height 600
Set Padding 24
Set Theme { "name": "ContextKeeper", "black": "#0a0a0b", "red": "#ea580c", "green": "#4ade80", "yellow": "#eab308", "blue": "#3b82f6", "magenta": "#a78bfa", "cyan": "#22d3ee", "white": "#fafafa", "brightBlack": "#71717a", "brightRed": "#fb923c", "brightGreen": "#86efac", "brightYellow": "#fde047", "brightBlue": "#60a5fa", "brightMagenta": "#c4b5fd", "brightCyan": "#67e8f9", "brightWhite": "#ffffff", "background": "#0a0a0b", "foreground": "#fafafa", "cursor": "#fb923c", "selectionBackground": "#27272a" }
Set TypingSpeed 30ms
Set WindowBar Colorful
Set WindowBarSize 28

Sleep 500ms

# Agent 1 writes a memory
Type "# Agent 1: store context about a project decision"
Enter
Sleep 300ms

Type @"curl -s http://localhost:3000/mcp \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"add_memory","arguments":{"text":"The team decided to migrate from PostgreSQL to SurrealDB for the knowledge graph. Migration starts Q2 2026.","source":"architecture-review","namespace":"engineering","agent_id":"agent-alpha"}}}' | jq ."@
Sleep 300ms
Enter
Sleep 4s

# Agent 2 reads the memory
Type "# Agent 2: search for what Agent 1 stored"
Enter
Sleep 300ms

Type @"curl -s http://localhost:3000/mcp \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"search_memory","arguments":{"query":"database migration plans","namespace":"engineering","agent_id":"agent-beta"}}}' | jq ."@
Sleep 300ms
Enter
Sleep 4s

Sleep 2s
