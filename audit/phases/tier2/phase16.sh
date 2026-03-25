#!/usr/bin/env bash
# audit/phases/tier2/phase16.sh -- Phase 16: Cascade Guard & Recovery
# Tier: 2 (Core Services)
# What: Cascade guard preventing fleet-wide failures. Recovery paths functional.
# Standing rule: Phase scripts always exit 0 -- errors encoded in emit_result status, never bash exit code.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase16() {
  local phase="16" tier="2"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # Cascade guard logs -- look for trigger count
  response=$(http_get "http://192.168.31.23:8080/api/v1/logs?lines=100" "$DEFAULT_TIMEOUT")
  if [[ -n "$response" ]]; then
    local cascade_count; cascade_count=$(printf '%s' "$response" | jq -r '.' 2>/dev/null | grep -ci "cascade_guard")
    if [[ "${cascade_count:-0}" -eq 0 ]]; then
      status="PASS"; severity="P3"; message="No cascade_guard triggers in recent logs"
    elif [[ "${cascade_count:-0}" -le 3 ]]; then
      status="WARN"; severity="P2"; message="cascade_guard triggered ${cascade_count} times in recent logs (threshold: > 3/day)"
    else
      status="FAIL"; severity="P2"; message="cascade_guard firing frequently: ${cascade_count} times -- investigate root cause"
    fi
  else
    status="WARN"; severity="P2"; message="Logs API unreachable -- cannot check cascade guard"
  fi
  emit_result "$phase" "$tier" "server-23-cascade-guard" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Pod healer -- check for recent healing
  if [[ -n "$response" ]]; then
    local healer_count; healer_count=$(printf '%s' "$response" | jq -r '.' 2>/dev/null | grep -ci "pod_healer")
    if [[ "${healer_count:-0}" -eq 0 ]]; then
      status="PASS"; severity="P3"; message="No pod_healer actions in recent logs"
    else
      status="WARN"; severity="P2"; message="pod_healer active: ${healer_count} entries in recent logs (review if loop)"
    fi
  else
    status="WARN"; severity="P2"; message="Logs API unreachable -- cannot check pod healer"
  fi
  emit_result "$phase" "$tier" "server-23-pod-healer" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase16
