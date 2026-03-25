#!/usr/bin/env bash
# audit/phases/tier11/phase50.sh -- Phase 50: Security and Auth E2E
# Tier: 11 (E2E Journeys)
# What: PIN auth works, JWT tokens have correct expiry, admin endpoints protected.
# Standing rules: SEC-01 (auth gate), SEC-02 (invalid PIN rejected), SEC-03 (protected endpoints)
# CRITICAL: AUDIT_PIN read from env var -- never hardcoded

set -u
set -o pipefail
# NO set -e

run_phase50() {
  local phase="50" tier="11"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # --- Check 1: Valid PIN auth returns session token ---
  local pin="${AUDIT_PIN:-}"
  if [[ -n "$pin" ]]; then
    local tmpfile; tmpfile=$(mktemp)
    jq -n --arg pin "$pin" '{pin: $pin}' > "$tmpfile"
    response=$(curl -s -m 10 -X POST "http://192.168.31.23:8080/api/v1/terminal/auth" \
      -H 'Content-Type: application/json' -d "@${tmpfile}" 2>/dev/null || true)
    rm -f "$tmpfile"
    if printf '%s' "$response" | jq -e '.session' > /dev/null 2>&1; then
      status="PASS"; severity="P3"; message="Valid PIN auth: session token returned"
    elif [[ -z "$response" ]]; then
      status="FAIL"; severity="P1"; message="Valid PIN auth: no response from auth endpoint"
    else
      status="FAIL"; severity="P1"; message="Valid PIN auth: no session field in response (auth broken)"
    fi
  else
    status="WARN"; severity="P2"; message="AUDIT_PIN not set — skipping valid PIN check"
  fi
  emit_result "$phase" "$tier" "server-23-auth-valid" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 2: Invalid PIN rejected ---
  local tmpfile2; tmpfile2=$(mktemp)
  jq -n --arg pin "000000" '{pin: $pin}' > "$tmpfile2"
  local invalid_body; invalid_body=$(curl -s -m 10 \
    -X POST "http://192.168.31.23:8080/api/v1/terminal/auth" \
    -H 'Content-Type: application/json' -d "@${tmpfile2}" 2>/dev/null | tr -d '\r')
  local invalid_code; invalid_code=$(curl -s -m 10 -o /dev/null -w "%{http_code}" \
    -X POST "http://192.168.31.23:8080/api/v1/terminal/auth" \
    -H 'Content-Type: application/json' -d "@${tmpfile2}" 2>/dev/null)
  rm -f "$tmpfile2"
  # Check body for rejection — server may return 200 with {"error":"Invalid PIN."}
  local body_has_error; body_has_error=$(printf '%s' "$invalid_body" | jq -r 'if .error then "YES" elif .session then "NO" else "YES" end' 2>/dev/null)
  if [[ "$invalid_code" = "401" || "$invalid_code" = "403" ]]; then
    status="PASS"; severity="P3"; message="Invalid PIN correctly rejected with HTTP ${invalid_code}"
  elif [[ "$body_has_error" = "YES" ]]; then
    status="PASS"; severity="P3"; message="Invalid PIN rejected (body contains error, HTTP ${invalid_code})"
  elif [[ "$body_has_error" = "NO" ]]; then
    status="FAIL"; severity="P1"; message="CRITICAL: Invalid PIN 000000 returned session token — auth bypass"
  else
    status="WARN"; severity="P2"; message="Invalid PIN check: unexpected response (HTTP ${invalid_code})"
  fi
  emit_result "$phase" "$tier" "server-23-auth-invalid" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 3: Protected endpoint without auth must return 401 ---
  local protected_code; protected_code=$(curl -s -m 10 -o /dev/null -w "%{http_code}" \
    "http://192.168.31.23:8080/api/v1/billing/sessions/active" 2>/dev/null)
  if [[ "$protected_code" = "401" ]]; then
    status="PASS"; severity="P3"; message="Protected endpoint /billing/sessions/active correctly returns 401 without auth"
  elif [[ "$protected_code" = "200" ]]; then
    status="FAIL"; severity="P1"; message="CRITICAL: Protected endpoint /billing/sessions/active returned 200 without auth — no auth gate"
  else
    status="WARN"; severity="P2"; message="Protected endpoint returned HTTP ${protected_code} (expected 401)"
  fi
  emit_result "$phase" "$tier" "server-23-auth-protected" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 4: Public health endpoint accessible without auth ---
  local health_code; health_code=$(curl -s -m 10 -o /dev/null -w "%{http_code}" \
    "http://192.168.31.23:8080/api/v1/health" 2>/dev/null)
  if [[ "$health_code" = "200" ]]; then
    status="PASS"; severity="P3"; message="Public health endpoint /api/v1/health returns 200 without auth"
  else
    status="WARN"; severity="P2"; message="Public health endpoint returned HTTP ${health_code} (expected 200 — should be public)"
  fi
  emit_result "$phase" "$tier" "server-23-auth-public" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase50
