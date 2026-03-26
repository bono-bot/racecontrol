#!/usr/bin/env bash
# audit/test/test-coordination.sh -- Offline test suite for coordination mutex (TEST-04)
# 6 tests: COORD-LOCK-WRITE COORD-LOCK-CLEAR COORD-STALE-DETECT COORD-STALE-EXPIRED
#          COORD-MUTEX-RACE COORD-SYNTAX
#
# Usage: bash audit/test/test-coordination.sh
# Exit: 0 if all 6 tests pass, 1 if any fails

set -u
set -o pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

PASS_COUNT=0
FAIL_COUNT=0
_pass() { local name="$1"; echo "PASS: $name"; PASS_COUNT=$((PASS_COUNT + 1)); }
_fail() { local name="$1"; local reason="${2:-}"; echo "FAIL: $name${reason:+ -- $reason}"; FAIL_COUNT=$((FAIL_COUNT + 1)); }

echo "=== audit/test/test-coordination.sh ==="
echo ""
echo "--- TEST-04: Coordination Mutex ---"
echo ""

# ---- COORD-LOCK-WRITE ----
TEST="COORD-LOCK-WRITE: write_active_lock creates valid JSON with agent=james and pid>0"
(
  tmp_dir=$(mktemp -d)
  export REPO_ROOT
  export COORD_LOCK_FILE="$tmp_dir/lock.json"
  export RELAY_URL="http://localhost:8766"
  source "$REPO_ROOT/scripts/coordination/coord-state.sh" 2>/dev/null
  write_active_lock
  if [[ ! -f "$COORD_LOCK_FILE" ]]; then rm -rf "$tmp_dir"; exit 1; fi
  agent=$(jq -r .agent "$COORD_LOCK_FILE" 2>/dev/null || echo "")
  pid_val=$(jq -r .pid "$COORD_LOCK_FILE" 2>/dev/null || echo "0")
  rm -rf "$tmp_dir"
  [[ "$agent" == "james" ]] && [[ "$pid_val" -gt 0 ]] 2>/dev/null && exit 0 || exit 1
) 2>/dev/null
if [ $? -eq 0 ]; then _pass "$TEST"; else _fail "$TEST" "lock file missing or agent!=james or pid<=0"; fi

# ---- COORD-LOCK-CLEAR ----
TEST="COORD-LOCK-CLEAR: write then clear -- lock file absent"
(
  tmp_dir=$(mktemp -d)
  export REPO_ROOT
  export COORD_LOCK_FILE="$tmp_dir/lock.json"
  export RELAY_URL="http://localhost:8766"
  source "$REPO_ROOT/scripts/coordination/coord-state.sh" 2>/dev/null
  write_active_lock
  clear_active_lock
  result=0
  [[ ! -f "$COORD_LOCK_FILE" ]] || result=1
  rm -rf "$tmp_dir"
  exit "$result"
) 2>/dev/null
if [ $? -eq 0 ]; then _pass "$TEST"; else _fail "$TEST" "COORD_LOCK_FILE still present after clear_active_lock"; fi

# ---- COORD-STALE-DETECT (fresh marker) ----
TEST="COORD-STALE-DETECT: fresh completion marker -- is_james_run_recent returns 0 (true)"
(
  tmp_dir=$(mktemp -d)
  export REPO_ROOT
  export COORD_LOCK_FILE="$tmp_dir/lock.json"
  export COORD_COMPLETION_FILE="$tmp_dir/completion.json"
  export COORD_STALE_SECS=600
  export RELAY_URL="http://localhost:8766"
  source "$REPO_ROOT/scripts/coordination/coord-state.sh" 2>/dev/null
  write_completion_marker "ok" 0 0
  is_james_run_recent
  result=$?
  rm -rf "$tmp_dir"
  exit "$result"
) 2>/dev/null
if [ $? -eq 0 ]; then _pass "$TEST"; else _fail "$TEST" "is_james_run_recent returned false for fresh marker"; fi

# ---- COORD-STALE-EXPIRED (epoch 1000 marker) ----
TEST="COORD-STALE-EXPIRED: completion marker with ts=1000 -- is_james_run_recent returns 1 (stale)"
(
  tmp_dir=$(mktemp -d)
  export REPO_ROOT
  export COORD_LOCK_FILE="$tmp_dir/lock.json"
  export COORD_COMPLETION_FILE="$tmp_dir/completion.json"
  export COORD_STALE_SECS=600
  export RELAY_URL="http://localhost:8766"
  source "$REPO_ROOT/scripts/coordination/coord-state.sh" 2>/dev/null
  printf '{"agent":"james","completed_ts":1000,"verdict":"ok","bugs_found":0,"bugs_fixed":0,"run_dir":"x"}
' > "$COORD_COMPLETION_FILE"
  is_james_run_recent && result=1 || result=0
  rm -rf "$tmp_dir"
  exit "$result"
) 2>/dev/null
if [ $? -eq 0 ]; then _pass "$TEST"; else _fail "$TEST" "is_james_run_recent returned true for epoch-1000 marker (should be stale)"; fi

# ---- COORD-MUTEX-RACE ----
TEST="COORD-MUTEX-RACE: two concurrent write_active_lock calls -- final file is valid JSON with agent=james"
(
  tmp_dir=$(mktemp -d)
  export REPO_ROOT
  lock_file="$tmp_dir/lock.json"
  export RELAY_URL="http://localhost:8766"
  source "$REPO_ROOT/scripts/coordination/coord-state.sh" 2>/dev/null
  export COORD_LOCK_FILE="$lock_file"
  ( export COORD_LOCK_FILE="$lock_file"; write_active_lock ) &
  pid1=$!
  ( export COORD_LOCK_FILE="$lock_file"; write_active_lock ) &
  pid2=$!
  wait "$pid1" "$pid2"
  if [[ ! -f "$lock_file" ]]; then rm -rf "$tmp_dir"; exit 1; fi
  agent=$(jq -r .agent "$lock_file" 2>/dev/null || echo "")
  rm -rf "$tmp_dir"
  [[ "$agent" == "james" ]] && exit 0 || exit 1
) 2>/dev/null
if [ $? -eq 0 ]; then _pass "$TEST"; else _fail "$TEST" "concurrent write_active_lock produced invalid JSON or wrong agent"; fi

# ---- COORD-SYNTAX ----
TEST="COORD-SYNTAX: bash -n on coord-state.sh"
if bash -n "$REPO_ROOT/scripts/coordination/coord-state.sh" 2>/dev/null; then
  _pass "$TEST"
else
  _fail "$TEST" "syntax error in coord-state.sh"
fi

echo ""
TOTAL=$((PASS_COUNT + FAIL_COUNT))
echo "${PASS_COUNT}/${TOTAL} tests passed."
[ "$FAIL_COUNT" -gt 0 ] && exit 1
exit 0
