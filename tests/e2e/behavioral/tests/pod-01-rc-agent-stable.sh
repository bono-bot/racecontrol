#!/usr/bin/env bash
# pod-01-rc-agent-stable
#
# BEHAVIOR: rc-agent.exe PID must not change across a 2-minute window
#   during normal idle operation (no scheduled restart, no deploy).
#   A changing PID indicates a crash-loop or watchdog respawn.
#
# METHOD:
#   1. Capture rc-agent.exe PID + /health build_id.
#   2. Wait 120 seconds.
#   3. Capture again. Assert PID unchanged AND build_id unchanged.
#
# PASS: PID and build_id stable.
# FAIL: PID changed (restart or crash-loop).
# SKIP: pod not reachable.

set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "${ROOT}/lib/common.sh"

TEST_ID="pod-01-rc-agent-stable"
test_start "$TEST_ID" "rc-agent PID + build_id stable over 120s"

if ! pod_preflight "$TARGET_POD"; then
    report "$TEST_ID" "preflight" "skip" "${TARGET_POD} not reachable"
    test_end "$TEST_ID" "skip"
    exit 2
fi

# Resolve pod IP from fleet health.
pod_ip=$(curl -s --max-time 3 "${SERVER_URL}/api/v1/fleet/health" 2>/dev/null | \
    python3 -c "
import sys, json
d = json.load(sys.stdin)
for p in d.get('pods', []):
    if p.get('pod_id') == '$TARGET_POD':
        print(p.get('ip_address', ''))
        break
" 2>/dev/null)

if [ -z "$pod_ip" ]; then
    report "$TEST_ID" "preflight" "skip" "could not resolve IP for ${TARGET_POD}"
    test_end "$TEST_ID" "skip"
    exit 2
fi

pid_before=$(pod_pid_of "$TARGET_POD" "rc-agent.exe")
build_before=$(curl -s --max-time 3 "http://${pod_ip}:8090/health" 2>/dev/null | \
    python3 -c "import sys,json; print(json.load(sys.stdin).get('build_id',''))" 2>/dev/null)

report "$TEST_ID" "snapshot-before" "observe" "PID=${pid_before} build_id=${build_before}"

if [ -z "$pid_before" ] || [ -z "$build_before" ]; then
    report "$TEST_ID" "preflight" "skip" "missing baseline: PID=${pid_before} build=${build_before}"
    test_end "$TEST_ID" "skip"
    exit 2
fi

sleep 120

pid_after=$(pod_pid_of "$TARGET_POD" "rc-agent.exe")
build_after=$(curl -s --max-time 3 "http://${pod_ip}:8090/health" 2>/dev/null | \
    python3 -c "import sys,json; print(json.load(sys.stdin).get('build_id',''))" 2>/dev/null)

report "$TEST_ID" "snapshot-after" "observe" "PID=${pid_after} build_id=${build_after}"

if [ "$pid_before" = "$pid_after" ] && [ "$build_before" = "$build_after" ]; then
    report "$TEST_ID" "assert" "pass" "rc-agent stable"
    test_end "$TEST_ID" "pass"
    exit 0
fi
report "$TEST_ID" "assert" "fail" "rc-agent changed: PID ${pid_before}->${pid_after} build ${build_before}->${build_after}"
test_end "$TEST_ID" "fail"
exit 1
