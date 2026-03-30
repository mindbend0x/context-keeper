# ── Builder stage ─────────────────────────────────────────────────────────
FROM rust:1.94-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    clang \
    libclang-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/context-keeper

# Copy workspace manifests first for layer caching
COPY Cargo.toml Cargo.lock ./
COPY crates/context-keeper-core/Cargo.toml crates/context-keeper-core/Cargo.toml
COPY crates/context-keeper-rig/Cargo.toml crates/context-keeper-rig/Cargo.toml
COPY crates/context-keeper-surreal/Cargo.toml crates/context-keeper-surreal/Cargo.toml
COPY crates/context-keeper-mcp/Cargo.toml crates/context-keeper-mcp/Cargo.toml
COPY crates/context-keeper-cli/Cargo.toml crates/context-keeper-cli/Cargo.toml
COPY test/Cargo.toml test/Cargo.toml
COPY crates/context-keeper-bench/Cargo.toml crates/context-keeper-bench/Cargo.toml

# Create stub source files so cargo can resolve the workspace
RUN mkdir -p crates/context-keeper-core/src && echo "" > crates/context-keeper-core/src/lib.rs \
    && mkdir -p crates/context-keeper-rig/src && echo "" > crates/context-keeper-rig/src/lib.rs \
    && mkdir -p crates/context-keeper-surreal/src && echo "" > crates/context-keeper-surreal/src/lib.rs \
    && mkdir -p crates/context-keeper-mcp/src && echo "fn main() {}" > crates/context-keeper-mcp/src/main.rs \
    && mkdir -p crates/context-keeper-cli/src && echo "fn main() {}" > crates/context-keeper-cli/src/main.rs \
    && mkdir -p crates/context-keeper-bench/src && echo "" > crates/context-keeper-bench/src/lib.rs \
    && mkdir -p crates/context-keeper-bench/benches && echo "fn main() {}" > crates/context-keeper-bench/benches/ingestion.rs \
    && echo "fn main() {}" > crates/context-keeper-bench/benches/search.rs \
    && mkdir -p test/src && echo "" > test/src/lib.rs

# Pre-build dependencies (cached layer)
RUN cargo build --release -p context-keeper-mcp 2>/dev/null || true

# Copy actual source code
COPY crates/ crates/
COPY migrations/ migrations/

# Touch source files to invalidate the stub cache
RUN touch crates/context-keeper-core/src/lib.rs \
    && touch crates/context-keeper-rig/src/lib.rs \
    && touch crates/context-keeper-surreal/src/lib.rs \
    && touch crates/context-keeper-mcp/src/main.rs

# Build the actual binary
RUN cargo build --release -p context-keeper-mcp

# ── Runtime stage ─────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/context-keeper/target/release/context-keeper-mcp /usr/local/bin/context-keeper-mcp

# Default data directory for RocksDB
RUN mkdir -p /data

ENV STORAGE_BACKEND=rocksdb:/data
ENV MCP_TRANSPORT=http
ENV MCP_HTTP_PORT=3000

EXPOSE 3000

ENTRYPOINT ["context-keeper-mcp"]
