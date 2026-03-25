#!/usr/bin/env bash
# audit/phases/tier17/phase59.sh -- Phase 59: Customer Flow E2E
# Tier: 17 (Customer & Staff Flow E2E)
# What: Complete customer flows that cross system boundaries — QR registration, PIN redeem, cafe order.
# Standing rules: customer-facing endpoints must return correct status codes (not 500)

set -u
set -o pipefail
# NO set -e

run_phase59() {
  local phase="59" tier="17"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # --- Check 1: QR Registration page loads (/register) ---
  local qr_response; qr_response=$(curl -s -m 10 "http://192.168.31.23:8080/register" 2>/dev/null || echo "")
  local qr_count; qr_count=$(printf '%s' "$qr_response" | grep -ci "html\|DOCTYPE\|register" 2>/dev/null)
  qr_count="${qr_count//[[:space:]]/}"
  if [[ "${qr_count:-0}" -gt 0 ]] 2>/dev/null; then
    status="PASS"; severity="P3"; message="QR registration page /register loads with HTML content"
  elif [[ -z "$qr_response" ]]; then
    status="WARN"; severity="P2"; message="QR registration page /register: no response from server (endpoint may not exist in current version)"
  else
    status="WARN"; severity="P2"; message="QR registration page /register: response did not contain html/DOCTYPE/register keywords"
  fi
  emit_result "$phase" "$tier" "server-23-qr-register" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 2: PIN Redeem endpoint (invalid PIN should return 400 or 404, not 500) ---
  local pin_tmpfile; pin_tmpfile=$(mktemp /tmp/audit-pin-XXXXXX.json)
  jq -n '{"pin":"000000"}' > "$pin_tmpfile" 2>/dev/null
  local pin_status; pin_status=$(curl -s -m 10 \
    -o /dev/null \
    -w "%{http_code}" \
    -X POST \
    -H "Content-Type: application/json" \
    "http://192.168.31.23:8080/api/v1/customer/redeem-pin" \
    -d @"$pin_tmpfile" 2>/dev/null)
  rm -f "$pin_tmpfile"
  pin_status="${pin_status//[[:space:]]/}"

  if [[ "$pin_status" = "400" || "$pin_status" = "404" || "$pin_status" = "422" ]]; then
    status="PASS"; severity="P3"; message="PIN redeem endpoint returns ${pin_status} for invalid PIN (expected error, not 500)"
  elif [[ "$pin_status" = "500" ]]; then
    status="FAIL"; severity="P1"; message="PIN redeem endpoint returned 500 (server error) for invalid PIN 000000"
  elif [[ "$pin_status" = "000" ]]; then
    status="WARN"; severity="P2"; message="PIN redeem endpoint unreachable (curl error/timeout)"
  else
    status="WARN"; severity="P2"; message="PIN redeem endpoint returned unexpected status ${pin_status} for invalid PIN 000000"
  fi
  emit_result "$phase" "$tier" "server-23-pin-redeem" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 3: Cafe menu endpoint (should return array of menu items) ---
  local token; token=$(get_session_token 2>/dev/null || echo "")
  local menu_response; menu_response=$(curl -s -m 10 \
    -H "x-terminal-session: ${token:-}" \
    "http://192.168.31.23:8080/api/v1/cafe/menu" 2>/dev/null || echo "")
  local menu_length; menu_length=$(printf '%s' "$menu_response" | jq 'length' 2>/dev/null)
  menu_length="${menu_length//[[:space:]]/}"

  if [[ "${menu_length:-0}" -gt 0 ]] 2>/dev/null; then
    status="PASS"; severity="P3"; message="Cafe menu endpoint returned ${menu_length} items"
  elif [[ -z "$menu_response" ]]; then
    status="WARN"; severity="P2"; message="Cafe menu endpoint: no response (endpoint may require auth or not exist)"
  else
    status="WARN"; severity="P2"; message="Cafe menu endpoint returned empty array or non-array response — no menu items seeded or auth required"
  fi
  emit_result "$phase" "$tier" "server-23-cafe-menu" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 4: Cafe order validation (empty items should return 400, not 500) ---
  local order_tmpfile; order_tmpfile=$(mktemp /tmp/audit-order-XXXXXX.json)
  jq -n '{"items":[],"payment_method":"cash"}' > "$order_tmpfile" 2>/dev/null
  local order_status; order_status=$(curl -s -m 10 \
    -o /dev/null \
    -w "%{http_code}" \
    -X POST \
    -H "Content-Type: application/json" \
    -H "x-terminal-session: ${token:-}" \
    "http://192.168.31.23:8080/api/v1/cafe/orders" \
    -d @"$order_tmpfile" 2>/dev/null)
  rm -f "$order_tmpfile"
  order_status="${order_status//[[:space:]]/}"

  if [[ "$order_status" = "400" || "$order_status" = "422" ]]; then
    status="PASS"; severity="P3"; message="Cafe order endpoint returns ${order_status} for empty items (expected validation error, not 500)"
  elif [[ "$order_status" = "500" ]]; then
    status="FAIL"; severity="P1"; message="Cafe order endpoint returned 500 (server error) for empty items payload"
  elif [[ "$order_status" = "000" ]]; then
    status="WARN"; severity="P2"; message="Cafe order endpoint unreachable (curl error/timeout)"
  else
    status="WARN"; severity="P2"; message="Cafe order endpoint returned unexpected status ${order_status} for empty items (expect 400/422)"
  fi
  emit_result "$phase" "$tier" "server-23-cafe-order" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase59
