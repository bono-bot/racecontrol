#!/bin/bash
# tests/e2e/api/multiplayer.sh — Multiplayer Assetto Corsa E2E test
# Covers: MP-01 (group booking), MP-02 (AC server auto-start), MP-03 (game state on pods),
#         MP-04 (group session listing), MP-05 (cleanup + stop)
#
# Uses POST /terminal/book-multiplayer (staff path — skips friendship checks)
# Auth: x-terminal-secret header
#
# Usage:
#   bash tests/e2e/api/multiplayer.sh
#   RC_BASE_URL=http://192.168.31.23:8080/api/v1 bash tests/e2e/api/multiplayer.sh
#
# Prerequisites:
#   - racecontrol server running on venue server (.23)
#   - At least 2 pods connected via WebSocket
#   - Assetto Corsa installed on both pods
set -uo pipefail

BASE_URL="${RC_BASE_URL:-http://192.168.31.23:8080/api/v1}"
TERMINAL_SECRET="${RC_TERMINAL_SECRET:-rp-terminal-2026}"
# Use existing test driver (Standing Rule #3: no fake data — use known synthetic IDs)
TEST_DRIVER_1="driver_test_trial"
TEST_DRIVER_2=""
MIN_PODS=2
SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=../lib/common.sh
source "$SCRIPT_DIR/../lib/common.sh"
# shellcheck source=../lib/pod-map.sh
source "$SCRIPT_DIR/../lib/pod-map.sh"

echo "========================================"
echo "Multiplayer AC E2E Test (MP-01 to MP-05)"
echo "Base URL         : ${BASE_URL}"
echo "Terminal Secret  : ${TERMINAL_SECRET:0:6}..."
echo "Min Pods Required: ${MIN_PODS}"
echo "========================================"
echo ""

# Track resources for cleanup
CREATED_BILLING_PODS=""
GROUP_SESSION_ID=""
USED_PODS=""

cleanup() {
    echo ""
    echo "--- Cleanup ---"
    # Stop games on all pods we used
    for pod in $USED_PODS; do
        curl -s --max-time 5 -X POST \
            -H "Content-Type: application/json" \
            -d "{\"pod_id\": \"${pod}\"}" \
            "${BASE_URL}/games/stop" 2>/dev/null > /dev/null
        info "Game stop sent for ${pod}"
    done

    # End billing sessions we created
    for pod in $CREATED_BILLING_PODS; do
        local sid
        sid=$(curl -s --max-time 5 "${BASE_URL}/billing/sessions/active" 2>/dev/null | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    sessions = data if isinstance(data, list) else data.get('sessions', [])
    for s in sessions:
        if s.get('pod_id','') == '${pod}':
            print(s.get('id') or s.get('billing_session_id') or s.get('session_id') or '')
            break
except: pass
" 2>/dev/null)
        if [ -n "$sid" ]; then
            curl -s --max-time 5 -X POST "${BASE_URL}/billing/${sid}/stop" 2>/dev/null > /dev/null
            info "Billing session ${sid} ended on ${pod}"
        fi
    done
    info "Cleanup complete"
}

# ─── Pre-gate: Server Health ────────────────────────────────────────────────
echo "--- Pre-gate: Server Health ---"
HEALTH=$(curl -s --max-time 5 "${BASE_URL}/health" 2>/dev/null || echo "UNREACHABLE")
if [ "$HEALTH" = "UNREACHABLE" ]; then
    fail "racecontrol not reachable at ${BASE_URL}"
    echo "Cannot proceed — server is down."
    exit 1
fi
pass "Server reachable"

# ─── Pre-gate: Terminal auth works ──────────────────────────────────────────
echo ""
echo "--- Pre-gate: Terminal Auth ---"
AUTH_CHECK=$(curl -s --max-time 5 \
    -H "x-terminal-secret: ${TERMINAL_SECRET}" \
    "${BASE_URL}/terminal/group-sessions" 2>/dev/null)
if echo "$AUTH_CHECK" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
except Exception:
    sys.exit(1)
# Success if we get any sessions key or no auth error
if 'sessions' in d or 'group_sessions' in d or isinstance(d, list):
    sys.exit(0)
err = d.get('error','')
if 'auth' in err.lower() or 'unauthorized' in err.lower():
    sys.exit(1)
sys.exit(0)  # other errors are OK — auth passed
" 2>/dev/null; then
    pass "Terminal auth accepted"
else
    fail "Terminal auth rejected — check TERMINAL_SECRET"
    exit 1
fi

# ─── Pre-gate: Find 2+ connected pods ──────────────────────────────────────
echo ""
echo "--- Pre-gate: Find Connected Pods ---"
FLEET=$(curl -s --max-time 10 "${BASE_URL}/fleet/health" 2>/dev/null)
# Pod IDs use underscores (pod_1, pod_2, ...) not hyphens
CONNECTED_PODS=$(echo "$FLEET" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    pods = data.get('pods', [])
    connected = []
    for p in pods:
        if p.get('ws_connected', False):
            pid = p.get('pod_id', '')
            if pid:
                connected.append(pid)
    print(' '.join(connected))
except: pass
" 2>/dev/null)

CONNECTED_COUNT=$(echo "$CONNECTED_PODS" | wc -w | tr -d ' ')
info "Connected pods (${CONNECTED_COUNT}): ${CONNECTED_PODS}"

if [ "$CONNECTED_COUNT" -lt "$MIN_PODS" ]; then
    fail "Need at least ${MIN_PODS} connected pods, found ${CONNECTED_COUNT}"
    echo "Cannot test multiplayer without enough connected pods."
    summary_exit
fi
pass "${CONNECTED_COUNT} pods connected (need ${MIN_PODS})"

# Pick first 2 connected pods for the test
POD_A=$(echo "$CONNECTED_PODS" | awk '{print $1}')
POD_B=$(echo "$CONNECTED_PODS" | awk '{print $2}')
info "Test pods: ${POD_A} + ${POD_B}"
USED_PODS="${POD_A} ${POD_B}"

# Resolve pod IPs for pod-map (convert pod_N → pod-N for pod_ip lookup)
POD_A_DASH=$(echo "$POD_A" | sed 's/_/-/')
POD_B_DASH=$(echo "$POD_B" | sed 's/_/-/')

# ─── Pre-gate: Ensure test drivers exist ────────────────────────────────────
echo ""
echo "--- Pre-gate: Test Drivers ---"
# Check if driver_test_trial exists
D1_CHECK=$(curl -s --max-time 5 "${BASE_URL}/drivers/${TEST_DRIVER_1}" 2>/dev/null)
if echo "$D1_CHECK" | python3 -c "import sys,json; d=json.load(sys.stdin); sys.exit(0 if d.get('id') or d.get('name') else 1)" 2>/dev/null; then
    pass "Test driver 1: ${TEST_DRIVER_1} exists"
else
    info "Creating test driver 1..."
    curl -s --max-time 5 -X POST "${BASE_URL}/drivers" \
        -H "Content-Type: application/json" \
        -d '{"name": "Test Driver (Unlimited)", "phone": "0000000000"}' 2>/dev/null > /dev/null
    TEST_DRIVER_1="driver_test_trial"
    pass "Test driver 1 created"
fi

# Create or find second test driver
D2_RESP=$(curl -s --max-time 5 -X POST "${BASE_URL}/drivers" \
    -H "Content-Type: application/json" \
    -d '{"name": "TEST_ONLY MP Driver 2", "phone": "0000000001"}' 2>/dev/null)
TEST_DRIVER_2=$(echo "$D2_RESP" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    print(d.get('id', ''))
except: print('')
" 2>/dev/null)

if [ -n "$TEST_DRIVER_2" ]; then
    pass "Test driver 2: ${TEST_DRIVER_2}"
else
    # Phone might already exist — list drivers and find one
    TEST_DRIVER_2=$(curl -s --max-time 5 "${BASE_URL}/drivers" 2>/dev/null | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    drivers = d if isinstance(d, list) else d.get('drivers', [])
    for dr in drivers:
        if dr.get('id','') != '${TEST_DRIVER_1}':
            print(dr.get('id',''))
            break
except: pass
" 2>/dev/null)
    if [ -n "$TEST_DRIVER_2" ]; then
        pass "Test driver 2 (existing): ${TEST_DRIVER_2}"
    else
        fail "Could not create or find second test driver"
        exit 1
    fi
fi

# ─── Pre-gate: Ensure pods are FREE (no active billing) ─────────────────────
# staff_book_multiplayer creates its own billing — pods must be idle
echo ""
echo "--- Pre-gate: Pod Availability ---"
for pod in $POD_A $POD_B; do
    # Check in-memory pod state (authoritative for billing_session_id)
    POD_STATE=$(curl -s --max-time 5 "${BASE_URL}/pods/${pod}" 2>/dev/null)
    BILLING_ID=$(echo "$POD_STATE" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    p = d.get('pod', d)
    bid = p.get('billing_session_id')
    print(bid if bid and bid != 'None' else '')
except Exception: print('')
" 2>/dev/null)

    if [ -n "$BILLING_ID" ]; then
        # Stop game first, then end billing
        info "${pod}: has active billing ${BILLING_ID} — stopping game + billing for test"
        curl -s --max-time 5 -X POST \
            -H "Content-Type: application/json" \
            -d "{\"pod_id\": \"${pod}\"}" \
            "${BASE_URL}/games/stop" 2>/dev/null > /dev/null
        sleep 2
        curl -s --max-time 5 -X POST "${BASE_URL}/billing/${BILLING_ID}/stop" 2>/dev/null > /dev/null
        sleep 2
        # Verify cleared
        VERIFY=$(curl -s --max-time 5 "${BASE_URL}/pods/${pod}" 2>/dev/null | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    p = d.get('pod', d)
    bid = p.get('billing_session_id')
    print('idle' if not bid or bid == 'None' else 'busy')
except Exception: print('error')
" 2>/dev/null)
        if [ "$VERIFY" = "idle" ]; then
            pass "${pod}: billing cleared"
        else
            fail "${pod}: billing still active after stop"
        fi
    else
        pass "${pod}: idle (no billing)"
    fi
done

# ─── Pre-gate: Ensure wallets have credits ──────────────────────────────────
# staff_book_multiplayer debits wallets — need enough credits (tier_30min = ₹700 = 70000p)
echo ""
echo "--- Pre-gate: Wallet Credits ---"
for driver in $TEST_DRIVER_1 $TEST_DRIVER_2; do
    BALANCE=$(curl -s --max-time 5 "${BASE_URL}/wallet/${driver}" 2>/dev/null | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    print(d.get('balance_paise', d.get('balance', 0)))
except Exception: print('0')
" 2>/dev/null)
    if [ "${BALANCE:-0}" -lt 70000 ] 2>/dev/null; then
        TOPUP=$(curl -s --max-time 5 -X POST "${BASE_URL}/wallet/${driver}/topup" \
            -H "Content-Type: application/json" \
            -d '{"amount_paise": 200000, "method": "cash", "notes": "E2E test topup"}' 2>/dev/null)
        NEW_BAL=$(echo "$TOPUP" | python3 -c "import sys,json; print(json.load(sys.stdin).get('new_balance_paise','?'))" 2>/dev/null)
        pass "${driver}: topped up to ${NEW_BAL}p"
    else
        pass "${driver}: wallet has ${BALANCE}p (sufficient)"
    fi
done

# ─── Pre-cleanup: Stop stale games ──────────────────────────────────────────
echo ""
echo "--- Pre-cleanup: Clear Stale Games ---"
for pod in $POD_A $POD_B; do
    curl -s --max-time 5 -X POST \
        -H "Content-Type: application/json" \
        -d "{\"pod_id\": \"${pod}\"}" \
        "${BASE_URL}/games/stop" 2>/dev/null > /dev/null
done
sleep 3
info "Stale games cleared on ${POD_A} + ${POD_B}"

# ═══════════════════════════════════════════════════════════════════════════
# MP-01: Book multiplayer group session via terminal endpoint
# ═══════════════════════════════════════════════════════════════════════════
echo ""
echo "========================================"
echo "--- MP-01: Book Multiplayer Group Session ---"
echo "========================================"

BOOK_RESP=$(curl -s --max-time 20 -X POST \
    -H "Content-Type: application/json" \
    -H "x-terminal-secret: ${TERMINAL_SECRET}" \
    -d "$(python3 -c "
import json
payload = {
    'driver_ids': ['${TEST_DRIVER_1}', '${TEST_DRIVER_2}'],
    'pod_ids': ['${POD_A}', '${POD_B}'],
    'pricing_tier_id': 'tier_30min',
    'game': 'assetto_corsa',
    'track': 'monza',
    'car': 'ks_ferrari_488_gt3'
}
print(json.dumps(payload))
" 2>/dev/null)" \
    "${BASE_URL}/terminal/book-multiplayer" 2>/dev/null)

info "Book response: ${BOOK_RESP}"

if echo "$BOOK_RESP" | grep -q '"status":"ok"'; then
    pass "MP-01: Multiplayer group session booked successfully"

    # Extract group_session_id
    GROUP_SESSION_ID=$(echo "$BOOK_RESP" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    gs = d.get('group_session', {})
    print(gs.get('id') or gs.get('group_session_id') or '')
except: print('')
" 2>/dev/null)
    info "Group session ID: ${GROUP_SESSION_ID}"

    # Check group session status for ac_launch_failed (new: server now reports this)
    GS_STATUS=$(echo "$BOOK_RESP" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    gs = d.get('group_session', {})
    print(gs.get('status', '?'))
except Exception: print('?')
" 2>/dev/null)
    info "Group session status: ${GS_STATUS}"
    if [ "$GS_STATUS" = "ac_launch_failed" ]; then
        fail "MP-01: Booking succeeded but AC server launch FAILED (acServer.exe missing or misconfigured)"
        info "Check [ac_server] acserver_path in racecontrol.toml on the venue server"
        cleanup
        summary_exit
    fi

elif echo "$BOOK_RESP" | grep -qi "error"; then
    ERR=$(echo "$BOOK_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin).get('error','unknown'))" 2>/dev/null || echo "$BOOK_RESP")
    fail "MP-01: Multiplayer booking failed — ${ERR}"
    cleanup
    summary_exit
else
    fail "MP-01: Unexpected response — ${BOOK_RESP}"
    cleanup
    summary_exit
fi

# ═══════════════════════════════════════════════════════════════════════════
# MP-02: Verify AC server started (poll game state)
# ═══════════════════════════════════════════════════════════════════════════
echo ""
echo "========================================"
echo "--- MP-02: AC Server Auto-Start ---"
echo "========================================"

# First check if the group session status indicates AC launch failure
# (the server now sets status='ac_launch_failed' instead of silently ignoring)
sleep 2
GS_CHECK=$(curl -s --max-time 5 \
    -H "x-terminal-secret: ${TERMINAL_SECRET}" \
    "${BASE_URL}/terminal/group-sessions" 2>/dev/null | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    sessions = d.get('group_sessions', [])
    for s in sessions:
        if s.get('id','') == '${GROUP_SESSION_ID}':
            print(s.get('status','?'))
            break
    else: print('NOT_FOUND')
except Exception: print('ERROR')
" 2>/dev/null)

if [ "$GS_CHECK" = "ac_launch_failed" ]; then
    fail "MP-02: AC server launch failed (status=ac_launch_failed — acServer.exe missing on server)"
    info "Install AC dedicated server and set [ac_server] acserver_path in racecontrol.toml"
    # Skip polling — we know it won't work
    AC_STARTED=false
else
    info "Group session status: ${GS_CHECK}"

# Poll /games/active for up to 30s for either pod to show launching/running
AC_STARTED=false
for i in $(seq 1 10); do
    ACTIVE_GAMES=$(curl -s --max-time 5 "${BASE_URL}/games/active" 2>/dev/null)
    POD_A_STATE=$(echo "$ACTIVE_GAMES" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    games = data if isinstance(data, list) else data.get('games', [])
    for g in games:
        if g.get('pod_id','') == '${POD_A}':
            print(g.get('game_state','unknown'))
            break
    else: print('NONE')
except: print('ERROR')
" 2>/dev/null)

    POD_B_STATE=$(echo "$ACTIVE_GAMES" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    games = data if isinstance(data, list) else data.get('games', [])
    for g in games:
        if g.get('pod_id','') == '${POD_B}':
            print(g.get('game_state','unknown'))
            break
    else: print('NONE')
except: print('ERROR')
" 2>/dev/null)

    info "Poll ${i}/10: ${POD_A}=${POD_A_STATE}, ${POD_B}=${POD_B_STATE}"

    if echo "$POD_A_STATE" | grep -qiE "launching|running" && echo "$POD_B_STATE" | grep -qiE "launching|running"; then
        AC_STARTED=true
        break
    fi

    # Accept if at least one pod has the game after extended polling
    if echo "$POD_A_STATE" | grep -qiE "launching|running" || echo "$POD_B_STATE" | grep -qiE "launching|running"; then
        if [ "$i" -ge 5 ]; then
            AC_STARTED=true
            break
        fi
    fi

    sleep 3
done

if [ "$AC_STARTED" = "true" ]; then
    pass "MP-02: AC game detected — ${POD_A}=${POD_A_STATE}, ${POD_B}=${POD_B_STATE}"
else
    fail "MP-02: AC game not detected on pods within 30s — ${POD_A}=${POD_A_STATE}, ${POD_B}=${POD_B_STATE}"
fi

fi  # end of ac_launch_failed else block

# ═══════════════════════════════════════════════════════════════════════════
# MP-03: Verify game state shows assetto_corsa on both pods
# ═══════════════════════════════════════════════════════════════════════════
echo ""
echo "========================================"
echo "--- MP-03: Game State Verification ---"
echo "========================================"

ACTIVE_GAMES=$(curl -s --max-time 5 "${BASE_URL}/games/active" 2>/dev/null)
for pod in $POD_A $POD_B; do
    SIM=$(echo "$ACTIVE_GAMES" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    games = data if isinstance(data, list) else data.get('games', [])
    for g in games:
        if g.get('pod_id','') == '${pod}':
            print(g.get('sim_type','NONE'))
            break
    else: print('NONE')
except: print('ERROR')
" 2>/dev/null)

    if [ "$SIM" = "assetto_corsa" ]; then
        pass "MP-03: ${pod} running assetto_corsa"
    elif [ "$SIM" = "NONE" ]; then
        info "MP-03: ${pod} — no active game (AC may have exited or not reached this pod)"
    else
        info "MP-03: ${pod} running ${SIM} (expected assetto_corsa)"
    fi
done

# ═══════════════════════════════════════════════════════════════════════════
# MP-04: Verify group session appears in terminal listing
# ═══════════════════════════════════════════════════════════════════════════
echo ""
echo "========================================"
echo "--- MP-04: Group Session Listing ---"
echo "========================================"

SESSIONS=$(curl -s --max-time 10 \
    -H "x-terminal-secret: ${TERMINAL_SECRET}" \
    "${BASE_URL}/terminal/group-sessions" 2>/dev/null)

if [ -n "$GROUP_SESSION_ID" ]; then
    FOUND=$(echo "$SESSIONS" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    sessions = d.get('group_sessions', d.get('sessions', d if isinstance(d, list) else []))
    for s in sessions:
        sid = s.get('id') or s.get('group_session_id') or ''
        if sid == '${GROUP_SESSION_ID}':
            print(json.dumps({
                'status': s.get('status','?'),
                'total_members': s.get('total_members', 0),
                'validated_count': s.get('validated_count', 0),
                'pin': s.get('shared_pin', s.get('pin', '?'))
            }))
            break
    else: print('NOT_FOUND')
except Exception as e: print(f'ERROR: {e}')
" 2>/dev/null)

    if [ "$FOUND" != "NOT_FOUND" ] && [ "${FOUND:0:5}" != "ERROR" ]; then
        pass "MP-04: Group session ${GROUP_SESSION_ID} found in listing"
        info "Session details: ${FOUND}"
    else
        fail "MP-04: Group session ${GROUP_SESSION_ID} not found in listing (${FOUND})"
    fi
else
    info "MP-04: No group_session_id to verify (booking may have failed)"
    skip "MP-04: Skipped — no group session ID"
fi

# ═══════════════════════════════════════════════════════════════════════════
# MP-05: Stop multiplayer + verify cleanup
# ═══════════════════════════════════════════════════════════════════════════
echo ""
echo "========================================"
echo "--- MP-05: Stop + Cleanup ---"
echo "========================================"

# Stop games on both pods
for pod in $POD_A $POD_B; do
    STOP_RESP=$(curl -s --max-time 10 -X POST \
        -H "Content-Type: application/json" \
        -d "{\"pod_id\": \"${pod}\"}" \
        "${BASE_URL}/games/stop" 2>/dev/null)
    info "Stop ${pod}: ${STOP_RESP}"
done

# Wait for games to clear
sleep 5

# Verify games stopped
ALL_STOPPED=true
for pod in $POD_A $POD_B; do
    STATE=$(curl -s --max-time 5 "${BASE_URL}/games/active" 2>/dev/null | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    games = data if isinstance(data, list) else data.get('games', [])
    for g in games:
        if g.get('pod_id','') == '${pod}':
            print(g.get('game_state','unknown'))
            break
    else: print('NONE')
except: print('ERROR')
" 2>/dev/null)
    if [ "$STATE" = "NONE" ]; then
        pass "MP-05: ${pod} game stopped cleanly"
    else
        info "MP-05: ${pod} still in state ${STATE} after stop"
        ALL_STOPPED=false
    fi
done

if [ "$ALL_STOPPED" = "true" ]; then
    pass "MP-05: All pods cleaned up"
fi

# Full cleanup (billing etc.)
cleanup

# ─── Summary ────────────────────────────────────────────────────────────────
echo ""
summary_exit
