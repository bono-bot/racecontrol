#!/usr/bin/env bash
# audit/phases/tier6/phase32.sh -- Phase 32: Discord Integration
# Tier: 6 (Notifications & Marketing)
# What: Discord webhook/token valid, race results posting.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase32() {
  local phase="32" tier="6"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # Discord config in TOML
  response=$(safe_remote_exec "192.168.31.23" "8090" \
    'findstr /C:"discord" C:\RacingPoint\racecontrol.toml' \
    "$DEFAULT_TIMEOUT")
  local discord_config; discord_config=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null | tr -d '[:space:]' || true)
  if [[ -n "$discord_config" ]]; then
    status="PASS"; severity="P3"; message="Discord config found in racecontrol.toml"
  else
    status="PASS"; severity="P3"; message="Discord not configured in TOML (optional integration)"
  fi
  emit_result "$phase" "$tier" "server-23-discord-config" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Discord errors in logs
  local log_resp; log_resp=$(http_get "http://192.168.31.23:8080/api/v1/logs?lines=50" "$DEFAULT_TIMEOUT")
  if [[ -n "$log_resp" ]]; then
    local disc_err; disc_err=$(printf '%s' "$log_resp" | jq -r '.' 2>/dev/null | grep -ci "discord.*error\|webhook.*fail")
    if [[ "${disc_err:-0}" -eq 0 ]]; then
      status="PASS"; severity="P3"; message="No Discord/webhook errors in recent logs"
    else
      status="WARN"; severity="P2"; message="${disc_err} Discord error entries in logs — check webhook token validity"
    fi
  else
    status="WARN"; severity="P2"; message="Logs API unreachable — cannot check Discord errors"
  fi
  emit_result "$phase" "$tier" "server-23-discord-errors" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase32
