#!/usr/bin/env bash
#
# Context Keeper — MCP Easy Config
#
# Generates a working MCP configuration for any supported client
# (Claude Desktop, Claude Code, Cursor) with API keys and custom URLs.
#
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

info()  { echo -e "${CYAN}[info]${NC}  $*"; }
ok()    { echo -e "${GREEN}[ok]${NC}    $*"; }
warn()  { echo -e "${YELLOW}[warn]${NC}  $*"; }
err()   { echo -e "${RED}[error]${NC} $*" >&2; }

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# ── Defaults ──────────────────────────────────────────────────────────
TARGET="claude-desktop"
API_URL=""
API_KEY=""
STORAGE="memory"
DB_FILE="context.sql"
BINARY=""
EMBEDDING_MODEL="text-embedding-3-small"
EXTRACTION_MODEL="gpt-4o-mini"
DRY_RUN=false

usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Generate an MCP configuration for Context Keeper.

Options:
  --target <client>         Target client: claude-desktop, claude-code, cursor (default: claude-desktop)
  --api-url <URL>           OpenAI-compatible API URL
  --api-key <KEY>           API key for LLM services
  --storage <BACKEND>       Storage backend: memory, rocksdb:<path> (default: memory)
  --db-file <PATH>          Persistence file path (default: context.sql)
  --binary <PATH>           Path to context-keeper-mcp binary (auto-detected if omitted)
  --embedding-model <MODEL> Embedding model (default: text-embedding-3-small)
  --extraction-model <MODEL> Extraction model (default: gpt-4o-mini)
  --dry-run                 Print config to stdout instead of writing to file
  -h, --help                Show this help message

Examples:
  # Claude Desktop with LLM extraction
  $(basename "$0") --api-url https://api.openai.com/v1 --api-key sk-...

  # Cursor with custom storage
  $(basename "$0") --target cursor --storage rocksdb:./my-data

  # Claude Code project-local config
  $(basename "$0") --target claude-code --dry-run
EOF
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --target)           TARGET="$2";            shift 2 ;;
        --api-url)          API_URL="$2";            shift 2 ;;
        --api-key)          API_KEY="$2";            shift 2 ;;
        --storage)          STORAGE="$2";            shift 2 ;;
        --db-file)          DB_FILE="$2";            shift 2 ;;
        --binary)           BINARY="$2";             shift 2 ;;
        --embedding-model)  EMBEDDING_MODEL="$2";    shift 2 ;;
        --extraction-model) EXTRACTION_MODEL="$2";   shift 2 ;;
        --dry-run)          DRY_RUN=true;            shift ;;
        -h|--help)          usage ;;
        *) err "Unknown option: $1"; usage ;;
    esac
done

# ── Detect binary ─────────────────────────────────────────────────────
USE_NPX=false
if [[ -z "$BINARY" ]]; then
    if command -v context-keeper-mcp &>/dev/null; then
        BINARY="$(command -v context-keeper-mcp)"
    elif [[ -x "$REPO_ROOT/target/release/context-keeper-mcp" ]]; then
        BINARY="$REPO_ROOT/target/release/context-keeper-mcp"
    elif [[ -x "$REPO_ROOT/target/debug/context-keeper-mcp" ]]; then
        BINARY="$REPO_ROOT/target/debug/context-keeper-mcp"
    elif command -v npx &>/dev/null; then
        USE_NPX=true
        info "Binary not found on PATH; using npx to run context-keeper-mcp"
    else
        BINARY="context-keeper-mcp"
        warn "Binary not found and npx unavailable; using 'context-keeper-mcp' (must be in PATH)"
    fi
fi

# ── Build config JSON ─────────────────────────────────────────────────
build_config() {
    local env_parts=()
    env_parts+=("\"STORAGE_BACKEND\": \"$STORAGE\"")
    env_parts+=("\"DB_FILE_PATH\": \"$DB_FILE\"")

    if [[ -n "$API_URL" ]]; then
        env_parts+=("\"OPENAI_API_URL\": \"$API_URL\"")
    fi
    if [[ -n "$API_KEY" ]]; then
        env_parts+=("\"OPENAI_API_KEY\": \"$API_KEY\"")
    fi
    if [[ -n "$API_URL" || -n "$API_KEY" ]]; then
        env_parts+=("\"EMBEDDING_MODEL\": \"$EMBEDDING_MODEL\"")
        env_parts+=("\"EXTRACTION_MODEL\": \"$EXTRACTION_MODEL\"")
    fi

    local env_json
    env_json=$(printf '%s' "${env_parts[0]}")
    for part in "${env_parts[@]:1}"; do
        env_json="$env_json, $part"
    done

    if $USE_NPX; then
        cat <<EOF
{
  "mcpServers": {
    "context-keeper": {
      "command": "npx",
      "args": ["context-keeper-mcp", "--transport", "stdio"],
      "env": { $env_json }
    }
  }
}
EOF
    else
        cat <<EOF
{
  "mcpServers": {
    "context-keeper": {
      "command": "$BINARY",
      "args": ["--transport", "stdio"],
      "env": { $env_json }
    }
  }
}
EOF
    fi
}

# ── Target path ───────────────────────────────────────────────────────
resolve_target_path() {
    case "$TARGET" in
        claude-desktop)
            case "$(uname -s)" in
                Darwin)  echo "$HOME/Library/Application Support/Claude/claude_desktop_config.json" ;;
                Linux)   echo "${XDG_CONFIG_HOME:-$HOME/.config}/Claude/claude_desktop_config.json" ;;
                MINGW*|MSYS*|CYGWIN*) echo "$APPDATA/Claude/claude_desktop_config.json" ;;
                *) err "Unsupported platform"; exit 1 ;;
            esac
            ;;
        claude-code)
            echo "$(pwd)/.mcp.json"
            ;;
        cursor)
            echo "$(pwd)/.cursor/mcp.json"
            ;;
        *)
            err "Unknown target: $TARGET (use claude-desktop, claude-code, or cursor)"
            exit 1
            ;;
    esac
}

CONFIG_JSON="$(build_config)"

if $DRY_RUN; then
    echo "$CONFIG_JSON"
    exit 0
fi

TARGET_PATH="$(resolve_target_path)"
TARGET_DIR="$(dirname "$TARGET_PATH")"
mkdir -p "$TARGET_DIR"

# ── Merge or write ────────────────────────────────────────────────────
if [[ -f "$TARGET_PATH" ]]; then
    if command -v jq &>/dev/null; then
        info "Merging into existing config at $TARGET_PATH"
        server_json="$(echo "$CONFIG_JSON" | jq '.mcpServers["context-keeper"]')"
        jq --argjson server "$server_json" \
            '.mcpServers["context-keeper"] = $server' "$TARGET_PATH" > "$TARGET_PATH.tmp" \
            && mv "$TARGET_PATH.tmp" "$TARGET_PATH"
    else
        warn "jq not found — overwriting $TARGET_PATH (install jq for merge support)"
        echo "$CONFIG_JSON" > "$TARGET_PATH"
    fi
else
    echo "$CONFIG_JSON" > "$TARGET_PATH"
fi

ok "Config written to $TARGET_PATH"

echo ""
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}  Context Keeper MCP configured${NC}"
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""
echo -e "  Target:   ${CYAN}${TARGET}${NC}"
if $USE_NPX; then
    echo -e "  Command:  ${CYAN}npx context-keeper-mcp${NC}"
else
    echo -e "  Binary:   ${CYAN}${BINARY}${NC}"
fi
echo -e "  Storage:  ${CYAN}${STORAGE}${NC}"
echo -e "  Config:   ${CYAN}${TARGET_PATH}${NC}"
if [[ -n "$API_URL" ]]; then
    echo -e "  LLM:      ${CYAN}${EXTRACTION_MODEL} via ${API_URL}${NC}"
else
    echo -e "  LLM:      ${YELLOW}mock mode (add --api-url and --api-key for real extraction)${NC}"
fi
echo ""
echo -e "  ${YELLOW}Restart your client to activate.${NC}"
echo ""
