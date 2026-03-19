#!/bin/bash
# tests/e2e/fleet/startup-verify.sh
# After agent restart, verify all pods have:
# - Remote ops :8090 reachable (ping returns pong)
# - Lock screen :18923 responding (HTTP server bound)
# - WebSocket connected to server (fleet/health ws_connected=true)
#
# This is the E2E verification for Phase 46 SAFETY-04 (BootVerification).
#
# Usage: bash tests/e2e/fleet/startup-verify.sh

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
source "$SCRIPT_DIR/../lib/common.sh"
source "$SCRIPT_DIR/../lib/pod-map.sh"

SERVER_URL="${RC_BASE_URL:-http://192.168.31.23:8080/api/v1}"

info "Pod Startup Verification (Phase 46: Crash Safety)"
echo ""

# Fetch fleet health once (reuse for all pods -- avoid redundant HTTP calls)
FLEET_HEALTH=$(curl -s --max-time 10 "${SERVER_URL}/fleet/health" 2>/dev/null)
if [ -z "$FLEET_HEALTH" ]; then
    fail "Could not reach server fleet/health endpoint at ${SERVER_URL}"
    summary_exit
fi

for POD_NUM in $(seq 1 8); do
    POD_ID="pod-${POD_NUM}"
    POD_IP=$(pod_ip "$POD_ID")

    if [ -z "$POD_IP" ]; then
        skip "${POD_ID}: no IP mapping"
        continue
    fi

    # Gate 1: Remote ops reachable (:8090/ping)
    PING=$(curl -s --connect-timeout 2 --max-time 3 "http://${POD_IP}:8090/ping" 2>/dev/null)
    if [ "$PING" = "pong" ]; then
        pass "${POD_ID}: remote ops :8090 reachable"
    else
        fail "${POD_ID}: remote ops :8090 not responding (got: '${PING}')"
        continue  # skip further checks if agent is unreachable
    fi

    # Gate 2: Lock screen port bound (:18923)
    # The lock screen serves HTML on 127.0.0.1:18923 -- not reachable from LAN.
    # Use remote_ops /exec to check if port 18923 is listening locally on the pod.
    LOCK_CHECK=$(curl -s --connect-timeout 2 --max-time 5 \
        -X POST "http://${POD_IP}:8090/exec" \
        -H "Content-Type: application/json" \
        -d '{"command":"powershell -Command \"(Test-NetConnection -ComputerName 127.0.0.1 -Port 18923).TcpTestSucceeded\""}' \
        2>/dev/null)
    if echo "$LOCK_CHECK" | grep -qi "True"; then
        pass "${POD_ID}: lock screen port 18923 bound"
    else
        fail "${POD_ID}: lock screen port 18923 not bound"
    fi

    # Gate 3: WebSocket connected (from fleet health data)
    WS_CONNECTED=$(echo "$FLEET_HEALTH" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    pods = data.get('pods', [])
    pod = next((p for p in pods if p.get('pod_id') == 'pod_${POD_NUM}' or p.get('id') == 'pod_${POD_NUM}'), None)
    if pod:
        print(pod.get('ws_connected', False))
    else:
        print('NotFound')
except:
    print('ParseError')
" 2>/dev/null)
    if [ "$WS_CONNECTED" = "True" ]; then
        pass "${POD_ID}: WebSocket connected to server"
    elif [ "$WS_CONNECTED" = "NotFound" ]; then
        skip "${POD_ID}: not in fleet health (may not have connected yet)"
    else
        fail "${POD_ID}: WebSocket not connected (ws_connected=${WS_CONNECTED})"
    fi
done

echo ""
summary_exit
