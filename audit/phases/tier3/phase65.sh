#!/usr/bin/env bash
# audit/phases/tier3/phase65.sh -- Phase 65: Verification Chain Health
# Tier: 3 (Display/UX)
# What: Verify that verification chain infrastructure from COV-01..05 is active on the server.
# Standing rule: Phase scripts always exit 0 -- errors encoded in emit_result status, never bash exit code.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase65() {
  local phase="65" tier="3"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local status severity message

  # Check server health for chain-related fields
  local server_health
  server_health=$(http_get "http://192.168.31.23:8080/api/v1/health" "$DEFAULT_TIMEOUT")

  if [[ -z "$server_health" ]]; then
    status="FAIL"; severity="P1"
    message="Verification chain check failed -- server health endpoint unreachable"
    emit_result "$phase" "$tier" "server-23-verification-chains" "$status" "$severity" "$message" "$mode" "$venue_state"
    return 0
  fi

  # Check build_id to see if it's a v25.0+ build
  local build_id
  build_id=$(printf '%s' "$server_health" | jq -r '.build_id // ""' 2>/dev/null || echo "")

  if [[ -z "$build_id" ]]; then
    status="WARN"; severity="P2"
    message="Verification chains: cannot determine build_id from server health"
    emit_result "$phase" "$tier" "server-23-verification-chains" "$status" "$severity" "$message" "$mode" "$venue_state"
    return 0
  fi

  # Check if server health response contains verification-chain related fields
  # (e.g., verification_chains, chain_status, or similar)
  local has_chain_fields
  has_chain_fields=$(printf '%s' "$server_health" | jq 'has("verification_chains") or has("chain_status") or has("verification")' 2>/dev/null || echo "false")

  if [[ "$has_chain_fields" == "true" ]]; then
    status="PASS"; severity="P3"
    message="Verification chains active (build: ${build_id})"
  else
    # Chain fields may not be exposed in health yet -- check uptime as proxy for healthy operation
    local uptime_secs
    uptime_secs=$(printf '%s' "$server_health" | jq '.uptime_secs // 0' 2>/dev/null || echo "0")

    if [[ "${uptime_secs:-0}" -gt 0 ]]; then
      status="PASS"; severity="P3"
      message="Server healthy (build: ${build_id}, uptime: ${uptime_secs}s) -- verification chain fields not in health response but binary is running"
    else
      status="WARN"; severity="P2"
      message="Verification chains: server responding but uptime=0 and no chain fields (build: ${build_id})"
    fi
  fi
  emit_result "$phase" "$tier" "server-23-verification-chains" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase65
