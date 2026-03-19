#!/bin/bash
# tests/e2e/fleet/close-wait.sh
# Verifies CLOSE_WAIT socket count on :8090 is <5 on all reachable pods.
#
# Strategy: Query each pod's :8090/exec endpoint to run `netstat -ano` locally
# and count CLOSE_WAIT lines containing :8090. This is the same check that
# self_monitor.rs uses internally (count_close_wait_on_8090).
#
# Note: This is a point-in-time check. For a full 30-minute soak test,
# run this script twice with 30 minutes between runs.
#
# Usage: bash tests/e2e/fleet/close-wait.sh

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
source "$SCRIPT_DIR/../lib/common.sh"
source "$SCRIPT_DIR/../lib/pod-map.sh"

THRESHOLD=5

info "CLOSE_WAIT Socket Hygiene Check (threshold: <${THRESHOLD} per pod)"
echo ""

for POD_NUM in $(seq 1 8); do
    POD_ID="pod-${POD_NUM}"
    POD_IP=$(pod_ip "$POD_ID")

    if [ -z "$POD_IP" ]; then
        skip "${POD_ID}: no IP mapping"
        continue
    fi

    # Check if pod is reachable first (1s timeout)
    PING_RESP=$(curl -s --connect-timeout 1 --max-time 2 "http://${POD_IP}:8090/ping" 2>/dev/null)
    if [ "$PING_RESP" != "pong" ]; then
        skip "${POD_ID} (${POD_IP}): rc-agent not reachable on :8090"
        continue
    fi

    # Run netstat on the pod via /exec endpoint and count CLOSE_WAIT on :8090
    EXEC_RESP=$(curl -s --connect-timeout 3 --max-time 10 \
        -X POST "http://${POD_IP}:8090/exec" \
        -H "Content-Type: application/json" \
        -d '{"cmd":"netstat -ano | findstr CLOSE_WAIT | findstr :8090 | find /c /v \"\"","timeout_ms":5000}' \
        2>/dev/null)

    if [ -z "$EXEC_RESP" ]; then
        skip "${POD_ID} (${POD_IP}): exec endpoint did not respond"
        continue
    fi

    # Parse the count from stdout field in the JSON response
    CW_COUNT=$(echo "$EXEC_RESP" | grep -oP '"stdout"\s*:\s*"\\?\K[0-9]+' | head -1)

    if [ -z "$CW_COUNT" ]; then
        # If parsing failed, try alternate extraction
        CW_COUNT=$(echo "$EXEC_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin).get('stdout','').strip())" 2>/dev/null)
    fi

    if [ -z "$CW_COUNT" ] || ! [[ "$CW_COUNT" =~ ^[0-9]+$ ]]; then
        skip "${POD_ID} (${POD_IP}): could not parse CLOSE_WAIT count from response"
        continue
    fi

    if [ "$CW_COUNT" -lt "$THRESHOLD" ]; then
        pass "${POD_ID} (${POD_IP}): ${CW_COUNT} CLOSE_WAIT sockets on :8090 (< ${THRESHOLD})"
    else
        fail "${POD_ID} (${POD_IP}): ${CW_COUNT} CLOSE_WAIT sockets on :8090 (>= ${THRESHOLD} threshold)"
    fi
done

echo ""
summary_exit
