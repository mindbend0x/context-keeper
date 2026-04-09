#!/usr/bin/env bash
# =============================================================================
# dev-server.sh — Run the MCP server locally for testing
#
# Builds and starts context-keeper-mcp in a clean, test-friendly config:
#   - In-memory storage (no leftover state between runs)
#   - HTTP transport on a predictable port
#   - No auth required
#   - Debug logging
#
# All defaults can be overridden via env vars or flags.
#
# Usage:
#   ./scripts/dev-server.sh                    # defaults: memory, HTTP :3000, debug
#   ./scripts/dev-server.sh --port 4000        # custom port
#   ./scripts/dev-server.sh --seed             # pre-load context.sql on startup
#   ./scripts/dev-server.sh --release          # build in release mode
#   ./scripts/dev-server.sh --log-level info   # less noisy logs
#   STORAGE_BACKEND=rocksdb:./test-data ./scripts/dev-server.sh  # override via env
# =============================================================================

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

# ── Defaults (overridable via env) ──────────────────────────────────────────
: "${STORAGE_BACKEND:=memory}"
: "${MCP_TRANSPORT:=http}"
: "${MCP_HTTP_PORT:=3000}"
: "${MCP_HTTP_HOST:=127.0.0.1}"
: "${MCP_ALLOW_INSECURE_HTTP:=1}"
: "${RUST_LOG:=context_keeper=debug}"

CARGO_PROFILE=""
SEED=false

# ── Parse flags ─────────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        --port)
            MCP_HTTP_PORT="$2"; shift 2 ;;
        --log-level)
            RUST_LOG="context_keeper=$2"; shift 2 ;;
        --seed)
            SEED=true; shift ;;
        --release)
            CARGO_PROFILE="--release"; shift ;;
        -h|--help)
            head -n 20 "$0" | tail -n +2 | sed 's/^# \?//'
            exit 0 ;;
        *)
            echo "Unknown flag: $1" >&2; exit 1 ;;
    esac
done

# ── Build ───────────────────────────────────────────────────────────────────
echo "Building context-keeper-mcp${CARGO_PROFILE:+ ($CARGO_PROFILE)}..."
cargo build -p context-keeper-mcp $CARGO_PROFILE

# ── Seed data ───────────────────────────────────────────────────────────────
SEED_ENV=""
if $SEED && [[ -f "$REPO_ROOT/context.sql" ]]; then
    SEED_ENV="DB_FILE_PATH=$REPO_ROOT/context.sql"
    echo "Seed data: context.sql will be loaded on startup"
elif $SEED; then
    echo "Warning: --seed passed but context.sql not found at repo root" >&2
fi

# ── Run ─────────────────────────────────────────────────────────────────────
echo ""
echo "─── Context Keeper Dev Server ───────────────────────────────"
echo "  URL:      http://${MCP_HTTP_HOST}:${MCP_HTTP_PORT}/mcp"
echo "  Storage:  ${STORAGE_BACKEND}"
echo "  Auth:     disabled (insecure HTTP)"
echo "  Logging:  ${RUST_LOG}"
echo ""
echo "  Smoke test:"
echo "    curl -s http://localhost:${MCP_HTTP_PORT}/mcp -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-03-26\",\"capabilities\":{},\"clientInfo\":{\"name\":\"test\",\"version\":\"0.1\"}}}'"
echo "────────────────────────────────────────────────────────────"
echo ""

cleanup() {
    echo ""
    echo "Shutting down dev server..."
}
trap cleanup INT TERM

export STORAGE_BACKEND MCP_TRANSPORT MCP_HTTP_PORT MCP_HTTP_HOST MCP_ALLOW_INSECURE_HTTP RUST_LOG

exec env $SEED_ENV cargo run -p context-keeper-mcp $CARGO_PROFILE
