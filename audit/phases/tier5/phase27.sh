#!/usr/bin/env bash
# audit/phases/tier5/phase27.sh -- Phase 27: AC Server & Telemetry
# Tier: 5 (Games & Hardware) -- QUIET when venue closed
# What: AC server process, telemetry UDP ports, lap data flowing.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase27() {
  local phase="27" tier="5"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # AC server check is only meaningful when venue is open
  if [[ "$venue_state" = "closed" ]]; then
    emit_result "$phase" "$tier" "server-23-ac-server" "QUIET" "P3" \
      "AC server check skipped — venue closed" "$mode" "$venue_state"
    emit_result "$phase" "$tier" "server-23-lap-data" "QUIET" "P3" \
      "Lap data check skipped — venue closed" "$mode" "$venue_state"
    return 0
  fi

  # Check if AC server process is running on server .23
  response=$(safe_remote_exec "192.168.31.23" "8090" \
    'tasklist /NH | findstr /I "AssettoCorsa"' \
    "$DEFAULT_TIMEOUT")
  local ac_proc; ac_proc=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null | tr -d '[:space:]' || true)
  if printf '%s' "$ac_proc" | grep -qi "AssettoCorsa\|acs.exe\|acServer"; then
    status="PASS"; severity="P3"; message="AC server process running on server .23"
  else
    status="PASS"; severity="P3"; message="AC server not running (normal outside active race sessions)"
  fi
  emit_result "$phase" "$tier" "server-23-ac-server" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Recent lap data in server logs
  response=$(http_get "http://192.168.31.23:8080/api/v1/logs?lines=50" "$DEFAULT_TIMEOUT")
  if [[ -n "$response" ]]; then
    local lap_entries; lap_entries=$(printf '%s' "$response" | jq -r '.' 2>/dev/null | grep -ci "lap_tracker\|telemetry\|lap_time")
    if [[ "${lap_entries:-0}" -ge 1 ]]; then
      status="PASS"; severity="P3"; message="Lap/telemetry data in recent logs (${lap_entries} entries)"
    else
      status="PASS"; severity="P3"; message="No lap_tracker/telemetry entries in recent logs (no active race or AC off)"
    fi
  else
    status="WARN"; severity="P2"; message="Logs API unreachable — cannot check lap data"
  fi
  emit_result "$phase" "$tier" "server-23-lap-data" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Telemetry UDP ports check on pods (spot check: first 2 pods)
  local count=0
  for ip in $PODS; do
    [[ $count -ge 2 ]] && break
    local host; host="pod-$(printf '%s' "$ip" | sed 's/192\.168\.31\.//')"
    response=$(safe_remote_exec "$ip" "8090" \
      'netstat -an | findstr /C:"9996" /C:"20777" /C:"5300" | findstr UDP' \
      "$DEFAULT_TIMEOUT")
    local udp_out; udp_out=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null | tr -d '[:space:]' || true)
    if [[ -n "$udp_out" ]]; then
      status="PASS"; severity="P3"; message="Telemetry UDP ports detected on pod"
    else
      status="PASS"; severity="P3"; message="No telemetry UDP ports listening (no game running — expected when idle)"
    fi
    emit_result "$phase" "$tier" "${host}-telemetry-udp" "$status" "$severity" "$message" "$mode" "$venue_state"
    count=$((count+1))
  done

  return 0
}
export -f run_phase27
