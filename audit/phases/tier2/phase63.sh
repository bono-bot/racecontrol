#!/usr/bin/env bash
# audit/phases/tier2/phase63.sh -- Phase 63: Boot Resilience Check
# Tier: 2 (Core Services)
# What: Verify periodic_tasks (boot resilience background tasks) are running on each pod.
# Standing rule: Phase scripts always exit 0 -- errors encoded in emit_result status, never bash exit code.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase63() {
  local phase="63" tier="2"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local status severity message

  local ip host
  for ip in $PODS; do
    host="pod-$(printf '%s' "$ip" | sed 's/192\.168\.31\.//')"

    local health_resp
    health_resp=$(http_get "http://${ip}:8090/health" "$DEFAULT_TIMEOUT")
    if [[ -z "$health_resp" ]]; then
      if [[ "$venue_state" == "closed" ]]; then
        status="QUIET"; severity="P3"
        message="Boot resilience check skipped -- pod unreachable, venue closed"
      else
        status="FAIL"; severity="P2"
        message="Boot resilience check failed -- pod unreachable"
      fi
      emit_result "$phase" "$tier" "${host}-boot-resilience" "$status" "$severity" "$message" "$mode" "$venue_state"
      continue
    fi

    # Check for periodic_tasks field in health response
    local has_periodic_tasks
    has_periodic_tasks=$(printf '%s' "$health_resp" | jq 'has("periodic_tasks")' 2>/dev/null || echo "false")

    if [[ "$has_periodic_tasks" == "true" ]]; then
      # Check task statuses
      local failed_count running_count total_count
      failed_count=$(printf '%s' "$health_resp" | jq '[.periodic_tasks[] | select(.status == "failed")] | length' 2>/dev/null || echo "0")
      running_count=$(printf '%s' "$health_resp" | jq '[.periodic_tasks[] | select(.status == "running")] | length' 2>/dev/null || echo "0")
      total_count=$(printf '%s' "$health_resp" | jq '.periodic_tasks | length' 2>/dev/null || echo "0")

      if [[ "${failed_count:-0}" -gt 0 ]]; then
        status="FAIL"; severity="P2"
        message="Boot resilience: ${failed_count}/${total_count} periodic tasks FAILED"
      elif [[ "${running_count:-0}" -gt 0 ]]; then
        status="PASS"; severity="P3"
        message="Boot resilience: ${running_count}/${total_count} periodic tasks running"
      else
        status="WARN"; severity="P2"
        message="Boot resilience: ${total_count} periodic tasks found but none running"
      fi
    else
      # periodic_tasks field not present -- older build without this feature
      status="WARN"; severity="P3"
      message="Boot resilience: periodic_tasks field not in health response (agent may be pre-v25.0 build)"
    fi
    emit_result "$phase" "$tier" "${host}-boot-resilience" "$status" "$severity" "$message" "$mode" "$venue_state"
  done

  return 0
}
export -f run_phase63
