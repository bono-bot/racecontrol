#!/usr/bin/env bash
# audit/phases/tier6/phase30.sh -- Phase 30: WhatsApp Alerter
# Tier: 6 (Notifications & Marketing)
# What: Evolution API connected, phone numbers correct, no send errors.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase30() {
  local phase="30" tier="6"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # WhatsApp config in racecontrol.toml
  response=$(safe_remote_exec "192.168.31.23" "8090" \
    'findstr /C:"whatsapp" /C:"evolution" C:\RacingPoint\racecontrol.toml' \
    "$DEFAULT_TIMEOUT")
  local wa_config; wa_config=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null | tr -d '[:space:]' || true)
  if [[ -n "$wa_config" ]]; then
    status="PASS"; severity="P3"; message="WhatsApp/Evolution config found in racecontrol.toml"
  else
    status="PASS"; severity="P3"; message="WhatsApp/Evolution not configured in TOML (optional integration)"
  fi
  emit_result "$phase" "$tier" "server-23-wa-config" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Evolution API live connection state check (CV-03)
  # Extract Evolution API base URL from TOML config
  local evo_url="http://srv1422716.hstgr.cloud:8080"
  local url_response; url_response=$(safe_remote_exec "192.168.31.23" "8090" \
    'findstr /C:"evolution_api_url" C:\RacingPoint\racecontrol.toml' \
    "$DEFAULT_TIMEOUT")
  local url_stdout; url_stdout=$(printf '%s' "$url_response" | jq -r '.stdout // ""' 2>/dev/null || true)
  local parsed_url; parsed_url=$(printf '%s' "$url_stdout" | grep -oE 'https?://[^"[:space:]]+' | head -1)
  if [[ -n "$parsed_url" ]]; then
    evo_url="$parsed_url"
  fi

  # Query Evolution API connection state
  local evo_response; evo_response=$(http_get "${evo_url}/api/instance/connectionState/racingpoint" 10)
  if [[ -n "$evo_response" ]]; then
    local conn_state; conn_state=$(printf '%s' "$evo_response" | jq -r '.instance.state // .state // "unknown"' 2>/dev/null || echo "unknown")
    if [[ "$conn_state" = "open" ]]; then
      status="PASS"; severity="P3"; message="Evolution API connected (state=open)"
    elif [[ "$conn_state" != "unknown" ]]; then
      status="WARN"; severity="P2"; message="Evolution API disconnected (state=${conn_state}) -- WhatsApp alerts may not send"
    else
      status="FAIL"; severity="P1"; message="Evolution API response unparseable at ${evo_url} -- WhatsApp alerting may be DOWN"
    fi
  else
    status="FAIL"; severity="P1"; message="Evolution API unreachable at ${evo_url} -- WhatsApp alerting DOWN"
  fi
  emit_result "$phase" "$tier" "server-23-wa-connection" "$status" "$severity" "$message" "$mode" "$venue_state"

  # WhatsApp send errors in logs
  local log_resp; log_resp=$(http_get "http://192.168.31.23:8080/api/v1/logs?lines=50" "$DEFAULT_TIMEOUT")
  if [[ -n "$log_resp" ]]; then
    local wa_err; wa_err=$(printf '%s' "$log_resp" | jq -r '.' 2>/dev/null | grep -ci "whatsapp.*error\|evolution.*error\|wa_send.*fail")
    if [[ "${wa_err:-0}" -eq 0 ]]; then
      status="PASS"; severity="P3"; message="No WhatsApp send errors in recent logs"
    else
      status="WARN"; severity="P2"; message="${wa_err} WhatsApp/Evolution error entries in logs"
    fi
  else
    status="WARN"; severity="P2"; message="Logs API unreachable — cannot check WhatsApp errors"
  fi
  emit_result "$phase" "$tier" "server-23-wa-errors" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase30
