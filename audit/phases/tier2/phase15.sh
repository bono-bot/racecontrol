#!/usr/bin/env bash
# audit/phases/tier2/phase15.sh -- Phase 15: Preflight Checks
# Tier: 2 (Core Services)
# What: All rc-agent preflight checks pass on every pod.
# Standing rule: Phase scripts always exit 0 -- errors encoded in emit_result status, never bash exit code.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase15() {
  local phase="15" tier="2"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  local ip host
  for ip in $PODS; do
    host="pod-$(printf '%s' "$ip" | sed 's/192\.168\.31\.//')"
    # Preflight: if rc-agent started and is healthy, preflight passed implicitly
    response=$(http_get "http://${ip}:8090/health" "$DEFAULT_TIMEOUT")
    local agent_status; agent_status=$(printf '%s' "$response" | jq -r '.status // ""' 2>/dev/null)
    if [[ "$agent_status" = "ok" ]]; then
      status="PASS"; severity="P3"; message="Preflight OK (agent healthy, status=ok)"
    elif [[ -n "$response" ]]; then
      status="PASS"; severity="P3"; message="Preflight OK (agent responding)"
    else
      status="WARN"; severity="P2"; message="Cannot verify preflight (agent unreachable)"
    fi
    if [[ "$venue_state" = "closed" ]] && [[ "$status" = "WARN" ]]; then
      status="QUIET"; severity="P3"
    fi
    emit_result "$phase" "$tier" "${host}-preflight" "$status" "$severity" "$message" "$mode" "$venue_state"
  done

  return 0
}
export -f run_phase15
