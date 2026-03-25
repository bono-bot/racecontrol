#!/usr/bin/env bash
# audit/phases/tier1/phase07.sh -- Phase 07: Process Guard & Allowlist
# Tier: 1 (Infrastructure Foundation)
# What: Guard scanning, violation count trending down, allowlist populated.
# Standing rule: Phase scripts always exit 0 -- errors encoded in emit_result status, never bash exit code.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase07() {
  local phase="07" tier="1"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # Fleet health -- pod violation_count_24h (high = empty allowlist or misconfigured guard)
  response=$(http_get "http://192.168.31.23:8080/api/v1/fleet/health" "$DEFAULT_TIMEOUT")
  if [[ -n "$response" ]]; then
    local max_violations; max_violations=$(printf '%s' "$response" | \
      jq '[.[] | .violation_count_24h // 0] | max' 2>/dev/null || echo "0")
    if [[ "${max_violations:-0}" -le 10 ]]; then
      status="PASS"; severity="P3"; message="Process guard violations normal: max=${max_violations} per pod"
    elif [[ "${max_violations:-0}" -le 100 ]]; then
      status="WARN"; severity="P2"; message="Process guard violations elevated: max=${max_violations} (allowlist may be incomplete)"
    else
      status="FAIL"; severity="P2"; message="Process guard violations at max (${max_violations}) -- likely empty allowlist (standing rule: empty allowlist blocks all)"
    fi
  else
    status="WARN"; severity="P2"; message="Fleet health API unreachable -- cannot check violation counts"
  fi
  emit_result "$phase" "$tier" "server-23-violations" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Allowlist count per pod (should be > 100 if populated)
  local n
  for n in 1 2 3 4 5 6 7 8; do
    response=$(http_get "http://192.168.31.23:8080/api/v1/guard/whitelist/pod-${n}" "$DEFAULT_TIMEOUT")
    local wl_count; wl_count=$(printf '%s' "$response" | jq 'length' 2>/dev/null || echo "0")
    if [[ "${wl_count:-0}" -ge 100 ]]; then
      status="PASS"; severity="P3"; message="Pod ${n} allowlist: ${wl_count} entries (populated)"
    elif [[ "${wl_count:-0}" -ge 10 ]]; then
      status="WARN"; severity="P2"; message="Pod ${n} allowlist: ${wl_count} entries (thin -- may generate false violations)"
    else
      status="FAIL"; severity="P2"; message="Pod ${n} allowlist: ${wl_count:-0} entries (empty -- all processes flagged)"
    fi
    emit_result "$phase" "$tier" "server-23-allowlist-pod${n}" "$status" "$severity" "$message" "$mode" "$venue_state"
  done

  return 0
}
export -f run_phase07
