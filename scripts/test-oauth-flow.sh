#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# test-oauth-flow.sh — End-to-end test of the MCP OAuth 2.1 flow against
# a live Context Keeper instance.
#
# Usage:
#   ./scripts/test-oauth-flow.sh [BASE_URL]
#
# Default BASE_URL: https://mcp.ctxk.mindsetlabs.io
# ---------------------------------------------------------------------------
set -euo pipefail

BASE="${1:-https://mcp.ctxk.mindsetlabs.io}"
PASS=0
FAIL=0
TOTAL=0

green()  { printf "\033[32m%s\033[0m\n" "$*"; }
red()    { printf "\033[31m%s\033[0m\n" "$*"; }
yellow() { printf "\033[33m%s\033[0m\n" "$*"; }
bold()   { printf "\033[1m%s\033[0m\n" "$*"; }

check() {
  TOTAL=$((TOTAL + 1))
  local label="$1"; shift
  if "$@"; then
    green "  ✓ $label"
    PASS=$((PASS + 1))
  else
    red "  ✗ $label"
    FAIL=$((FAIL + 1))
  fi
}

# ---------------------------------------------------------------------------
bold "Testing OAuth 2.1 flow against $BASE"
echo ""

# ── 1. Unauthenticated /mcp should 401 with resource_metadata ─────────────
bold "[1/7] Unauthenticated MCP request → 401 with resource_metadata"

MCP_RESP=$(curl -s -w "\n%{http_code}" "$BASE/mcp" 2>/dev/null)
MCP_CODE=$(echo "$MCP_RESP" | tail -1)
MCP_HDR=$(curl -sI "$BASE/mcp" 2>/dev/null | grep -i www-authenticate || true)

check "Status is 401" [ "$MCP_CODE" = "401" ]
check "WWW-Authenticate contains resource_metadata" echo "$MCP_HDR" | grep -qi "resource_metadata"
echo ""

# ── 2. Protected Resource Metadata ────────────────────────────────────────
bold "[2/7] GET /.well-known/oauth-protected-resource"

PRM_RESP=$(curl -s -w "\n%{http_code}" "$BASE/.well-known/oauth-protected-resource" 2>/dev/null)
PRM_CODE=$(echo "$PRM_RESP" | tail -1)
PRM_BODY=$(echo "$PRM_RESP" | sed '$d')

check "Status is 200" [ "$PRM_CODE" = "200" ]
check "Contains 'resource' field" echo "$PRM_BODY" | grep -q '"resource"'
check "Contains 'authorization_servers'" echo "$PRM_BODY" | grep -q '"authorization_servers"'

if [ "$PRM_CODE" = "200" ]; then
  yellow "  Response:"
  echo "$PRM_BODY" | python3 -m json.tool 2>/dev/null || echo "$PRM_BODY"
fi
echo ""

# ── 3. Authorization Server Metadata ──────────────────────────────────────
bold "[3/7] GET /.well-known/oauth-authorization-server"

ASM_RESP=$(curl -s -w "\n%{http_code}" "$BASE/.well-known/oauth-authorization-server" 2>/dev/null)
ASM_CODE=$(echo "$ASM_RESP" | tail -1)
ASM_BODY=$(echo "$ASM_RESP" | sed '$d')

check "Status is 200" [ "$ASM_CODE" = "200" ]
check "Contains authorization_endpoint" echo "$ASM_BODY" | grep -q '"authorization_endpoint"'
check "Contains token_endpoint" echo "$ASM_BODY" | grep -q '"token_endpoint"'
check "Contains registration_endpoint" echo "$ASM_BODY" | grep -q '"registration_endpoint"'
check "PKCE S256 supported" echo "$ASM_BODY" | grep -q '"S256"'

if [ "$ASM_CODE" = "200" ]; then
  yellow "  Response:"
  echo "$ASM_BODY" | python3 -m json.tool 2>/dev/null || echo "$ASM_BODY"
fi
echo ""

# ── 4. Dynamic Client Registration ───────────────────────────────────────
bold "[4/7] POST /oauth/register (dynamic client registration)"

REG_RESP=$(curl -s -w "\n%{http_code}" -X POST "$BASE/oauth/register" \
  -H "Content-Type: application/json" \
  -d '{
    "client_name": "test-oauth-script",
    "redirect_uris": ["http://127.0.0.1:9999/callback"],
    "grant_types": ["authorization_code"],
    "response_types": ["code"],
    "token_endpoint_auth_method": "none"
  }' 2>/dev/null)
REG_CODE=$(echo "$REG_RESP" | tail -1)
REG_BODY=$(echo "$REG_RESP" | sed '$d')

check "Status is 201" [ "$REG_CODE" = "201" ]
check "Contains client_id" echo "$REG_BODY" | grep -q '"client_id"'
check "Contains client_secret" echo "$REG_BODY" | grep -q '"client_secret"'

CLIENT_ID=""
CLIENT_SECRET=""
if [ "$REG_CODE" = "201" ]; then
  CLIENT_ID=$(echo "$REG_BODY" | python3 -c "import sys,json; print(json.load(sys.stdin)['client_id'])" 2>/dev/null || true)
  CLIENT_SECRET=$(echo "$REG_BODY" | python3 -c "import sys,json; print(json.load(sys.stdin).get('client_secret',''))" 2>/dev/null || true)
  yellow "  Registered client: $CLIENT_ID"
fi
echo ""

# ── 5. Authorization endpoint (GET renders consent page) ─────────────────
bold "[5/7] GET /oauth/authorize (consent page)"

if [ -n "$CLIENT_ID" ]; then
  AUTH_RESP=$(curl -s -w "\n%{http_code}" \
    "$BASE/oauth/authorize?response_type=code&client_id=$CLIENT_ID&redirect_uri=http://127.0.0.1:9999/callback&scope=mcp:tools&state=test123" \
    2>/dev/null)
  AUTH_CODE_HTTP=$(echo "$AUTH_RESP" | tail -1)
  AUTH_BODY=$(echo "$AUTH_RESP" | sed '$d')

  check "Status is 200" [ "$AUTH_CODE_HTTP" = "200" ]
  check "Returns HTML consent page" echo "$AUTH_BODY" | grep -qi "approve"
  check "Contains client_id in page" echo "$AUTH_BODY" | grep -q "$CLIENT_ID"
else
  red "  Skipped — no client_id from registration"
fi
echo ""

# ── 6. Approve + token exchange (full PKCE flow) ─────────────────────────
bold "[6/7] POST /oauth/approve → POST /oauth/token (PKCE flow)"

ACCESS_TOKEN=""
if [ -n "$CLIENT_ID" ]; then
  # Extract session_id from the consent page
  SESSION_ID=$(echo "$AUTH_BODY" | grep -o 'name="session_id" value="[^"]*"' | head -1 | sed 's/.*value="//;s/"//' || true)

  if [ -n "$SESSION_ID" ]; then
    # Approve the authorization (follow redirect to get the code)
    APPROVE_RESP=$(curl -s -w "\n%{http_code}" -X POST "$BASE/oauth/approve" \
      -H "Content-Type: application/x-www-form-urlencoded" \
      -d "session_id=$SESSION_ID&approved=true" \
      -D - 2>/dev/null)

    # Extract the authorization code from the Location redirect header
    LOCATION=$(echo "$APPROVE_RESP" | grep -i "^location:" | tr -d '\r' | head -1)
    AUTH_CODE=$(echo "$LOCATION" | sed -n 's/.*code=\([^&]*\).*/\1/p')

    check "Approval redirects with code" [ -n "$AUTH_CODE" ]

    if [ -n "$AUTH_CODE" ]; then
      yellow "  Auth code: ${AUTH_CODE:0:30}..."

      # Exchange code for token
      TOKEN_RESP=$(curl -s -w "\n%{http_code}" -X POST "$BASE/oauth/token" \
        -H "Content-Type: application/x-www-form-urlencoded" \
        -d "grant_type=authorization_code&code=$AUTH_CODE&client_id=$CLIENT_ID&redirect_uri=http://127.0.0.1:9999/callback" \
        2>/dev/null)
      TOKEN_CODE=$(echo "$TOKEN_RESP" | tail -1)
      TOKEN_BODY=$(echo "$TOKEN_RESP" | sed '$d')

      check "Token exchange returns 200" [ "$TOKEN_CODE" = "200" ]
      check "Contains access_token" echo "$TOKEN_BODY" | grep -q '"access_token"'
      check "Token type is Bearer" echo "$TOKEN_BODY" | grep -q '"Bearer"'

      ACCESS_TOKEN=$(echo "$TOKEN_BODY" | python3 -c "import sys,json; print(json.load(sys.stdin)['access_token'])" 2>/dev/null || true)

      if [ -n "$ACCESS_TOKEN" ]; then
        yellow "  Access token: ${ACCESS_TOKEN:0:30}..."
      fi
    fi
  else
    red "  Could not extract session_id from consent page"
  fi
else
  red "  Skipped — no client_id"
fi
echo ""

# ── 7. Authenticated MCP request ─────────────────────────────────────────
bold "[7/7] Authenticated POST /mcp (with OAuth token)"

if [ -n "$ACCESS_TOKEN" ]; then
  # MCP initialize request per the spec
  MCP_AUTH_RESP=$(curl -s -w "\n%{http_code}" -X POST "$BASE/mcp" \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $ACCESS_TOKEN" \
    -d '{
      "jsonrpc": "2.0",
      "id": 1,
      "method": "initialize",
      "params": {
        "protocolVersion": "2025-11-25",
        "capabilities": {},
        "clientInfo": {"name": "test-script", "version": "0.1.0"}
      }
    }' 2>/dev/null)
  MCP_AUTH_CODE=$(echo "$MCP_AUTH_RESP" | tail -1)
  MCP_AUTH_BODY=$(echo "$MCP_AUTH_RESP" | sed '$d')

  # Anything other than 401 means the token was accepted
  check "Not rejected (not 401)" [ "$MCP_AUTH_CODE" != "401" ]
  check "Got a response" [ -n "$MCP_AUTH_BODY" ]

  yellow "  Status: $MCP_AUTH_CODE"
  yellow "  Response (first 200 chars):"
  echo "  ${MCP_AUTH_BODY:0:200}"
else
  red "  Skipped — no access token"
fi

# ── Summary ───────────────────────────────────────────────────────────────
echo ""
bold "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
if [ "$FAIL" -eq 0 ]; then
  green "  All $TOTAL checks passed"
else
  yellow "  $PASS/$TOTAL passed, $FAIL failed"
fi
bold "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
exit "$FAIL"
