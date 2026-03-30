# Rig Crate Agent

You are a specialist for `context-keeper-rig`, the LLM integration layer using the Rig framework.

## Ownership

This crate implements core traits against OpenAI-compatible LLM endpoints:
- **Embeddings** (`src/embeddings.rs`): `RigEmbedder` implements `Embedder`
- **Extraction** (`src/extraction.rs`): `RigEntityExtractor` implements `EntityExtractor`, `RigRelationExtractor` implements `RelationExtractor`
- **Query rewriting** (`src/rewriting.rs`): `RigQueryRewriter` implements `QueryRewriter`
- **Tool definitions** (`src/tools.rs`): Rig-compatible tool wrappers

## Constraints

- Depends on `context-keeper-core` (traits + models) and `rig-core`.
- Does **not** depend on `context-keeper-surreal` — no direct DB access.
- All public types implement traits from core. The concrete types here are wired in at the binary level (MCP/CLI).

## LLM Extraction Patterns

Extractors use structured output via Rig's `Extractor` trait:
- Define a prompt that instructs the LLM to return JSON matching the `ExtractedEntity`/`ExtractedRelation` schemas.
- The response is parsed into the `schemars::JsonSchema`-derived types.
- Currently single-pass; ADR-001 R2 recommends retry-with-backoff + output validation.

## When Modifying

- Changing extraction quality → update the system prompts in `extraction.rs`.
- Adding a new LLM provider → Rig supports multiple providers; add the provider dep and create a new constructor variant.
- Adding retry logic → wrap the `extract_*` calls with exponential backoff; validate parsed output (reject empty names, dangling relations).
- The `api_url` + `api_key` constructor pattern allows any OpenAI-compatible endpoint (OpenRouter, Ollama, etc.).
