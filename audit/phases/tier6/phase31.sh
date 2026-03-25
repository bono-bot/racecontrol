#!/usr/bin/env bash
# audit/phases/tier6/phase31.sh -- Phase 31: Email Alerts
# Tier: 6 (Notifications & Marketing)
# What: Gmail OAuth token fresh, email script exists, no send failures.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase31() {
  local phase="31" tier="6"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # Email config in TOML
  response=$(safe_remote_exec "192.168.31.23" "8090" \
    'findstr /C:"email" /C:"gmail" /C:"smtp" C:\RacingPoint\racecontrol.toml' \
    "$DEFAULT_TIMEOUT")
  local email_config; email_config=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null | tr -d '[:space:]' || true)
  if [[ -n "$email_config" ]]; then
    status="PASS"; severity="P3"; message="Email/Gmail/SMTP config found in racecontrol.toml"
  else
    status="WARN"; severity="P2"; message="Email config not found in TOML (or server offline)"
  fi
  emit_result "$phase" "$tier" "server-23-email-config" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Email script exists
  response=$(safe_remote_exec "192.168.31.23" "8090" \
    'dir C:\RacingPoint\send-email.ps1 2>nul || echo MISSING' \
    "$DEFAULT_TIMEOUT")
  local script_out; script_out=$(printf '%s' "$response" | jq -r '.stdout // "MISSING"' 2>/dev/null || echo "MISSING")
  if printf '%s' "$script_out" | grep -qi "MISSING"; then
    status="WARN"; severity="P2"; message="send-email.ps1 not found at C:\\RacingPoint\\"
  else
    status="PASS"; severity="P3"; message="send-email.ps1 present"
  fi
  emit_result "$phase" "$tier" "server-23-email-script" "$status" "$severity" "$message" "$mode" "$venue_state"

  # OAuth/email errors in logs
  local log_resp; log_resp=$(http_get "http://192.168.31.23:8080/api/v1/logs?lines=50" "$DEFAULT_TIMEOUT")
  if [[ -n "$log_resp" ]]; then
    local email_err; email_err=$(printf '%s' "$log_resp" | jq -r '.' 2>/dev/null | grep -ci "email.*error\|gmail.*token\|smtp.*fail")
    if [[ "${email_err:-0}" -eq 0 ]]; then
      status="PASS"; severity="P3"; message="No email/Gmail errors in recent logs"
    else
      status="WARN"; severity="P2"; message="${email_err} email/OAuth error entries — check for expired token (403)"
    fi
  else
    status="WARN"; severity="P2"; message="Logs API unreachable — cannot check email errors"
  fi
  emit_result "$phase" "$tier" "server-23-email-errors" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase31
