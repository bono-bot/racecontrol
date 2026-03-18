#!/bin/bash
# =============================================================================
# Game Launch E2E Test — F1 25 via Kiosk Flow
#
# Tests the full kiosk → racecontrol → agent launch pipeline for non-AC games.
# Exercises each gate: billing check, double-launch guard, agent connectivity,
# and the actual launch command delivery.
#
# Usage:
#   ./game-launch.sh                        # defaults to localhost:8080
#   RC_BASE_URL=http://192.168.31.23:8080/api/v1 ./game-launch.sh
#   SIM_TYPE=le_mans_ultimate POD_ID=pod-3 ./game-launch.sh
#
# Prerequisites:
#   - racecontrol server running
#   - At least one pod with active billing (for full launch test)
# =============================================================================

set -uo pipefail

BASE_URL="${RC_BASE_URL:-http://localhost:8080/api/v1}"
SIM_TYPE="${SIM_TYPE:-f1_25}"
POD_ID="${POD_ID:-pod-8}"
PASS=0
FAIL=0
SKIP=0
TOTAL=0

# Colors
if [ -t 1 ]; then
    GREEN='\033[0;32m'
    RED='\033[0;31m'
    YELLOW='\033[0;33m'
    CYAN='\033[0;36m'
    NC='\033[0m'
else
    GREEN='' RED='' YELLOW='' CYAN='' NC=''
fi

pass() { TOTAL=$((TOTAL+1)); PASS=$((PASS+1)); echo -e "  ${GREEN}PASS${NC}  $1"; }
fail() { TOTAL=$((TOTAL+1)); FAIL=$((FAIL+1)); echo -e "  ${RED}FAIL${NC}  $1"; }
skip() { TOTAL=$((TOTAL+1)); SKIP=$((SKIP+1)); echo -e "  ${YELLOW}SKIP${NC}  $1"; }
info() { echo -e "  ${CYAN}INFO${NC}  $1"; }

echo "========================================"
echo "Game Launch E2E Test"
echo "Base URL : ${BASE_URL}"
echo "Sim Type : ${SIM_TYPE}"
echo "Pod ID   : ${POD_ID}"
echo "========================================"
echo ""

# ─── Gate 0: Server reachable ─────────────────────────────────────────────
echo "--- Gate 0: Server Health ---"
HEALTH=$(curl -s --max-time 5 "${BASE_URL}/health" 2>/dev/null || echo "UNREACHABLE")
if [ "$HEALTH" = "UNREACHABLE" ]; then
    fail "racecontrol not reachable at ${BASE_URL}"
    echo ""
    echo -e "${RED}Cannot proceed — server is down.${NC}"
    exit 1
fi
pass "Server reachable"

# ─── Gate 1: SimType accepted ─────────────────────────────────────────────
echo ""
echo "--- Gate 1: SimType Parsing ---"

# Test that the sim_type is accepted (will fail on billing, but NOT on "Unknown sim_type")
RESPONSE=$(curl -s --max-time 10 -X POST \
    -H "Content-Type: application/json" \
    -d "{\"pod_id\": \"${POD_ID}\", \"sim_type\": \"${SIM_TYPE}\"}" \
    "${BASE_URL}/games/launch" 2>/dev/null)

if echo "$RESPONSE" | grep -q "Unknown sim_type"; then
    fail "sim_type '${SIM_TYPE}' rejected as unknown"
    info "Response: $RESPONSE"
else
    pass "sim_type '${SIM_TYPE}' accepted by server"
fi

# Test invalid sim_type for comparison
RESPONSE_BAD=$(curl -s --max-time 10 -X POST \
    -H "Content-Type: application/json" \
    -d '{"pod_id": "pod-99", "sim_type": "mario_kart"}' \
    "${BASE_URL}/games/launch" 2>/dev/null)

if echo "$RESPONSE_BAD" | grep -q "Unknown sim_type"; then
    pass "Invalid sim_type 'mario_kart' correctly rejected"
else
    fail "Invalid sim_type 'mario_kart' was NOT rejected"
    info "Response: $RESPONSE_BAD"
fi

# ─── Gate 2: Billing Gate ─────────────────────────────────────────────────
echo ""
echo "--- Gate 2: Billing Gate ---"

# Launch without billing should fail with "no active billing"
RESPONSE=$(curl -s --max-time 10 -X POST \
    -H "Content-Type: application/json" \
    -d "{\"pod_id\": \"pod-99\", \"sim_type\": \"${SIM_TYPE}\"}" \
    "${BASE_URL}/games/launch" 2>/dev/null)

if echo "$RESPONSE" | grep -qi "no active billing"; then
    pass "${SIM_TYPE} correctly requires billing session (pod-99 has none)"
elif echo "$RESPONSE" | grep -qi "error"; then
    pass "${SIM_TYPE} rejected on pod-99 (error: $(echo "$RESPONSE" | python3 -c 'import sys,json; print(json.load(sys.stdin).get("error","?"))' 2>/dev/null || echo "$RESPONSE"))"
else
    fail "Expected billing rejection for pod-99, got: $RESPONSE"
fi

# ─── Gate 3: Active Billing Check ─────────────────────────────────────────
echo ""
echo "--- Gate 3: Active Sessions ---"

ACTIVE=$(curl -s --max-time 10 "${BASE_URL}/billing/sessions/active" 2>/dev/null)
ACTIVE_PODS=$(echo "$ACTIVE" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    sessions = data if isinstance(data, list) else data.get('sessions', [])
    for s in sessions:
        print(s.get('pod_id', ''))
except: pass
" 2>/dev/null)

if [ -z "$ACTIVE_PODS" ]; then
    info "No active billing sessions found"
    info "To test full launch: start a billing session on ${POD_ID} first"
    HAS_BILLING=false
else
    info "Active billing on pods: $(echo "$ACTIVE_PODS" | tr '\n' ' ')"
    if echo "$ACTIVE_PODS" | grep -q "${POD_ID}"; then
        pass "${POD_ID} has active billing"
        HAS_BILLING=true
    else
        info "${POD_ID} has no billing — picking first available pod"
        POD_ID=$(echo "$ACTIVE_PODS" | head -1)
        if [ -n "$POD_ID" ]; then
            info "Switched to ${POD_ID}"
            HAS_BILLING=true
        else
            HAS_BILLING=false
        fi
    fi
fi

# ─── Gate 4: Pod Agent Connected ──────────────────────────────────────────
echo ""
echo "--- Gate 4: Pod Agent Connectivity ---"

# Use /fleet/health which has the real ws_connected status (not /pods which lacks it)
FLEET=$(curl -s --max-time 10 "${BASE_URL}/fleet/health" 2>/dev/null)
POD_CONNECTED=$(echo "$FLEET" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    pods = data.get('pods', [])
    # Match by pod_id or pod_number
    for p in pods:
        pid = p.get('pod_id', '')
        pnum = 'pod_' + str(p.get('pod_number', 0))
        pnum2 = 'pod-' + str(p.get('pod_number', 0))
        if pid == '${POD_ID}' or pnum == '${POD_ID}' or pnum2 == '${POD_ID}':
            if p.get('ws_connected', False):
                print('CONNECTED')
            else:
                print('DISCONNECTED')
            break
    else:
        print('DISCONNECTED')
except: print('PARSE_ERROR')
" 2>/dev/null)

if [ "$POD_CONNECTED" = "CONNECTED" ]; then
    pass "${POD_ID} agent is connected via WebSocket"
    AGENT_UP=true
elif [ "$POD_CONNECTED" = "DISCONNECTED" ]; then
    info "${POD_ID} agent is NOT connected — launch will fail at agent delivery"
    AGENT_UP=false
else
    info "Could not determine ${POD_ID} connection status"
    AGENT_UP=false
fi

# ─── Gate 5: Active Games Check ───────────────────────────────────────────
echo ""
echo "--- Gate 5: Double-Launch Guard ---"

GAMES=$(curl -s --max-time 10 "${BASE_URL}/games/active" 2>/dev/null)
GAME_ON_POD=$(echo "$GAMES" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    games = data if isinstance(data, list) else data.get('games', [])
    for g in games:
        if g.get('pod_id','') == '${POD_ID}':
            print(g.get('sim_type','unknown') + ':' + g.get('game_state','unknown'))
            break
    else:
        print('NONE')
except: print('PARSE_ERROR')
" 2>/dev/null)

if [ "$GAME_ON_POD" = "NONE" ]; then
    pass "No game running on ${POD_ID} — clear to launch"
elif [ "$GAME_ON_POD" = "PARSE_ERROR" ]; then
    info "Could not parse active games response"
else
    info "Game already on ${POD_ID}: ${GAME_ON_POD}"
    info "Double-launch guard will block — stop game first"
fi

# ─── Gate 6: Full Launch Attempt ──────────────────────────────────────────
echo ""
echo "--- Gate 6: Full ${SIM_TYPE} Launch ---"

if [ "$HAS_BILLING" = "true" ]; then
    # Build launch_args matching kiosk wizard output
    LAUNCH_ARGS=$(python3 -c "
import json
args = {
    'game': '${SIM_TYPE}',
    'game_mode': 'single',
    'session_type': 'practice',
    'difficulty': 'medium',
    'transmission': 'auto',
    'ffb': 'medium',
    'aids': {'abs': 1, 'tc': 1, 'stability': 0, 'autoclutch': 1, 'ideal_line': 0},
    'conditions': {'damage': 0},
    'ai_enabled': False,
    'ai_count': 0
}
print(json.dumps(args))
" 2>/dev/null)

    info "Sending launch command: sim_type=${SIM_TYPE}, pod=${POD_ID}"
    info "launch_args: ${LAUNCH_ARGS}"

    RESPONSE=$(curl -s --max-time 15 -X POST \
        -H "Content-Type: application/json" \
        -d "{\"pod_id\": \"${POD_ID}\", \"sim_type\": \"${SIM_TYPE}\", \"launch_args\": $(echo "$LAUNCH_ARGS" | python3 -c 'import sys,json; print(json.dumps(sys.stdin.read().strip()))' 2>/dev/null)}" \
        "${BASE_URL}/games/launch" 2>/dev/null)

    info "Response: ${RESPONSE}"

    if echo "$RESPONSE" | grep -q '"ok":true'; then
        pass "Launch command accepted by server"

        # Wait 2s and check game state
        sleep 2
        GAMES_AFTER=$(curl -s --max-time 10 "${BASE_URL}/games/active" 2>/dev/null)
        GAME_STATE=$(echo "$GAMES_AFTER" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    games = data if isinstance(data, list) else data.get('games', [])
    for g in games:
        if g.get('pod_id','') == '${POD_ID}':
            print(json.dumps({
                'sim_type': g.get('sim_type'),
                'state': g.get('game_state'),
                'pid': g.get('pid'),
                'error': g.get('error_message')
            }))
            break
    else:
        print('NOT_FOUND')
except: print('PARSE_ERROR')
" 2>/dev/null)

        info "Game state after 2s: ${GAME_STATE}"

        if echo "$GAME_STATE" | grep -q '"state":"Running"'; then
            pass "${SIM_TYPE} is RUNNING on ${POD_ID}!"
        elif echo "$GAME_STATE" | grep -q '"state":"Launching"'; then
            info "${SIM_TYPE} still launching (may need Steam startup time)"
            pass "${SIM_TYPE} reached Launching state"
        elif echo "$GAME_STATE" | grep -q '"state":"Error"'; then
            ERRMSG=$(echo "$GAME_STATE" | python3 -c "import sys,json; print(json.load(sys.stdin).get('error',''))" 2>/dev/null)
            fail "${SIM_TYPE} launch ERROR: ${ERRMSG}"
        elif [ "$GAME_STATE" = "NOT_FOUND" ]; then
            fail "No game tracker found after launch — agent may not have received command"
        else
            info "Unexpected game state: ${GAME_STATE}"
        fi

        # Cleanup: stop the game
        echo ""
        echo "--- Cleanup: Stopping game ---"
        STOP_RESP=$(curl -s --max-time 10 -X POST \
            -H "Content-Type: application/json" \
            -d "{\"pod_id\": \"${POD_ID}\"}" \
            "${BASE_URL}/games/stop" 2>/dev/null)
        info "Stop response: ${STOP_RESP}"

    elif echo "$RESPONSE" | grep -q "No agent connected"; then
        if [ "$AGENT_UP" = "false" ]; then
            pass "Launch correctly failed: agent not connected (expected)"
        else
            fail "Launch failed with 'No agent connected' but agent appeared connected"
        fi
    elif echo "$RESPONSE" | grep -q "already has a game"; then
        info "Double-launch guard triggered — game already running"
        pass "Double-launch guard works for ${SIM_TYPE}"
    else
        ERROR=$(echo "$RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin).get('error','unknown'))" 2>/dev/null || echo "$RESPONSE")
        fail "Launch failed: ${ERROR}"
    fi
else
    skip "No active billing session — cannot test full launch"
    info "Start billing on a pod, then re-run: POD_ID=pod-X ./game-launch.sh"
fi

# ─── Gate 7: Kiosk Experiences (non-AC) ───────────────────────────────────
echo ""
echo "--- Gate 7: Kiosk Experiences for ${SIM_TYPE} ---"

EXPERIENCES=$(curl -s --max-time 10 "${BASE_URL}/kiosk/experiences" 2>/dev/null)
SIM_EXPS=$(echo "$EXPERIENCES" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    exps = data if isinstance(data, list) else data.get('experiences', [])
    count = sum(1 for e in exps if e.get('game','') == '${SIM_TYPE}')
    print(count)
except: print('0')
" 2>/dev/null)

if [ "$SIM_EXPS" -gt 0 ] 2>/dev/null; then
    pass "${SIM_EXPS} kiosk experience(s) exist for ${SIM_TYPE}"
else
    info "No kiosk experiences configured for ${SIM_TYPE}"
    info "Customers can still use 'Custom' mode in wizard, but presets would be better"
fi

# ─── Summary ──────────────────────────────────────────────────────────────
echo ""
echo "========================================"
echo -e "Results: ${GREEN}${PASS} passed${NC}, ${RED}${FAIL} failed${NC}, ${YELLOW}${SKIP} skipped${NC} ($((PASS+FAIL+SKIP)) total)"
echo "========================================"

if [ "$FAIL" -gt 0 ]; then
    echo -e "${RED}LAUNCH TEST FAILED${NC}"
    exit 1
else
    echo -e "${GREEN}LAUNCH TEST PASSED${NC}"
    exit 0
fi
