#!/usr/bin/env bash
#
# Context Keeper — Claude Desktop Plugin Installer
#
# Detects or builds the context-keeper-mcp binary and registers it
# as an MCP server in Claude Desktop's configuration.
#
set -euo pipefail

# ── Colours ──────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Colour

info()  { echo -e "${CYAN}[info]${NC}  $*"; }
ok()    { echo -e "${GREEN}[ok]${NC}    $*"; }
warn()  { echo -e "${YELLOW}[warn]${NC}  $*"; }
err()   { echo -e "${RED}[error]${NC} $*" >&2; }

# ── Defaults ─────────────────────────────────────────────────────────────
TRANSPORT="stdio"           # stdio | http | both
HTTP_PORT=3000
STORAGE="memory"            # memory | rocksdb:<path>
DB_FILE_PATH="context.sql"
BINARY_PATH=""
OPENAI_API_URL=""
OPENAI_API_KEY=""
EMBEDDING_MODEL="text-embedding-3-small"
EMBEDDING_DIMS=1536
EXTRACTION_MODEL="gpt-4o-mini"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# ── Parse flags ──────────────────────────────────────────────────────────
usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Options:
  --transport <stdio|http|both>   MCP transport mode (default: stdio)
  --http-port <PORT>              HTTP port when using http transport (default: 3000)
  --binary <PATH>                 Path to pre-built context-keeper-mcp binary
  --storage <BACKEND>             Storage backend: memory, rocksdb:<path> (default: memory)
  --db-file <PATH>                Path to context.sql file (default: context.sql)
  --api-url <URL>                 OpenAI-compatible API base URL
  --api-key <KEY>                 API key for LLM services
  --embedding-model <MODEL>       Embedding model name (default: text-embedding-3-small)
  --embedding-dims <DIMS>         Embedding dimensions (default: 1536)
  --extraction-model <MODEL>      Extraction model name (default: gpt-4o-mini)
  --uninstall                     Remove Context Keeper from Claude Desktop config
  -h, --help                      Show this help message
EOF
    exit 0
}

UNINSTALL=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --transport)       TRANSPORT="$2";          shift 2 ;;
        --http-port)       HTTP_PORT="$2";           shift 2 ;;
        --binary)          BINARY_PATH="$2";         shift 2 ;;
        --storage)         STORAGE="$2";             shift 2 ;;
        --db-file)         DB_FILE_PATH="$2";        shift 2 ;;
        --api-url)         OPENAI_API_URL="$2";      shift 2 ;;
        --api-key)         OPENAI_API_KEY="$2";      shift 2 ;;
        --embedding-model) EMBEDDING_MODEL="$2";     shift 2 ;;
        --embedding-dims)  EMBEDDING_DIMS="$2";      shift 2 ;;
        --extraction-model) EXTRACTION_MODEL="$2";   shift 2 ;;
        --uninstall)       UNINSTALL=true;           shift ;;
        -h|--help)         usage ;;
        *) err "Unknown option: $1"; usage ;;
    esac
done

# ── Detect Claude Desktop config path ────────────────────────────────────
detect_config_path() {
    local candidates=()

    case "$(uname -s)" in
        Darwin)
            candidates+=("$HOME/Library/Application Support/Claude/claude_desktop_config.json")
            ;;
        Linux)
            candidates+=("${XDG_CONFIG_HOME:-$HOME/.config}/Claude/claude_desktop_config.json")
            candidates+=("$HOME/.config/Claude/claude_desktop_config.json")
            ;;
        MINGW*|MSYS*|CYGWIN*)
            if [[ -n "${APPDATA:-}" ]]; then
                candidates+=("$APPDATA/Claude/claude_desktop_config.json")
            fi
            ;;
    esac

    for candidate in "${candidates[@]}"; do
        if [[ -f "$candidate" ]]; then
            echo "$candidate"
            return
        fi
    done

    # Return the platform-appropriate default even if file doesn't exist yet
    if [[ ${#candidates[@]} -gt 0 ]]; then
        echo "${candidates[0]}"
    else
        err "Cannot determine Claude Desktop config path for this platform."
        exit 1
    fi
}

CONFIG_PATH="$(detect_config_path)"
CONFIG_DIR="$(dirname "$CONFIG_PATH")"

# ── Uninstall ────────────────────────────────────────────────────────────
if $UNINSTALL; then
    info "Removing Context Keeper from Claude Desktop config..."
    if [[ ! -f "$CONFIG_PATH" ]]; then
        warn "Config file not found at $CONFIG_PATH — nothing to do."
        exit 0
    fi

    # Remove the "context-keeper" key from mcpServers using jq or python
    if command -v jq &>/dev/null; then
        tmp=$(mktemp)
        jq 'del(.mcpServers["context-keeper"])' "$CONFIG_PATH" > "$tmp" && mv "$tmp" "$CONFIG_PATH"
    elif command -v python3 &>/dev/null; then
        python3 -c "
import json, sys
with open('$CONFIG_PATH') as f:
    cfg = json.load(f)
cfg.get('mcpServers', {}).pop('context-keeper', None)
with open('$CONFIG_PATH', 'w') as f:
    json.dump(cfg, f, indent=2)
"
    else
        err "Need jq or python3 to modify config. Please remove 'context-keeper' from mcpServers manually."
        exit 1
    fi

    ok "Context Keeper removed from Claude Desktop config."
    info "Restart Claude Desktop to apply changes."
    exit 0
fi

# ── Locate or build binary ───────────────────────────────────────────────
find_binary() {
    # 1. Explicit path
    if [[ -n "$BINARY_PATH" ]]; then
        if [[ -x "$BINARY_PATH" ]]; then
            echo "$BINARY_PATH"
            return
        else
            err "Specified binary not found or not executable: $BINARY_PATH"
            exit 1
        fi
    fi

    # 2. Check PATH
    if command -v context-keeper-mcp &>/dev/null; then
        echo "$(command -v context-keeper-mcp)"
        return
    fi

    # 3. Check cargo target directory
    local release_bin="$REPO_ROOT/target/release/context-keeper-mcp"
    local debug_bin="$REPO_ROOT/target/debug/context-keeper-mcp"
    if [[ -x "$release_bin" ]]; then
        echo "$release_bin"
        return
    elif [[ -x "$debug_bin" ]]; then
        warn "Found debug build — consider building with --release for better performance."
        echo "$debug_bin"
        return
    fi

    # 4. Not found — offer to build
    return 1
}

BINARY=""
if BINARY="$(find_binary)"; then
    ok "Found binary: $BINARY"
else
    info "context-keeper-mcp binary not found."

    if ! command -v cargo &>/dev/null; then
        err "Rust toolchain not found. Please either:"
        err "  1. Install Rust: https://rustup.rs"
        err "  2. Build the binary manually and pass --binary <path>"
        exit 1
    fi

    info "Building context-keeper-mcp from source (this may take a few minutes)..."
    (cd "$REPO_ROOT" && cargo build --release -p context-keeper-mcp)
    BINARY="$REPO_ROOT/target/release/context-keeper-mcp"

    if [[ ! -x "$BINARY" ]]; then
        err "Build completed but binary not found at $BINARY"
        exit 1
    fi
    ok "Built successfully: $BINARY"
fi

# ── Build environment variables ──────────────────────────────────────────
build_env() {
    local env_obj="{"
    local first=true

    add_env() {
        local key="$1" val="$2"
        if [[ -n "$val" ]]; then
            if ! $first; then env_obj+=","; fi
            env_obj+="\"$key\":\"$val\""
            first=false
        fi
    }

    add_env "STORAGE_BACKEND" "$STORAGE"
    add_env "DB_FILE_PATH"    "$DB_FILE_PATH"

    if [[ -n "$OPENAI_API_URL" ]]; then
        add_env "OPENAI_API_URL"   "$OPENAI_API_URL"
        add_env "OPENAI_API_KEY"   "$OPENAI_API_KEY"
        add_env "EMBEDDING_MODEL"  "$EMBEDDING_MODEL"
        add_env "EMBEDDING_DIMS"   "$EMBEDDING_DIMS"
        add_env "EXTRACTION_MODEL" "$EXTRACTION_MODEL"
    fi

    env_obj+="}"
    echo "$env_obj"
}

# ── Build MCP server config entries ──────────────────────────────────────
build_stdio_config() {
    local env_json
    env_json="$(build_env)"
    cat <<EOF
{
  "command": "$BINARY",
  "args": ["--transport", "stdio"],
  "env": $env_json
}
EOF
}

build_http_config() {
    cat <<EOF
{
  "url": "http://localhost:${HTTP_PORT}/mcp"
}
EOF
}

# ── Write Claude Desktop config ──────────────────────────────────────────
write_config() {
    mkdir -p "$CONFIG_DIR"

    # Start with existing config or empty object
    local existing="{}"
    if [[ -f "$CONFIG_PATH" ]]; then
        existing="$(cat "$CONFIG_PATH")"
    fi

    local server_config=""

    case "$TRANSPORT" in
        stdio)
            server_config="$(build_stdio_config)"
            ;;
        http)
            server_config="$(build_http_config)"
            ;;
        both)
            # For "both", register stdio as primary (Claude Desktop uses stdio natively)
            # and add a comment-like note about HTTP availability
            server_config="$(build_stdio_config)"
            ;;
        *)
            err "Unknown transport: $TRANSPORT"
            exit 1
            ;;
    esac

    # Merge into existing config using jq or python3
    if command -v jq &>/dev/null; then
        echo "$existing" | jq --argjson server "$server_config" \
            '.mcpServers["context-keeper"] = $server' > "$CONFIG_PATH"
    elif command -v python3 &>/dev/null; then
        python3 -c "
import json, sys
existing = json.loads('''$existing''')
server = json.loads('''$server_config''')
existing.setdefault('mcpServers', {})['context-keeper'] = server
with open('$CONFIG_PATH', 'w') as f:
    json.dump(existing, f, indent=2)
"
    else
        err "Need jq or python3 to write config."
        exit 1
    fi
}

write_config

ok "Claude Desktop config updated at:"
info "  $CONFIG_PATH"

# ── Print summary ────────────────────────────────────────────────────────
echo ""
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}  Context Keeper plugin installed for Claude Desktop${NC}"
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""
echo -e "  Transport:  ${CYAN}${TRANSPORT}${NC}"
echo -e "  Binary:     ${CYAN}${BINARY}${NC}"
echo -e "  Storage:    ${CYAN}${STORAGE}${NC}"

if [[ -n "$OPENAI_API_URL" ]]; then
    echo -e "  LLM:        ${CYAN}${EXTRACTION_MODEL}${NC} via ${CYAN}${OPENAI_API_URL}${NC}"
else
    echo -e "  LLM:        ${YELLOW}mock (set --api-url and --api-key for real extraction)${NC}"
fi

if [[ "$TRANSPORT" == "both" || "$TRANSPORT" == "http" ]]; then
    echo -e "  HTTP URL:   ${CYAN}http://localhost:${HTTP_PORT}/mcp${NC}"
fi

echo ""
echo -e "  ${YELLOW}Restart Claude Desktop to activate the plugin.${NC}"
echo ""

if [[ "$TRANSPORT" == "both" ]]; then
    info "To also run the HTTP server (for other clients):"
    info "  $BINARY --transport http --http-port $HTTP_PORT"
    echo ""
fi
