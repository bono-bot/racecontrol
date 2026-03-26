#!/usr/bin/env bash
# scripts/coordination/coord-state.sh — James/Bono coordination state
#
# Implements: COORD-01 (AUTO_DETECT_ACTIVE mutex), COORD-04 (completion marker)
#
# Source this file from auto-detect.sh and bono-auto-detect.sh.
# REPO_ROOT must be set before sourcing.

# Use set -uo pipefail but NOT set -e — sourced files with set -e cause silent
# exits on non-zero returns from functions like is_james_run_recent.
set -uo pipefail

# ─── Path constants (computed from REPO_ROOT set by caller) ──────────────────
COORD_LOCK_FILE="${REPO_ROOT}/audit/results/auto-detect-active.lock"
COORD_COMPLETION_FILE="${REPO_ROOT}/audit/results/last-run-summary.json"
COORD_STALE_SECS=600   # 10 minutes — skip Bono run if James completed within this window

# ─── write_active_lock ───────────────────────────────────────────────────────
# Writes JSON to $COORD_LOCK_FILE indicating James auto-detect is running.
# JSON: {"agent":"james","pid":$$,"started_ts":<unix epoch>,"relay_url":"$RELAY_URL"}
write_active_lock() {
  local ts
  ts=$(date +%s)
  local relay="${RELAY_URL:-http://localhost:8766}"
  mkdir -p "$(dirname "$COORD_LOCK_FILE")"
  jq -n \
    --arg agent "james" \
    --argjson pid $$ \
    --argjson started_ts "$ts" \
    --arg relay_url "$relay" \
    '{"agent":$agent,"pid":$pid,"started_ts":$started_ts,"relay_url":$relay_url}' \
    > "$COORD_LOCK_FILE"
}
export -f write_active_lock

# ─── clear_active_lock ───────────────────────────────────────────────────────
# Removes $COORD_LOCK_FILE if it exists. Silent on missing file.
clear_active_lock() {
  rm -f "$COORD_LOCK_FILE"
}
export -f clear_active_lock

# ─── write_completion_marker ─────────────────────────────────────────────────
# Writes JSON to $COORD_COMPLETION_FILE so Bono can check freshness.
# Args: $1=verdict  $2=bugs_found  $3=bugs_fixed
# RESULT_DIR must be in scope when called (it is a global in auto-detect.sh).
write_completion_marker() {
  local verdict="$1"
  local bugs_found="${2:-0}"
  local bugs_fixed="${3:-0}"
  local ts
  ts=$(date +%s)
  local run_dir="${RESULT_DIR:-unknown}"
  mkdir -p "$(dirname "$COORD_COMPLETION_FILE")"
  jq -n \
    --arg agent "james" \
    --argjson completed_ts "$ts" \
    --arg verdict "$verdict" \
    --argjson bugs_found "$bugs_found" \
    --argjson bugs_fixed "$bugs_fixed" \
    --arg run_dir "$run_dir" \
    '{"agent":$agent,"completed_ts":$completed_ts,"verdict":$verdict,"bugs_found":$bugs_found,"bugs_fixed":$bugs_fixed,"run_dir":$run_dir}' \
    > "$COORD_COMPLETION_FILE"
}
export -f write_completion_marker

# ─── is_james_run_recent ─────────────────────────────────────────────────────
# Returns 0 (true) if $COORD_COMPLETION_FILE exists AND completed_ts is within
# COORD_STALE_SECS of now. Returns 1 (false) otherwise.
# Used by Bono to decide whether to skip its scheduled run.
is_james_run_recent() {
  if [[ ! -f "$COORD_COMPLETION_FILE" ]]; then return 1; fi
  local now_ts completed_ts elapsed
  now_ts=$(date +%s)
  completed_ts=$(jq -r '.completed_ts // 0' "$COORD_COMPLETION_FILE" 2>/dev/null || echo "0")
  elapsed=$(( now_ts - completed_ts ))
  [[ "$elapsed" -lt "$COORD_STALE_SECS" ]]
}
export -f is_james_run_recent

# ─── read_active_lock ────────────────────────────────────────────────────────
# Outputs the content of $COORD_LOCK_FILE as JSON if it exists, else outputs {}.
# Used by Bono (or relay query) to check if James is currently running.
read_active_lock() {
  if [[ -f "$COORD_LOCK_FILE" ]]; then
    cat "$COORD_LOCK_FILE"
  else
    echo "{}"
  fi
}
export -f read_active_lock
