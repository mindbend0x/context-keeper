# Context Keeper — Development & Deployment Makefile
# Usage: make <target>

# ── Configuration ────────────────────────────────────────────────────────────
COMPOSE_FILE   := docker-compose.staging.yml
DEPLOY_SCRIPT  := ./scripts/deploy-staging.sh
RUST_LOG       ?= context_keeper=info,warn

# ── Development ──────────────────────────────────────────────────────────────

.PHONY: build test lint fmt check run run-http clean

build: ## Build the full workspace
	cargo build

build-release: ## Build release binaries
	cargo build --release

test: ## Run all workspace tests
	cargo test --workspace

lint: ## Run clippy with warnings as errors
	cargo clippy --workspace -- -D warnings

fmt: ## Check formatting
	cargo fmt --check

fmt-fix: ## Auto-fix formatting
	cargo fmt

check: lint fmt test ## Run lint + fmt + tests (CI equivalent)

run: ## Run MCP server locally (stdio transport)
	RUST_LOG=$(RUST_LOG) cargo run -p context-keeper-mcp

run-http: ## Run MCP server locally (HTTP on port 3000)
	RUST_LOG=$(RUST_LOG) MCP_TRANSPORT=http MCP_HTTP_PORT=3000 cargo run -p context-keeper-mcp

run-debug: ## Run MCP server with debug logging
	RUST_LOG=context_keeper=debug cargo run -p context-keeper-mcp

run-http-debug: ## Run MCP server HTTP with debug logging
	RUST_LOG=context_keeper=debug MCP_TRANSPORT=http MCP_HTTP_PORT=3000 cargo run -p context-keeper-mcp

cli: ## Run CLI (pass ARGS, e.g. make cli ARGS="search --query 'test'")
	RUST_LOG=$(RUST_LOG) cargo run -p context-keeper-cli -- $(ARGS)

clean: ## Clean build artifacts
	cargo clean

# ── Docker (local) ───────────────────────────────────────────────────────────

.PHONY: docker-build docker-up docker-down docker-logs docker-ps docker-rebuild

docker-build: ## Build Docker image locally
	docker compose -f $(COMPOSE_FILE) build context-keeper-mcp

docker-up: ## Start the full stack locally
	docker compose -f $(COMPOSE_FILE) up -d

docker-down: ## Stop all services
	docker compose -f $(COMPOSE_FILE) down

docker-logs: ## Tail logs from all services
	docker compose -f $(COMPOSE_FILE) logs -f --tail=100

docker-ps: ## Show service status
	docker compose -f $(COMPOSE_FILE) ps

docker-rebuild: ## Rebuild and restart the MCP service
	docker compose -f $(COMPOSE_FILE) up -d --build context-keeper-mcp

# ── Staging Deployment ───────────────────────────────────────────────────────

.PHONY: deploy deploy-env deploy-restart deploy-logs deploy-status deploy-down

deploy: ## Full deploy to staging (sync + build + up)
	$(DEPLOY_SCRIPT) deploy

deploy-env: ## Sync .env.staging to devbox and restart
	$(DEPLOY_SCRIPT) --sync-env

deploy-restart: ## Restart services on staging (no rebuild)
	$(DEPLOY_SCRIPT) --restart

deploy-logs: ## Tail staging logs
	$(DEPLOY_SCRIPT) --logs

deploy-status: ## Show staging service status
	$(DEPLOY_SCRIPT) --status

deploy-down: ## Stop staging services
	$(DEPLOY_SCRIPT) --down

# ── Help ─────────────────────────────────────────────────────────────────────

.PHONY: help
help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-18s\033[0m %s\n", $$1, $$2}'

.DEFAULT_GOAL := help
