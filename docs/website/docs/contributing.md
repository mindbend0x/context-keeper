---

## sidebar_position: 9
title: Contributing
description: How to contribute to Context Keeper.

# Contributing

Context Keeper is open source under the MIT license. Contributions are welcome and encouraged! Whether you're fixing bugs, adding features, improving documentation, or sharing feedback, we'd love your help.

## Development Setup

### Prerequisites

- Rust 1.70+ (install via [rustup](https://rustup.rs))
- Cargo (included with Rust)
- Git

### Clone and Build

```bash
git clone https://github.com/mindbend0x/context-keeper.git
cd context-keeper
cargo build
```

### Run Tests

No API key is required. Tests use mock implementations of the LLM and embedder:

```bash
# Run all tests
cargo test

# Run integration tests only
cargo test -p context-keeper-test

# Run tests for a specific crate
cargo test -p context-keeper-core
cargo test -p context-keeper-surreal
```

## Project Structure

Context Keeper is organized as a Rust workspace with five crates:

### crates/context-keeper-core

Pure logic layer: data models, ingestion pipeline, hybrid search (RRF), temporal management, and trait definitions. Zero heavyweight dependencies.

- Key files: `ingestion/pipeline.rs`, `search/engine.rs`, `entities.rs`
- No external API calls or database code

### crates/context-keeper-rig

Rig framework integration for embeddings and LLM extraction. Implements the traits defined in `context-keeper-core`.

- Key files: `extraction.rs`, `embedder.rs`
- Depends on: Rig, OpenAI-compatible clients

### crates/context-keeper-surreal

SurrealDB client with ~35+ CRUD methods, vector/keyword search, graph traversal, and temporal queries. All database operations live here.

- Key file: `repository.rs` (~700 lines)
- Depends on: SurrealDB, surrealdb crate

### crates/context-keeper-mcp

MCP server binary exposing 10 tools, resources, and prompts. Supports both stdio and HTTP transports.

- Key file: `tools.rs`
- Depends on: rmcp (MCP SDK), context-keeper-core, context-keeper-rig, context-keeper-surreal

### crates/context-keeper-cli

Developer CLI for adding, searching, and querying memories. Reuses the core ingestion and search pipeline.

- Depends on: context-keeper-core, context-keeper-rig, context-keeper-surreal

## Running Tests

### Unit Tests

```bash
cargo test --lib
```

Tests run against mock implementations, making them fast and free.

### Integration Tests

```bash
cargo test -p context-keeper-test
```

Five integration test suites cover end-to-end workflows:

- Memory ingestion
- Entity extraction and updates
- Relation tracking
- Search and retrieval
- Temporal queries

### Testing Without API Keys

All tests work out of the box without environment variables. The core uses mock implementations when LLM settings are absent:

```bash
# No setup needed
cargo test
```

## Code Style

### Error Handling

Use `thiserror` for all error types. Custom error enums provide context and enable structured error handling:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContextKeeperError {
    #[error("Entity not found: {0}")]
    EntityNotFound(String),
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("Database error: {0}")]
    Database(#[from] surrealdb::Error),
}
```

### Async Everywhere

Use `async/await` with the `tokio` runtime. All I/O operations—database, HTTP, embeddings—are async:

```rust
pub async fn add_memory(&self, text: &str) -> Result<Memory> {
    let entities = self.extractor.extract_entities(text).await?;
    self.repository.store_entities(entities).await?;
    Ok(memory)
}
```

### Trait-Based Design

Define traits in `context-keeper-core` for extensibility. Implement them in `context-keeper-rig` and `context-keeper-surreal`:

```rust
// In core
#[async_trait]
pub trait Embedder: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
}

// In rig
pub struct OpenAIEmbedder { /* ... */ }

#[async_trait]
impl Embedder for OpenAIEmbedder { /* ... */ }
```

### Parameter Binding in SurrealQL

Always use parameter binding to prevent injection attacks:

```rust
// Good
let query = "SELECT * FROM memories WHERE created_at > $after";
let results = self.db.query(query).bind(("after", timestamp)).await?;

// Avoid
let query = format!("SELECT * FROM memories WHERE created_at > {}", timestamp);
```

### Test Naming

Use descriptive test names that indicate what is being tested and the expected outcome:

```rust
#[tokio::test]
async fn test_extract_entities_from_multi_sentence_text() { /* ... */ }

#[tokio::test]
async fn test_search_returns_top_5_by_default() { /* ... */ }
```

## Submitting Changes

1. **Fork the repository** — Create a personal fork on GitHub.
2. **Create a branch** — Use a descriptive name like `feat/fz-XX-description` or `fix/fz-XX-description` (where XX is the Linear issue number, if applicable).
3. **Make changes** — Implement your feature or fix, adding tests as needed.
4. **Run tests** — Ensure all tests pass:
  ```bash
   cargo test
  ```
5. **Commit with a clear message** — Reference the Linear issue if applicable.
6. **Push and open a pull request** — Link the PR to the corresponding Linear issue.

## Release Process

Releases are fully automated via GitHub Actions. Pushing a version tag to `main` triggers the entire pipeline.

### Triggering a Release

```bash
git tag v0.1.0
git push origin v0.1.0
```

The tag **must** match the `v`* pattern (e.g. `v0.1.0`, `v1.2.3`). This triggers `.github/workflows/release.yml`, which runs three jobs in parallel after the build completes:


| Job             | What it does                                                                                        |
| --------------- | --------------------------------------------------------------------------------------------------- |
| **release**     | Creates a GitHub Release with pre-built binaries and checksums for all 4 targets                    |
| **update-tap**  | Renders the Homebrew formula template and pushes it to the `mindbend0x/homebrew-context-keeper` tap |
| **publish-npm** | Publishes the MCP server to npm as `context-keeper-mcp` with platform-specific binary packages      |


### Build Targets

The `build` job compiles both `context-keeper-cli` and `context-keeper-mcp` for:


| Target                      | Runner             |
| --------------------------- | ------------------ |
| `x86_64-unknown-linux-gnu`  | `ubuntu-latest`    |
| `aarch64-unknown-linux-gnu` | `ubuntu-24.04-arm` |
| `x86_64-apple-darwin`       | `macos-latest`     |
| `aarch64-apple-darwin`      | `macos-latest`     |


### npm Publishing

The MCP server is distributed via npm so users can run `npx context-keeper-mcp` without a manual install. The `npm/` directory at the repo root contains the package scaffolding:

```
npm/
  context-keeper-mcp/        # Main package (JS shim + optionalDependencies)
    package.json
    bin/run.js               # Resolves platform → binary, execs with stdio inherited
  darwin-arm64/              # @context-keeper/mcp-darwin-arm64
  darwin-x64/                # @context-keeper/mcp-darwin-x64
  linux-arm64/               # @context-keeper/mcp-linux-arm64
  linux-x64/                 # @context-keeper/mcp-linux-x64
```

All `package.json` files use `@@VERSION@@` placeholders. During release, the CI pipeline:

1. Copies each compiled `context-keeper-mcp` binary into its matching platform package
2. Replaces `@@VERSION@@` with the tag version (e.g. `0.1.0` from `v0.1.0`)
3. Publishes the 4 platform packages first (`@context-keeper/mcp-*`)
4. Publishes the main `context-keeper-mcp` package last (it depends on the platform packages via `optionalDependencies`)

npm uses the `os` and `cpu` fields in each platform package's `package.json` to install only the matching binary for the user's system.

### Homebrew Publishing

The Homebrew tap publishes the **CLI only** (not the MCP server). The CI:

1. Computes SHA-256 checksums of the CLI binaries
2. Renders `Formula/context-keeper.rb.template` with the checksums and version
3. Pushes the rendered formula to the `mindbend0x/homebrew-context-keeper` tap repo

### Required Secrets


| Secret           | Purpose                                              |
| ---------------- | ---------------------------------------------------- |
| `TAP_REPO_TOKEN` | GitHub PAT with push access to the Homebrew tap repo |
| `NPM_TOKEN`      | npm token with **publish** access to `@context-keeper` (automation or granular). Must be exposed as `NODE_AUTH_TOKEN` for the entire `publish-npm` job so `actions/setup-node` can write `.npmrc`; setting it only on `npm publish` steps causes `ENEEDAUTH` in CI. |


> Note: These values are currently added as secrets in Github within the release workflow.

### Version Alignment

All npm packages share the same version derived from the git tag. The Homebrew formula version and GitHub Release tag are also aligned. Keep the workspace `Cargo.toml` version in sync with the tag you plan to push.

## Issues and Feature Requests

- Check [existing issues](https://github.com/mindbend0x/context-keeper/issues) to avoid duplicates.
- Use clear, descriptive titles and provide context (e.g., error messages, expected vs. actual behavior).
- For feature requests, explain the use case and desired outcome.

## License

By contributing to Context Keeper, you agree that your contributions will be licensed under the MIT License.