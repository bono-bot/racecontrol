#!/usr/bin/env bash
# audit/phases/tier1/phase05.sh -- Phase 05: Pod Power & WoL
# Tier: 1 (Infrastructure Foundation)
# What: All 8 pods powered on. Uptime checked for unexpected reboots.
# Standing rule: Phase scripts always exit 0 -- errors encoded in emit_result status, never bash exit code.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase05() {
  local phase="05" tier="1"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  local ip host
  for ip in $PODS; do
    host="pod-$(printf '%s' "$ip" | sed 's/192\.168\.31\.//')"
    response=$(http_get "http://${ip}:8090/health" "$DEFAULT_TIMEOUT")
    if [[ -n "$response" ]] && printf '%s' "$response" | grep -q "build_id"; then
      # Check uptime for unexpected recent reboots (< 300s)
      local uptime_secs; uptime_secs=$(printf '%s' "$response" | jq -r '.uptime_secs // "9999"' 2>/dev/null)
      if [[ "${uptime_secs:-9999}" -lt 300 ]]; then
        status="WARN"; severity="P2"; message="Pod UP but recently rebooted: uptime=${uptime_secs}s (< 5 min)"
      else
        status="PASS"; severity="P3"; message="Pod UP, uptime=${uptime_secs}s"
      fi
    else
      status="FAIL"; severity="P1"; message="Pod DOWN -- not responding on :8090"
    fi
    if [[ "$venue_state" = "closed" ]] && [[ "$status" = "FAIL" || "$status" = "WARN" ]]; then
      status="QUIET"; severity="P3"
    fi
    emit_result "$phase" "$tier" "${host}-power" "$status" "$severity" "$message" "$mode" "$venue_state"
  done

  return 0
}
export -f run_phase05
