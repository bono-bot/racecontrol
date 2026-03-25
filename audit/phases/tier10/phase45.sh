#!/usr/bin/env bash
# audit/phases/tier10/phase45.sh -- Phase 45: Log Health and Rotation
# Tier: 10 (Ops and Compliance)
# What: Log files not bloated, rotation working, no flooding.
# Standing rules: OP-01 (log rotation), LOG-02 (error rate < 10/hour)

set -u
set -o pipefail
# NO set -e

run_phase45() {
  local phase="45" tier="10"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # --- Check 1: Server log sizes ---
  local exec_result; exec_result=$(safe_remote_exec "192.168.31.23" "8090" \
    "for %f in (C:\\RacingPoint\\logs\\racecontrol-*.jsonl) do echo %f %~zf" 10)
  local server_stdout; server_stdout=$(printf '%s' "$exec_result" | jq -r '.stdout // ""' 2>/dev/null || true)
  if [[ -n "$server_stdout" ]]; then
    # Find any file size > 50MB (50000000 bytes)
    local oversize; oversize=$(printf '%s' "$server_stdout" | awk '{if ($NF+0 > 50000000) print $0}' || true)
    if [[ -n "$oversize" ]]; then
      status="WARN"; severity="P2"; message="Server log(s) > 50MB: $oversize"
    else
      status="PASS"; severity="P3"; message="Server logs all < 50MB (rotation healthy)"
    fi
  else
    status="WARN"; severity="P2"; message="Could not read server log sizes (exec failed or no logs)"
  fi
  emit_result "$phase" "$tier" "server-23-logs" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 2: Pod log sizes (spot check pod 1 and pod 4) ---
  local pod1; pod1=$(printf '%s' "${PODS:-}" | awk '{print $1}')
  local pod4; pod4=$(printf '%s' "${PODS:-}" | awk '{print $4}')
  for pod_ip in "$pod1" "$pod4"; do
    [[ -z "$pod_ip" ]] && continue
    local pod_result; pod_result=$(safe_remote_exec "$pod_ip" "8090" \
      "for %f in (C:\\RacingPoint\\rc-agent-*.jsonl) do echo %f %~zf" 10)
    local pod_stdout; pod_stdout=$(printf '%s' "$pod_result" | jq -r '.stdout // ""' 2>/dev/null || true)
    local pod_host; pod_host="pod-$(printf '%s' "$pod_ip" | awk -F'.' '{print $4}')-logs"
    if [[ -n "$pod_stdout" ]]; then
      local pod_oversize; pod_oversize=$(printf '%s' "$pod_stdout" | awk '{if ($NF+0 > 50000000) print $0}' || true)
      if [[ -n "$pod_oversize" ]]; then
        status="WARN"; severity="P2"; message="Pod $pod_ip log(s) > 50MB: $pod_oversize"
      else
        status="PASS"; severity="P3"; message="Pod $pod_ip logs all < 50MB"
      fi
    else
      status="WARN"; severity="P2"; message="Pod $pod_ip log size check failed (agent may be offline)"
    fi
    if [[ "$venue_state" = "closed" ]] && [[ "$status" = "FAIL" || "$status" = "WARN" ]]; then
      status="QUIET"; severity="P3"
    fi
    emit_result "$phase" "$tier" "$pod_host" "$status" "$severity" "$message" "$mode" "$venue_state"
  done

  # --- Check 3: James rc-sentry-ai log size ---
  local sentry_size; sentry_size=$(stat -c %s C:/RacingPoint/rc-sentry-ai.log 2>/dev/null)
  if [[ "$sentry_size" = "0" ]]; then
    status="WARN"; severity="P2"; message="rc-sentry-ai.log missing or empty on James"
  elif [[ "$sentry_size" -gt 50000000 ]]; then
    status="WARN"; severity="P2"; message="rc-sentry-ai.log > 50MB (${sentry_size} bytes) — rotation needed"
  else
    status="PASS"; severity="P3"; message="rc-sentry-ai.log exists and < 50MB (${sentry_size} bytes)"
  fi
  emit_result "$phase" "$tier" "james-sentry-logs" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 4: Error rate from server log API ---
  response=$(http_get "http://192.168.31.23:8080/api/v1/logs?level=error&lines=1" 10)
  if [[ -n "$response" ]]; then
    local error_count; error_count=$(printf '%s' "$response" | jq -r '.filtered // 0' 2>/dev/null)
    if [[ "${error_count:-0}" -lt 10 ]]; then
      status="PASS"; severity="P3"; message="Error rate: ${error_count} (< 10 threshold)"
    else
      status="WARN"; severity="P2"; message="Error rate elevated: ${error_count} errors (>= 10 threshold)"
    fi
  else
    status="WARN"; severity="P2"; message="Log API not responding — cannot check error rate"
  fi
  emit_result "$phase" "$tier" "server-23-error-rate" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase45
