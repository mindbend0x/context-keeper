#!/usr/bin/env bash
#
# Context Keeper — Claude Code Plugin Installer
#
# Builds the MCP server binary and sets up the .mcp.json configuration
# for Claude Code. Supports both local project and global installation.
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
PLUGIN_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="$(cd "$PLUGIN_DIR/../.." && pwd)"

GLOBAL=false
API_URL=""
API_KEY=""

usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Install Context Keeper as a Claude Code MCP plugin.

Options:
  --global                Install globally to ~/.claude/.mcp.json
  --api-url <URL>         OpenAI-compatible API URL (enables LLM extraction)
  --api-key <KEY>         API key for LLM services
  -h, --help              Show this help message

Without --global, copies .mcp.json to the current project directory.
EOF
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --global)   GLOBAL=true;         shift ;;
        --api-url)  API_URL="$2";        shift 2 ;;
        --api-key)  API_KEY="$2";        shift 2 ;;
        -h|--help)  usage ;;
        *) err "Unknown option: $1"; usage ;;
    esac
done

# ── Build binary ──────────────────────────────────────────────────────
BINARY=""
if command -v context-keeper-mcp &>/dev/null; then
    BINARY="$(command -v context-keeper-mcp)"
    ok "Found context-keeper-mcp in PATH: $BINARY"
elif [[ -x "$REPO_ROOT/target/release/context-keeper-mcp" ]]; then
    BINARY="$REPO_ROOT/target/release/context-keeper-mcp"
    ok "Found release build: $BINARY"
else
    if ! command -v cargo &>/dev/null; then
        err "Rust toolchain not found. Install from https://rustup.rs"
        exit 1
    fi
    info "Building context-keeper-mcp..."
    (cd "$REPO_ROOT" && cargo build --release -p context-keeper-mcp)
    BINARY="$REPO_ROOT/target/release/context-keeper-mcp"
    ok "Built: $BINARY"
fi

# ── Generate .mcp.json ────────────────────────────────────────────────
generate_config() {
    local env_block='"STORAGE_BACKEND": "memory", "DB_FILE_PATH": "context.sql"'
    if [[ -n "$API_URL" && -n "$API_KEY" ]]; then
        env_block="$env_block, \"OPENAI_API_URL\": \"$API_URL\", \"OPENAI_API_KEY\": \"$API_KEY\", \"EMBEDDING_MODEL\": \"text-embedding-3-small\", \"EXTRACTION_MODEL\": \"gpt-4o-mini\""
    fi

    cat <<EOF
{
  "mcpServers": {
    "context-keeper": {
      "command": "$BINARY",
      "args": ["--transport", "stdio"],
      "env": { $env_block }
    }
  }
}
EOF
}

if $GLOBAL; then
    TARGET_DIR="$HOME/.claude"
    mkdir -p "$TARGET_DIR"
    TARGET="$TARGET_DIR/.mcp.json"
else
    TARGET="$(pwd)/.mcp.json"
fi

if [[ -f "$TARGET" ]]; then
    if command -v jq &>/dev/null; then
        info "Merging into existing $TARGET..."
        config_json="$(generate_config)"
        server_json="$(echo "$config_json" | jq '.mcpServers["context-keeper"]')"
        jq --argjson server "$server_json" \
            '.mcpServers["context-keeper"] = $server' "$TARGET" > "$TARGET.tmp" \
            && mv "$TARGET.tmp" "$TARGET"
    else
        warn "$TARGET exists. Overwriting (install jq for merge support)."
        generate_config > "$TARGET"
    fi
else
    generate_config > "$TARGET"
fi

ok "Configuration written to $TARGET"

echo ""
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}  Context Keeper installed for Claude Code${NC}"
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""
echo -e "  Binary:  ${CYAN}${BINARY}${NC}"
echo -e "  Config:  ${CYAN}${TARGET}${NC}"
if [[ -n "$API_URL" ]]; then
    echo -e "  LLM:     ${CYAN}enabled${NC}"
else
    echo -e "  LLM:     ${YELLOW}mock mode (pass --api-url and --api-key for real extraction)${NC}"
fi
echo ""
