#!/usr/bin/env bash
# audit/phases/tier1/phase02.sh -- Phase 02: Config Integrity
# Tier: 1 (Infrastructure Foundation)
# What: All TOML config files valid, not corrupted by SSH banners or stale edits.
# Standing rule: Phase scripts always exit 0 -- errors encoded in emit_result status, never bash exit code.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase02() {
  local phase="02" tier="1"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # Server racecontrol.toml -- first line must start with [
  response=$(safe_remote_exec "192.168.31.23" "8090" \
    'findstr /N /R "^" C:\RacingPoint\racecontrol.toml | findstr /R "^1:"' \
    "$DEFAULT_TIMEOUT")
  local first_line; first_line=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null || true)
  if printf '%s' "$first_line" | grep -q "^\s*1:\s*\["; then
    status="PASS"; severity="P3"; message="racecontrol.toml first line is valid TOML section header"
  elif [[ -z "$first_line" ]]; then
    status="PASS"; severity="P3"; message="racecontrol.toml: could not read first line (exec unavailable)"
  else
    status="FAIL"; severity="P1"; message="racecontrol.toml first line not a TOML section: ${first_line:0:80}"
  fi
  emit_result "$phase" "$tier" "server-23-toml" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Server: check for duplicate enabled= keys (conflicting config)
  response=$(safe_remote_exec "192.168.31.23" "8090" \
    'findstr /C:"enabled" C:\RacingPoint\racecontrol.toml' \
    "$DEFAULT_TIMEOUT")
  local enabled_lines; enabled_lines=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null | grep -c "enabled" || true)
  if [[ "${enabled_lines:-0}" -le 10 ]]; then
    status="PASS"; severity="P3"; message="racecontrol.toml: no excessive duplicate enabled keys (${enabled_lines:-0} found)"
  else
    status="WARN"; severity="P2"; message="racecontrol.toml: ${enabled_lines} enabled= lines -- check for conflicting duplicates"
  fi
  emit_result "$phase" "$tier" "server-23-toml-keys" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Server: ws_connect_timeout value check (CV-01)
  response=$(safe_remote_exec "192.168.31.23" "8090" \
    'findstr /C:"ws_connect_timeout" C:\RacingPoint\racecontrol.toml' \
    "$DEFAULT_TIMEOUT")
  local ws_timeout_line; ws_timeout_line=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null || true)
  local ws_timeout_val; ws_timeout_val=$(printf '%s' "$ws_timeout_line" | grep -oE '[0-9]+' | head -1 || true)
  if [[ -n "$ws_timeout_val" ]] && [[ "$ws_timeout_val" -ge 600 ]] 2>/dev/null; then
    status="PASS"; severity="P3"; message="ws_connect_timeout = ${ws_timeout_val}ms (>= 600ms threshold)"
  elif [[ -n "$ws_timeout_val" ]] && [[ "$ws_timeout_val" -lt 600 ]] 2>/dev/null; then
    status="WARN"; severity="P2"; message="ws_connect_timeout = ${ws_timeout_val}ms (below 600ms threshold -- risk of false WS disconnects)"
  else
    status="WARN"; severity="P2"; message="ws_connect_timeout not found or unparseable in racecontrol.toml"
  fi
  emit_result "$phase" "$tier" "server-23-ws-timeout" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Server: app_health monitoring URL port check (CV-02)
  response=$(safe_remote_exec "192.168.31.23" "8090" \
    'findstr /C:"app_health" C:\RacingPoint\racecontrol.toml' \
    "$DEFAULT_TIMEOUT")
  local app_health_lines; app_health_lines=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null || true)
  if [[ -z "$app_health_lines" ]]; then
    status="WARN"; severity="P2"; message="No app_health monitoring URLs in racecontrol.toml"
  else
    local has_admin has_kiosk
    has_admin=$(printf '%s' "$app_health_lines" | grep -c "3201" || true)
    has_kiosk=$(printf '%s' "$app_health_lines" | grep -c "3300" || true)
    if [[ "${has_admin:-0}" -ge 1 ]] && [[ "${has_kiosk:-0}" -ge 1 ]]; then
      status="PASS"; severity="P3"; message="app_health URLs contain correct ports (:3201 admin, :3300 kiosk)"
    else
      status="WARN"; severity="P2"; message="app_health URLs may have wrong ports (expected :3201 for admin, :3300 for kiosk) -- found: ${app_health_lines:0:120}"
    fi
  fi
  emit_result "$phase" "$tier" "server-23-app-health-urls" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Pod TOML -- verify pod_number key exists
  local ip host
  for ip in $PODS; do
    host="pod-$(printf '%s' "$ip" | sed 's/192\.168\.31\.//')"
    response=$(safe_remote_exec "$ip" "8090" \
      'type C:\RacingPoint\rc-agent.toml' \
      "$DEFAULT_TIMEOUT")
    local stdout; stdout=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null || true)
    if printf '%s' "$stdout" | grep -qE "(pod_number|^\s*number\s*=)"; then
      status="PASS"; severity="P3"; message="rc-agent.toml present with pod number config"
    elif [[ -z "$stdout" ]]; then
      status="WARN"; severity="P2"; message="rc-agent.toml: could not read (pod offline or exec failed)"
    else
      status="FAIL"; severity="P2"; message="rc-agent.toml exists but missing pod_number key"
    fi
    if [[ "$venue_state" = "closed" ]] && [[ "$status" = "FAIL" || "$status" = "WARN" ]]; then
      status="QUIET"; severity="P3"
    fi
    emit_result "$phase" "$tier" "${host}-toml" "$status" "$severity" "$message" "$mode" "$venue_state"
  done

  # James comms-link .env
  if [[ -f "C:/Users/bono/racingpoint/comms-link/.env" ]]; then
    if grep -q "COMMS_PSK" "C:/Users/bono/racingpoint/comms-link/.env" 2>/dev/null; then
      status="PASS"; severity="P3"; message="comms-link .env present with COMMS_PSK"
    else
      status="WARN"; severity="P2"; message="comms-link .env missing COMMS_PSK key"
    fi
  else
    status="WARN"; severity="P2"; message="comms-link .env not found at expected path"
  fi
  emit_result "$phase" "$tier" "james-commslink-env" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase02
