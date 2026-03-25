#!/usr/bin/env bash
# audit/phases/tier5/phase29.sh -- Phase 29: Multiplayer & Friends
# Tier: 5 (Games & Hardware)
# What: Multiplayer sessions endpoint, friends system functional.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase29() {
  local phase="29" tier="5"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message
  local token; token=$(get_session_token)

  # Multiplayer endpoint
  response=$(curl -s -m "$DEFAULT_TIMEOUT" \
    "http://192.168.31.23:8080/api/v1/multiplayer" \
    -H "x-terminal-session: ${token:-}" 2>/dev/null | tr -d '"')
  if [[ -n "$response" ]]; then
    status="PASS"; severity="P3"; message="Multiplayer endpoint responding"
  else
    status="WARN"; severity="P2"; message="Multiplayer endpoint unreachable or not implemented"
  fi
  emit_result "$phase" "$tier" "server-23-multiplayer" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Friends endpoint
  response=$(curl -s -m "$DEFAULT_TIMEOUT" \
    "http://192.168.31.23:8080/api/v1/friends" \
    -H "x-terminal-session: ${token:-}" 2>/dev/null | tr -d '"')
  if [[ -n "$response" ]]; then
    status="PASS"; severity="P3"; message="Friends endpoint responding"
  else
    status="WARN"; severity="P2"; message="Friends endpoint unreachable or not implemented"
  fi
  emit_result "$phase" "$tier" "server-23-friends" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Multiplayer errors in logs
  local log_resp; log_resp=$(http_get "http://192.168.31.23:8080/api/v1/logs?lines=30" "$DEFAULT_TIMEOUT")
  if [[ -n "$log_resp" ]]; then
    local mp_err; mp_err=$(printf '%s' "$log_resp" | jq -r '.' 2>/dev/null | grep -ci "multiplayer.*error" || echo "0")
    if [[ "${mp_err:-0}" -eq 0 ]]; then
      status="PASS"; severity="P3"; message="No multiplayer errors in recent logs"
    else
      status="WARN"; severity="P2"; message="${mp_err} multiplayer error entries in logs"
    fi
  else
    status="WARN"; severity="P2"; message="Logs API unreachable — cannot check multiplayer errors"
  fi
  emit_result "$phase" "$tier" "server-23-multiplayer-errors" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase29
