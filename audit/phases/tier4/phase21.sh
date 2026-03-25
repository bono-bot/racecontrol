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

  # Pricing tiers endpoint — check reachability first, then auth
  local pricing_code; pricing_code=$(curl -s -m "$DEFAULT_TIMEOUT" -o /dev/null -w "%{http_code}" \
    "http://192.168.31.23:8080/api/v1/pricing" \
    -H "x-terminal-session: ${token:-}" 2>/dev/null)
  response=$(curl -s -m "$DEFAULT_TIMEOUT" \
    "http://192.168.31.23:8080/api/v1/pricing" \
    -H "x-terminal-session: ${token:-}" 2>/dev/null | tr -d '\r')
  if [[ "$pricing_code" = "000" ]] && [[ "$venue_state" != "closed" ]]; then
    status="FAIL"; severity="P1"; message="Pricing endpoint unreachable (server down)"
  elif [[ "$pricing_code" = "000" ]] && [[ "$venue_state" = "closed" ]]; then
    status="QUIET"; severity="P3"; message="Pricing endpoint unreachable (venue closed)"
  elif [[ "$pricing_code" = "401" || "$pricing_code" = "403" ]]; then
    status="PASS"; severity="P3"; message="Pricing endpoint exists (HTTP ${pricing_code} — auth required)"
  elif [[ -n "$response" ]]; then
    local tier_count; tier_count=$(printf '%s' "$response" | jq 'if type == "array" then length else 0 end' 2>/dev/null)
    if [[ "${tier_count:-0}" -ge 1 ]]; then
      status="PASS"; severity="P3"; message="Pricing tiers loaded: ${tier_count} tier(s)"
    else
      status="WARN"; severity="P2"; message="Pricing endpoint responds but returned no tiers"
    fi
  else
    status="WARN"; severity="P2"; message="Pricing endpoint HTTP ${pricing_code} — unexpected response"
  fi
  emit_result "$phase" "$tier" "server-23-pricing" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Active billing sessions
  response=$(curl -s -m "$DEFAULT_TIMEOUT" \
    "http://192.168.31.23:8080/api/v1/billing/sessions/active" \
    -H "x-terminal-session: ${token:-}" 2>/dev/null | tr -d '\r')
  if [[ -n "$response" ]]; then
    # 0 active sessions is fine if venue closed
    status="PASS"; severity="P3"; message="Active billing sessions endpoint responding"
  elif [[ "$venue_state" != "closed" ]]; then
    status="WARN"; severity="P2"; message="Active billing sessions endpoint unreachable during venue hours"
  else
    status="QUIET"; severity="P3"; message="Active billing sessions endpoint unreachable (venue closed)"
  fi
  emit_result "$phase" "$tier" "server-23-billing-active" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Recent sessions (verify billing history exists)
  response=$(curl -s -m "$DEFAULT_TIMEOUT" \
    "http://192.168.31.23:8080/api/v1/billing/sessions?limit=3" \
    -H "x-terminal-session: ${token:-}" 2>/dev/null | tr -d '\r')
  if [[ -n "$response" ]]; then
    status="PASS"; severity="P3"; message="Billing sessions history endpoint responding"
  elif [[ "$venue_state" != "closed" ]]; then
    status="WARN"; severity="P2"; message="Billing sessions history unreachable during venue hours"
  else
    status="QUIET"; severity="P3"; message="Billing sessions history unreachable (venue closed)"
  fi
  emit_result "$phase" "$tier" "server-23-billing-history" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase21
