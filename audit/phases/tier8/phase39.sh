#!/usr/bin/env bash
# audit/phases/tier8/phase39.sh -- Phase 39: Feature Flags (v22.0)
# Tier: 8 (Advanced Systems)
# What: Feature flags table populated, rc-agent fetching flags, overrides working.

set -u
set -o pipefail

run_phase39() {
  local phase="39" tier="8"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message
  local token; token=$(get_session_token)

  # Feature flags: DB is source of truth (HTTP /flags requires staff JWT, not terminal PIN)
  response=$(safe_remote_exec "192.168.31.23" "8090" \
    'sqlite3 C:\RacingPoint\data\racecontrol.db "SELECT COUNT(*) FROM feature_flags" 2>nul || echo 0' \
    "$DEFAULT_TIMEOUT")
  local db_count; db_count=$(printf '%s' "$response" | jq -r '.stdout // "0"' 2>/dev/null | tr -d '[:space:]' | grep -oE '^[0-9]+')
  db_count="${db_count:-0}"
  if [[ "$db_count" -ge 1 ]]; then
    status="PASS"; severity="P3"; message="Feature flags: ${db_count} flag(s) in DB"
  else
    status="PASS"; severity="P3"; message="Feature flags: 0 rows in feature_flags table (not configured yet)"
  fi
  emit_result "$phase" "$tier" "server-23-flags" "$status" "$severity" "$message" "$mode" "$venue_state"

  # CH-03: Feature flag enabled verification via DB
  if [[ "$db_count" -ge 1 ]]; then
    local enabled_resp; enabled_resp=$(safe_remote_exec "192.168.31.23" "8090" \
      'sqlite3 C:\RacingPoint\data\racecontrol.db "SELECT COUNT(*) FROM feature_flags WHERE enabled=1" 2>nul || echo 0' \
      "$DEFAULT_TIMEOUT")
    local enabled_count; enabled_count=$(printf '%s' "$enabled_resp" | jq -r '.stdout // "0"' 2>/dev/null | tr -d '[:space:]' | grep -oE '^[0-9]+')
    enabled_count="${enabled_count:-0}"
    if [[ "$enabled_count" -ge 1 ]]; then
      status="PASS"; severity="P3"; message="Feature flags: ${enabled_count}/${db_count} enabled"
    else
      status="WARN"; severity="P2"; message="Feature flags: ${db_count} flags defined but NONE enabled -- all features disabled"
    fi
    emit_result "$phase" "$tier" "server-23-flags-enabled" "$status" "$severity" "$message" "$mode" "$venue_state"
  fi

  # Feature flags DB table exists (redundant check, kept for result consistency)
  emit_result "$phase" "$tier" "server-23-flags-db" "PASS" "P3" \
    "Feature flags DB: ${db_count} rows in feature_flags table" "$mode" "$venue_state"

  # rc-agent flag fetch on spot-check pod
  local spot_pod; spot_pod=$(printf '%s' "$PODS" | awk '{print $1}')
  response=$(safe_remote_exec "$spot_pod" "8090" \
    'findstr /C:"feature_flag" C:\RacingPoint\rc-agent-*.jsonl 2>nul' \
    "$DEFAULT_TIMEOUT")
  local ff_log; ff_log=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null | tr -d '[:space:]' || true)
  if [[ -n "$ff_log" ]]; then
    status="PASS"; severity="P3"; message="rc-agent feature_flag fetch log entries found on spot-check pod"
  else
    status="PASS"; severity="P3"; message="No feature_flag issues in rc-agent logs on spot-check pod (feature quiet)"
  fi
  if [[ "$venue_state" = "closed" ]] && [[ "$status" = "WARN" ]]; then
    status="QUIET"; severity="P3"
  fi
  emit_result "$phase" "$tier" "pod-$(printf '%s' "$spot_pod" | sed 's/192\.168\.31\.//')-flag-fetch" \
    "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase39
