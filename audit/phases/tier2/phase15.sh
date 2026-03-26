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

    # First pass: agent reachability
    response=$(http_get "http://${ip}:8090/health" "$DEFAULT_TIMEOUT")
    if [[ -z "$response" ]]; then
      status="WARN"; severity="P2"; message="Cannot verify preflight (agent unreachable)"
      if [[ "$venue_state" = "closed" ]] && [[ "$status" = "WARN" ]]; then
        status="QUIET"; severity="P3"
      fi
      emit_result "$phase" "$tier" "${host}-preflight" "$status" "$severity" "$message" "$mode" "$venue_state"
      continue
    fi

    # WL-03: Check preflight subsystem fields, not just status=ok
    local preflight_val; preflight_val=$(printf '%s' "$response" | jq -r '.preflight_passed // .preflight // empty' 2>/dev/null)
    if [[ "$preflight_val" = "true" ]]; then
      status="PASS"; severity="P3"; message="Preflight subsystem passed (preflight_passed=true)"
    elif [[ "$preflight_val" = "false" ]]; then
      status="WARN"; severity="P2"; message="Preflight subsystem reports failure on pod"
    elif [[ -n "$preflight_val" ]]; then
      # Non-boolean preflight value -- treat as present
      status="PASS"; severity="P3"; message="Preflight subsystem present (value: ${preflight_val})"
    else
      # Legacy binary: field not in response, fall back to status=ok
      local agent_status; agent_status=$(printf '%s' "$response" | jq -r '.status // ""' 2>/dev/null)
      if [[ "$agent_status" = "ok" ]]; then
        status="PASS"; severity="P3"; message="Preflight field not in health response (legacy binary) -- falling back to status=ok"
      else
        status="WARN"; severity="P2"; message="Preflight field missing and status != ok (agent_status=${agent_status})"
      fi
    fi

    # Sub-check: MAINTENANCE_MODE sentinel file
    local maint_response; maint_response=$(safe_remote_exec "$ip" "8090" \
      'if exist C:\RacingPoint\MAINTENANCE_MODE (echo MAINTENANCE) else (echo CLEAR)' \
      "$DEFAULT_TIMEOUT")
    local maint_out; maint_out=$(printf '%s' "$maint_response" | jq -r '.stdout // ""' 2>/dev/null | tr -d '\r\n' | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')
    if [[ "$maint_out" = "MAINTENANCE" ]]; then
      status="WARN"; severity="P2"; message="MAINTENANCE_MODE active -- preflight blocked"
    fi

    if [[ "$venue_state" = "closed" ]] && [[ "$status" = "WARN" ]]; then
      status="QUIET"; severity="P3"
    fi
    emit_result "$phase" "$tier" "${host}-preflight" "$status" "$severity" "$message" "$mode" "$venue_state"
  done

  return 0
}
export -f run_phase15
