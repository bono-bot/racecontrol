#!/usr/bin/env bash
# audit/phases/tier4/phase22.sh -- Phase 22: Wallet & Payments
# Tier: 4 (Billing & Commerce)
# What: Wallet system functional — balance queries work, no stuck debit_intents.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase22() {
  local phase="22" tier="4"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message
  local token; token=$(get_session_token)

  # Wallet endpoint
  response=$(curl -s -m "$DEFAULT_TIMEOUT" \
    "http://192.168.31.23:8080/api/v1/wallets" \
    -H "x-terminal-session: ${token:-}" 2>/dev/null | tr -d '"')
  if [[ -n "$response" ]]; then
    status="PASS"; severity="P3"; message="Wallet endpoint responding"
  else
    status="FAIL"; severity="P1"; message="Wallet endpoint unreachable"
  fi
  emit_result "$phase" "$tier" "server-23-wallets" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Check for stuck debit_intents in logs
  local log_resp; log_resp=$(http_get "http://192.168.31.23:8080/api/v1/logs?lines=50" "$DEFAULT_TIMEOUT")
  if [[ -n "$log_resp" ]]; then
    local stuck; stuck=$(printf '%s' "$log_resp" | jq -r '.' 2>/dev/null | grep -ci "debit_intent.*pending\|wallet.*error")
    if [[ "${stuck:-0}" -eq 0 ]]; then
      status="PASS"; severity="P3"; message="No stuck debit_intents or wallet errors in recent logs"
    else
      status="WARN"; severity="P2"; message="${stuck} debit_intent/wallet error entries in logs — check for stuck transactions"
    fi
  else
    status="WARN"; severity="P2"; message="Logs API unreachable — cannot check for stuck debit_intents"
  fi
  emit_result "$phase" "$tier" "server-23-wallet-errors" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase22
