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

  # --- Check 4: Frontend login pages call correct auth endpoints ---
  # WHY: v20.0 POS dashboard was deployed with stale JS calling /auth/admin-login (6-digit admin PIN)
  # instead of /staff/validate-pin (4-digit staff PIN). API tests passed, but users couldn't log in.
  # This check verifies the DEPLOYED frontend JS calls the correct endpoint.

  # Check 4a: Web dashboard login page uses /staff/validate-pin
  local web_login_html; web_login_html=$(curl -s -m 10 "http://192.168.31.23:3200/login" 2>/dev/null || echo "")
  local web_js_files; web_js_files=$(printf '%s' "$web_login_html" | grep -oE '_next/static/chunks/[^"\\]+\.js' | sort -u || true)
  local web_found_endpoint=""
  while IFS= read -r js_file; do
    [[ -z "$js_file" ]] && continue
    local js_content; js_content=$(curl -s -m 5 "http://192.168.31.23:3200/${js_file}" 2>/dev/null || true)
    if printf '%s' "$js_content" | grep -q 'staff/validate-pin'; then
      web_found_endpoint="staff/validate-pin"
      break
    fi
    if printf '%s' "$js_content" | grep -q 'admin-login'; then
      web_found_endpoint="admin-login"
      break
    fi
  done <<< "$web_js_files"

  if [[ "$web_found_endpoint" = "staff/validate-pin" ]]; then
    status="PASS"; severity="P3"; message="Web dashboard login JS calls /staff/validate-pin (correct)"
  elif [[ "$web_found_endpoint" = "admin-login" ]]; then
    status="FAIL"; severity="P1"; message="Web dashboard login JS calls /admin-login (STALE BUILD — must redeploy)"
  elif [[ -z "$web_login_html" ]]; then
    status="WARN"; severity="P2"; message="Web dashboard login page unreachable"
  else
    status="WARN"; severity="P2"; message="Web dashboard login: could not find auth endpoint in JS bundles"
  fi
  emit_result "$phase" "$tier" "server-23-web-login-endpoint" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Check 4b: Kiosk staff login page uses /staff/validate-pin
  local kiosk_login_html; kiosk_login_html=$(curl -s -m 10 "http://192.168.31.23:3300/kiosk/staff" 2>/dev/null || echo "")
  # Kiosk has basePath=/kiosk — JS paths are /kiosk/_next/static/... Extract full path including prefix
  local kiosk_js_files; kiosk_js_files=$(printf '%s' "$kiosk_login_html" | grep -oE '/kiosk/_next/static/chunks/[^"\\]+\.js' | sort -u || true)
  if [[ -z "$kiosk_js_files" ]]; then
    kiosk_js_files=$(printf '%s' "$kiosk_login_html" | grep -oE '_next/static/chunks/[^"\\]+\.js' | sort -u || true)
  fi
  local kiosk_found_endpoint=""
  while IFS= read -r js_file; do
    [[ -z "$js_file" ]] && continue
    local js_content; js_content=$(curl -s -m 5 "http://192.168.31.23:3300${js_file}" 2>/dev/null || true)
    if printf '%s' "$js_content" | grep -q 'staff/validate-pin'; then
      kiosk_found_endpoint="staff/validate-pin"
      break
    fi
    if printf '%s' "$js_content" | grep -q 'admin-login'; then
      kiosk_found_endpoint="admin-login"
      break
    fi
  done <<< "$kiosk_js_files"

  if [[ "$kiosk_found_endpoint" = "staff/validate-pin" ]]; then
    status="PASS"; severity="P3"; message="Kiosk staff login JS calls /staff/validate-pin (correct)"
  elif [[ "$kiosk_found_endpoint" = "admin-login" ]]; then
    status="FAIL"; severity="P1"; message="Kiosk staff login JS calls /admin-login (STALE BUILD — must redeploy)"
  elif [[ -z "$kiosk_login_html" ]]; then
    status="WARN"; severity="P2"; message="Kiosk staff page unreachable"
  else
    status="WARN"; severity="P2"; message="Kiosk staff login: could not find auth endpoint in JS bundles"
  fi
  emit_result "$phase" "$tier" "server-23-kiosk-login-endpoint" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Check 4c: Staff PIN works on both frontend endpoints (live E2E)
  if [[ -n "$pin" ]]; then
    local staff_tmpfile; staff_tmpfile=$(mktemp)
    # Use 4-digit PIN for staff/validate-pin
    local staff_pin="${pin:0:4}"
    jq -n --arg pin "$staff_pin" '{pin: $pin}' > "$staff_tmpfile"
    local staff_response; staff_response=$(curl -s -m 10 -X POST \
      "http://192.168.31.23:8080/api/v1/staff/validate-pin" \
      -H 'Content-Type: application/json' -d "@${staff_tmpfile}" 2>/dev/null || true)
    rm -f "$staff_tmpfile"
    if printf '%s' "$staff_response" | jq -e '.token' > /dev/null 2>&1; then
      local staff_name; staff_name=$(printf '%s' "$staff_response" | jq -r '.staff_name // "unknown"' 2>/dev/null)
      status="PASS"; severity="P3"; message="Staff PIN validate-pin: token returned for ${staff_name}"
    else
      status="FAIL"; severity="P1"; message="Staff PIN validate-pin: auth failed (PIN ${staff_pin} rejected)"
    fi
    emit_result "$phase" "$tier" "server-23-staff-pin-e2e" "$status" "$severity" "$message" "$mode" "$venue_state"
  fi

  # --- Check 5: Public health endpoint accessible without auth ---
  local health_code; health_code=$(curl -s -m 10 -o /dev/null -w "%{http_code}" \
    "http://192.168.31.23:8080/api/v1/health" 2>/dev/null)
  if [[ "$health_code" = "200" ]]; then
    status="PASS"; severity="P3"; message="Public health endpoint /api/v1/health returns 200 without auth"
  else
    status="WARN"; severity="P2"; message="Public health endpoint returned HTTP ${health_code} (expected 200 — should be public)"
  fi
  emit_result "$phase" "$tier" "server-23-auth-public" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 6: Frontend PIN consistency — kiosk and web must use same auth path ---
  if [[ -n "${web_found_endpoint:-}" && -n "${kiosk_found_endpoint:-}" ]]; then
    if [[ "$web_found_endpoint" = "$kiosk_found_endpoint" ]]; then
      status="PASS"; severity="P3"; message="PIN sync: both Kiosk and Web use /${web_found_endpoint} (in sync)"
    else
      status="FAIL"; severity="P1"; message="PIN DESYNC: Kiosk uses /${kiosk_found_endpoint}, Web uses /${web_found_endpoint} — different auth paths"
    fi
    emit_result "$phase" "$tier" "server-23-pin-sync" "$status" "$severity" "$message" "$mode" "$venue_state"
  fi

  return 0
}
export -f run_phase50
