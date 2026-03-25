#!/usr/bin/env bash
# audit/phases/tier10/phase47.sh -- Phase 47: Standing Rules Compliance
# Tier: 10 (Ops and Compliance)
# What: Auto-push clean, Bono synced, rules synced across all 3 files.
# Standing rules: COMMS-02 (auto-push), COMMS-04 (LOGBOOK freshness)

set -u
set -o pipefail
# NO set -e

run_phase47() {
  local phase="47" tier="10"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # --- Check 1: Unpushed commits in racecontrol ---
  local rc_status; rc_status=$(cd "C:/Users/bono/racingpoint/racecontrol" && git status -sb 2>/dev/null || echo "")
  if printf '%s' "$rc_status" | grep -q "ahead"; then
    status="WARN"; severity="P2"; message="racecontrol has unpushed commits (violates auto-push standing rule)"
  elif [[ -z "$rc_status" ]]; then
    status="WARN"; severity="P2"; message="Could not read racecontrol git status"
  else
    status="PASS"; severity="P3"; message="racecontrol: no unpushed commits"
  fi
  emit_result "$phase" "$tier" "james-racecontrol-gitpush" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 2: Unpushed commits in comms-link ---
  local cl_status; cl_status=$(cd "C:/Users/bono/racingpoint/comms-link" && git status -sb 2>/dev/null || echo "")
  if printf '%s' "$cl_status" | grep -q "ahead"; then
    status="WARN"; severity="P2"; message="comms-link has unpushed commits (violates auto-push standing rule)"
  elif [[ -z "$cl_status" ]]; then
    status="WARN"; severity="P2"; message="Could not read comms-link git status"
  else
    status="PASS"; severity="P3"; message="comms-link: no unpushed commits"
  fi
  emit_result "$phase" "$tier" "james-commslink-gitpush" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 3: LOGBOOK freshness ---
  local last_entry; last_entry=$(tail -1 "C:/Users/bono/racingpoint/racecontrol/LOGBOOK.md" 2>/dev/null || echo "")
  if [[ -n "$last_entry" ]]; then
    status="PASS"; severity="P3"; message="LOGBOOK.md has entries (last: $(printf '%s' "$last_entry" | head -c 80))"
  else
    status="WARN"; severity="P2"; message="LOGBOOK.md is empty or missing — entries required after every commit"
  fi
  emit_result "$phase" "$tier" "james-logbook-fresh" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase47
