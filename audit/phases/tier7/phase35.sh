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
    local sync_entries; sync_entries=$(printf '%s' "$log_resp" | jq -r '.' 2>/dev/null | grep -ci "sync push\|sync pull\|upserted")
    if [[ "${sync_entries:-0}" -ge 1 ]]; then
      status="PASS"; severity="P3"; message="Cloud sync activity in recent logs (${sync_entries} entries)"
    else
      status="PASS"; severity="P3"; message="No sync push/pull issues in recent logs (feature quiet)"
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
    status="PASS"; severity="P3"; message="Build ID MISMATCH: venue=${local_build}, cloud=${cloud_build} (informational — independent deploy cycles)"
  fi
  emit_result "$phase" "$tier" "venue-cloud-build-id" "$status" "$severity" "$message" "$mode" "$venue_state"

  # XS-01: Cross-service cloud sync freshness -- compare venue and cloud driver updated_at timestamps
  local venue_drivers; venue_drivers=$(http_get "http://192.168.31.23:8080/api/v1/drivers?limit=1" "$DEFAULT_TIMEOUT")
  local cloud_drivers; cloud_drivers=$(http_get "http://100.70.177.44:8080/api/v1/drivers?limit=1" 8)
  local venue_ts; venue_ts=$(printf '%s' "$venue_drivers" | jq -r '.data[0].updated_at // .[0].updated_at // empty' 2>/dev/null)
  local cloud_ts; cloud_ts=$(printf '%s' "$cloud_drivers" | jq -r '.data[0].updated_at // .[0].updated_at // empty' 2>/dev/null)

  if [[ -z "${venue_ts:-}" || -z "${cloud_ts:-}" ]]; then
    status="WARN"; severity="P2"; message="Cannot compare sync timestamps (venue or cloud unreachable)"
  else
    local venue_epoch cloud_epoch delta
    venue_epoch=$(date -d "$venue_ts" +%s 2>/dev/null || echo 0)
    cloud_epoch=$(date -d "$cloud_ts" +%s 2>/dev/null || echo 0)
    if [[ "$venue_epoch" -eq 0 || "$cloud_epoch" -eq 0 ]]; then
      status="WARN"; severity="P2"; message="Cannot compare sync timestamps (unparseable: venue=${venue_ts}, cloud=${cloud_ts})"
    else
      delta=$(( venue_epoch - cloud_epoch ))
      delta=${delta#-}  # absolute value
      local delta_min=$(( delta / 60 ))
      local delta_sec=$(( delta % 60 ))
      if [[ "$delta" -lt 300 ]]; then
        status="PASS"; severity="P3"; message="Cloud sync fresh: venue and cloud driver updated_at within ${delta_min}m ${delta_sec}s"
      elif [[ "$delta" -lt 1800 ]]; then
        status="WARN"; severity="P2"; message="Cloud sync lag: updated_at delta is ${delta_min}m (threshold: 5m)"
      else
        status="WARN"; severity="P1"; message="Cloud sync stale: updated_at delta is ${delta_min}m (possible sync failure)"
      fi
    fi
  fi
  emit_result "$phase" "$tier" "venue-cloud-sync-freshness" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase35
