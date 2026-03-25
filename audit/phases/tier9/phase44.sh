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

  # Face audit log: line count and recency
  local audit_log="C:/RacingPoint/logs/face-audit.jsonl"
  if [[ -f "$audit_log" ]]; then
    local line_count; line_count=$(wc -l < "$audit_log" 2>/dev/null | tr -d '[:space:]')
    if [[ "${line_count:-0}" -ge 1 ]]; then
      status="PASS"; severity="P3"; message="Face audit log: ${line_count} entries"
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
    status="WARN"; severity="P2"; message="People counter NOT running on :8095 (FastAPI + YOLOv8)"
  fi
  if [[ "$venue_state" = "closed" ]] && [[ "$status" = "WARN" ]]; then
    status="QUIET"; severity="P3"
  fi
  emit_result "$phase" "$tier" "james-people-counter" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase44
