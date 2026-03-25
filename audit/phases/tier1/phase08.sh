#!/usr/bin/env bash
# audit/phases/tier1/phase08.sh -- Phase 08: Sentinel Files & Stale State
# Tier: 1 (Infrastructure Foundation)
# What: No stale MAINTENANCE_MODE, GRACEFUL_RELAUNCH, or restart sentinels on pods.
# Standing rule: Phase scripts always exit 0 -- errors encoded in emit_result status, never bash exit code.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase08() {
  local phase="08" tier="1"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  local ip host
  for ip in $PODS; do
    host="pod-$(printf '%s' "$ip" | sed 's/192\.168\.31\.//')"
    # Use rc-sentry (:8091) per standing rule -- sentry can exec even when rc-agent is in maintenance
    response=$(safe_remote_exec "$ip" "8091" \
      'dir C:\RacingPoint\MAINTENANCE_MODE C:\RacingPoint\GRACEFUL_RELAUNCH C:\RacingPoint\rcagent-restart-sentinel.txt 2>nul || echo CLEAN' \
      "$DEFAULT_TIMEOUT")
    local sentinel_out; sentinel_out=$(printf '%s' "$response" | jq -r '.stdout // "CLEAN"' 2>/dev/null || echo "CLEAN")
    if printf '%s' "$sentinel_out" | grep -qi "CLEAN"; then
      status="PASS"; severity="P3"; message="No stale sentinel files on pod"
    elif printf '%s' "$sentinel_out" | grep -qi "MAINTENANCE_MODE"; then
      status="FAIL"; severity="P1"; message="MAINTENANCE_MODE sentinel present -- rc-agent locked out (silent pod killer)"
    else
      status="WARN"; severity="P2"; message="Unexpected sentinel file: ${sentinel_out:0:80}"
    fi
    if [[ "$venue_state" = "closed" ]] && [[ "$status" = "WARN" ]]; then
      status="QUIET"; severity="P3"
    fi
    emit_result "$phase" "$tier" "${host}-sentinels" "$status" "$severity" "$message" "$mode" "$venue_state"
  done

  # Server .23 -- MAINTENANCE_MODE (should not exist on server)
  response=$(safe_remote_exec "192.168.31.23" "8090" \
    'dir C:\RacingPoint\MAINTENANCE_MODE 2>nul || echo CLEAN' \
    "$DEFAULT_TIMEOUT")
  local server_sentinel; server_sentinel=$(printf '%s' "$response" | jq -r '.stdout // "CLEAN"' 2>/dev/null || echo "CLEAN")
  if printf '%s' "$server_sentinel" | grep -qi "CLEAN"; then
    status="PASS"; severity="P3"; message="No MAINTENANCE_MODE on server .23"
  else
    status="WARN"; severity="P2"; message="MAINTENANCE_MODE on server .23 -- investigate"
  fi
  emit_result "$phase" "$tier" "server-23-sentinels" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase08
