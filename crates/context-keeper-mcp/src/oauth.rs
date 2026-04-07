use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Instant,
};

use axum::{
    body::Body,
    extract::{Form, Query, State},
    http::{HeaderMap, Request, StatusCode},
    middleware::Next,
    response::{Html, IntoResponse, Redirect, Response},
    Json,
};
use rand::distr::Alphanumeric;
use rand::Rng;
use rmcp::transport::auth::{AuthorizationMetadata, ClientRegistrationResponse, OAuthClientConfig};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::tenant::{resolve_tenant, TenantContext};

const CONSENT_HTML: &str = include_str!("html/consent.html");

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct OAuthStore {
    pub clients: Arc<RwLock<HashMap<String, OAuthClientConfig>>>,
    pub auth_sessions: Arc<RwLock<HashMap<String, AuthSession>>>,
    pub access_tokens: Arc<RwLock<HashMap<String, McpAccessToken>>>,
    pub rate_limits: Arc<Mutex<HashMap<String, (u64, Instant)>>>,
}

#[derive(Clone, Debug)]
pub struct AuthSession {
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: Option<String>,
    pub state: Option<String>,
    pub code_challenge: Option<String>,
    pub code_challenge_method: Option<String>,
    pub auth_code: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct McpAccessToken {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub refresh_token: String,
    pub scope: Option<String>,
    pub client_id: String,
    pub tenant_id: Option<String>,
}

impl OAuthStore {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            auth_sessions: Arc::new(RwLock::new(HashMap::new())),
            access_tokens: Arc::new(RwLock::new(HashMap::new())),
            rate_limits: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn validate_client(
        &self,
        client_id: &str,
        redirect_uri: &str,
    ) -> Option<OAuthClientConfig> {
        let clients = self.clients.read().await;
        if let Some(client) = clients.get(client_id) {
            if client.redirect_uri == redirect_uri {
                return Some(client.clone());
            }
        }
        None
    }

    pub async fn validate_token(&self, token: &str) -> Option<McpAccessToken> {
        self.access_tokens.read().await.get(token).cloned()
    }

    /// Returns `true` if the request is within the rate limit window.
    /// Tracks per-key counts with a 60-second sliding window.
    pub fn check_rate_limit(&self, key: &str, max_per_minute: u64) -> bool {
        let mut limits = self.rate_limits.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        let entry = limits.entry(key.to_string()).or_insert((0, now));

        if now.duration_since(entry.1).as_secs() >= 60 {
            *entry = (1, now);
            return true;
        }

        entry.0 += 1;
        entry.0 <= max_per_minute
    }
}

fn random_string(len: usize) -> String {
    rand::rng()
        .sample_iter(Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}

// ---------------------------------------------------------------------------
// Metadata endpoints
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct OAuthConfig {
    pub issuer: String,
    pub oauth_store: Arc<OAuthStore>,
    pub static_tokens: Arc<Vec<String>>,
    pub registration_token: Option<String>,
    pub tenant_map: Arc<HashMap<String, String>>,
}

#[derive(Serialize)]
struct ProtectedResourceMetadata {
    resource: String,
    authorization_servers: Vec<String>,
    scopes_supported: Vec<String>,
    bearer_methods_supported: Vec<String>,
}

pub async fn protected_resource_metadata(State(cfg): State<OAuthConfig>) -> impl IntoResponse {
    let meta = ProtectedResourceMetadata {
        resource: cfg.issuer.clone(),
        authorization_servers: vec![cfg.issuer.clone()],
        scopes_supported: vec!["mcp:tools".into()],
        bearer_methods_supported: vec!["header".into()],
    };
    (StatusCode::OK, Json(meta))
}

pub async fn authorization_server_metadata(State(cfg): State<OAuthConfig>) -> impl IntoResponse {
    let mut metadata = AuthorizationMetadata::default();
    metadata.issuer = Some(cfg.issuer.clone());
    metadata.authorization_endpoint = format!("{}/oauth/authorize", cfg.issuer);
    metadata.token_endpoint = format!("{}/oauth/token", cfg.issuer);
    metadata.registration_endpoint = Some(format!("{}/oauth/register", cfg.issuer));
    metadata.scopes_supported = Some(vec!["mcp:tools".into()]);
    metadata.response_types_supported = Some(vec!["code".into()]);
    metadata.code_challenge_methods_supported = Some(vec!["S256".into(), "plain".into()]);
    metadata.additional_fields.insert(
        "grant_types_supported".into(),
        serde_json::json!(["authorization_code", "refresh_token"]),
    );
    metadata.additional_fields.insert(
        "token_endpoint_auth_methods_supported".into(),
        serde_json::json!(["none"]),
    );
    (StatusCode::OK, Json(metadata))
}

// ---------------------------------------------------------------------------
// Dynamic client registration (RFC 7591)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct RegistrationRequest {
    client_name: Option<String>,
    redirect_uris: Vec<String>,
    #[serde(default)]
    #[allow(dead_code)]
    grant_types: Vec<String>,
    #[serde(default)]
    #[allow(dead_code)]
    response_types: Vec<String>,
    #[serde(default)]
    token_endpoint_auth_method: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    scope: Option<String>,
}

pub async fn oauth_register(
    State(cfg): State<OAuthConfig>,
    headers: HeaderMap,
    Json(req): Json<RegistrationRequest>,
) -> impl IntoResponse {
    if let Some(ref required_token) = cfg.registration_token {
        let authorized = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .map(|provided| {
                use subtle::ConstantTimeEq;
                bool::from(provided.as_bytes().ct_eq(required_token.as_bytes()))
            })
            .unwrap_or(false);

        if !authorized {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "unauthorized",
                    "error_description": "valid registration token required in Authorization header"
                })),
            )
                .into_response();
        }
    }

    if req.redirect_uris.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "invalid_request",
                "error_description": "at least one redirect_uri is required"
            })),
        )
            .into_response();
    }

    let client_id = format!("client-{}", Uuid::new_v4());
    let is_public = req
        .token_endpoint_auth_method
        .as_deref()
        .map(|m| m == "none")
        .unwrap_or(true);

    let client = if is_public {
        OAuthClientConfig::new(client_id.clone(), req.redirect_uris[0].clone())
    } else {
        let secret = random_string(32);
        OAuthClientConfig::new(client_id.clone(), req.redirect_uris[0].clone())
            .with_client_secret(secret)
    };

    let client_secret = client.client_secret.clone();
    let display_name = req.client_name.as_deref().unwrap_or("unnamed");

    cfg.oauth_store
        .clients
        .write()
        .await
        .insert(client_id.clone(), client);

    tracing::info!(client_id = %client_id, name = %display_name, public = %is_public, "OAuth client registered");

    let mut response = ClientRegistrationResponse::new(client_id, req.redirect_uris);
    response.client_secret = client_secret;
    response.client_name = req.client_name;
    response.additional_fields.insert(
        "token_endpoint_auth_method".into(),
        serde_json::json!(if is_public {
            "none"
        } else {
            "client_secret_post"
        }),
    );

    (StatusCode::CREATED, Json(response)).into_response()
}

// ---------------------------------------------------------------------------
// Authorization endpoint
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct AuthorizeQuery {
    response_type: String,
    client_id: String,
    redirect_uri: String,
    scope: Option<String>,
    state: Option<String>,
    code_challenge: Option<String>,
    code_challenge_method: Option<String>,
}

pub async fn oauth_authorize(
    Query(params): Query<AuthorizeQuery>,
    State(cfg): State<OAuthConfig>,
) -> impl IntoResponse {
    tracing::info!(
        client_id = %params.client_id,
        has_pkce = params.code_challenge.is_some(),
        "Authorization request received"
    );

    if params.response_type != "code" {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "unsupported_response_type",
                "error_description": "only 'code' is supported"
            })),
        )
            .into_response();
    }

    if cfg
        .oauth_store
        .validate_client(&params.client_id, &params.redirect_uri)
        .await
        .is_none()
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "invalid_client",
                "error_description": "unknown client_id or redirect_uri mismatch"
            })),
        )
            .into_response();
    }

    let session_id = Uuid::new_v4().to_string();
    let session = AuthSession {
        client_id: params.client_id.clone(),
        redirect_uri: params.redirect_uri.clone(),
        scope: params.scope.clone(),
        state: params.state.clone(),
        code_challenge: params.code_challenge.clone(),
        code_challenge_method: params.code_challenge_method.clone(),
        auth_code: None,
    };
    cfg.oauth_store
        .auth_sessions
        .write()
        .await
        .insert(session_id.clone(), session);

    let scope_display = params.scope.as_deref().unwrap_or("mcp:tools");
    let html = CONSENT_HTML
        .replace("{{client_id}}", &params.client_id)
        .replace("{{redirect_uri}}", &params.redirect_uri)
        .replace("{{scope}}", scope_display)
        .replace("{{state}}", params.state.as_deref().unwrap_or(""))
        .replace("{{session_id}}", &session_id)
        .replace("{{issuer}}", &cfg.issuer);

    Html(html).into_response()
}

// ---------------------------------------------------------------------------
// Approval handler
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ApprovalForm {
    session_id: String,
    approved: String,
}

pub async fn oauth_approve(
    State(cfg): State<OAuthConfig>,
    headers: HeaderMap,
    Form(form): Form<ApprovalForm>,
) -> impl IntoResponse {
    let admin_token = headers.get("x-admin-token").and_then(|v| v.to_str().ok());

    if !cfg.static_tokens.is_empty() {
        let authorized = admin_token
            .map(|provided| {
                use subtle::ConstantTimeEq;
                cfg.static_tokens
                    .iter()
                    .any(|t| bool::from(t.as_bytes().ct_eq(provided.as_bytes())))
            })
            .unwrap_or(false);

        if !authorized {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "unauthorized",
                    "error_description": "valid X-Admin-Token header required to approve grants"
                })),
            )
                .into_response();
        }
    } else if admin_token.is_none() {
        tracing::warn!(
            "OAuth approval without admin token verification — no MCP_AUTH_TOKENS configured. \
             Set MCP_AUTH_TOKENS to secure the approval flow."
        );
    }

    let mut sessions = cfg.oauth_store.auth_sessions.write().await;
    let Some(session) = sessions.get_mut(&form.session_id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "invalid_session"})),
        )
            .into_response();
    };

    if form.approved != "true" {
        let redirect = format!(
            "{}?error=access_denied{}",
            session.redirect_uri,
            session
                .state
                .as_deref()
                .map(|s| format!("&state={s}"))
                .unwrap_or_default()
        );
        return Redirect::to(&redirect).into_response();
    }

    let auth_code = format!("ck-code-{}", Uuid::new_v4());
    session.auth_code = Some(auth_code.clone());

    let redirect = format!(
        "{}?code={}{}",
        session.redirect_uri,
        auth_code,
        session
            .state
            .as_deref()
            .map(|s| format!("&state={s}"))
            .unwrap_or_default()
    );

    tracing::info!(client_id = %session.client_id, "Authorization granted");
    Redirect::to(&redirect).into_response()
}

// ---------------------------------------------------------------------------
// Token endpoint
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct TokenRequest {
    grant_type: String,
    #[serde(default)]
    code: String,
    #[serde(default)]
    client_id: String,
    #[serde(default)]
    #[allow(dead_code)]
    redirect_uri: String,
    #[serde(default)]
    code_verifier: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    refresh_token: String,
    #[serde(default)]
    #[allow(dead_code)]
    resource: Option<String>,
}

pub async fn oauth_token(
    State(cfg): State<OAuthConfig>,
    request: Request<Body>,
) -> impl IntoResponse {
    let content_type = request
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let bytes = match axum::body::to_bytes(request.into_body(), 1024 * 64).await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!("Token request body read error: {e}");
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "invalid_request"})),
            )
                .into_response();
        }
    };

    tracing::debug!(
        content_type = %content_type,
        has_auth_header = auth_header.is_some(),
        body_len = bytes.len(),
        "Token exchange request received"
    );

    let token_req: TokenRequest = match serde_urlencoded::from_bytes(&bytes) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(
                error = %e,
                body = %String::from_utf8_lossy(&bytes),
                "Token request form decode failed"
            );
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "invalid_request",
                    "error_description": format!("bad form data: {e}")
                })),
            )
                .into_response();
        }
    };

    if token_req.grant_type != "authorization_code" {
        tracing::warn!(grant_type = %token_req.grant_type, "Unsupported grant type");
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "unsupported_grant_type",
                "error_description": "only authorization_code is supported"
            })),
        )
            .into_response();
    }

    let mut effective_client_id = token_req.client_id.clone();
    if effective_client_id.is_empty() {
        if let Some(ref auth) = auth_header {
            if let Some(basic) = auth.strip_prefix("Basic ") {
                if let Ok(decoded) = base64_decode(basic) {
                    if let Some((cid, _)) = decoded.split_once(':') {
                        effective_client_id = cid.to_string();
                    }
                }
            }
        }
    }

    let rate_key = if effective_client_id.is_empty() {
        "__anonymous__".to_string()
    } else {
        effective_client_id.clone()
    };
    if !cfg.oauth_store.check_rate_limit(&rate_key, 10) {
        tracing::warn!(client_id = %rate_key, "Token endpoint rate limit exceeded");
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({
                "error": "rate_limit_exceeded",
                "error_description": "too many token requests; try again later"
            })),
        )
            .into_response();
    }

    let sessions = cfg.oauth_store.auth_sessions.read().await;
    let session = sessions.values().find(|s| {
        s.auth_code.as_deref() == Some(&token_req.code)
            && (effective_client_id.is_empty() || s.client_id == effective_client_id)
    });

    let Some(session) = session else {
        tracing::warn!(
            code_prefix = token_req.code.get(..20).unwrap_or(&token_req.code),
            client_id = %effective_client_id,
            session_count = sessions.len(),
            "Token exchange failed: no matching session for code"
        );
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "invalid_grant",
                "error_description": "invalid or expired authorization code"
            })),
        )
            .into_response();
    };

    if let Some(challenge) = &session.code_challenge {
        let method = session.code_challenge_method.as_deref().unwrap_or("plain");
        let verifier = token_req.code_verifier.as_deref().unwrap_or("");

        let valid = match method {
            "S256" => {
                use sha2::Digest;
                let digest = sha2::Sha256::digest(verifier.as_bytes());
                let encoded = base64_url_encode(&digest);
                encoded == *challenge
            }
            "plain" => verifier == challenge,
            _ => false,
        };

        if !valid {
            tracing::warn!(
                method = method,
                has_verifier = !verifier.is_empty(),
                client_id = %session.client_id,
                "PKCE verification failed"
            );
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "invalid_grant",
                    "error_description": "PKCE verification failed"
                })),
            )
                .into_response();
        }
    }

    let access_token = format!("ck-token-{}", Uuid::new_v4());
    let refresh_token = format!("ck-refresh-{}", Uuid::new_v4());
    let mcp_token = McpAccessToken {
        access_token: access_token.clone(),
        token_type: "Bearer".into(),
        expires_in: 3600,
        refresh_token: refresh_token.clone(),
        scope: session.scope.clone(),
        client_id: session.client_id.clone(),
        tenant_id: None,
    };

    cfg.oauth_store
        .access_tokens
        .write()
        .await
        .insert(access_token.clone(), mcp_token);

    tracing::info!(client_id = %session.client_id, "Access token issued");

    let mut body = serde_json::json!({
        "access_token": access_token,
        "token_type": "Bearer",
        "expires_in": 3600,
        "refresh_token": refresh_token,
    });
    if let Some(scope) = &session.scope {
        body["scope"] = serde_json::json!(scope);
    }

    (
        StatusCode::OK,
        [("cache-control", "no-store"), ("pragma", "no-cache")],
        Json(body),
    )
        .into_response()
}

fn base64_url_encode(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn base64_decode(input: &str) -> Result<String, ()> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(input.trim())
        .map_err(|_| ())?;
    String::from_utf8(bytes).map_err(|_| ())
}

// ---------------------------------------------------------------------------
// Unified token validation middleware
// ---------------------------------------------------------------------------

pub async fn unified_auth_middleware(
    State(cfg): State<OAuthConfig>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    use subtle::ConstantTimeEq;

    let path = req.uri().path().to_string();

    if let Some(auth) = req.headers().get("authorization") {
        if let Ok(auth_str) = auth.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                let token_bytes = token.as_bytes();
                if cfg
                    .static_tokens
                    .iter()
                    .any(|t| t.as_bytes().ct_eq(token_bytes).into())
                {
                    let tenant_ctx = resolve_tenant(token, &cfg.tenant_map);
                    tracing::debug!(path = %path, tenant = %tenant_ctx.tenant_id, "Authenticated via static token");
                    req.extensions_mut().insert(tenant_ctx);
                    return next.run(req).await;
                }

                if let Some(mcp_token) = cfg.oauth_store.validate_token(token).await {
                    let tenant_ctx = mcp_token
                        .tenant_id
                        .as_ref()
                        .map(|tid| TenantContext {
                            tenant_id: tid.clone(),
                        })
                        .unwrap_or_else(|| resolve_tenant(token, &cfg.tenant_map));
                    tracing::debug!(path = %path, tenant = %tenant_ctx.tenant_id, "Authenticated via OAuth token");
                    req.extensions_mut().insert(tenant_ctx);
                    return next.run(req).await;
                }

                tracing::debug!(
                    path = %path,
                    token_prefix = token.get(..20).unwrap_or(token),
                    "Bearer token rejected (no match)"
                );
            }
        }
    } else {
        tracing::debug!(path = %path, "No Authorization header, returning 401 with OAuth discovery");
    }

    let resource_metadata_url = format!("{}/.well-known/oauth-protected-resource", cfg.issuer);

    Response::builder()
        .status(401)
        .header(
            "www-authenticate",
            format!(
                "Bearer resource_metadata=\"{}\", scope=\"mcp:tools\"",
                resource_metadata_url
            ),
        )
        .body(Body::from("Unauthorized"))
        .unwrap()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_and_validate_client() {
        let store = OAuthStore::new();
        let client_id = "test-client".to_string();
        let redirect = "http://localhost:3000/callback".to_string();

        let client = OAuthClientConfig::new(client_id.clone(), redirect.clone());
        store
            .clients
            .write()
            .await
            .insert(client_id.clone(), client);

        let found = store.validate_client(&client_id, &redirect).await;
        assert!(found.is_some());

        let not_found = store
            .validate_client(&client_id, "http://evil.com/callback")
            .await;
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_token_store_and_validate() {
        let store = OAuthStore::new();
        let token = McpAccessToken {
            access_token: "ck-token-abc".into(),
            token_type: "Bearer".into(),
            expires_in: 3600,
            refresh_token: "ck-refresh-abc".into(),
            scope: Some("mcp:tools".into()),
            client_id: "test".into(),
            tenant_id: None,
        };

        store
            .access_tokens
            .write()
            .await
            .insert("ck-token-abc".into(), token);

        assert!(store.validate_token("ck-token-abc").await.is_some());
        assert!(store.validate_token("invalid").await.is_none());
    }

    #[test]
    fn test_pkce_s256_verification() {
        use sha2::Digest;
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let digest = sha2::Sha256::digest(verifier.as_bytes());
        let challenge = base64_url_encode(&digest);
        assert!(!challenge.is_empty());
        assert!(!challenge.contains('='));
        assert!(!challenge.contains('+'));
        assert!(!challenge.contains('/'));
    }

    #[test]
    fn test_rate_limiting() {
        let store = OAuthStore::new();
        let client = "test-client";

        for _ in 0..10 {
            assert!(store.check_rate_limit(client, 10));
        }
        assert!(
            !store.check_rate_limit(client, 10),
            "11th request should be rejected"
        );

        assert!(
            store.check_rate_limit("other-client", 10),
            "different client should have its own window"
        );
    }
}
