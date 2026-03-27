---
name: add-core-trait
description: >-
  Add a new trait to context-keeper-core with a mock implementation.
  Use when adding a new abstraction, capability, or extension point
  to the core crate.
---

# Add Core Trait

## Steps

### 1. Define the Trait in Core

Add to `crates/context-keeper-core/src/traits.rs`:

```rust
#[async_trait]
pub trait MyTrait: Send + Sync {
    async fn method(&self, input: &str) -> Result<Output>;
}
```

- Trait must be `Send + Sync` for use behind `Arc<dyn MyTrait>`.
- Use `Result<T>` (currently `anyhow::Result`, migrating to typed errors).
- If the trait needs data types, define `Extracted*` structs above the trait.

### 2. Add a Mock Implementation

In the same file, below the `// ── Mock implementations` section:

```rust
pub struct MockMyTrait;

#[async_trait]
impl MyTrait for MockMyTrait {
    async fn method(&self, input: &str) -> Result<Output> {
        // Deterministic, no I/O, no API keys
        Ok(Output::default())
    }
}
```

### 3. Add Unit Tests

In the `#[cfg(test)] mod tests` block at the bottom of `traits.rs`:

```rust
#[tokio::test]
async fn test_mock_my_trait() {
    let mock = MockMyTrait;
    let result = mock.method("test input").await.unwrap();
    assert!(!result.is_empty());
}
```

### 4. Implement in Rig or Surreal

- **LLM-backed** traits → `crates/context-keeper-rig/src/` (new file or extend existing)
- **Storage-backed** traits → `crates/context-keeper-surreal/src/repository.rs`

### 5. Wire into Binaries

Update `ContextKeeperServer::new()` in MCP and/or the CLI `main()` to accept and use the new trait.

## Checklist

- [ ] Trait in `core/src/traits.rs` with `#[async_trait]` + `Send + Sync`
- [ ] Mock implementation in same file (no I/O, deterministic)
- [ ] Unit test for mock
- [ ] Real implementation in rig or surreal crate
- [ ] Re-exported from crate `lib.rs` if public
- [ ] Wired into MCP server and/or CLI
