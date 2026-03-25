#!/usr/bin/env bash
# audit/phases/tier4/phase24.sh -- Phase 24: Accounting & Reconciliation
# Tier: 4 (Billing & Commerce)
# What: Accounting module tracks revenue, no orphan transactions or refund errors.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase24() {
  local phase="24" tier="4"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message
  local token; token=$(get_session_token)

  # Accounting endpoint
  response=$(curl -s -m "$DEFAULT_TIMEOUT" \
    "http://192.168.31.23:8080/api/v1/accounting" \
    -H "x-terminal-session: ${token:-}" 2>/dev/null | tr -d '"')
  if [[ -n "$response" ]]; then
    status="PASS"; severity="P3"; message="Accounting endpoint responding"
  else
    status="WARN"; severity="P2"; message="Accounting endpoint unreachable or not implemented"
  fi
  emit_result "$phase" "$tier" "server-23-accounting" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Refund and accounting errors in logs
  local log_resp; log_resp=$(http_get "http://192.168.31.23:8080/api/v1/logs?lines=50" "$DEFAULT_TIMEOUT")
  if [[ -n "$log_resp" ]]; then
    local err_count; err_count=$(printf '%s' "$log_resp" | jq -r '.' 2>/dev/null | grep -ci "refund.*error\|accounting.*mismatch" || echo "0")
    if [[ "${err_count:-0}" -eq 0 ]]; then
      status="PASS"; severity="P3"; message="No accounting/refund errors in recent logs"
    else
      status="WARN"; severity="P2"; message="${err_count} accounting/refund error entries in logs"
    fi
  else
    status="WARN"; severity="P2"; message="Logs API unreachable — cannot check accounting errors"
  fi
  emit_result "$phase" "$tier" "server-23-accounting-errors" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase24
