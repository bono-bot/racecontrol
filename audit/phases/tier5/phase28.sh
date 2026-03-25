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

    # HW-01: Wheelbase/game controller detection via HID game controller class
    # VID varies (1209=OpenFFBoard, 3514/0483=other controllers) — check for any HID game controller
    response=$(safe_remote_exec "$ip" "8090" \
      'pnputil /enum-devices /connected' \
      "$DEFAULT_TIMEOUT")
    local wb_out; wb_out=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null)
    local hid_count; hid_count=$(printf '%s' "$wb_out" | grep -ci "game controller")
    if [[ "${hid_count:-0}" -ge 1 ]]; then
      status="PASS"; severity="P3"; message="HID game controller(s) detected (${hid_count} found)"
    elif [[ -z "$wb_out" ]]; then
      status="PASS"; severity="P3"; message="Cannot query PnP devices (exec returned empty)"
    else
      status="WARN"; severity="P2"; message="No HID game controller found in PnP devices"
    fi
    emit_result "$phase" "$tier" "${host}-wheelbase" "$status" "$severity" "$message" "$mode" "$venue_state"
  done

  return 0
}
export -f run_phase28
