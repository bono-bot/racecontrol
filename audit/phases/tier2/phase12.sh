#!/usr/bin/env bash
# audit/phases/tier2/phase12.sh -- Phase 12: WebSocket Flows
# Tier: 2 (Core Services)
# What: WS endpoints exist (400 = upgrade required, not 404). ws_connected status for all pods.
# Standing rule: Phase scripts always exit 0 -- errors encoded in emit_result status, never bash exit code.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase12() {
  local phase="12" tier="2"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # Dashboard WS endpoint -- must return 400 (upgrade required), not 404
  local ws_code; ws_code=$(curl -s -o /dev/null -w "%{http_code}" \
    "http://192.168.31.23:8080/ws/dashboard" 2>/dev/null)
  if [[ "$ws_code" = "400" || "$ws_code" = "101" ]]; then
    status="PASS"; severity="P3"; message="Dashboard WS endpoint present (HTTP ${ws_code})"
  elif [[ "$ws_code" = "404" ]]; then
    status="FAIL"; severity="P1"; message="Dashboard WS endpoint 404 -- not registered"
  else
    status="WARN"; severity="P2"; message="Dashboard WS HTTP ${ws_code} (expected 400 upgrade-required)"
  fi
  emit_result "$phase" "$tier" "server-23-ws-dashboard" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Agent WS endpoint -- must return 400 (upgrade required), not 404
  ws_code=$(curl -s -o /dev/null -w "%{http_code}" \
    "http://192.168.31.23:8080/ws/agent" 2>/dev/null)
  if [[ "$ws_code" = "400" || "$ws_code" = "101" ]]; then
    status="PASS"; severity="P3"; message="Agent WS endpoint present (HTTP ${ws_code})"
  elif [[ "$ws_code" = "404" ]]; then
    status="FAIL"; severity="P1"; message="Agent WS endpoint 404 -- not registered"
  else
    status="WARN"; severity="P2"; message="Agent WS HTTP ${ws_code} (expected 400 upgrade-required)"
  fi
  emit_result "$phase" "$tier" "server-23-ws-agent" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Fleet health -- ws_connected must be true for powered-on pods
  response=$(http_get "http://192.168.31.23:8080/api/v1/fleet/health" "$DEFAULT_TIMEOUT")
  if [[ -n "$response" ]]; then
    local ws_false; ws_false=$(printf '%s' "$response" | \
      jq '[.[] | select(.ws_connected==false or .ws_connected==null)] | length' 2>/dev/null)
    if [[ "${ws_false:-0}" -eq 0 ]]; then
      status="PASS"; severity="P3"; message="All pods ws_connected=true"
    elif [[ "${ws_false:-0}" -le 2 ]]; then
      status="WARN"; severity="P2"; message="${ws_false} pod(s) with ws_connected=false (may be offline pods)"
    else
      status="FAIL"; severity="P2"; message="${ws_false} pods ws_connected=false -- WS connectivity degraded"
    fi
  else
    status="WARN"; severity="P2"; message="Fleet health unreachable -- cannot verify ws_connected"
  fi
  if [[ "$venue_state" = "closed" ]] && [[ "$status" = "WARN" ]]; then
    status="QUIET"; severity="P3"
  fi
  emit_result "$phase" "$tier" "server-23-ws-connected" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase12
