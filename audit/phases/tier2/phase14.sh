#!/usr/bin/env bash
# audit/phases/tier2/phase14.sh -- Phase 14: rc-sentry Health
# Tier: 2 (Core Services)
# What: Every pod's rc-sentry is running and can exec commands.
# Standing rule: Phase scripts always exit 0 -- errors encoded in emit_result status, never bash exit code.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase14() {
  local phase="14" tier="2"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  local ip host
  for ip in $PODS; do
    host="pod-$(printf '%s' "$ip" | sed 's/192\.168\.31\.//')"

    # Sentry health
    response=$(http_get "http://${ip}:8091/health" "$DEFAULT_TIMEOUT")
    if [[ -n "$response" ]] && printf '%s' "$response" | grep -q "build_id"; then
      status="PASS"; severity="P3"; message="rc-sentry healthy"
    elif [[ -z "$response" ]]; then
      status="FAIL"; severity="P2"; message="rc-sentry unreachable on :8091"
    else
      status="WARN"; severity="P2"; message="rc-sentry unexpected response: ${response:0:50}"
    fi
    if [[ "$venue_state" = "closed" ]] && [[ "$status" = "FAIL" || "$status" = "WARN" ]]; then
      status="QUIET"; severity="P3"
    fi
    emit_result "$phase" "$tier" "${host}-rcsentry-health" "$status" "$severity" "$message" "$mode" "$venue_state"

    # Sentry exec capability
    response=$(safe_remote_exec "$ip" "8091" "hostname" "$DEFAULT_TIMEOUT")
    local sentry_stdout; sentry_stdout=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null | tr -d '[:space:]' || true)
    if [[ -n "$sentry_stdout" ]]; then
      status="PASS"; severity="P3"; message="rc-sentry exec capability OK"
    else
      status="WARN"; severity="P2"; message="rc-sentry exec returned no stdout"
    fi
    if [[ "$venue_state" = "closed" ]] && [[ "$status" = "WARN" ]]; then
      status="QUIET"; severity="P3"
    fi
    emit_result "$phase" "$tier" "${host}-rcsentry-exec" "$status" "$severity" "$message" "$mode" "$venue_state"

    # Sentry can see rc-agent process
    response=$(safe_remote_exec "$ip" "8091" \
      'tasklist /NH | findstr rc-agent.exe' \
      "$DEFAULT_TIMEOUT")
    local agent_visible; agent_visible=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null || true)
    if printf '%s' "$agent_visible" | grep -qi "rc-agent"; then
      status="PASS"; severity="P3"; message="rc-sentry can see rc-agent.exe process"
    else
      status="WARN"; severity="P2"; message="rc-sentry cannot detect rc-agent.exe process (dead or not running)"
    fi
    if [[ "$venue_state" = "closed" ]] && [[ "$status" = "WARN" ]]; then
      status="QUIET"; severity="P3"
    fi
    emit_result "$phase" "$tier" "${host}-rcsentry-sees-agent" "$status" "$severity" "$message" "$mode" "$venue_state"
  done

  return 0
}
export -f run_phase14
