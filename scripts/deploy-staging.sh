#!/usr/bin/env bash
# =============================================================================
# deploy-staging.sh — Deploy Context Keeper to the staging devbox
#
# Builds the Docker image on the devbox and starts the stack.
# Run from your local machine at the repo root.
#
# Usage:
#   ./scripts/deploy-staging.sh              # Full deploy (sync + build + up)
#   ./scripts/deploy-staging.sh --restart    # Just restart services (no rebuild)
#   ./scripts/deploy-staging.sh --logs       # Tail logs
#   ./scripts/deploy-staging.sh --status     # Show service status
#   ./scripts/deploy-staging.sh --down       # Stop all services
# =============================================================================
set -euo pipefail

# ── Configuration ────────────────────────────────────────────────────────────
DEVBOX_HOST="${DEVBOX_HOST:-root@37.27.205.25}"
DEVBOX_APP_DIR="/opt/context-keeper"
COMPOSE_FILE="docker-compose.staging.yml"

# ── Helpers ──────────────────────────────────────────────────────────────────
ssh_cmd() {
    ssh -o StrictHostKeyChecking=accept-new "$DEVBOX_HOST" "$@"
}

log() {
    echo "==> $*"
}

# ── Commands ─────────────────────────────────────────────────────────────────
case "${1:-deploy}" in
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
            -e "ssh -o StrictHostKeyChecking=accept-new" \
            ./ \
            "$DEVBOX_HOST:$DEVBOX_APP_DIR/"

        # Build and start
        log "Building Docker image on devbox (this may take a while on first run)..."
        ssh_cmd "cd $DEVBOX_APP_DIR && docker compose -f $COMPOSE_FILE build"

        log "Starting services..."
        ssh_cmd "cd $DEVBOX_APP_DIR && docker compose -f $COMPOSE_FILE up -d"

        log "Waiting for services to start..."
        sleep 5
        ssh_cmd "cd $DEVBOX_APP_DIR && docker compose -f $COMPOSE_FILE ps"

        DEVBOX_IP=$(echo "$DEVBOX_HOST" | grep -oP '[\d.]+')
        echo ""
        echo "============================================"
        echo "  Deployment complete!"
        echo "============================================"
        echo ""
        echo "  MCP Server:  http://$DEVBOX_IP:3000"
        echo "  SurrealDB:   ws://localhost:8000 (internal only)"
        echo ""
        echo "  Useful commands:"
        echo "    ./scripts/deploy-staging.sh --logs     # Tail logs"
        echo "    ./scripts/deploy-staging.sh --status   # Service status"
        echo "    ./scripts/deploy-staging.sh --restart  # Restart services"
        echo "    ./scripts/deploy-staging.sh --down     # Stop everything"
        echo ""
        ;;
esac
