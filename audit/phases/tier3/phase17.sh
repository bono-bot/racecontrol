#!/usr/bin/env bash
# audit/phases/tier3/phase17.sh -- Phase 17: Lock Screen & Blanking
# Tier: 3 (Display & UX) -- ALL checks QUIET when venue closed
# What: Every idle pod shows correct lock/blanking screen (Edge/kiosk as foreground).
# Standing rule: Phase scripts always exit 0 -- errors encoded in emit_result status, never bash exit code.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase17() {
  local phase="17" tier="3"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  local ip host
  for ip in $PODS; do
    host="pod-$(printf '%s' "$ip" | sed 's/192\.168\.31\.//')"

    # QUIET when venue closed -- display checks irrelevant when closed
    if [[ "$venue_state" = "closed" ]]; then
      emit_result "$phase" "$tier" "${host}-lockscreen" "QUIET" "P3" \
        "Lock screen check skipped -- venue closed" "$mode" "$venue_state"
      continue
    fi

    # Check Edge/kiosk foreground process
    response=$(safe_remote_exec "$ip" "8090" \
      'tasklist /V /FO CSV /NH | findstr /C:"kiosk" /C:"Edge" /C:"chrome"' \
      "$DEFAULT_TIMEOUT")
    local proc_out; proc_out=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null || true)
    if printf '%s' "$proc_out" | grep -qi "edge\|kiosk\|chrome"; then
      # Check Edge count (> 5 = stacking bug)
      local edge_count_resp; edge_count_resp=$(safe_remote_exec "$ip" "8090" \
        'tasklist /NH | find /C "msedge.exe"' \
        "$DEFAULT_TIMEOUT")
      local edge_count; edge_count=$(printf '%s' "$edge_count_resp" | jq -r '.stdout // "0"' 2>/dev/null | tr -d '[:space:]' | grep -oE '^[0-9]+' || echo "0")
      if [[ "${edge_count:-0}" -gt 5 ]]; then
        status="WARN"; severity="P2"; message="Edge stacking: ${edge_count} msedge.exe processes (> 5)"
      else
        status="PASS"; severity="P3"; message="Kiosk/Edge running as foreground (${edge_count:-?} processes)"
      fi
    else
      status="WARN"; severity="P2"; message="No Edge/kiosk process found -- lock screen may not be showing"
    fi
    emit_result "$phase" "$tier" "${host}-lockscreen" "$status" "$severity" "$message" "$mode" "$venue_state"
  done

  return 0
}
export -f run_phase17
