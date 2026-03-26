# Level 2: Publishing & Release Readiness

**Goal:** Everything needed to make `feat/prototype-v1` mergeable to `main` and publishable as a public GitHub release + crates.io + Docker Hub.
**Timeline:** 2-3 days
**Depends on:** Level 1 (Core Correctness)
**Unlocks:** Public launch, community feedback

## Tasks

### 2.1 — Working Examples

**Linear:** New issue needed
**Files:** `examples/` directory

Create runnable example binaries that showcase the product:

- [ ] **`examples/quickstart.rs`** — 3 episodes, basic search, entity lookup. Under 50 lines. This is the README hero example.
- [ ] **`examples/temporal_demo.rs`** — The "Alice leaves Acme" scenario. Shows temporal updates, snapshot, before/after queries.
- [ ] **`examples/feature_showcase.rs`** — Comprehensive demo: ingestion, hybrid search, expand_search, entity graph, temporal snapshot. Print timings.

All examples must work with mock extractors (no API key needed). Add a `[[example]]` section to the root Cargo.toml for each.

### 2.2 — CI/CD Pipeline

**Linear:** New issue needed
**Files:** `.github/workflows/ci.yml` (new)

GitHub Actions workflow:
- [ ] **Build matrix** — `cargo build` on Ubuntu, macOS, Windows
- [ ] **Test** — `cargo test` (all crates, mock mode)
- [ ] **Clippy** — `cargo clippy -- -D warnings`
- [ ] **Format** — `cargo fmt --check`
- [ ] **Docker build** — Build and verify the Dockerfile
- [ ] **Release job** — On tag push: build binaries for linux-x64/arm64, macos-x64/arm64, windows-x64. Create GitHub Release with artifacts.

### 2.3 — README Polish

**Linear:** New issue needed
**Files:** `README.md`

The README is already comprehensive (~10K lines) but needs tightening for public launch:

- [ ] **Hero section** — One-liner + badges (crates.io, docs.rs, CI, Docker, license)
- [ ] **30-second quickstart** — `cargo install context-keeper-cli` → `ck add` → `ck search`. Three commands.
- [ ] **Feature comparison table** — CK vs Graphiti vs Mem0 vs LangMem (from benchmark strategy doc)
- [ ] **MCP integration section** — Claude Desktop + Cursor setup in under 5 lines each
- [ ] **Architecture diagram** — ASCII or Mermaid, kept from existing docs
- [ ] **Trim verbosity** — Current README has duplicated sections. Consolidate.

### 2.4 — crates.io Publishing Prep

**Linear:** New issue needed
**Files:** All `Cargo.toml` files

- [ ] **Package metadata** — Ensure all crates have: description, license, repository, homepage, keywords, categories
- [ ] **Version alignment** — All crates at 0.1.0
- [ ] **Publish order** — core → surreal → rig → cli → mcp (respects dependency graph)
- [ ] **`cargo publish --dry-run`** for each crate
- [ ] **Reserve crate names** on crates.io

### 2.5 — Docker Hub Publishing

**Linear:** New issue needed
**Files:** `Dockerfile`, `docker-compose.yml`, `.github/workflows/docker.yml` (new)

- [ ] **Multi-arch build** — linux/amd64 + linux/arm64
- [ ] **Tags** — `latest`, `0.1.0`, git SHA
- [ ] **Docker Hub repo** — Create `contextkeeper/context-keeper-mcp` (or similar)
- [ ] **Compose file polish** — Ensure `docker compose up` works out of the box with sensible defaults

### 2.6 — License & Legal

**Files:** `LICENSE`, all source file headers

- [ ] **LICENSE file** — MIT, already specified in Cargo.toml but verify file exists
- [ ] **SPDX headers** — Optional but nice for enterprise adoption
- [ ] **CONTRIBUTING.md** — Brief contributor guide

## Definition of Done

- [ ] `cargo test` passes on CI (GitHub Actions green)
- [ ] All 3 examples run successfully with `cargo run --example <name>`
- [ ] `cargo publish --dry-run` succeeds for all crates
- [ ] Docker image builds and runs with `docker compose up`
- [ ] README is under 500 lines, has badges, quickstart works as documented
- [ ] LICENSE file present at repo root

## Delete This File When

First public release (v0.1.0) is tagged and published.
