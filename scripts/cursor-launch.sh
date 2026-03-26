#!/usr/bin/env bash
# =============================================================================
# Context Keeper — Cursor CLI Work Item Launcher
# =============================================================================
# Run this script from the repo root to open work items in Cursor.
#
# Prerequisites:
#   - Cursor CLI installed and in PATH (`cursor` command available)
#   - You're in the context-keeper repo directory
#
# Usage:
#   ./scripts/cursor-launch.sh              # Open project + show menu
#   ./scripts/cursor-launch.sh level0       # Launch Level 0 tasks
#   ./scripts/cursor-launch.sh level1       # Launch Level 1 tasks
#   ./scripts/cursor-launch.sh level2       # Launch Level 2 tasks
#   ./scripts/cursor-launch.sh <issue>      # Launch a specific issue (e.g., fz-59)
# =============================================================================

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

info()  { echo -e "${BLUE}[INFO]${NC} $1"; }
ok()    { echo -e "${GREEN}[OK]${NC} $1"; }
warn()  { echo -e "${YELLOW}[WARN]${NC} $1"; }

# ---------------------------------------------------------------------------
# Open project in Cursor (idempotent)
# ---------------------------------------------------------------------------
open_project() {
    info "Opening project in Cursor..."
    cursor "$REPO_ROOT" 2>/dev/null || warn "Could not open Cursor. Is it installed?"
}

# ---------------------------------------------------------------------------
# Level 0: Foundation Sprint
# ---------------------------------------------------------------------------
launch_fz59() {
    info "FZ-59: Typed error hierarchy (ContextKeeperError enum)"
    cursor "$REPO_ROOT/crates/context-keeper-core/src/lib.rs" \
           "$REPO_ROOT/crates/context-keeper-mcp/src/tools.rs" \
           "$REPO_ROOT/docs/plans/LEVEL-0-foundation.md" 2>/dev/null

    echo ""
    echo "Cursor Agent prompt for FZ-59:"
    echo "---"
    cat <<'PROMPT'
Read docs/plans/LEVEL-0-foundation.md task 0.1 and CLAUDE.md for project context.

Create a typed error hierarchy for Context Keeper:
1. Create crates/context-keeper-core/src/error.rs with ContextKeeperError enum (variants: LlmUnavailable, ExtractionFailed, EntityNotFound, StorageError, ValidationError, BudgetExceeded)
2. Use thiserror for derive macros
3. Replace anyhow::Result with Result<T, ContextKeeperError> in core crate public functions
4. Replace anyhow::Result in rig crate public functions
5. Implement From<ContextKeeperError> for McpError in crates/context-keeper-mcp/src/tools.rs
6. Update surreal crate to wrap DB errors in StorageError
7. Run cargo test to verify everything passes

Branch: mindbend0x/fz-59-typed-error-hierarchy-contextkeepererror-enum
PROMPT
    echo "---"
}

launch_fz57() {
    info "FZ-57: LLM extraction retry + output validation"
    cursor "$REPO_ROOT/crates/context-keeper-rig/src/extraction.rs" \
           "$REPO_ROOT/crates/context-keeper-core/src/ingestion/pipeline.rs" \
           "$REPO_ROOT/docs/plans/LEVEL-0-foundation.md" 2>/dev/null

    echo ""
    echo "Cursor Agent prompt for FZ-57:"
    echo "---"
    cat <<'PROMPT'
Read docs/plans/LEVEL-0-foundation.md task 0.2 and CLAUDE.md for project context.

Add retry and validation to LLM extraction in crates/context-keeper-rig/src/extraction.rs:
1. Add retry-with-backoff (3 attempts, exponential: 100ms, 400ms, 1600ms) for RigEntityExtractor and RigRelationExtractor
2. Retry on: network errors, rate limits, malformed JSON. Don't retry on valid empty results.
3. Validate entities: reject empty name, empty summary, invalid entity_type. Log warnings.
4. Validate relations: reject self-referential, empty predicate, confidence outside 0-100, references to non-extracted entities.
5. Add tests: mock extractor that fails N times then succeeds, validation rejection tests.
6. Run cargo test to verify.

Branch: mindbend0x/fz-57-llm-extraction-retry-output-validation
PROMPT
    echo "---"
}

launch_fz58() {
    info "FZ-58: Composite entity identity (name + entity_type)"
    cursor "$REPO_ROOT/crates/context-keeper-surreal/src/schema.rs" \
           "$REPO_ROOT/crates/context-keeper-surreal/src/repository.rs" \
           "$REPO_ROOT/crates/context-keeper-core/src/models.rs" \
           "$REPO_ROOT/docs/plans/LEVEL-0-foundation.md" 2>/dev/null

    echo ""
    echo "Cursor Agent prompt for FZ-58:"
    echo "---"
    cat <<'PROMPT'
Read docs/plans/LEVEL-0-foundation.md task 0.3 and CLAUDE.md for project context.

Change entity identity from name-only to composite (name + entity_type):
1. In crates/context-keeper-surreal/src/schema.rs: change unique index from name to (name, entity_type)
2. In crates/context-keeper-surreal/src/repository.rs: update upsert_entity() to match on composite key, add entity_type filter to find_entities_by_name()
3. In crates/context-keeper-core/src/models.rs: ensure EntityType is always populated (no Option, default to Other)
4. Update ALL tests to create entities with explicit entity types
5. Test that "Alice (Person)" and "Alice (Organization)" coexist as separate nodes
6. Run cargo test to verify.

Branch: mindbend0x/fz-58-composite-entity-identity-name-entity_type
PROMPT
    echo "---"
}

level0() {
    info "=== LEVEL 0: Foundation Sprint ==="
    echo ""
    launch_fz59
    echo ""
    launch_fz57
    echo ""
    launch_fz58
    echo ""
    ok "Level 0 tasks displayed. Copy prompts into Cursor Agent (Cmd+I) to start work."
}

# ---------------------------------------------------------------------------
# Level 1: Core Correctness
# ---------------------------------------------------------------------------
launch_fz12() {
    info "FZ-12: Memory updates / negation detection"
    cursor "$REPO_ROOT/crates/context-keeper-core/src/ingestion/pipeline.rs" \
           "$REPO_ROOT/crates/context-keeper-rig/src/extraction.rs" \
           "$REPO_ROOT/crates/context-keeper-surreal/src/repository.rs" \
           "$REPO_ROOT/docs/plans/LEVEL-1-correctness.md" 2>/dev/null

    echo ""
    echo "Cursor Agent prompt for FZ-12:"
    echo "---"
    cat <<'PROMPT'
Read docs/plans/LEVEL-1-correctness.md task 1.1 and CLAUDE.md for project context.

Implement robust memory updates and negation detection:
1. Semantic contradiction detection: use LLM to compare new text against existing entity summaries. If contradicts, set old entity valid_until = now.
2. Entity re-identification: use embedding similarity + LLM confirmation to resolve "Alice at Acme" and "Alice (software engineer)" to same node.
3. Update IngestionDiff to clearly report: entities created/updated/invalidated, relations invalidated.
4. Temporal invalidation cascade: when entity invalidated, also invalidate its relations.
5. Add tests: ingest "Alice works at Acme", then "Alice left Acme for BigCo" → search "who works at Acme?" returns only non-Alice results.

Branch: mindbend0x/fz-12-memory-updates
PROMPT
    echo "---"
}

launch_fz13() {
    info "FZ-13: Entity relationship quality"
    cursor "$REPO_ROOT/crates/context-keeper-core/src/models.rs" \
           "$REPO_ROOT/crates/context-keeper-rig/src/extraction.rs" \
           "$REPO_ROOT/crates/context-keeper-surreal/src/repository.rs" \
           "$REPO_ROOT/docs/plans/LEVEL-1-correctness.md" 2>/dev/null

    echo ""
    echo "Cursor Agent prompt for FZ-13:"
    echo "---"
    cat <<'PROMPT'
Read docs/plans/LEVEL-1-correctness.md task 1.2 and CLAUDE.md for project context.

Improve entity relationship quality:
1. Tighten extraction prompts to use canonical RelationType enum values. Reject/remap unknown predicates.
2. Relation deduplication during ingestion: check for semantically equivalent relations (same subject + object + similar predicate), merge instead of duplicate.
3. Evaluate confidence threshold (currently 50). Test against existing fixtures.
4. Bidirectional normalization: "Alice knows Bob" and "Bob knows Alice" = one edge.
5. Run existing tests + add new ones for dedup and normalization.

Branch: mindbend0x/fz-13-improve-entity-relationship-management
PROMPT
    echo "---"
}

launch_fz14() {
    info "FZ-14: Expand entity types (light pass)"
    cursor "$REPO_ROOT/crates/context-keeper-core/src/models.rs" \
           "$REPO_ROOT/crates/context-keeper-rig/src/extraction.rs" \
           "$REPO_ROOT/docs/plans/LEVEL-1-correctness.md" 2>/dev/null

    echo ""
    echo "Cursor Agent prompt for FZ-14:"
    echo "---"
    cat <<'PROMPT'
Read docs/plans/LEVEL-1-correctness.md task 1.3 and CLAUDE.md for project context.

Light pass on entity type expansion:
1. Update extraction prompts to instruct LLM to classify entities using existing EntityType enum (Person, Organization, Location, Event, Product, Service, Concept, File, Other).
2. Add optional entity_type filter to search queries in repository.
3. Surface entity type in MCP tools: get_entity and search_memory should return entity types.
4. Run cargo test.

Branch: mindbend0x/fz-14-expand-entity-types
PROMPT
    echo "---"
}

level1() {
    info "=== LEVEL 1: Core Correctness ==="
    warn "Prerequisite: Level 0 must be complete first!"
    echo ""
    launch_fz12
    echo ""
    launch_fz13
    echo ""
    launch_fz14
    echo ""
    ok "Level 1 tasks displayed. Copy prompts into Cursor Agent (Cmd+I) to start work."
}

# ---------------------------------------------------------------------------
# Level 2: Publishing & Release
# ---------------------------------------------------------------------------
launch_fz60() {
    info "FZ-60: Create runnable examples"
    cursor "$REPO_ROOT/Cargo.toml" \
           "$REPO_ROOT/docs/plans/LEVEL-2-publish.md" 2>/dev/null

    echo ""
    echo "Cursor Agent prompt for FZ-60:"
    echo "---"
    cat <<'PROMPT'
Read docs/plans/LEVEL-2-publish.md task 2.1 and CLAUDE.md for project context.

Create runnable examples in examples/ directory. All must work with mock extractors (no API key):
1. examples/quickstart.rs — 3 episodes, basic search, entity lookup. Under 50 lines. README hero example.
2. examples/temporal_demo.rs — "Alice leaves Acme" scenario. Temporal updates, snapshot, before/after queries.
3. examples/feature_showcase.rs — Comprehensive: ingestion, hybrid search, expand_search, entity graph, temporal snapshot. Print wall-clock timings.
4. Add [[example]] sections to root Cargo.toml for each.
5. Verify all examples compile and run: cargo run --example quickstart, etc.

Branch: mindbend0x/fz-60-create-runnable-examples-quickstart-temporal-showcase
PROMPT
    echo "---"
}

launch_fz61() {
    info "FZ-61: CI/CD pipeline"
    cursor "$REPO_ROOT/Dockerfile" \
           "$REPO_ROOT/docs/plans/LEVEL-2-publish.md" 2>/dev/null

    echo ""
    echo "Cursor Agent prompt for FZ-61:"
    echo "---"
    cat <<'PROMPT'
Read docs/plans/LEVEL-2-publish.md task 2.2 and CLAUDE.md for project context.

Create GitHub Actions CI/CD pipeline in .github/workflows/ci.yml:
1. Build matrix: cargo build on ubuntu-latest, macos-latest, windows-latest
2. Test: cargo test (all crates, mock mode, no API keys)
3. Clippy: cargo clippy -- -D warnings
4. Format: cargo fmt --check
5. Docker: build and verify the Dockerfile on ubuntu
6. Release job: on tag push (v*), build release binaries for linux-x64/arm64, macos-x64/arm64, windows-x64. Create GitHub Release with artifacts.
Note: SurrealDB requires clang for building. Add clang to build deps.

Branch: mindbend0x/fz-61-cicd-pipeline-github-actions
PROMPT
    echo "---"
}

launch_fz62() {
    info "FZ-62: README polish"
    cursor "$REPO_ROOT/README.md" \
           "$REPO_ROOT/docs/plans/LEVEL-2-publish.md" 2>/dev/null

    echo ""
    echo "Cursor Agent prompt for FZ-62:"
    echo "---"
    cat <<'PROMPT'
Read docs/plans/LEVEL-2-publish.md task 2.3, docs/benchmark-and-demo-strategy.md, and CLAUDE.md.

Polish README.md for public launch. Target under 500 lines:
1. Hero section: one-liner description + badges (crates.io, docs.rs, CI status, Docker pulls, MIT license)
2. 30-second quickstart: cargo install → add → search, 3 commands max
3. Feature comparison table: CK vs Graphiti vs Mem0 vs LangMem (architecture, local-first, MCP support, temporal queries)
4. MCP integration: Claude Desktop + Cursor setup in 5 lines each
5. Architecture diagram (Mermaid)
6. Consolidate duplicated sections, trim verbosity

Branch: mindbend0x/fz-62-readme-polish-for-public-release
PROMPT
    echo "---"
}

level2() {
    info "=== LEVEL 2: Publishing & Release ==="
    warn "Prerequisite: Level 1 must be complete first!"
    echo ""
    launch_fz60
    echo ""
    launch_fz61
    echo ""
    launch_fz62
    echo ""
    info "Remaining Level 2 tasks (FZ-64 crates.io, FZ-63 Docker Hub, FZ-65 license) are smaller — handle manually."
    ok "Level 2 tasks displayed. Copy prompts into Cursor Agent (Cmd+I) to start work."
}

# ---------------------------------------------------------------------------
# Menu
# ---------------------------------------------------------------------------
show_menu() {
    echo ""
    echo "╔═══════════════════════════════════════════════════════════╗"
    echo "║         Context Keeper — Cursor Work Item Launcher       ║"
    echo "╠═══════════════════════════════════════════════════════════╣"
    echo "║                                                           ║"
    echo "║  Level 0: Foundation Sprint  (1-2 days)  ← START HERE    ║"
    echo "║    FZ-59  Typed error hierarchy                           ║"
    echo "║    FZ-57  Extraction retry + validation                   ║"
    echo "║    FZ-58  Composite entity identity                       ║"
    echo "║                                                           ║"
    echo "║  Level 1: Core Correctness   (3-5 days)                   ║"
    echo "║    FZ-12  Memory updates / negation                       ║"
    echo "║    FZ-13  Entity relationship quality                     ║"
    echo "║    FZ-14  Expand entity types                             ║"
    echo "║                                                           ║"
    echo "║  Level 2: Publishing         (2-3 days)                   ║"
    echo "║    FZ-60  Runnable examples                               ║"
    echo "║    FZ-61  CI/CD pipeline                                  ║"
    echo "║    FZ-62  README polish                                   ║"
    echo "║    FZ-64  crates.io prep                                  ║"
    echo "║    FZ-63  Docker Hub publishing                           ║"
    echo "║    FZ-65  License & legal                                 ║"
    echo "║                                                           ║"
    echo "╚═══════════════════════════════════════════════════════════╝"
    echo ""
    echo "Usage: ./scripts/cursor-launch.sh [level0|level1|level2|fz-XX]"
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
case "${1:-menu}" in
    level0)  open_project; level0 ;;
    level1)  open_project; level1 ;;
    level2)  open_project; level2 ;;
    fz-59|FZ-59) open_project; launch_fz59 ;;
    fz-57|FZ-57) open_project; launch_fz57 ;;
    fz-58|FZ-58) open_project; launch_fz58 ;;
    fz-12|FZ-12) open_project; launch_fz12 ;;
    fz-13|FZ-13) open_project; launch_fz13 ;;
    fz-14|FZ-14) open_project; launch_fz14 ;;
    fz-60|FZ-60) open_project; launch_fz60 ;;
    fz-61|FZ-61) open_project; launch_fz61 ;;
    fz-62|FZ-62) open_project; launch_fz62 ;;
    menu|*)  show_menu ;;
esac
