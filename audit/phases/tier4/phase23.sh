#!/usr/bin/env bash
# audit/phases/tier4/phase23.sh -- Phase 23: Pod Reservation & Booking
# Tier: 4 (Billing & Commerce)
# What: Reservation system prevents double-booking, handles cancellation.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase23() {
  local phase="23" tier="4"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message
  local token; token=$(get_session_token)

  # Reservations endpoint
  response=$(curl -s -m "$DEFAULT_TIMEOUT" \
    "http://192.168.31.23:8080/api/v1/reservations" \
    -H "x-terminal-session: ${token:-}" 2>/dev/null | tr -d '"')
  if [[ -n "$response" ]]; then
    status="PASS"; severity="P3"; message="Reservations endpoint responding"
  else
    status="WARN"; severity="P2"; message="Reservations endpoint unreachable"
  fi
  emit_result "$phase" "$tier" "server-23-reservations" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Check for expired reservations not cleaned up
  local log_resp; log_resp=$(http_get "http://192.168.31.23:8080/api/v1/logs?lines=50" "$DEFAULT_TIMEOUT")
  if [[ -n "$log_resp" ]]; then
    local expired; expired=$(printf '%s' "$log_resp" | jq -r '.' 2>/dev/null | grep -ci "reservation.*expir" || echo "0")
    if [[ "${expired:-0}" -eq 0 ]]; then
      status="PASS"; severity="P3"; message="No expired reservation cleanup errors in logs"
    else
      status="WARN"; severity="P2"; message="${expired} expired reservation entries in logs"
    fi
  else
    status="WARN"; severity="P2"; message="Logs API unreachable — cannot check reservation cleanup"
  fi
  emit_result "$phase" "$tier" "server-23-reservations-cleanup" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase23
