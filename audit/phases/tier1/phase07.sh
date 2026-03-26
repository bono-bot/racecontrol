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
      jq '[.[] | .violation_count_24h // 0] | max' 2>/dev/null)
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
    local wl_count; wl_count=$(printf '%s' "$response" | jq '.processes | length' 2>/dev/null)
    if [[ "${wl_count:-0}" -ge 100 ]]; then
      status="PASS"; severity="P3"; message="Pod ${n} allowlist: ${wl_count} entries (populated)"
    elif [[ "${wl_count:-0}" -ge 10 ]]; then
      status="WARN"; severity="P2"; message="Pod ${n} allowlist: ${wl_count} entries (thin -- may generate false violations)"
    else
      status="FAIL"; severity="P2"; message="Pod ${n} allowlist: ${wl_count:-0} entries (empty -- all processes flagged)"
    fi
    emit_result "$phase" "$tier" "server-23-allowlist-pod${n}" "$status" "$severity" "$message" "$mode" "$venue_state"

    # CH-01: Allowlist content spot-verification -- svchost.exe must be present in any valid Windows allowlist
    if [[ "${wl_count:-0}" -ge 10 ]]; then
      if printf '%s' "$response" | jq -r '.processes[]?' 2>/dev/null | grep -qi 'svchost\.exe'; then
        status="PASS"; severity="P3"; message="Pod ${n} allowlist content verified (svchost.exe present)"
      else
        status="WARN"; severity="P2"; message="Pod ${n} allowlist populated (${wl_count}) but svchost.exe missing -- allowlist may be from wrong source"
      fi
      emit_result "$phase" "$tier" "server-23-allowlist-pod${n}-content" "$status" "$severity" "$message" "$mode" "$venue_state"
    fi
  done

  # XS-02: Cross-check allowlist background task recency (spot-check pod 1)
  # If safe_mode is inactive but allowlist was never refreshed, pods run on stale/empty list
  local pod1_ip; pod1_ip=$(printf '%s' "${PODS:-}" | awk '{print $1}')
  if [[ -n "$pod1_ip" ]]; then
    local log_resp; log_resp=$(safe_remote_exec "$pod1_ip" "8090" \
      'findstr /I "whitelist" C:\RacingPoint\rc-agent.jsonl' "$DEFAULT_TIMEOUT")
    local log_out; log_out=$(printf '%s' "$log_resp" | jq -r '.stdout // ""' 2>/dev/null | tail -1)
    if [[ -n "$log_out" ]]; then
      status="PASS"; severity="P3"; message="Allowlist refresh activity found in pod 1 logs"
    else
      status="WARN"; severity="P2"; message="No allowlist refresh entries in pod 1 logs -- background task may not be running"
    fi
  else
    status="WARN"; severity="P2"; message="Cannot check allowlist refresh -- no pod IPs available"
  fi
  if [[ "$venue_state" = "closed" ]] && [[ "$status" = "WARN" ]]; then
    status="QUIET"; severity="P3"
  fi
  emit_result "$phase" "$tier" "pod1-allowlist-refresh" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase07
