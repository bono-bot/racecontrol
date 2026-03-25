#!/usr/bin/env bash
# audit/phases/tier1/phase09.sh -- Phase 09: Self-Monitor & Self-Heal
# Tier: 1 (Infrastructure Foundation)
# What: rc-agent self_monitor, self_heal, failure_monitor active on pods.
# Standing rule: Phase scripts always exit 0 -- errors encoded in emit_result status, never bash exit code.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase09() {
  local phase="09" tier="1"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  local ip host
  for ip in $PODS; do
    host="pod-$(printf '%s' "$ip" | sed 's/192\.168\.31\.//')"

    # Check for self_monitor heartbeat in logs
    response=$(safe_remote_exec "$ip" "8090" \
      'findstr /C:"self_monitor" C:\RacingPoint\rc-agent-*.jsonl 2>nul | findstr /C:"heartbeat" | findstr /V /C:"debug"' \
      "$DEFAULT_TIMEOUT")
    local hb_out; hb_out=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null || true)
    if [[ -n "$hb_out" ]]; then
      status="PASS"; severity="P3"; message="self_monitor heartbeat found in logs"
    else
      status="WARN"; severity="P2"; message="No self_monitor heartbeat in logs (pod may have just started or logs rotated)"
    fi
    if [[ "$venue_state" = "closed" ]] && [[ "$status" = "WARN" ]]; then
      status="QUIET"; severity="P3"
    fi
    emit_result "$phase" "$tier" "${host}-self-monitor" "$status" "$severity" "$message" "$mode" "$venue_state"

    # Check for safe_mode active (must NOT be active)
    response=$(safe_remote_exec "$ip" "8090" \
      'findstr /C:"safe_mode" /C:"SAFE_MODE" C:\RacingPoint\rc-agent-*.jsonl 2>nul | findstr /V /C:"disabled"' \
      "$DEFAULT_TIMEOUT")
    local sm_out; sm_out=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null | tr -d '[:space:]' || true)
    if [[ -z "$sm_out" ]]; then
      status="PASS"; severity="P3"; message="safe_mode not active on pod"
    else
      status="WARN"; severity="P2"; message="safe_mode references found in logs -- verify mode is disabled"
    fi
    if [[ "$venue_state" = "closed" ]] && [[ "$status" = "WARN" ]]; then
      status="QUIET"; severity="P3"
    fi
    emit_result "$phase" "$tier" "${host}-safe-mode" "$status" "$severity" "$message" "$mode" "$venue_state"
  done

  return 0
}
export -f run_phase09
