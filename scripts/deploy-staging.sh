#!/usr/bin/env bash
# =============================================================================
# deploy-staging.sh — Deploy Context Keeper to the staging devbox
#
# Builds the Docker image on the devbox and starts the stack.
# Run from your local machine at the repo root.
#
# Usage:
#   ./scripts/deploy-staging.sh              # Full deploy (sync code + env + build + up)
#   ./scripts/deploy-staging.sh --sync-env   # Push .env.staging to devbox and restart
#   ./scripts/deploy-staging.sh --restart    # Just restart services (no rebuild)
#   ./scripts/deploy-staging.sh --logs       # Tail logs
#   ./scripts/deploy-staging.sh --status     # Show service status
#   ./scripts/deploy-staging.sh --down       # Stop all services
#
# Environment:
#   ENV_FILE  — Path to staging env file (default: .env.staging)
# =============================================================================
set -euo pipefail

# ── Configuration ────────────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SSH_KEY="$REPO_ROOT/.keys/devbox_ed25519"
DEVBOX_HOST="${DEVBOX_HOST:-root@37.27.205.25}"
DEVBOX_APP_DIR="/opt/context-keeper"
COMPOSE_FILE="docker-compose.staging.yml"
ENV_FILE="${ENV_FILE:-$REPO_ROOT/.env.staging}"

# ── Helpers ──────────────────────────────────────────────────────────────────
SSH_OPTS="-o StrictHostKeyChecking=accept-new"
if [ -f "$SSH_KEY" ]; then
    SSH_OPTS="$SSH_OPTS -i $SSH_KEY"
fi

ssh_cmd() {
    ssh $SSH_OPTS "$DEVBOX_HOST" "$@"
}

log() {
    echo "==> $*"
}

sync_env() {
    if [ -f "$ENV_FILE" ]; then
        log "Syncing staging env ($ENV_FILE) to $DEVBOX_HOST:$DEVBOX_APP_DIR/.env"
        scp $SSH_OPTS "$ENV_FILE" "$DEVBOX_HOST:$DEVBOX_APP_DIR/.env"
    else
        log "No staging env file found at $ENV_FILE — skipping env sync"
        log "  Create one with: cp .env.staging.example .env.staging"
    fi
}

# ── Commands ─────────────────────────────────────────────────────────────────
case "${1:-deploy}" in
    --sync-env)
        sync_env
        log "Restarting services to pick up new env..."
        ssh_cmd "cd $DEVBOX_APP_DIR && docker compose -f $COMPOSE_FILE up -d"
        ssh_cmd "cd $DEVBOX_APP_DIR && docker compose -f $COMPOSE_FILE ps"
        ;;

    --restart)
        log "Restarting services on $DEVBOX_HOST..."
        ssh_cmd "cd $DEVBOX_APP_DIR && docker compose -f $COMPOSE_FILE restart"
        ssh_cmd "cd $DEVBOX_APP_DIR && docker compose -f $COMPOSE_FILE ps"
        ;;

    --logs)
        log "Tailing logs on $DEVBOX_HOST..."
        ssh_cmd "cd $DEVBOX_APP_DIR && docker compose -f $COMPOSE_FILE logs -f --tail=100"
        ;;

    --status)
        log "Service status on $DEVBOX_HOST..."
        ssh_cmd "cd $DEVBOX_APP_DIR && docker compose -f $COMPOSE_FILE ps"
        ;;

    --down)
        log "Stopping services on $DEVBOX_HOST..."
        ssh_cmd "cd $DEVBOX_APP_DIR && docker compose -f $COMPOSE_FILE down"
        ;;

    deploy|*)
        log "Deploying Context Keeper to $DEVBOX_HOST..."

        # Sync source code to devbox (exclude build artifacts and git internals)
        log "Syncing source code..."
        rsync -avz --delete \
            --exclude='target/' \
            --exclude='.git/' \
            --exclude='.claude/' \
            --exclude='node_modules/' \
            --exclude='.env' \
            --exclude='.env.staging' \
            --exclude='.keys/' \
            -e "ssh $SSH_OPTS" \
            ./ \
            "$DEVBOX_HOST:$DEVBOX_APP_DIR/"

        # Sync staging environment variables
        sync_env

        # Build and start
        log "Building Docker image on devbox (this may take a while on first run)..."
        ssh_cmd "cd $DEVBOX_APP_DIR && docker compose -f $COMPOSE_FILE build"

        log "Starting services..."
        ssh_cmd "cd $DEVBOX_APP_DIR && docker compose -f $COMPOSE_FILE up -d"

        log "Waiting for services to start..."
        sleep 5
        ssh_cmd "cd $DEVBOX_APP_DIR && docker compose -f $COMPOSE_FILE ps"

        DEVBOX_IP=$(echo "$DEVBOX_HOST" | sed 's/.*@//')
        echo ""
        echo "============================================"
        echo "  Deployment complete!"
        echo "============================================"
        echo ""
        echo "  CK MCP Server:     http://$DEVBOX_IP:3000"
        echo "  Devbox MCP Server: http://$DEVBOX_IP:4000/mcp"
        echo "  SurrealDB:         ws://localhost:8000 (internal only)"
        echo ""
        echo "  Useful commands:"
        echo "    ./scripts/deploy-staging.sh --logs     # Tail logs"
        echo "    ./scripts/deploy-staging.sh --status   # Service status"
        echo "    ./scripts/deploy-staging.sh --restart  # Restart services"
        echo "    ./scripts/deploy-staging.sh --down     # Stop everything"
        echo ""
        ;;
esac
