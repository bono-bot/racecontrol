#!/usr/bin/env bash
# audit/phases/tier3/phase18.sh -- Phase 18: Overlay Suppression
# Tier: 3 (Display & UX) -- ALL checks QUIET when venue closed
# What: No unwanted overlays (Copilot, NVIDIA, AMD DVR, OneDrive, Steam, GameBar).
# Standing rule: Phase scripts always exit 0 -- errors encoded in emit_result status, never bash exit code.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase18() {
  local phase="18" tier="3"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  local ip host
  for ip in $PODS; do
    host="pod-$(printf '%s' "$ip" | sed 's/192\.168\.31\.//')"

    if [[ "$venue_state" = "closed" ]]; then
      emit_result "$phase" "$tier" "${host}-overlays" "QUIET" "P3" \
        "Overlay check skipped -- venue closed" "$mode" "$venue_state"
      continue
    fi

    response=$(safe_remote_exec "$ip" "8090" \
      'tasklist /V /FO CSV /NH | findstr /I /C:"Copilot" /C:"NVIDIA Overlay" /C:"AMD DVR" /C:"OneDrive" /C:"Widgets" /C:"Steam" /C:"GameBar"' \
      "$DEFAULT_TIMEOUT")
    local overlay_out; overlay_out=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null | tr -d '[:space:]' || true)
    if [[ -z "$overlay_out" ]]; then
      status="PASS"; severity="P3"; message="No overlay processes found (Copilot/NVIDIA/AMD/Steam/GameBar)"
    else
      status="WARN"; severity="P2"; message="Overlay process(es) running: ${overlay_out:0:80}"
    fi
    emit_result "$phase" "$tier" "${host}-overlays" "$status" "$severity" "$message" "$mode" "$venue_state"
  done

  return 0
}
export -f run_phase18
