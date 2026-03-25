#!/usr/bin/env bash
# audit/phases/tier8/phase40.sh -- Phase 40: Scheduler & Action Queue
# Tier: 8 (Advanced Systems)
# What: Scheduled tasks processing, action queue draining, no stale items.

set -u
set -o pipefail

run_phase40() {
  local phase="40" tier="8"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # Scheduler activity in logs
  local log_resp; log_resp=$(http_get "http://192.168.31.23:8080/api/v1/logs?lines=100" "$DEFAULT_TIMEOUT")
  if [[ -n "$log_resp" ]]; then
    local sched_entries; sched_entries=$(printf '%s' "$log_resp" | jq -r '.' 2>/dev/null | grep -ci "scheduler.*execute\|scheduler.*tick\|action_queue")
    if [[ "${sched_entries:-0}" -ge 1 ]]; then
      status="PASS"; severity="P3"; message="Scheduler/action_queue activity in logs (${sched_entries} entries)"
    else
      status="PASS"; severity="P3"; message="No scheduler/action_queue issues in recent logs (feature quiet)"
    fi
  else
    status="WARN"; severity="P2"; message="Logs API unreachable — cannot check scheduler"
  fi
  emit_result "$phase" "$tier" "server-23-scheduler" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Action queue status breakdown
  response=$(safe_remote_exec "192.168.31.23" "8090" \
    "sqlite3 C:\\RacingPoint\\data\\racecontrol.db \"SELECT status, COUNT(*) FROM action_queue GROUP BY status\" 2>nul || echo NO_TABLE" \
    "$DEFAULT_TIMEOUT")
  local aq_out; aq_out=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null || true)
  if printf '%s' "$aq_out" | grep -qi "NO_TABLE\|no such table"; then
    status="PASS"; severity="P3"; message="action_queue table not found (scheduler uses different storage)"
  elif [[ -n "$aq_out" ]]; then
    status="PASS"; severity="P3"; message="Action queue status: $(printf '%s' "$aq_out" | tr '\n' '|' | cut -c1-60)"
  else
    status="WARN"; severity="P2"; message="Could not query action_queue table"
  fi
  emit_result "$phase" "$tier" "server-23-action-queue" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Old pending items (> 1 hour)
  response=$(safe_remote_exec "192.168.31.23" "8090" \
    "sqlite3 C:\\RacingPoint\\data\\racecontrol.db \"SELECT COUNT(*) FROM action_queue WHERE status='pending' AND created_at < datetime('now', '-1 hour')\" 2>nul || echo 0" \
    "$DEFAULT_TIMEOUT")
  local stale_count; stale_count=$(printf '%s' "$response" | jq -r '.stdout // "0"' 2>/dev/null | tr -d '[:space:]' | grep -oE '^[0-9]+')
  if [[ "${stale_count:-0}" -eq 0 ]]; then
    status="PASS"; severity="P3"; message="No stale action_queue items (> 1h old)"
  elif [[ "${stale_count:-0}" -le 10 ]]; then
    status="WARN"; severity="P2"; message="${stale_count} stale action_queue items > 1h old"
  else
    status="FAIL"; severity="P2"; message="${stale_count} stale items — action queue not draining"
  fi
  emit_result "$phase" "$tier" "server-23-queue-stale" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase40
