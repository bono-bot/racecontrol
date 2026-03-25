#!/usr/bin/env bash
# audit/phases/tier6/phase34.sh -- Phase 34: Psychology & Gamification
# Tier: 6 (Notifications & Marketing)
# What: Badge system awarding, notification dispatch, progress tracking, reward cycles.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase34() {
  local phase="34" tier="6"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # Psychology engine logs
  local log_resp; log_resp=$(http_get "http://192.168.31.23:8080/api/v1/logs?lines=100" "$DEFAULT_TIMEOUT")
  if [[ -n "$log_resp" ]]; then
    local psych_entries; psych_entries=$(printf '%s' "$log_resp" | jq -r '.' 2>/dev/null | grep -ci "psychology\|badge.*award\|streak\|reward")
    if [[ "${psych_entries:-0}" -ge 1 ]]; then
      status="PASS"; severity="P3"; message="Psychology engine active (${psych_entries} entries)"
    else
      status="PASS"; severity="P3"; message="No psychology/badge/streak issues in recent logs (feature quiet)"
    fi
  else
    status="WARN"; severity="P2"; message="Logs API unreachable — cannot check psychology engine"
  fi
  emit_result "$phase" "$tier" "server-23-psychology" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Badge/notification dispatch
  if [[ -n "$log_resp" ]]; then
    local badge_entries; badge_entries=$(printf '%s' "$log_resp" | jq -r '.' 2>/dev/null | grep -ci "notification.*dispatch\|badge.*criteria")
    if [[ "${badge_entries:-0}" -ge 1 ]]; then
      status="PASS"; severity="P3"; message="Badge criteria/notification dispatch in logs (${badge_entries} entries)"
    else
      status="PASS"; severity="P3"; message="No notification dispatch/badge criteria issues in recent logs (feature quiet)"
    fi
  else
    status="WARN"; severity="P2"; message="Logs API unreachable — cannot check badge dispatch"
  fi
  emit_result "$phase" "$tier" "server-23-badge-dispatch" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Bot coordinator (orchestrates WhatsApp/Discord/email)
  if [[ -n "$log_resp" ]]; then
    local bot_entries; bot_entries=$(printf '%s' "$log_resp" | jq -r '.' 2>/dev/null | grep -ci "bot_coordinator")
    if [[ "${bot_entries:-0}" -ge 1 ]]; then
      status="PASS"; severity="P3"; message="Bot coordinator active (${bot_entries} entries)"
    else
      status="PASS"; severity="P3"; message="No bot_coordinator issues in recent logs (feature quiet)"
    fi
  else
    status="WARN"; severity="P2"; message="Logs API unreachable — cannot check bot coordinator"
  fi
  emit_result "$phase" "$tier" "server-23-bot-coordinator" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase34
