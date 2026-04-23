#!/usr/bin/env bash
# game-01-no-auto-steam-spawn
#
# BEHAVIOR: After PR #18 (removing game_doctor Check 12 + ai_debugger
#   Steam kill-restart), a failed game-launch retry must NOT spawn new
#   steam.exe processes.
#
# METHOD:
#   1. Snapshot current steam.exe PID (if any).
#   2. Wait 90 seconds (long enough for ~3 diagnostic ticks on event_loop).
#   3. Re-snapshot. Compare.
#
# PASS: steam.exe PID count unchanged.
# FAIL: steam.exe PID count increased.
# SKIP: pod not reachable, or no rc-agent running.
#
# Safe: does NOT trigger a launch. Passive observation of current state.

set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "${ROOT}/lib/common.sh"

TEST_ID="game-01-no-auto-steam-spawn"
test_start "$TEST_ID" "steam.exe PID count stable over 90s of passive observation"

if ! pod_preflight "$TARGET_POD"; then
    report "$TEST_ID" "preflight" "skip" "${TARGET_POD} not reachable"
    test_end "$TEST_ID" "skip"
    exit 2
fi

before=$(pod_proc_count "$TARGET_POD" "steam.exe")
report "$TEST_ID" "snapshot-before" "observe" "steam.exe count=${before}" "before=${before}"

sleep 90

after=$(pod_proc_count "$TARGET_POD" "steam.exe")
report "$TEST_ID" "snapshot-after" "observe" "steam.exe count=${after}" "after=${after}"

if [ "$after" -le "$before" ]; then
    report "$TEST_ID" "assert" "pass" "count did not grow: ${before} -> ${after}"
    test_end "$TEST_ID" "pass"
    exit 0
else
    report "$TEST_ID" "assert" "fail" "steam.exe count grew: ${before} -> ${after}" "delta=$((after - before))"
    test_end "$TEST_ID" "fail"
    exit 1
fi
