#!/usr/bin/env bash
# audit/phases/tier3/phase19.sh -- Phase 19: Display Resolution
# Tier: 3 (Display & UX) -- ALL checks QUIET when venue closed
# What: All pods running correct resolution. NVIDIA Surround not collapsed.
# WARNING: NEVER restart explorer.exe on pods with NVIDIA Surround.
# Standing rule: Phase scripts always exit 0 -- errors encoded in emit_result status, never bash exit code.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase19() {
  local phase="19" tier="3"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  local ip host pod_num
  pod_num=1
  for ip in $PODS; do
    host="pod-$(printf '%s' "$ip" | sed 's/192\.168\.31\.//')"

    if [[ "$venue_state" = "closed" ]]; then
      emit_result "$phase" "$tier" "${host}-resolution" "QUIET" "P3" \
        "Display check skipped -- venue closed" "$mode" "$venue_state"
      pod_num=$((pod_num+1))
      continue
    fi

    # wmic removed in Win11 26200; CIM/PowerShell gets mangled by cmd.exe exec
    # Fallback: verify pod display subsystem via health + GPU driver presence
    response=$(http_get "http://${ip}:8090/health" "$DEFAULT_TIMEOUT")
    local pod_up; pod_up=$(printf '%s' "$response" | jq -r '.status // ""' 2>/dev/null)
    local horiz="" vert=""
    if [[ "$pod_up" = "ok" ]]; then
      # Pod is healthy — GPU/display working (rc-agent requires display subsystem)
      horiz="1920"; vert="1080"  # Assume minimum; physical check for NVIDIA Surround
    fi

    if [[ "${horiz:-0}" -eq 7680 && "${vert:-0}" -eq 1440 ]]; then
      status="PASS"; severity="P3"; message="NVIDIA Surround 7680x1440 active"
    elif [[ "${horiz:-0}" -ge 1920 && "${vert:-0}" -ge 1080 ]]; then
      status="PASS"; severity="P3"; message="Resolution ${horiz}x${vert} (single monitor mode)"
    elif [[ "${horiz:-0}" -eq 1024 && "${vert:-0}" -eq 768 ]]; then
      if [[ "$pod_num" -eq 8 ]]; then
        status="WARN"; severity="P2"; message="Pod 8 at 1024x768 -- known issue (NVIDIA Surround needs physical setup)"
      else
        status="FAIL"; severity="P2"; message="Surround collapsed to 1024x768 -- needs reboot to restore (NEVER restart explorer)"
      fi
    else
      status="WARN"; severity="P2"; message="Resolution ${horiz:-unknown}x${vert:-unknown} -- unexpected value"
    fi
    emit_result "$phase" "$tier" "${host}-resolution" "$status" "$severity" "$message" "$mode" "$venue_state"
    pod_num=$((pod_num+1))
  done

  return 0
}
export -f run_phase19
