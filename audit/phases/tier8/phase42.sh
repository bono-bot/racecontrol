#!/usr/bin/env bash
# audit/phases/tier8/phase42.sh -- Phase 42: Error Aggregator & Fleet Alerts
# Tier: 8 (Advanced Systems)
# What: Error rates tracked, fleet alerts dispatching, escalation chain working.

set -u
set -o pipefail

run_phase42() {
  local phase="42" tier="8"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # Error aggregator logs
  local log_resp; log_resp=$(http_get "http://192.168.31.23:8080/api/v1/logs?lines=100" "$DEFAULT_TIMEOUT")
  if [[ -n "$log_resp" ]]; then
    local agg_entries; agg_entries=$(printf '%s' "$log_resp" | jq -r '.' 2>/dev/null | grep -ci "error_aggregator\|error_rate")
    if [[ "${agg_entries:-0}" -ge 1 ]]; then
      status="PASS"; severity="P3"; message="Error aggregator active in logs (${agg_entries} entries)"
    else
      status="WARN"; severity="P2"; message="No error_aggregator entries in recent logs"
    fi
  else
    status="WARN"; severity="P2"; message="Logs API unreachable — cannot check error aggregator"
  fi
  emit_result "$phase" "$tier" "server-23-error-aggregator" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Fleet alert dispatch
  if [[ -n "$log_resp" ]]; then
    local alert_entries; alert_entries=$(printf '%s' "$log_resp" | jq -r '.' 2>/dev/null | grep -ci "fleet_alert.*dispatch\|fleet_alert.*send")
    if [[ "${alert_entries:-0}" -ge 1 ]]; then
      status="PASS"; severity="P3"; message="Fleet alert dispatch entries found (${alert_entries})"
    else
      status="WARN"; severity="P2"; message="No fleet_alert dispatch entries in recent logs"
    fi
  else
    status="WARN"; severity="P2"; message="Logs API unreachable — cannot check fleet alerts"
  fi
  emit_result "$phase" "$tier" "server-23-fleet-alerts" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Current error rate from filtered logs endpoint
  local err_resp; err_resp=$(http_get "http://192.168.31.23:8080/api/v1/logs?level=error&lines=1" "$DEFAULT_TIMEOUT")
  if [[ -n "$err_resp" ]]; then
    local filtered_count; filtered_count=$(printf '%s' "$err_resp" | jq -r '.filtered // 0' 2>/dev/null)
    if [[ "${filtered_count:-0}" -le 50 ]]; then
      status="PASS"; severity="P3"; message="Error rate: ${filtered_count}/hour (normal)"
    elif [[ "${filtered_count:-0}" -le 500 ]]; then
      status="WARN"; severity="P2"; message="Error rate elevated: ${filtered_count}/hour (> 50, typical during audit probes)"
    else
      status="FAIL"; severity="P1"; message="Error rate CRITICAL: ${filtered_count}/hour (> 500 — investigate)"
    fi
  else
    status="WARN"; severity="P2"; message="Logs API unreachable — cannot check error rate"
  fi
  emit_result "$phase" "$tier" "server-23-error-rate" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase42
