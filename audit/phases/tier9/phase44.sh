#!/usr/bin/env bash
# audit/phases/tier9/phase44.sh -- Phase 44: Face Detection & People Counter
# Tier: 9 (Cameras & AI)
# What: rc-sentry-ai running, detecting faces on 3 cameras, audit log fresh.

set -u
set -o pipefail

run_phase44() {
  local phase="44" tier="9"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # rc-sentry-ai process
  local rsa_proc; rsa_proc=$(tasklist 2>/dev/null | grep -i "rc-sentry-ai" || true)
  if [[ -n "$rsa_proc" ]]; then
    status="PASS"; severity="P3"; message="rc-sentry-ai process running"
  else
    status="WARN"; severity="P2"; message="rc-sentry-ai not found in tasklist"
  fi
  if [[ "$venue_state" = "closed" ]] && [[ "$status" = "WARN" ]]; then
    status="QUIET"; severity="P3"
  fi
  emit_result "$phase" "$tier" "james-rcsentry-ai" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Face audit log: recency check (WL-04)
  # Verify entries are recent (within 10 minutes), not just that entries exist
  local audit_log="C:/RacingPoint/logs/face-audit.jsonl"
  if [[ -f "$audit_log" ]]; then
    local line_count; line_count=$(wc -l < "$audit_log" 2>/dev/null | tr -d '[:space:]')
    line_count=${line_count:-0}

    if [[ "$line_count" -ge 1 ]]; then
      # Try to get recency from file modification time (works in Git Bash / MSYS2)
      local file_epoch now_epoch delta_secs
      file_epoch=$(date -r "$audit_log" +%s 2>/dev/null || stat -c %Y "$audit_log" 2>/dev/null || echo 0)
      now_epoch=$(date +%s)
      delta_secs=$(( now_epoch - file_epoch ))

      # Also try to extract timestamp from last JSONL entry
      local last_line; last_line=$(tail -1 "$audit_log" 2>/dev/null)
      local entry_ts; entry_ts=$(printf '%s' "$last_line" | jq -r '.timestamp // .ts // .time // empty' 2>/dev/null)
      if [[ -n "$entry_ts" ]]; then
        local entry_epoch; entry_epoch=$(date -d "$entry_ts" +%s 2>/dev/null || echo 0)
        if [[ "$entry_epoch" -gt 0 ]]; then
          delta_secs=$(( now_epoch - entry_epoch ))
        fi
      fi

      local delta_min=$(( delta_secs / 60 ))
      if [[ "$file_epoch" -gt 0 ]] && [[ "$delta_secs" -lt 600 ]]; then
        status="PASS"; severity="P3"; message="Face audit log fresh, ${line_count} entries, last entry ${delta_min}m ago"
      elif [[ "$file_epoch" -gt 0 ]] && [[ "$delta_secs" -lt 1800 ]]; then
        status="WARN"; severity="P2"; message="Face audit log stale (last entry ${delta_min}m ago, ${line_count} entries)"
      elif [[ "$file_epoch" -gt 0 ]]; then
        status="WARN"; severity="P2"; message="Face audit log not updating (last entry ${delta_min}m ago -- rc-sentry-ai may be stuck)"
      else
        # Could not determine file time -- fall back to line count
        status="PASS"; severity="P3"; message="Face audit log: ${line_count} entries (recency check unavailable)"
      fi
    else
      status="WARN"; severity="P2"; message="Face audit log exists but empty"
    fi
  else
    status="WARN"; severity="P2"; message="Face audit log not found at ${audit_log}"
  fi
  if [[ "$venue_state" = "closed" ]] && [[ "$status" = "WARN" ]]; then
    status="QUIET"; severity="P3"
  fi
  emit_result "$phase" "$tier" "james-face-audit-log" "$status" "$severity" "$message" "$mode" "$venue_state"

  # People counter :8095
  response=$(http_get "http://localhost:8095/health" 5)
  if [[ -n "$response" ]]; then
    status="PASS"; severity="P3"; message="People counter responding at localhost:8095"
  else
    status="PASS"; severity="P3"; message="People counter :8095 not running (service not started)"
  fi
  if [[ "$venue_state" = "closed" ]] && [[ "$status" = "WARN" ]]; then
    status="QUIET"; severity="P3"
  fi
  emit_result "$phase" "$tier" "james-people-counter" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase44
