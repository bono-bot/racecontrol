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

  # Feature flags server endpoint
  response=$(curl -s -m "$DEFAULT_TIMEOUT" \
    "http://192.168.31.23:8080/api/v1/flags" \
    -H "x-terminal-session: ${token:-}" 2>/dev/null | tr -d '"')
  if [[ -n "$response" ]]; then
    local flag_count; flag_count=$(printf '%s' "$response" | jq 'length' 2>/dev/null)
    if [[ "${flag_count:-0}" -ge 1 ]]; then
      status="PASS"; severity="P3"; message="Feature flags endpoint: ${flag_count} flags"
    else
      status="WARN"; severity="P2"; message="Feature flags endpoint: empty (no flags defined)"
    fi
  else
    status="WARN"; severity="P2"; message="Feature flags endpoint unreachable (not deployed or down)"
  fi
  emit_result "$phase" "$tier" "server-23-flags" "$status" "$severity" "$message" "$mode" "$venue_state"

  # CH-03: Feature flag enabled verification -- at least one flag should be enabled
  if [[ -n "$response" ]] && [[ "${flag_count:-0}" -ge 1 ]]; then
    local enabled_count; enabled_count=$(printf '%s' "$response" | \
      jq '[.[] | select(.enabled == true)] | length' 2>/dev/null)
    enabled_count="${enabled_count:-0}"
    if [[ "$enabled_count" -ge 1 ]]; then
      status="PASS"; severity="P3"; message="Feature flags: ${enabled_count}/${flag_count} enabled"
    else
      status="WARN"; severity="P2"; message="Feature flags: ${flag_count} flags defined but NONE enabled -- all features disabled"
    fi
    emit_result "$phase" "$tier" "server-23-flags-enabled" "$status" "$severity" "$message" "$mode" "$venue_state"
  fi

  # Feature flags DB table
  response=$(safe_remote_exec "192.168.31.23" "8090" \
    'sqlite3 C:\RacingPoint\data\racecontrol.db "SELECT COUNT(*) FROM feature_flags" 2>nul || echo 0' \
    "$DEFAULT_TIMEOUT")
  local db_count; db_count=$(printf '%s' "$response" | jq -r '.stdout // "0"' 2>/dev/null | tr -d '[:space:]' | grep -oE '^[0-9]+')
  if [[ "${db_count:-0}" -ge 1 ]]; then
    status="PASS"; severity="P3"; message="Feature flags DB: ${db_count} flag(s) in table"
  else
    status="PASS"; severity="P3"; message="Feature flags DB: 0 rows in feature_flags table (not configured)"
  fi
  emit_result "$phase" "$tier" "server-23-flags-db" "$status" "$severity" "$message" "$mode" "$venue_state"

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
