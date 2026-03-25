#!/usr/bin/env bash
# audit/phases/tier2/phase11.sh -- Phase 11: API Data Integrity
# Tier: 2 (Core Services)
# What: Every API endpoint returns correct DATA, not just HTTP 200.
# Standing rule: Phase scripts always exit 0 -- errors encoded in emit_result status, never bash exit code.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase11() {
  local phase="11" tier="2"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # Fleet health -- must return pods array with real data
  response=$(http_get "http://192.168.31.23:8080/api/v1/fleet/health" "$DEFAULT_TIMEOUT")
  if [[ -n "$response" ]]; then
    local pod_count; pod_count=$(printf '%s' "$response" | jq 'length' 2>/dev/null)
    if [[ "${pod_count:-0}" -ge 8 ]]; then
      status="PASS"; severity="P3"; message="Fleet health: ${pod_count} pods in response"
    elif [[ "${pod_count:-0}" -ge 1 ]]; then
      status="WARN"; severity="P2"; message="Fleet health: only ${pod_count}/8 pods in response"
    else
      status="FAIL"; severity="P2"; message="Fleet health: empty response or no pods (possible DB issue)"
    fi
  else
    status="FAIL"; severity="P1"; message="Fleet health API unreachable"
  fi
  emit_result "$phase" "$tier" "server-23-fleet-health" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Logs API -- must reference .jsonl file (standing rule: rolling log filename)
  response=$(http_get "http://192.168.31.23:8080/api/v1/logs?lines=1" "$DEFAULT_TIMEOUT")
  if [[ -n "$response" ]]; then
    local log_file; log_file=$(printf '%s' "$response" | jq -r '.file // ""' 2>/dev/null || true)
    if printf '%s' "$log_file" | grep -q "\.jsonl"; then
      status="PASS"; severity="P3"; message="Logs API: referencing .jsonl file (${log_file##*/})"
    elif [[ -z "$log_file" ]]; then
      status="WARN"; severity="P2"; message="Logs API: .file field missing in response"
    else
      status="FAIL"; severity="P2"; message="Logs API: file is not .jsonl format: ${log_file##*/} (F12 standing rule)"
    fi
  else
    status="FAIL"; severity="P1"; message="Logs API unreachable"
  fi
  emit_result "$phase" "$tier" "server-23-logs-api" "$status" "$severity" "$message" "$mode" "$venue_state"

  # App health endpoint (v20.1)
  response=$(http_get "http://192.168.31.23:8080/api/v1/app-health" "$DEFAULT_TIMEOUT")
  if [[ -n "$response" ]]; then
    local ok_count; ok_count=$(printf '%s' "$response" | jq '[.[] | select(.status=="ok")] | length' 2>/dev/null)
    status="PASS"; severity="P3"; message="App health endpoint responding (${ok_count} apps ok)"
  else
    status="WARN"; severity="P2"; message="App health endpoint not responding (added in v20.1)"
  fi
  emit_result "$phase" "$tier" "server-23-app-health" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Server health fields completeness
  response=$(http_get "http://192.168.31.23:8080/api/v1/health" "$DEFAULT_TIMEOUT")
  if [[ -n "$response" ]]; then
    local has_build; has_build=$(printf '%s' "$response" | jq 'has("build_id")' 2>/dev/null || echo "false")
    if [[ "$has_build" = "true" ]]; then
      status="PASS"; severity="P3"; message="Server health has expected fields including build_id"
    else
      status="WARN"; severity="P2"; message="Server health missing build_id field"
    fi
  else
    status="FAIL"; severity="P1"; message="Server health endpoint unreachable"
  fi
  emit_result "$phase" "$tier" "server-23-health-schema" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase11
