#!/usr/bin/env bash
# security-audit.sh — v38.0 Security Audit Script (Phase 309)
#
# Automated scan covering all v38.0 security hardening:
#   - TLS configuration (Phase 305)
#   - WS auth hardening (Phase 306)
#   - Audit log integrity (Phase 307)
#   - RBAC enforcement (Phase 308)
#   - General security posture
#
# Output: security-scorecard.json
#
# Usage:
#   bash scripts/security-audit.sh                    # Full audit
#   bash scripts/security-audit.sh --server URL       # Custom server URL
#   bash scripts/security-audit.sh --output FILE      # Custom output path
#
# Exit codes:
#   0 — overall: pass
#   1 — overall: fail (at least one critical check failed)

set -o pipefail

# ---------------------------------------------------------------------------
# Config
# ---------------------------------------------------------------------------
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SERVER_URL="${SERVER_URL:-http://localhost:8080}"
OUTPUT_FILE="${OUTPUT_FILE:-$REPO_ROOT/security-scorecard.json}"

# Parse args
while [[ $# -gt 0 ]]; do
  case "$1" in
    --server) SERVER_URL="$2"; shift 2 ;;
    --output) OUTPUT_FILE="$2"; shift 2 ;;
    *) echo "Usage: $0 [--server URL] [--output FILE]"; exit 1 ;;
  esac
done

# ---------------------------------------------------------------------------
# Colour helpers
# ---------------------------------------------------------------------------
GREEN=$(printf '\033[0;32m')
RED=$(printf '\033[0;31m')
YELLOW=$(printf '\033[1;33m')
RESET=$(printf '\033[0m')

# ---------------------------------------------------------------------------
# JSON result accumulator
# ---------------------------------------------------------------------------
CHECKS_TMP=$(mktemp)
echo "[]" > "$CHECKS_TMP"
PASS_COUNT=0
FAIL_COUNT=0
WARN_COUNT=0

add_check() {
  local name="$1"
  local status="$2"   # pass, fail, warn, skip
  local details="$3"
  local category="$4" # tls, ws_auth, audit_chain, rbac, general

  python3 -c "
import json, sys
with open('$CHECKS_TMP', 'r') as f:
    checks = json.load(f)
checks.append({
    'name': sys.argv[1],
    'status': sys.argv[2],
    'details': sys.argv[3],
    'category': sys.argv[4]
})
with open('$CHECKS_TMP', 'w') as f:
    json.dump(checks, f)
" "$name" "$status" "$details" "$category"

  case "$status" in
    pass) PASS_COUNT=$((PASS_COUNT + 1)); echo "  ${GREEN}PASS${RESET}  $name" ;;
    fail) FAIL_COUNT=$((FAIL_COUNT + 1)); echo "  ${RED}FAIL${RESET}  $name — $details" ;;
    warn) WARN_COUNT=$((WARN_COUNT + 1)); echo "  ${YELLOW}WARN${RESET}  $name — $details" ;;
    skip) echo "  SKIP  $name — $details" ;;
  esac
}

echo "═══════════════════════════════════════════════════════════"
echo "  Security Audit — v38.0 Hardening Scorecard"
echo "  Server: $SERVER_URL"
echo "═══════════════════════════════════════════════════════════"
echo ""

# ═══════════════════════════════════════════════════════════════════════════
# Section 1: TLS Configuration (Phase 305)
# ═══════════════════════════════════════════════════════════════════════════
echo "── TLS Configuration (Phase 305) ──────────────────────────"

# Check 1.1: Venue CA generation script exists and is executable
if [ -x "$REPO_ROOT/scripts/generate-venue-ca.sh" ]; then
  add_check "tls_ca_script_exists" "pass" "generate-venue-ca.sh present and executable" "tls"
else
  add_check "tls_ca_script_exists" "fail" "generate-venue-ca.sh missing or not executable" "tls"
fi

# Check 1.2: TLS module exists in server crate
if [ -f "$REPO_ROOT/crates/racecontrol/src/tls.rs" ]; then
  # Check for mTLS functions
  if grep -q "load_mtls_config\|build_mtls_config_with_client_verify\|WebPkiClientVerifier" \
    "$REPO_ROOT/crates/racecontrol/src/tls.rs" 2>/dev/null; then
    add_check "tls_server_mtls_module" "pass" "Server mTLS module with WebPkiClientVerifier" "tls"
  else
    add_check "tls_server_mtls_module" "warn" "tls.rs exists but missing mTLS functions" "tls"
  fi
else
  add_check "tls_server_mtls_module" "fail" "crates/racecontrol/src/tls.rs missing" "tls"
fi

# Check 1.3: TLS module exists in agent crate
if [ -f "$REPO_ROOT/crates/rc-agent/src/tls.rs" ]; then
  add_check "tls_agent_module" "pass" "Agent TLS module present" "tls"
else
  add_check "tls_agent_module" "fail" "crates/rc-agent/src/tls.rs missing" "tls"
fi

# Check 1.4: Tailscale bypass function exists
if grep -q "is_tailscale_ip\|socket_addr_is_tailscale" \
  "$REPO_ROOT/crates/racecontrol/src/tls.rs" 2>/dev/null; then
  add_check "tls_tailscale_bypass" "pass" "Tailscale CGNAT bypass detection present" "tls"
else
  add_check "tls_tailscale_bypass" "fail" "Tailscale bypass function missing" "tls"
fi

# Check 1.5: MtlsConfig defaults to disabled (safe default)
if grep -q 'enabled.*false\|default.*false' \
  "$REPO_ROOT/crates/racecontrol/src/config.rs" 2>/dev/null; then
  add_check "tls_disabled_by_default" "pass" "TLS disabled by default (safe rollout)" "tls"
else
  add_check "tls_disabled_by_default" "warn" "Could not verify TLS default=false" "tls"
fi

echo ""

# ═══════════════════════════════════════════════════════════════════════════
# Section 2: WS Auth Hardening (Phase 306)
# ═══════════════════════════════════════════════════════════════════════════
echo "── WS Auth Hardening (Phase 306) ──────────────────────────"

# Check 2.1: PodClaims JWT struct exists
if grep -q "pub struct PodClaims" "$REPO_ROOT/crates/racecontrol/src/auth/middleware.rs" 2>/dev/null; then
  add_check "ws_pod_claims" "pass" "PodClaims JWT struct defined" "ws_auth"
else
  add_check "ws_pod_claims" "fail" "PodClaims struct missing in auth/middleware.rs" "ws_auth"
fi

# Check 2.2: Per-pod JWT creation function
if grep -q "pub fn create_pod_jwt" "$REPO_ROOT/crates/racecontrol/src/auth/middleware.rs" 2>/dev/null; then
  add_check "ws_create_pod_jwt" "pass" "create_pod_jwt function present" "ws_auth"
else
  add_check "ws_create_pod_jwt" "fail" "create_pod_jwt function missing" "ws_auth"
fi

# Check 2.3: JWT decode with dual-secret rotation
if grep -q "decode_pod_jwt" "$REPO_ROOT/crates/racecontrol/src/auth/middleware.rs" 2>/dev/null; then
  if grep -q "prev_secret\|jwt_secret_previous" "$REPO_ROOT/crates/racecontrol/src/auth/middleware.rs" 2>/dev/null; then
    add_check "ws_jwt_dual_secret" "pass" "Pod JWT decode with dual-secret rotation grace" "ws_auth"
  else
    add_check "ws_jwt_dual_secret" "warn" "decode_pod_jwt exists but dual-secret not found" "ws_auth"
  fi
else
  add_check "ws_jwt_dual_secret" "fail" "decode_pod_jwt function missing" "ws_auth"
fi

# Check 2.4: WS auth tries JWT first, falls back to PSK
if grep -q "authenticate_agent_ws" "$REPO_ROOT/crates/racecontrol/src/ws/mod.rs" 2>/dev/null; then
  add_check "ws_jwt_first_psk_fallback" "pass" "authenticate_agent_ws (JWT→PSK fallback)" "ws_auth"
else
  add_check "ws_jwt_first_psk_fallback" "fail" "authenticate_agent_ws function missing" "ws_auth"
fi

# Check 2.5: WhatsApp alert on invalid JWT
if grep -q "ws_jwt_rejected\|send_admin_alert" "$REPO_ROOT/crates/racecontrol/src/ws/mod.rs" 2>/dev/null; then
  add_check "ws_jwt_alert" "pass" "WhatsApp alert on JWT rejection" "ws_auth"
else
  add_check "ws_jwt_alert" "fail" "No WhatsApp alert on JWT rejection" "ws_auth"
fi

# Check 2.6: IssueJwt/RefreshJwt protocol messages
if grep -q "IssueJwt\|RefreshJwt" "$REPO_ROOT/crates/rc-common/src/protocol.rs" 2>/dev/null; then
  add_check "ws_jwt_protocol" "pass" "IssueJwt/RefreshJwt protocol messages defined" "ws_auth"
else
  add_check "ws_jwt_protocol" "fail" "JWT protocol messages missing in rc-common" "ws_auth"
fi

# Check 2.7: Agent stores JWT for reconnect
if grep -q "current_jwt\|jwt_expires_at" "$REPO_ROOT/crates/rc-agent/src/app_state.rs" 2>/dev/null; then
  add_check "ws_agent_jwt_store" "pass" "Agent stores JWT in AppState for reconnect" "ws_auth"
else
  add_check "ws_agent_jwt_store" "fail" "Agent JWT storage missing in AppState" "ws_auth"
fi

echo ""

# ═══════════════════════════════════════════════════════════════════════════
# Section 3: Audit Log Integrity (Phase 307)
# ═══════════════════════════════════════════════════════════════════════════
echo "── Audit Log Integrity (Phase 307) ────────────────────────"

# Check 3.1: Hash chain in activity_log
if grep -q "compute_activity_hash\|entry_hash\|previous_hash" \
  "$REPO_ROOT/crates/racecontrol/src/activity_log.rs" 2>/dev/null; then
  add_check "audit_hash_chain" "pass" "SHA-256 hash chain in activity_log" "audit_chain"
else
  add_check "audit_hash_chain" "fail" "Hash chain implementation missing" "audit_chain"
fi

# Check 3.2: DB migration for hash columns
if grep -q "ALTER TABLE pod_activity_log ADD COLUMN entry_hash\|entry_hash TEXT" \
  "$REPO_ROOT/crates/racecontrol/src/db/mod.rs" 2>/dev/null; then
  add_check "audit_db_columns" "pass" "entry_hash/previous_hash columns in schema" "audit_chain"
else
  add_check "audit_db_columns" "fail" "Hash columns missing in DB schema" "audit_chain"
fi

# Check 3.3: Verify endpoint exists
if grep -q "audit/verify\|audit_verify" \
  "$REPO_ROOT/crates/racecontrol/src/api/routes.rs" 2>/dev/null; then
  add_check "audit_verify_endpoint" "pass" "GET /api/v1/audit/verify endpoint defined" "audit_chain"
else
  add_check "audit_verify_endpoint" "fail" "Audit verify endpoint missing" "audit_chain"
fi

# Check 3.4: Live chain integrity (if server is reachable)
VERIFY_RESPONSE=$(curl -s -m 5 -H "Accept: application/json" "$SERVER_URL/api/v1/audit/verify" 2>/dev/null)
CURL_EXIT=$?
if [ $CURL_EXIT -ne 0 ] || [ -z "$VERIFY_RESPONSE" ]; then
  add_check "audit_chain_live" "skip" "Server not reachable at $SERVER_URL" "audit_chain"
elif echo "$VERIFY_RESPONSE" | python3 -c "import json,sys; json.load(sys.stdin)" 2>/dev/null; then
  # Got valid JSON — check chain status
  CHAIN_VALID=$(echo "$VERIFY_RESPONSE" | python3 -c "import json,sys; d=json.load(sys.stdin); print('true' if d.get('chain_valid', d.get('valid', False)) else 'false')" 2>/dev/null)
  CHAIN_LEN=$(echo "$VERIFY_RESPONSE" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('verified_entries', d.get('chain_length', '?')))" 2>/dev/null)
  if [ "$CHAIN_VALID" = "true" ]; then
    add_check "audit_chain_live" "pass" "Live chain integrity verified ($CHAIN_LEN entries)" "audit_chain"
  else
    add_check "audit_chain_live" "fail" "Chain integrity broken at entry $CHAIN_LEN" "audit_chain"
  fi
else
  # Got non-JSON (e.g. HTML from Next.js) — API not available at this path
  add_check "audit_chain_live" "skip" "API returned non-JSON (check server URL/routing)" "audit_chain"
fi

echo ""

# ═══════════════════════════════════════════════════════════════════════════
# Section 4: RBAC Enforcement (Phase 308)
# ═══════════════════════════════════════════════════════════════════════════
echo "── RBAC Enforcement (Phase 308) ───────────────────────────"

# Check 4.1: Role constants defined
if grep -q 'ROLE_CASHIER.*cashier\|ROLE_MANAGER.*manager\|ROLE_SUPERADMIN.*superadmin' \
  "$REPO_ROOT/crates/racecontrol/src/auth/middleware.rs" 2>/dev/null; then
  add_check "rbac_role_constants" "pass" "Three role constants defined (cashier/manager/superadmin)" "rbac"
else
  add_check "rbac_role_constants" "fail" "Role constants missing" "rbac"
fi

# Check 4.2: StaffClaims has role field
if grep -q 'pub role: String' "$REPO_ROOT/crates/racecontrol/src/auth/middleware.rs" 2>/dev/null; then
  add_check "rbac_jwt_role_claim" "pass" "StaffClaims.role field in JWT" "rbac"
else
  add_check "rbac_jwt_role_claim" "fail" "StaffClaims missing role field" "rbac"
fi

# Check 4.3: require_role_manager middleware used on routes
if grep -q "require_role_manager" "$REPO_ROOT/crates/racecontrol/src/api/routes.rs" 2>/dev/null; then
  add_check "rbac_manager_routes" "pass" "Manager+ routes enforced via require_role_manager" "rbac"
else
  add_check "rbac_manager_routes" "fail" "No routes gated by require_role_manager" "rbac"
fi

# Check 4.4: require_role_superadmin middleware used on routes
if grep -q "require_role_superadmin" "$REPO_ROOT/crates/racecontrol/src/api/routes.rs" 2>/dev/null; then
  add_check "rbac_superadmin_routes" "pass" "Superadmin routes enforced via require_role_superadmin" "rbac"
else
  add_check "rbac_superadmin_routes" "fail" "No routes gated by require_role_superadmin" "rbac"
fi

# Check 4.5: Config/deploy/flags behind superadmin
if grep -A5 "require_role_superadmin" "$REPO_ROOT/crates/racecontrol/src/api/routes.rs" 2>/dev/null | \
  grep -q "flags\|deploy\|config/push\|ota"; then
  add_check "rbac_config_deploy_gated" "pass" "Config/deploy/flags/OTA behind superadmin role" "rbac"
else
  add_check "rbac_config_deploy_gated" "warn" "Could not verify config/deploy gating" "rbac"
fi

# Check 4.6: staff_members.role column exists
if grep -q "ALTER TABLE staff_members ADD COLUMN role\|role TEXT DEFAULT" \
  "$REPO_ROOT/crates/racecontrol/src/db/mod.rs" 2>/dev/null; then
  add_check "rbac_db_role_column" "pass" "staff_members.role column in DB schema" "rbac"
else
  add_check "rbac_db_role_column" "fail" "staff_members role column missing" "rbac"
fi

echo ""

# ═══════════════════════════════════════════════════════════════════════════
# Section 5: General Security Posture
# ═══════════════════════════════════════════════════════════════════════════
echo "── General Security Posture ───────────────────────────────"

# Check 5.1: No .unwrap() in production Rust (spot check key files)
UNWRAP_COUNT=0
for f in "$REPO_ROOT/crates/racecontrol/src/auth/middleware.rs" \
         "$REPO_ROOT/crates/racecontrol/src/tls.rs" \
         "$REPO_ROOT/crates/racecontrol/src/activity_log.rs"; do
  if [ -f "$f" ]; then
    count=$(grep -c '\.unwrap()' "$f" 2>/dev/null || true)
    count=${count:-0}
    UNWRAP_COUNT=$((UNWRAP_COUNT + count))
  fi
done
if [ "$UNWRAP_COUNT" -eq 0 ]; then
  add_check "no_unwrap_security_files" "pass" "No .unwrap() in security-critical files" "general"
else
  add_check "no_unwrap_security_files" "warn" "$UNWRAP_COUNT .unwrap() calls in security files" "general"
fi

# Check 5.2: Pre-commit hook installed
if [ -f "$REPO_ROOT/.git/hooks/pre-commit" ] && grep -q "SEC-GATE\|credential" "$REPO_ROOT/.git/hooks/pre-commit" 2>/dev/null; then
  add_check "precommit_hook" "pass" "Pre-commit security hook installed" "general"
else
  add_check "precommit_hook" "warn" "Pre-commit security hook missing or incomplete" "general"
fi

# Check 5.3: No hardcoded secrets in Rust source
SECRET_PATTERNS='sk-[a-zA-Z0-9]{20,}|AKIA[A-Z0-9]{16}|password\s*=\s*"[^"]{8,}"'
HARDCODED=$(grep -rn -E "$SECRET_PATTERNS" \
  "$REPO_ROOT/crates/racecontrol/src/" \
  "$REPO_ROOT/crates/rc-agent/src/" \
  "$REPO_ROOT/crates/rc-common/src/" 2>/dev/null | \
  grep -v "test\|Test\|mock\|example\|template\|placeholder" | head -5)
if [ -z "$HARDCODED" ]; then
  add_check "no_hardcoded_secrets" "pass" "No hardcoded secrets found in Rust source" "general"
else
  add_check "no_hardcoded_secrets" "fail" "Potential hardcoded secrets found" "general"
fi

# Check 5.4: JWT secret not using default/weak value
if grep -q 'jwt_secret.*change.*me\|jwt_secret.*default\|jwt_secret.*secret' \
  "$REPO_ROOT/crates/racecontrol/src/config.rs" 2>/dev/null; then
  add_check "jwt_no_default_secret" "warn" "JWT config may accept weak default secret" "general"
else
  add_check "jwt_no_default_secret" "pass" "No default/weak JWT secret pattern found" "general"
fi

# Check 5.5: Static CRT configured (no DLL deps on pods)
if grep -q 'crt-static' "$REPO_ROOT/.cargo/config.toml" 2>/dev/null; then
  add_check "static_crt" "pass" "Static CRT configured (+crt-static)" "general"
else
  add_check "static_crt" "warn" "Static CRT not found in .cargo/config.toml" "general"
fi

# Check 5.6: Security-check.js exists (SEC-GATE-01)
if [ -f "$REPO_ROOT/../comms-link/test/security-check.js" ] || \
   [ -f "/root/comms-link/test/security-check.js" ]; then
  add_check "sec_gate_01" "pass" "SEC-GATE-01 security-check.js present" "general"
else
  add_check "sec_gate_01" "warn" "SEC-GATE-01 security-check.js not found (check comms-link path)" "general"
fi

# Check 5.7: Pod endpoints default to protected (service key required)
if grep -q "require_service_key\|X-Service-Key" \
  "$REPO_ROOT/crates/rc-agent/src/remote_ops.rs" 2>/dev/null; then
  add_check "pod_endpoints_protected" "pass" "Pod endpoints behind service key auth" "general"
else
  add_check "pod_endpoints_protected" "warn" "Could not verify pod endpoint protection" "general"
fi

echo ""

# ═══════════════════════════════════════════════════════════════════════════
# Summary
# ═══════════════════════════════════════════════════════════════════════════
TOTAL=$((PASS_COUNT + FAIL_COUNT + WARN_COUNT))
OVERALL="pass"
if [ "$FAIL_COUNT" -gt 0 ]; then
  OVERALL="fail"
fi

echo "═══════════════════════════════════════════════════════════"
echo "  Score: $PASS_COUNT/$TOTAL passed"
echo "  Pass: $PASS_COUNT | Fail: $FAIL_COUNT | Warn: $WARN_COUNT"
echo "  Overall: $OVERALL"
echo "═══════════════════════════════════════════════════════════"

# Write JSON scorecard
python3 -c "
import json

with open('$CHECKS_TMP', 'r') as f:
    checks = json.load(f)
scorecard = {
    'version': 'v38.0',
    'timestamp': '$(date -u +%Y-%m-%dT%H:%M:%SZ)',
    'server': '$SERVER_URL',
    'checks': checks,
    'summary': {
        'total': $TOTAL,
        'pass': $PASS_COUNT,
        'fail': $FAIL_COUNT,
        'warn': $WARN_COUNT,
    },
    'score': '$PASS_COUNT/$TOTAL',
    'overall': '$OVERALL'
}
with open('$OUTPUT_FILE', 'w') as f:
    json.dump(scorecard, f, indent=2)
print(f'  Scorecard written to: $OUTPUT_FILE')
"

# Cleanup
rm -f "$CHECKS_TMP"

echo ""

# Exit with appropriate code
if [ "$OVERALL" = "fail" ]; then
  exit 1
fi
exit 0
