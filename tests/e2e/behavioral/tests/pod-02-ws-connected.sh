#!/usr/bin/env bash
# pod-02-ws-connected
#
# BEHAVIOR: Target pod's ws_connected must remain true across a 2-min
#   window in fleet health. Intermittent ws drops indicate silent
#   reconnect loops or server-side backpressure.
#
# METHOD:
#   1. Poll /api/v1/fleet/health every 10s for 120s.
#   2. Record ws_connected + ws_reconnects_5m per sample.
#   3. PASS if ws_connected true for all samples AND ws_reconnects_5m
#      did not increment.
#
# Safe: read-only.

set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "${ROOT}/lib/common.sh"

TEST_ID="pod-02-ws-connected"
test_start "$TEST_ID" "ws_connected true + no new reconnects over 120s (12 samples)"

if ! pod_preflight "$TARGET_POD"; then
    report "$TEST_ID" "preflight" "skip" "${TARGET_POD} not reachable"
    test_end "$TEST_ID" "skip"
    exit 2
fi

drops=0
reconnects_initial=""
samples=0
for i in $(seq 1 12); do
    sample=$(curl -s --max-time 3 "${SERVER_URL}/api/v1/fleet/health" 2>/dev/null | \
        python3 -c "
import sys, json
d = json.load(sys.stdin)
for p in d.get('pods', []):
    if p.get('pod_id') == '$TARGET_POD':
        print(f\"{p.get('ws_connected', False)} {p.get('ws_reconnects_5m', 0)}\")
        break
" 2>/dev/null)
    [ -z "$sample" ] && continue
    samples=$((samples + 1))
    ws_connected=$(echo "$sample" | awk '{print $1}')
    reconnects=$(echo "$sample" | awk '{print $2}')
    [ -z "$reconnects_initial" ] && reconnects_initial="$reconnects"

    if [ "$ws_connected" != "True" ]; then
        drops=$((drops + 1))
    fi
    sleep 10
done

report "$TEST_ID" "sampled" "observe" "samples=${samples} drops=${drops} reconnects_delta=$((reconnects - reconnects_initial))"

if [ "$drops" -eq 0 ] && [ "$reconnects" = "$reconnects_initial" ]; then
    report "$TEST_ID" "assert" "pass" "ws stable across ${samples} samples"
    test_end "$TEST_ID" "pass"
    exit 0
fi
report "$TEST_ID" "assert" "fail" "ws flap: drops=${drops} reconnects ${reconnects_initial}->${reconnects}"
test_end "$TEST_ID" "fail"
exit 1
