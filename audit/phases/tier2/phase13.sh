#!/usr/bin/env bash
# audit/phases/tier2/phase13.sh -- Phase 13: rc-agent Exec Capability
# Tier: 2 (Core Services)
# What: Every pod can execute commands via rc-agent :8090.
# Standing rule: Phase scripts always exit 0 -- errors encoded in emit_result status, never bash exit code.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase13() {
  local phase="13" tier="2"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  local ip host
  for ip in $PODS; do
    host="pod-$(printf '%s' "$ip" | sed 's/192\.168\.31\.//')"

    # Exec test: hostname command
    response=$(safe_remote_exec "$ip" "8090" "hostname" "$DEFAULT_TIMEOUT")
    local exit_code; exit_code=$(printf '%s' "$response" | jq -r '.exit_code // 1' 2>/dev/null)
    local stdout; stdout=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null | tr -d '[:space:]' || true)
    if [[ "${exit_code:-1}" -eq 0 ]] && [[ -n "$stdout" ]]; then
      status="PASS"; severity="P3"; message="Exec capability OK, hostname=${stdout:0:20}"
    elif [[ -z "$(printf '%s' "$response" | tr -d '[:space:]')" ]]; then
      status="FAIL"; severity="P1"; message="rc-agent exec unreachable (pod offline)"
    else
      status="WARN"; severity="P2"; message="Exec returned exit_code=${exit_code}, stdout empty"
    fi
    if [[ "$venue_state" = "closed" ]] && [[ "$status" = "FAIL" || "$status" = "WARN" ]]; then
      status="QUIET"; severity="P3"
    fi
    emit_result "$phase" "$tier" "${host}-exec" "$status" "$severity" "$message" "$mode" "$venue_state"

    # Exec slots available (health endpoint)
    local health; health=$(http_get "http://${ip}:8090/health" "$DEFAULT_TIMEOUT")
    local slots; slots=$(printf '%s' "$health" | jq -r '.exec_slots_available // "unknown"' 2>/dev/null || echo "unknown")
    if [[ "$slots" = "unknown" || -z "$slots" ]]; then
      status="WARN"; severity="P2"; message="exec_slots_available not in health response"
    elif [[ "${slots:-0}" -eq 0 ]]; then
      status="WARN"; severity="P2"; message="exec_slots_available=0 -- exec queue exhausted"
    else
      status="PASS"; severity="P3"; message="exec_slots_available=${slots}"
    fi
    if [[ "$venue_state" = "closed" ]] && [[ "$status" = "WARN" ]]; then
      status="QUIET"; severity="P3"
    fi
    emit_result "$phase" "$tier" "${host}-exec-slots" "$status" "$severity" "$message" "$mode" "$venue_state"
  done

  return 0
}
export -f run_phase13
