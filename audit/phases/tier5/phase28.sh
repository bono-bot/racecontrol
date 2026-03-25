#!/usr/bin/env bash
# audit/phases/tier5/phase28.sh -- Phase 28: FFB & Hardware Detection
# Tier: 5 (Games & Hardware) -- QUIET when venue closed
# What: Wheelbase USB detected (VID:1209 PID:FFB0), driving_detector active.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase28() {
  local phase="28" tier="5"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  local ip host
  for ip in $PODS; do
    host="pod-$(printf '%s' "$ip" | sed 's/192\.168\.31\.//')"

    if [[ "$venue_state" = "closed" ]]; then
      emit_result "$phase" "$tier" "${host}-wheelbase" "QUIET" "P3" \
        "Hardware check skipped — venue closed" "$mode" "$venue_state"
      continue
    fi

    # HW-01: Wheelbase USB detection (VID:1209 PID:FFB0 = Conspit Ares 8Nm OpenFFBoard)
    response=$(safe_remote_exec "$ip" "8090" \
      'wmic path Win32_PnPEntity where "DeviceID like ''%1209%FFB0%''" get Name /value 2>nul || echo NO_WHEELBASE' \
      "$DEFAULT_TIMEOUT")
    local wb_out; wb_out=$(printf '%s' "$response" | jq -r '.stdout // "NO_WHEELBASE"' 2>/dev/null || echo "NO_WHEELBASE")
    if printf '%s' "$wb_out" | grep -qi "NO_WHEELBASE\|no instance"; then
      status="WARN"; severity="P2"; message="Wheelbase USB (VID:1209 PID:FFB0) not detected — may need physical check"
    elif [[ -z "$(printf '%s' "$wb_out" | tr -d '[:space:]')" ]]; then
      status="WARN"; severity="P2"; message="Could not query PnP devices (pod offline or wmic failed)"
    else
      status="PASS"; severity="P3"; message="Wheelbase USB detected: $(printf '%s' "$wb_out" | grep -i "Name" | head -1 | tr -d '[:space:]' | cut -c1-40)"
    fi
    emit_result "$phase" "$tier" "${host}-wheelbase" "$status" "$severity" "$message" "$mode" "$venue_state"
  done

  return 0
}
export -f run_phase28
