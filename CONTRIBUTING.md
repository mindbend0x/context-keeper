# Contributing to Context Keeper

Thank you for your interest in contributing to Context Keeper! This guide covers the essentials for getting started.

## Prerequisites

- **Rust toolchain** (stable, latest recommended) via [rustup](https://rustup.rs)
- **Git** for version control
- No API keys or running database required for development and testing

## Getting Started

```bash
git clone https://github.com/0x313/context-keeper.git
cd context-keeper
cargo build
cargo test
```

All tests run with mock implementations by default - no OpenAI key, no SurrealDB instance needed.

## Project Structure

Context Keeper is a Cargo workspace with five crates. Understanding the dependency flow is essential before making changes:

| Crate | Role |
|-------|------|
| `context-keeper-core` | Pure logic: models, traits, ingestion pipeline, search, temporal management. **Zero heavyweight deps.** |
| `context-keeper-rig` | LLM integration via the Rig framework (embeddings, extraction, rewriting) |
| `context-keeper-surreal` | SurrealDB storage: repository, schema, vector store |
| `context-keeper-mcp` | MCP server binary (stdio + streamable HTTP transports) |
| `context-keeper-cli` | Developer CLI binary |

**Key rule:** traits live in `core`, implementations live in `rig` (LLM) or `surreal` (storage). Never pull LLM or DB dependencies into `core`.

## Development Workflow

1. Create a feature branch off `main`
2. Make your changes, following the conventions below
3. Run `cargo test --workspace` and `cargo clippy -- -D warnings`
4. Open a pull request with a clear description of the change

## Conventions

- **Error handling:** use `thiserror` for new error types; prefer typed `Result<T, ContextKeeperError>` over `anyhow::Result`
- **Async:** all I/O-bound operations are async on `tokio`; avoid blocking calls on async paths
- **Testing:** every new trait gets a `Mock*` implementation in `core/src/traits.rs`; tests must pass without API keys
- **SurrealQL:** hand-constructed queries with parameter binding (`$param` syntax) to prevent injection
- **Naming:** `snake_case` functions, `PascalCase` types, `SCREAMING_SNAKE_CASE` constants; extracted data types prefixed with `Extracted*`

## Reporting Issues

Please file issues on [GitHub](https://github.com/0x313/context-keeper/issues) with:
- A clear description of the problem or enhancement
- Steps to reproduce (for bugs)
- Expected vs actual behavior

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
