use std::collections::HashMap;

use context_keeper_surreal::DEFAULT_TENANT_ID;

/// Per-request tenant identity resolved from the auth layer.
///
/// Inserted into HTTP request extensions by auth middleware and
/// extracted inside tool handlers via `http::request::Parts`.
#[derive(Debug, Clone)]
pub struct TenantContext {
    pub tenant_id: String,
}

impl Default for TenantContext {
    fn default() -> Self {
        Self {
            tenant_id: DEFAULT_TENANT_ID.to_string(),
        }
    }
}

/// Parses a comma-separated mapping of `token:tenant_id` pairs.
///
/// Format: `"tok1:tenant_a,tok2:tenant_b"`.
/// Tokens without a `:tenant` suffix map to [`DEFAULT_TENANT_ID`].
pub fn parse_tenant_map(raw: &str) -> HashMap<String, String> {
    raw.split(',')
        .map(|entry| entry.trim())
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            if let Some((token, tenant)) = entry.split_once(':') {
                (token.to_string(), tenant.to_string())
            } else {
                (entry.to_string(), DEFAULT_TENANT_ID.to_string())
            }
        })
        .collect()
}

/// Resolve a bearer token to a [`TenantContext`].
///
/// Checks the static tenant map first; falls back to the default tenant
/// when no mapping exists (single-tenant backward compat).
pub fn resolve_tenant(token: &str, tenant_map: &HashMap<String, String>) -> TenantContext {
    let tenant_id = tenant_map
        .get(token)
        .cloned()
        .unwrap_or_else(|| DEFAULT_TENANT_ID.to_string());
    TenantContext { tenant_id }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tenant_map_basic() {
        let map = parse_tenant_map("tok1:alpha,tok2:beta");
        assert_eq!(map.get("tok1").unwrap(), "alpha");
        assert_eq!(map.get("tok2").unwrap(), "beta");
    }

    #[test]
    fn parse_tenant_map_bare_token_gets_default() {
        let map = parse_tenant_map("tok_no_tenant,tok2:beta");
        assert_eq!(map.get("tok_no_tenant").unwrap(), DEFAULT_TENANT_ID);
    }

    #[test]
    fn resolve_known_token() {
        let map = parse_tenant_map("secret:acme");
        let ctx = resolve_tenant("secret", &map);
        assert_eq!(ctx.tenant_id, "acme");
    }

    #[test]
    fn resolve_unknown_token_falls_back() {
        let map = parse_tenant_map("secret:acme");
        let ctx = resolve_tenant("other", &map);
        assert_eq!(ctx.tenant_id, DEFAULT_TENANT_ID);
    }
}
