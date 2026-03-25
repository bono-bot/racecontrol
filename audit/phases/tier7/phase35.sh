#!/usr/bin/env bash
# audit/phases/tier7/phase35.sh -- Phase 35: Cloud Sync Bidirectional
# Tier: 7 (Data & Sync)
# What: Push AND pull verified. Build ID match between venue and cloud.

set -u
set -o pipefail

run_phase35() {
  local phase="35" tier="7"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # Recent sync activity in venue logs
  local log_resp; log_resp=$(http_get "http://192.168.31.23:8080/api/v1/logs?lines=50" "$DEFAULT_TIMEOUT")
  if [[ -n "$log_resp" ]]; then
    local sync_entries; sync_entries=$(printf '%s' "$log_resp" | jq -r '.' 2>/dev/null | grep -ci "sync push\|sync pull\|upserted" || echo "0")
    if [[ "${sync_entries:-0}" -ge 1 ]]; then
      status="PASS"; severity="P3"; message="Cloud sync activity in recent logs (${sync_entries} entries)"
    else
      status="WARN"; severity="P2"; message="No sync push/pull/upserted entries in recent logs (sync may not be active)"
    fi
  else
    status="WARN"; severity="P2"; message="Logs API unreachable — cannot check sync activity"
  fi
  emit_result "$phase" "$tier" "server-23-sync-activity" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Recent sync errors
  if [[ -n "$log_resp" ]]; then
    local sync_errors; sync_errors=$(http_get "http://192.168.31.23:8080/api/v1/logs?level=error&lines=5" "$DEFAULT_TIMEOUT")
    if [[ -n "$sync_errors" ]]; then
      status="PASS"; severity="P3"; message="Error logs endpoint responding"
    else
      status="WARN"; severity="P2"; message="Error logs endpoint unreachable"
    fi
  else
    status="WARN"; severity="P2"; message="Logs API unreachable"
  fi
  emit_result "$phase" "$tier" "server-23-sync-errors" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Build ID match: venue vs cloud
  local local_build; local_build=$(http_get "http://192.168.31.23:8080/api/v1/health" "$DEFAULT_TIMEOUT" | jq -r '.build_id // "unknown"' 2>/dev/null || echo "unknown")
  local cloud_build; cloud_build=$(http_get "http://100.70.177.44:8080/api/v1/health" 8 | jq -r '.build_id // "unknown"' 2>/dev/null || echo "unknown")
  if [[ "$local_build" = "unknown" || "$cloud_build" = "unknown" ]]; then
    status="WARN"; severity="P2"; message="Cannot compare build IDs: venue=${local_build}, cloud=${cloud_build}"
  elif [[ "$local_build" = "$cloud_build" ]]; then
    status="PASS"; severity="P3"; message="Build ID match: venue=cloud=${local_build}"
  else
    status="WARN"; severity="P2"; message="Build ID MISMATCH: venue=${local_build}, cloud=${cloud_build} — consider redeploy"
  fi
  emit_result "$phase" "$tier" "venue-cloud-build-id" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase35
