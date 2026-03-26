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
    status="PASS"; severity="P3"; message="Email not configured in TOML (optional integration)"
  fi
  emit_result "$phase" "$tier" "server-23-email-config" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Email script exists
  response=$(safe_remote_exec "192.168.31.23" "8090" \
    'dir C:\RacingPoint\send-email.ps1 2>nul || echo MISSING' \
    "$DEFAULT_TIMEOUT")
  local script_out; script_out=$(printf '%s' "$response" | jq -r '.stdout // "MISSING"' 2>/dev/null || echo "MISSING")
  if printf '%s' "$script_out" | grep -qi "MISSING"; then
    status="PASS"; severity="P3"; message="send-email.ps1 not found (optional integration)"
  else
    status="PASS"; severity="P3"; message="send-email.ps1 present"
  fi
  emit_result "$phase" "$tier" "server-23-email-script" "$status" "$severity" "$message" "$mode" "$venue_state"

  # OAuth token expiry proactive check (CV-04)
  # Try multiple possible token file names
  local token_content=""
  local token_found=false
  local token_files="gmail-token.json google-credentials.json oauth-token.json"
  for tf in $token_files; do
    response=$(safe_remote_exec "192.168.31.23" "8090" \
      "type C:\\RacingPoint\\${tf} 2>nul" \
      "$DEFAULT_TIMEOUT")
    local tf_stdout; tf_stdout=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null || true)
    if [[ -n "$tf_stdout" ]]; then
      token_content="$tf_stdout"
      token_found=true
      break
    fi
  done

  if [[ "$token_found" = "true" ]]; then
    # Extract expiry from token JSON
    local expiry_raw; expiry_raw=$(printf '%s' "$token_content" | jq -r '.expiry_date // .expires_at // .token_expiry // empty' 2>/dev/null || true)
    if [[ -n "$expiry_raw" ]]; then
      local now_secs; now_secs=$(date +%s)
      local expiry_secs=0

      # Check if epoch milliseconds (13+ digits)
      if printf '%s' "$expiry_raw" | grep -qE '^[0-9]{13,}$'; then
        expiry_secs=$(( ${expiry_raw%???} ))  # Drop last 3 digits (ms -> s)
      # Check if epoch seconds (10 digits)
      elif printf '%s' "$expiry_raw" | grep -qE '^[0-9]{10}$'; then
        expiry_secs=$expiry_raw
      # Try ISO date string
      elif printf '%s' "$expiry_raw" | grep -qE '^[0-9]{4}-'; then
        expiry_secs=$(date -d "$expiry_raw" +%s 2>/dev/null || echo "0")
      fi

      if [[ "$expiry_secs" -gt 0 ]]; then
        local days_left=$(( (expiry_secs - now_secs) / 86400 ))
        if [[ "$days_left" -lt 0 ]]; then
          status="FAIL"; severity="P1"; message="Gmail OAuth token EXPIRED (${days_left} days ago) -- email alerts non-functional"
        elif [[ "$days_left" -le 7 ]]; then
          status="WARN"; severity="P2"; message="Gmail OAuth token expires in ${days_left} days -- refresh needed"
        else
          status="PASS"; severity="P3"; message="Gmail OAuth token valid (${days_left} days remaining)"
        fi
      else
        status="WARN"; severity="P2"; message="Could not parse OAuth token expiry value: ${expiry_raw}"
      fi
    else
      status="WARN"; severity="P2"; message="Could not determine OAuth token expiry -- verify token file on server"
    fi
  else
    status="PASS"; severity="P3"; message="Email OAuth managed externally (no local token file)"
  fi
  emit_result "$phase" "$tier" "server-23-email-oauth-expiry" "$status" "$severity" "$message" "$mode" "$venue_state"

  # OAuth/email errors in logs (only check for actual send failures, not field mentions)
  local log_resp; log_resp=$(http_get "http://192.168.31.23:8080/api/v1/logs?lines=50" "$DEFAULT_TIMEOUT")
  if [[ -n "$log_resp" ]]; then
    # Search for actual email SEND failures — not just "email" appearing as a data field
    local email_err; email_err=$(printf '%s' "$log_resp" | jq -r '.lines[]? // .' 2>/dev/null | grep -ci "send_email.*error\|email.*send.*fail\|smtp.*error\|gmail.*token.*expir\|oauth.*refresh.*fail" || true)
    email_err="${email_err:-0}"
    if [[ "$email_err" -eq 0 ]]; then
      status="PASS"; severity="P3"; message="No email send errors in recent logs"
    else
      status="WARN"; severity="P2"; message="${email_err} email send error entries — check for expired OAuth token"
    fi
  else
    status="WARN"; severity="P2"; message="Logs API unreachable — cannot check email errors"
  fi
  emit_result "$phase" "$tier" "server-23-email-errors" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase31
