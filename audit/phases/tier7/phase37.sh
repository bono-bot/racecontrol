#!/usr/bin/env bash
# audit/phases/tier7/phase37.sh -- Phase 37: Activity Log & Compliance
# Tier: 7 (Data & Sync)
# What: Audit trail recording, PII not leaked in logs, retention policy configured.

set -u
set -o pipefail

run_phase37() {
  local phase="37" tier="7"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # Activity log entries in last 24h
  response=$(safe_remote_exec "192.168.31.23" "8090" \
    "sqlite3 C:\\RacingPoint\\data\\racecontrol.db \"SELECT COUNT(*) FROM activity_log WHERE created_at > datetime('now', '-24 hours')\" 2>nul || echo 0" \
    "$DEFAULT_TIMEOUT")
  local act_count; act_count=$(printf '%s' "$response" | jq -r '.stdout // "0"' 2>/dev/null | tr -d '[:space:]' | grep -oE '^[0-9]+' || echo "0")
  if [[ "${act_count:-0}" -ge 1 ]]; then
    status="PASS"; severity="P3"; message="Activity log: ${act_count} entries in last 24h"
  else
    status="WARN"; severity="P2"; message="Activity log: 0 entries in last 24h (audit trail not recording)"
  fi
  emit_result "$phase" "$tier" "server-23-activity-log" "$status" "$severity" "$message" "$mode" "$venue_state"

  # PII check: phone numbers in log output (should NOT appear in plaintext)
  local log_resp; log_resp=$(http_get "http://192.168.31.23:8080/api/v1/logs?lines=100" "$DEFAULT_TIMEOUT")
  if [[ -n "$log_resp" ]]; then
    local pii_count; pii_count=$(printf '%s' "$log_resp" | grep -oE "[0-9]{10}" | wc -l | tr -d '[:space:]' || echo "0")
    if [[ "${pii_count:-0}" -eq 0 ]]; then
      status="PASS"; severity="P3"; message="No 10-digit phone numbers in plaintext log output"
    else
      status="WARN"; severity="P2"; message="${pii_count} potential phone numbers in log output — review PII masking"
    fi
  else
    status="WARN"; severity="P2"; message="Logs API unreachable — cannot check PII in logs"
  fi
  emit_result "$phase" "$tier" "server-23-pii-check" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Retention config in TOML
  response=$(safe_remote_exec "192.168.31.23" "8090" \
    'findstr /C:"retention" /C:"deletion" /C:"dpdp" C:\RacingPoint\racecontrol.toml' \
    "$DEFAULT_TIMEOUT")
  local ret_config; ret_config=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null | tr -d '[:space:]' || true)
  if [[ -n "$ret_config" ]]; then
    status="PASS"; severity="P3"; message="Retention/DPDP config found in racecontrol.toml"
  else
    status="WARN"; severity="P2"; message="No retention/deletion/dpdp config in TOML (DPDP compliance gap)"
  fi
  emit_result "$phase" "$tier" "server-23-retention-config" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase37
