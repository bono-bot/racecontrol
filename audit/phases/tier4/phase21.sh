#!/usr/bin/env bash
# audit/phases/tier4/phase21.sh -- Phase 21: Pricing & Billing Sessions
# Tier: 4 (Billing & Commerce)
# What: Pricing tiers loaded, billing sessions trackable, active sessions tracked.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase21() {
  local phase="21" tier="4"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message
  local token; token=$(get_session_token)

  # Pricing tiers endpoint
  response=$(curl -s -m "$DEFAULT_TIMEOUT" \
    "http://192.168.31.23:8080/api/v1/pricing" \
    -H "x-terminal-session: ${token:-}" 2>/dev/null | tr -d '"')
  if [[ -n "$response" ]]; then
    local tier_count; tier_count=$(printf '%s' "$response" | jq 'length' 2>/dev/null)
    if [[ "${tier_count:-0}" -ge 1 ]]; then
      status="PASS"; severity="P3"; message="Pricing tiers loaded: ${tier_count} tier(s)"
    else
      status="FAIL"; severity="P2"; message="Pricing tiers: empty response — no billing tiers defined"
    fi
  else
    status="FAIL"; severity="P1"; message="Pricing endpoint unreachable"
  fi
  emit_result "$phase" "$tier" "server-23-pricing" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Active billing sessions
  response=$(curl -s -m "$DEFAULT_TIMEOUT" \
    "http://192.168.31.23:8080/api/v1/billing/sessions/active" \
    -H "x-terminal-session: ${token:-}" 2>/dev/null | tr -d '"')
  if [[ -n "$response" ]]; then
    # 0 active sessions is fine if venue closed
    status="PASS"; severity="P3"; message="Active billing sessions endpoint responding"
  else
    status="WARN"; severity="P2"; message="Active billing sessions endpoint unreachable (auth or server issue)"
  fi
  emit_result "$phase" "$tier" "server-23-billing-active" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Recent sessions (verify billing history exists)
  response=$(curl -s -m "$DEFAULT_TIMEOUT" \
    "http://192.168.31.23:8080/api/v1/billing/sessions?limit=3" \
    -H "x-terminal-session: ${token:-}" 2>/dev/null | tr -d '"')
  if [[ -n "$response" ]]; then
    status="PASS"; severity="P3"; message="Billing sessions history endpoint responding"
  else
    status="WARN"; severity="P2"; message="Billing sessions history unreachable"
  fi
  emit_result "$phase" "$tier" "server-23-billing-history" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase21
