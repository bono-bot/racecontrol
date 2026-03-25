#!/usr/bin/env bash
# audit/phases/tier3/phase20.sh -- Phase 20: Kiosk Browser Health
# Tier: 3 (Display & UX) -- ALL checks QUIET when venue closed
# What: Edge kiosk mode running with correct URL. Kiosk page accessible from pod.
# Standing rule: Phase scripts always exit 0 -- errors encoded in emit_result status, never bash exit code.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase20() {
  local phase="20" tier="3"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  local ip host
  for ip in $PODS; do
    host="pod-$(printf '%s' "$ip" | sed 's/192\.168\.31\.//')"

    if [[ "$venue_state" = "closed" ]]; then
      emit_result "$phase" "$tier" "${host}-kiosk-mode" "QUIET" "P3" \
        "Kiosk browser check skipped -- venue closed" "$mode" "$venue_state"
      emit_result "$phase" "$tier" "${host}-kiosk-reachable" "QUIET" "P3" \
        "Kiosk reachability check skipped -- venue closed" "$mode" "$venue_state"
      continue
    fi

    # Verify Edge command line contains kiosk flag and port 3300
    response=$(safe_remote_exec "$ip" "8090" \
      'wmic process where "name=''msedge.exe''" get CommandLine /value 2>nul | findstr /C:"kiosk" /C:"3300"' \
      "$DEFAULT_TIMEOUT")
    local cmd_out; cmd_out=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null | tr -d '[:space:]' || true)
    if [[ -n "$cmd_out" ]]; then
      status="PASS"; severity="P3"; message="Edge running in kiosk mode with :3300 URL"
    else
      status="WARN"; severity="P2"; message="Edge kiosk flag or :3300 URL not found in Edge CommandLine"
    fi
    emit_result "$phase" "$tier" "${host}-kiosk-mode" "$status" "$severity" "$message" "$mode" "$venue_state"

    # Kiosk page accessible from pod
    response=$(safe_remote_exec "$ip" "8090" \
      'curl.exe -s -o nul -w "%{http_code}" http://192.168.31.23:3300/kiosk' \
      "$DEFAULT_TIMEOUT")
    local http_code; http_code=$(printf '%s' "$response" | jq -r '.stdout // "000"' 2>/dev/null | tr -d '[:space:]')
    if [[ "$http_code" = "200" ]]; then
      status="PASS"; severity="P3"; message="Kiosk page :3300/kiosk returns 200 from pod"
    elif [[ "$http_code" = "000" || -z "$http_code" ]]; then
      status="WARN"; severity="P2"; message="Pod cannot reach kiosk server (exec failed or connection timeout)"
    else
      status="WARN"; severity="P2"; message="Kiosk page returned HTTP ${http_code} from pod (expected 200)"
    fi
    emit_result "$phase" "$tier" "${host}-kiosk-reachable" "$status" "$severity" "$message" "$mode" "$venue_state"
  done

  return 0
}
export -f run_phase20
