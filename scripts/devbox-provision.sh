#!/usr/bin/env bash
# =============================================================================
# devbox-provision.sh — One-time setup for the Context Keeper staging devbox
#
# Run this ON the devbox (or via: ssh root@37.27.205.25 'bash -s' < scripts/devbox-provision.sh)
#
# What it does:
#   1. Installs Docker Engine + Docker Compose plugin
#   2. Configures UFW firewall (SSH + MCP HTTP port)
#   3. Creates the app directory and .env template
#   4. Sets up a non-root deploy user (optional but recommended)
# =============================================================================
set -euo pipefail

# ── Configuration ────────────────────────────────────────────────────────────
APP_DIR="/opt/context-keeper"
DEPLOY_USER="deploy"
MCP_PORT=3000

echo "==> Context Keeper devbox provisioning starting..."
echo "    OS: $(lsb_release -ds 2>/dev/null || cat /etc/os-release | grep PRETTY_NAME | cut -d= -f2)"

# ── 1. Install Docker ────────────────────────────────────────────────────────
if command -v docker &>/dev/null; then
    echo "==> Docker already installed: $(docker --version)"
else
    echo "==> Installing Docker Engine..."
    apt-get update
    apt-get install -y ca-certificates curl gnupg

    install -m 0755 -d /etc/apt/keyrings
    curl -fsSL https://download.docker.com/linux/ubuntu/gpg | gpg --dearmor -o /etc/apt/keyrings/docker.gpg
    chmod a+r /etc/apt/keyrings/docker.gpg

    echo \
      "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] \
      https://download.docker.com/linux/ubuntu \
      $(. /etc/os-release && echo "$VERSION_CODENAME") stable" | \
      tee /etc/apt/sources.list.d/docker.list > /dev/null

    apt-get update
    apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin

    systemctl enable docker
    systemctl start docker
    echo "==> Docker installed: $(docker --version)"
fi

# ── 2. Create deploy user ────────────────────────────────────────────────────
if id "$DEPLOY_USER" &>/dev/null; then
    echo "==> User '$DEPLOY_USER' already exists"
else
    echo "==> Creating deploy user '$DEPLOY_USER'..."
    adduser --disabled-password --gecos "Context Keeper Deploy" "$DEPLOY_USER"
    usermod -aG docker "$DEPLOY_USER"

    # Copy root's authorized_keys so you can SSH as deploy user too
    mkdir -p /home/$DEPLOY_USER/.ssh
    cp /root/.ssh/authorized_keys /home/$DEPLOY_USER/.ssh/authorized_keys
    chown -R $DEPLOY_USER:$DEPLOY_USER /home/$DEPLOY_USER/.ssh
    chmod 700 /home/$DEPLOY_USER/.ssh
    chmod 600 /home/$DEPLOY_USER/.ssh/authorized_keys
    echo "==> Deploy user created. You can now: ssh $DEPLOY_USER@$(hostname -I | awk '{print $1}')"
fi

# ── 3. Configure firewall ────────────────────────────────────────────────────
echo "==> Configuring UFW firewall..."
apt-get install -y ufw

ufw default deny incoming
ufw default allow outgoing
ufw allow ssh
ufw allow $MCP_PORT/tcp comment "Context Keeper MCP HTTP"

# Allow SurrealDB port only if you want external access (optional)
# ufw allow 8000/tcp comment "SurrealDB"

echo "y" | ufw enable
ufw status verbose
echo "==> Firewall configured (SSH + port $MCP_PORT open)"

# ── 4. Create app directory ──────────────────────────────────────────────────
echo "==> Setting up app directory at $APP_DIR..."
mkdir -p "$APP_DIR"
chown $DEPLOY_USER:$DEPLOY_USER "$APP_DIR"

# Create .env template
if [ ! -f "$APP_DIR/.env" ]; then
    cat > "$APP_DIR/.env" <<'ENVEOF'
# Context Keeper Staging Environment
# Fill in your LLM provider details below.

# OpenAI-compatible endpoint
OPENAI_API_URL=
OPENAI_API_KEY=

# Embedding config
EMBEDDING_MODEL=text-embedding-3-small
EMBEDDING_DIMS=1536

# Extraction model
EXTRACTION_MODEL=gpt-4o-mini
ENVEOF
    chown $DEPLOY_USER:$DEPLOY_USER "$APP_DIR/.env"
    echo "==> Created $APP_DIR/.env template — fill in your API keys"
else
    echo "==> $APP_DIR/.env already exists, skipping"
fi

# ── 5. Done ──────────────────────────────────────────────────────────────────
echo ""
echo "============================================"
echo "  Devbox provisioning complete!"
echo "============================================"
echo ""
echo "  Next steps:"
echo "    1. Edit $APP_DIR/.env with your API keys"
echo "    2. Run the deploy script from your local machine:"
echo "       ./scripts/deploy-staging.sh"
echo ""
