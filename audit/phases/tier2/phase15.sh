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
    response=$(safe_remote_exec "$ip" "8090" \
      'findstr /C:"preflight" /C:"FAIL" C:\RacingPoint\rc-agent-*.jsonl 2>nul' \
      "$DEFAULT_TIMEOUT")
    local pf_out; pf_out=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null || true)
    local fail_count; fail_count=$(printf '%s' "$pf_out" | grep -c "FAIL" 2>/dev/null)
    if [[ "${fail_count:-0}" -eq 0 ]]; then
      if [[ -z "$pf_out" ]]; then
        status="WARN"; severity="P2"; message="No preflight log entries found (logs may be rotated or pod just started)"
      else
        status="PASS"; severity="P3"; message="No FAIL entries in preflight logs"
      fi
    else
      status="WARN"; severity="P2"; message="${fail_count} preflight FAIL entries in logs -- check DISP/NET/HW checks"
    fi
    if [[ "$venue_state" = "closed" ]] && [[ "$status" = "WARN" ]]; then
      status="QUIET"; severity="P3"
    fi
    emit_result "$phase" "$tier" "${host}-preflight" "$status" "$severity" "$message" "$mode" "$venue_state"
  done

  return 0
}
export -f run_phase15
