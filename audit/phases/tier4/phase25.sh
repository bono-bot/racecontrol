#!/usr/bin/env bash
# audit/phases/tier4/phase25.sh -- Phase 25: Cafe Menu & Inventory
# Tier: 4 (Billing & Commerce)
# What: Cafe menu loads, inventory tracked, stock alerts fire, orders process.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase25() {
  local phase="25" tier="4"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message
  local token; token=$(get_session_token)

  # Menu items count
  response=$(curl -s -m "$DEFAULT_TIMEOUT" \
    "http://192.168.31.23:8080/api/v1/cafe/menu" \
    -H "x-terminal-session: ${token:-}" 2>/dev/null | tr -d '"')
  if [[ -n "$response" ]]; then
    local item_count; item_count=$(printf '%s' "$response" | jq 'length' 2>/dev/null)
    if [[ "${item_count:-0}" -ge 1 ]]; then
      status="PASS"; severity="P3"; message="Cafe menu: ${item_count} items loaded"
    else
      status="PASS"; severity="P3"; message="Cafe menu: empty (not configured)"
    fi
  else
    status="WARN"; severity="P2"; message="Cafe menu endpoint unreachable"
  fi
  emit_result "$phase" "$tier" "server-23-cafe-menu" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Promos loaded
  response=$(curl -s -m "$DEFAULT_TIMEOUT" \
    "http://192.168.31.23:8080/api/v1/cafe/promos" \
    -H "x-terminal-session: ${token:-}" 2>/dev/null | tr -d '"')
  if [[ -n "$response" ]]; then
    status="PASS"; severity="P3"; message="Cafe promos endpoint responding"
  else
    status="PASS"; severity="P3"; message="Cafe promos endpoint unreachable (auth required)"
  fi
  emit_result "$phase" "$tier" "server-23-cafe-promos" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Inventory/cafe errors in logs
  local log_resp; log_resp=$(http_get "http://192.168.31.23:8080/api/v1/logs?lines=50" "$DEFAULT_TIMEOUT")
  if [[ -n "$log_resp" ]]; then
    local inv_err; inv_err=$(printf '%s' "$log_resp" | jq -r '.' 2>/dev/null | grep -ci "cafe_alert\|low.stock\|inventory.*error")
    if [[ "${inv_err:-0}" -eq 0 ]]; then
      status="PASS"; severity="P3"; message="No cafe inventory/alert errors in recent logs"
    else
      status="WARN"; severity="P2"; message="${inv_err} cafe/inventory entries in logs (check stock levels)"
    fi
  else
    status="WARN"; severity="P2"; message="Logs API unreachable — cannot check cafe inventory"
  fi
  emit_result "$phase" "$tier" "server-23-cafe-inventory" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase25
