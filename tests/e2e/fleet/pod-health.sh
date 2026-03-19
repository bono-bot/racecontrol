#!/bin/bash
# tests/e2e/fleet/pod-health.sh
# Phase 50: Fleet-wide self-test -- triggers GET /api/v1/pods/{id}/self-test
# on all 8 pods, asserts HEALTHY verdict for each.
#
# Gates per pod:
#   1. Pod reachable (:8090/ping = pong)
#   2. Self-test endpoint returns HTTP 200 within 35s
#   3. Verdict is HEALTHY
#
# Usage: bash tests/e2e/fleet/pod-health.sh

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
source "$SCRIPT_DIR/../lib/common.sh"
source "$SCRIPT_DIR/../lib/pod-map.sh"

SERVER_URL="${RC_BASE_URL:-http://192.168.31.23:8080/api/v1}"
SELFTEST_TIMEOUT=35

info "Fleet Pod Self-Test (Phase 50: LLM Self-Test + Fleet Health)"
echo ""

for POD_NUM in $(seq 1 8); do
    POD_ID="pod-${POD_NUM}"
    POD_IP=$(pod_ip "$POD_ID")

    if [ -z "$POD_IP" ]; then
        skip "${POD_ID}: no IP mapping"
        continue
    fi

    # Gate 1: Pod reachable via rc-agent remote_ops
    PING=$(curl -s --connect-timeout 2 --max-time 3 "http://${POD_IP}:8090/ping" 2>/dev/null)
    if [ "$PING" != "pong" ]; then
        skip "${POD_ID}: rc-agent not reachable on :8090"
        continue
    fi

    # Gate 2: Self-test returns 200 within 35s
    HTTP_CODE=$(curl -s -o /tmp/selftest_${POD_NUM}.json -w "%{http_code}" \
        --connect-timeout 5 --max-time ${SELFTEST_TIMEOUT} \
        "${SERVER_URL}/pods/${POD_ID}/self-test" 2>/dev/null)

    if [ "$HTTP_CODE" != "200" ]; then
        fail "${POD_ID}: self-test returned HTTP ${HTTP_CODE}"
        continue
    fi

    RESP=$(cat /tmp/selftest_${POD_NUM}.json 2>/dev/null)
    if [ -z "$RESP" ]; then
        fail "${POD_ID}: self-test returned empty response"
        continue
    fi

    # Gate 3: Verdict is HEALTHY
    VERDICT=$(echo "$RESP" | python3 -c "
import sys, json
try:
    d = json.loads(sys.stdin.read())
    v = d.get('verdict', {})
    if isinstance(v, dict):
        print(v.get('level', 'UNKNOWN'))
    else:
        print('UNKNOWN')
except Exception:
    print('PARSE_ERROR')
" 2>/dev/null)

    if [ "$VERDICT" = "HEALTHY" ]; then
        pass "${POD_ID}: self-test HEALTHY"
    elif [ "$VERDICT" = "DEGRADED" ]; then
        # Log probe details for diagnostics
        FAILED_PROBES=$(echo "$RESP" | python3 -c "
import sys, json
try:
    d = json.loads(sys.stdin.read())
    failed = [p['name'] for p in d.get('probes', []) if p.get('status') == 'fail']
    print(', '.join(failed) if failed else 'unknown')
except Exception:
    print('unknown')
" 2>/dev/null)
        fail "${POD_ID}: self-test DEGRADED (failed: ${FAILED_PROBES})"
    elif [ "$VERDICT" = "CRITICAL" ]; then
        fail "${POD_ID}: self-test CRITICAL"
    else
        fail "${POD_ID}: unexpected verdict '${VERDICT}'"
    fi
done

echo ""
summary_exit
