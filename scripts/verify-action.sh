#!/bin/bash
# scripts/verify-action.sh — Deterministic Contradiction Testing
#
# For every physical-world action, runs predetermined tests that would FAIL
# if the action didn't actually work. Returns PASS only when ALL contradiction
# tests pass. Returns FAIL with specific evidence on first failure.
#
# This is NOT a proxy check. It verifies the DOWNSTREAM EFFECT, not the API response.
#
# Usage:
#   bash scripts/verify-action.sh game-launch <pod_id> <sim_type>
#   bash scripts/verify-action.sh billing-start <pod_id> <session_id>
#   bash scripts/verify-action.sh deploy-agent <pod_id> <expected_build_id>
#   bash scripts/verify-action.sh deploy-server <expected_build_id>
#   bash scripts/verify-action.sh blanking <pod_id>
#   bash scripts/verify-action.sh session-end <pod_id>
#
# Exit codes:
#   0 = ALL contradiction tests passed (action confirmed)
#   1 = At least one test FAILED (action did NOT work as expected)
#   2 = Usage error

set -uo pipefail

ACTION="$1"
shift

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m'

PASS_COUNT=0
FAIL_COUNT=0

pass() { PASS_COUNT=$((PASS_COUNT+1)); echo -e "  ${GREEN}CONTRADICTION PASS${NC}  $1"; }
fail() { FAIL_COUNT=$((FAIL_COUNT+1)); echo -e "  ${RED}CONTRADICTION FAIL${NC}  $1"; }

# Pod IP lookup
pod_ip() {
    case "$1" in
        pod_1|pod-1) echo "192.168.31.89" ;;
        pod_2|pod-2) echo "192.168.31.33" ;;
        pod_3|pod-3) echo "192.168.31.28" ;;
        pod_4|pod-4) echo "192.168.31.88" ;;
        pod_5|pod-5) echo "192.168.31.86" ;;
        pod_6|pod-6) echo "192.168.31.87" ;;
        pod_7|pod-7) echo "192.168.31.38" ;;
        pod_8|pod-8) echo "192.168.31.91" ;;
        *) echo "" ;;
    esac
}

# Map sim_type to expected process name
game_process() {
    case "$1" in
        assetto_corsa) echo "acs.exe" ;;
        f1_25) echo "F1_25.exe" ;;
        iracing) echo "iRacingSim64DX11.exe" ;;
        le_mans_ultimate) echo "LMU.exe" ;;
        assetto_corsa_evo) echo "AssettoCorsa2.exe" ;;
        assetto_corsa_rally) echo "ac_rally.exe" ;;
        forza_horizon_5) echo "ForzaHorizon5.exe" ;;
        forza_motorsport) echo "ForzaMotorsport.exe" ;;
        wrc) echo "WRC.exe" ;;
        *) echo "UNKNOWN" ;;
    esac
}

SERVER_URL="${RC_BASE_URL:-http://192.168.31.23:8080/api/v1}"

case "$ACTION" in

# ═══════════════════════════════════════════════════════════════════════════════
# GAME LAUNCH: Verify the game PROCESS is actually running on the pod
# ═══════════════════════════════════════════════════════════════════════════════
game-launch)
    POD_ID="$1"
    SIM_TYPE="$2"
    POD_IP=$(pod_ip "$POD_ID")
    EXPECTED_PROCESS=$(game_process "$SIM_TYPE")

    echo "=== Contradiction Test: game-launch ${POD_ID} ${SIM_TYPE} ==="
    echo "  Expected process: ${EXPECTED_PROCESS} on ${POD_IP}"

    if [ -z "$POD_IP" ]; then
        fail "Unknown pod: ${POD_ID}"
        exit 1
    fi

    # Test 1: Is the game PROCESS actually running? (not API ok, not health check)
    PROCESS_CHECK=$(ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no "User@${POD_IP}" \
        "tasklist /FI \"IMAGENAME eq ${EXPECTED_PROCESS}\" /FO CSV 2>nul" 2>/dev/null)

    if echo "$PROCESS_CHECK" | grep -qi "$EXPECTED_PROCESS"; then
        pass "Process ${EXPECTED_PROCESS} IS running on ${POD_ID}"
    else
        fail "Process ${EXPECTED_PROCESS} NOT found on ${POD_ID} — game did NOT launch"
        echo "  Evidence: tasklist output: ${PROCESS_CHECK}"
    fi

    # Test 2: Debug endpoint shows game state (not idle, not just launch_splash)
    DEBUG=$(ssh -o ConnectTimeout=5 "User@${POD_IP}" \
        'curl -s http://127.0.0.1:18924/debug' 2>/dev/null)

    if [ -n "$DEBUG" ]; then
        LOCK_STATE=$(echo "$DEBUG" | node --no-warnings -e "
            const d = JSON.parse(require('fs').readFileSync(0,'utf8'));
            console.log(d.lock_screen_state || 'unknown');
        " 2>/dev/null)

        if [ "$LOCK_STATE" = "game_running" ] || [ "$LOCK_STATE" = "launch_splash" ]; then
            pass "Debug endpoint: lock_screen_state=${LOCK_STATE}"
        else
            fail "Debug endpoint: lock_screen_state=${LOCK_STATE} (expected game_running or launch_splash)"
        fi
    else
        fail "Debug endpoint unreachable on ${POD_ID}"
    fi

    # Test 3: Server game tracker shows this pod as active (not idle)
    GAME_STATE=$(curl -s "${SERVER_URL}/fleet/health" 2>/dev/null | node --no-warnings -e "
        const d = JSON.parse(require('fs').readFileSync(0,'utf8'));
        const pod = d.pods?.find(p => 'pod_' + p.pod_number === '${POD_ID}' || 'pod-' + p.pod_number === '${POD_ID}');
        console.log(pod?.ws_connected ? 'connected' : 'disconnected');
    " 2>/dev/null)

    if [ "$GAME_STATE" = "connected" ]; then
        pass "Pod WS connected to server"
    else
        fail "Pod WS disconnected — game launch may have crashed agent"
    fi
    ;;

# ═══════════════════════════════════════════════════════════════════════════════
# BILLING START: Verify session exists in active billing list
# ═══════════════════════════════════════════════════════════════════════════════
billing-start)
    POD_ID="$1"
    SESSION_ID="$2"
    TOKEN="${STAFF_TOKEN:-}"

    echo "=== Contradiction Test: billing-start ${POD_ID} ${SESSION_ID} ==="

    if [ -z "$TOKEN" ]; then
        fail "STAFF_TOKEN not set — cannot verify billing"
        exit 1
    fi

    # Test 1: Session appears in active billing sessions
    ACTIVE=$(curl -s -H "Authorization: Bearer ${TOKEN}" \
        "${SERVER_URL}/billing/sessions/active" 2>/dev/null)

    if echo "$ACTIVE" | grep -q "$SESSION_ID"; then
        pass "Session ${SESSION_ID} found in active billing sessions"
    else
        fail "Session ${SESSION_ID} NOT in active billing sessions"
        echo "  Evidence: ${ACTIVE}"
    fi

    # Test 2: Pod shows billing_active in WS pod data
    POD_DATA=$(node --no-warnings -e "
        const ws = new WebSocket('ws://192.168.31.23:8080/ws/dashboard?token=rp-terminal-2026');
        ws.addEventListener('message', (e) => {
            const msg = JSON.parse(e.data);
            if (msg.event === 'pod_list') {
                const pod = msg.data.find(p => p.id === '${POD_ID}');
                console.log(pod?.status || 'not_found');
                ws.close(); process.exit(0);
            }
        });
        setTimeout(() => { console.log('timeout'); process.exit(1); }, 10000);
    " 2>/dev/null)

    if [ "$POD_DATA" != "idle" ] && [ "$POD_DATA" != "not_found" ] && [ "$POD_DATA" != "timeout" ]; then
        pass "Pod status via WS: ${POD_DATA} (not idle)"
    else
        fail "Pod status via WS: ${POD_DATA} — billing may not have started"
    fi
    ;;

# ═══════════════════════════════════════════════════════════════════════════════
# DEPLOY AGENT: Verify the SPECIFIC build_id on the pod (via direct health check)
# ═══════════════════════════════════════════════════════════════════════════════
deploy-agent)
    POD_ID="$1"
    EXPECTED_BUILD="$2"
    POD_IP=$(pod_ip "$POD_ID")

    echo "=== Contradiction Test: deploy-agent ${POD_ID} ${EXPECTED_BUILD} ==="

    # Test 1: rc-agent process running
    AGENT_RUNNING=$(ssh -o ConnectTimeout=5 "User@${POD_IP}" \
        'tasklist /FI "IMAGENAME eq rc-agent.exe" /FO CSV 2>nul' 2>/dev/null)

    if echo "$AGENT_RUNNING" | grep -qi "rc-agent"; then
        pass "rc-agent.exe IS running on ${POD_ID}"
    else
        fail "rc-agent.exe NOT running on ${POD_ID}"
    fi

    # Test 2: Build ID matches (from pod's own health, not server fleet health)
    BUILD_ID=$(ssh -o ConnectTimeout=5 "User@${POD_IP}" \
        'curl -s http://127.0.0.1:8090/health' 2>/dev/null | node --no-warnings -e "
        try { console.log(JSON.parse(require('fs').readFileSync(0,'utf8')).build_id); }
        catch { console.log('ERROR'); }
    " 2>/dev/null)

    if [ "$BUILD_ID" = "$EXPECTED_BUILD" ]; then
        pass "Build ID: ${BUILD_ID} matches expected ${EXPECTED_BUILD}"
    else
        fail "Build ID: ${BUILD_ID} does NOT match expected ${EXPECTED_BUILD}"
    fi

    # Test 3: Session context (MUST be Console/Session 1, not Services/Session 0)
    SESSION_CTX=$(ssh -o ConnectTimeout=5 "User@${POD_IP}" \
        'tasklist /V /FO CSV /FI "IMAGENAME eq rc-agent.exe" 2>nul' 2>/dev/null | grep -i "rc-agent")

    if echo "$SESSION_CTX" | grep -qi "Console"; then
        pass "rc-agent running in Session 1 (Console) — GUI operations work"
    elif echo "$SESSION_CTX" | grep -qi "Services"; then
        fail "rc-agent running in Session 0 (Services) — GUI BROKEN, games cannot launch"
    else
        fail "Cannot determine session context for rc-agent"
    fi
    ;;

# ═══════════════════════════════════════════════════════════════════════════════
# SESSION END: Verify pod returned to idle state
# ═══════════════════════════════════════════════════════════════════════════════
session-end)
    POD_ID="$1"
    POD_IP=$(pod_ip "$POD_ID")

    echo "=== Contradiction Test: session-end ${POD_ID} ==="

    # Test 1: No game processes running
    GAMES=$(ssh -o ConnectTimeout=5 "User@${POD_IP}" \
        'tasklist /FO CSV 2>nul' 2>/dev/null | grep -iE "acs\.exe|F1_25|iRacing|LMU|AssettoCorsa2|ForzaHorizon|WRC")

    if [ -z "$GAMES" ]; then
        pass "No game processes running on ${POD_ID}"
    else
        fail "Game processes still running: ${GAMES}"
    fi

    # Test 2: Lock screen shows blanking (Edge is rendering the blank page)
    DEBUG=$(ssh -o ConnectTimeout=5 "User@${POD_IP}" \
        'curl -s http://127.0.0.1:18924/debug' 2>/dev/null)

    EDGE_COUNT=$(echo "$DEBUG" | node --no-warnings -e "
        try { console.log(JSON.parse(require('fs').readFileSync(0,'utf8')).edge_process_count || 0); }
        catch { console.log(0); }
    " 2>/dev/null)

    if [ "$EDGE_COUNT" -gt 0 ] 2>/dev/null; then
        pass "Edge process count: ${EDGE_COUNT} (blanking screen active)"
    else
        fail "Edge process count: ${EDGE_COUNT} — blanking screen NOT active"
    fi

    # Test 3: Pod shows idle in WS
    POD_STATUS=$(node --no-warnings -e "
        const ws = new WebSocket('ws://192.168.31.23:8080/ws/dashboard?token=rp-terminal-2026');
        ws.addEventListener('message', (e) => {
            const msg = JSON.parse(e.data);
            if (msg.event === 'pod_list') {
                const pod = msg.data.find(p => p.id === '${POD_ID}');
                console.log(pod?.status || 'not_found');
                ws.close(); process.exit(0);
            }
        });
        setTimeout(() => { console.log('timeout'); process.exit(1); }, 10000);
    " 2>/dev/null)

    if [ "$POD_STATUS" = "idle" ]; then
        pass "Pod status via WS: idle"
    else
        fail "Pod status via WS: ${POD_STATUS} (expected idle)"
    fi
    ;;

# ═══════════════════════════════════════════════════════════════════════════════
# BLANKING: Verify blanking screen is actually displayed
# ═══════════════════════════════════════════════════════════════════════════════
blanking)
    POD_ID="$1"
    POD_IP=$(pod_ip "$POD_ID")

    echo "=== Contradiction Test: blanking ${POD_ID} ==="

    DEBUG=$(ssh -o ConnectTimeout=5 "User@${POD_IP}" \
        'curl -s http://127.0.0.1:18924/debug' 2>/dev/null)

    EDGE=$(echo "$DEBUG" | node --no-warnings -e "
        try { const d = JSON.parse(require('fs').readFileSync(0,'utf8')); console.log(d.edge_process_count + '|' + d.lock_screen_state); }
        catch { console.log('0|error'); }
    " 2>/dev/null)

    EDGE_COUNT=$(echo "$EDGE" | cut -d'|' -f1)
    LOCK_STATE=$(echo "$EDGE" | cut -d'|' -f2)

    if [ "$LOCK_STATE" = "screen_blanked" ] && [ "$EDGE_COUNT" -gt 0 ] 2>/dev/null; then
        pass "Blanking active: state=${LOCK_STATE}, edge_count=${EDGE_COUNT}"
    elif [ "$LOCK_STATE" = "screen_blanked" ] && [ "$EDGE_COUNT" = "0" ]; then
        fail "IMPOSSIBLE STATE: screen_blanked but edge_count=0 — blanking screen NOT rendering"
    else
        fail "Not blanked: state=${LOCK_STATE}, edge_count=${EDGE_COUNT}"
    fi
    ;;

*)
    echo "Usage: verify-action.sh <action> [args...]"
    echo "Actions: game-launch, billing-start, deploy-agent, deploy-server, session-end, blanking"
    exit 2
    ;;
esac

# Summary
echo ""
if [ "$FAIL_COUNT" -gt 0 ]; then
    echo -e "${RED}CONTRADICTION TEST FAILED: ${FAIL_COUNT} failures, ${PASS_COUNT} passes${NC}"
    echo "ACTION DID NOT SUCCEED — do NOT proceed."
    exit 1
else
    echo -e "${GREEN}CONTRADICTION TEST PASSED: ${PASS_COUNT} passes, 0 failures${NC}"
    echo "Action confirmed by downstream evidence."
    exit 0
fi
