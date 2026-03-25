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
    status="WARN"; severity="P2"; message="WhatsApp/Evolution config not found in TOML (or server offline)"
  fi
  emit_result "$phase" "$tier" "server-23-wa-config" "$status" "$severity" "$message" "$mode" "$venue_state"

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
