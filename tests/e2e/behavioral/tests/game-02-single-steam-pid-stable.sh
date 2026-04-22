#!/usr/bin/env bash
# game-02-single-steam-pid-stable
#
# BEHAVIOR: The steam.exe main process PID must remain stable across a
#   2-minute window. If Steam is being killed + restarted by an overlay
#   (game_doctor / ai_debugger / process_guard), its PID will change.
#
# METHOD:
#   1. Capture first PID of steam.exe.
#   2. Wait 120 seconds.
#   3. Capture PID again. Assert identical.
#
# PASS: PID unchanged.
# FAIL: PID changed (Steam was killed + respawned).
# SKIP: Steam not running at all (no baseline).

set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "${ROOT}/lib/common.sh"

TEST_ID="game-02-single-steam-pid-stable"
test_start "$TEST_ID" "steam.exe PID stable over 120s"

if ! pod_preflight "$TARGET_POD"; then
    report "$TEST_ID" "preflight" "skip" "${TARGET_POD} not reachable"
    test_end "$TEST_ID" "skip"
    exit 2
fi

pid_before=$(pod_pid_of "$TARGET_POD" "steam.exe")
if [ -z "$pid_before" ]; then
    report "$TEST_ID" "preflight" "skip" "steam.exe not running — no baseline"
    test_end "$TEST_ID" "skip"
    exit 2
fi
report "$TEST_ID" "snapshot-before" "observe" "steam.exe PID=${pid_before}"

sleep 120

pid_after=$(pod_pid_of "$TARGET_POD" "steam.exe")
report "$TEST_ID" "snapshot-after" "observe" "steam.exe PID=${pid_after:-<none>}"

if [ "$pid_before" = "$pid_after" ]; then
    report "$TEST_ID" "assert" "pass" "PID stable at ${pid_before}"
    test_end "$TEST_ID" "pass"
    exit 0
fi
report "$TEST_ID" "assert" "fail" "PID changed: ${pid_before} -> ${pid_after:-<dead>}" "before=${pid_before} after=${pid_after:-<dead>}"
test_end "$TEST_ID" "fail"
exit 1
