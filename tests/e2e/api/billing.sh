#!/bin/bash
# tests/e2e/api/billing.sh — Billing lifecycle E2E test (API-01)
# Tests: create session, billing gate rejection, active session check, end session
# Usage: bash tests/e2e/api/billing.sh
#   RC_BASE_URL=http://192.168.31.23:8080/api/v1 bash tests/e2e/api/billing.sh
set -uo pipefail

BASE_URL="${RC_BASE_URL:-http://192.168.31.23:8080/api/v1}"
POD_ID="${TEST_POD_ID:-pod-8}"
SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=../lib/common.sh
source "$SCRIPT_DIR/../lib/common.sh"
# shellcheck source=../lib/pod-map.sh
source "$SCRIPT_DIR/../lib/pod-map.sh"

echo "========================================"
echo "Billing Lifecycle E2E Test (API-01)"
echo "Base URL : ${BASE_URL}"
echo "Pod ID   : ${POD_ID}"
echo "========================================"
echo ""

SESSION_ID=""

# ─── Gate 0: Server reachable ─────────────────────────────────────────────
echo "--- Gate 0: Server Health ---"
HEALTH=$(curl -s --max-time 5 "${BASE_URL}/health" 2>/dev/null || echo "UNREACHABLE")
if [ "$HEALTH" = "UNREACHABLE" ]; then
    fail "racecontrol not reachable at ${BASE_URL}"
    echo ""
    echo "Cannot proceed — server is down."
    exit 1
fi
pass "Server reachable"

# ─── Gate 1: Billing gate rejection (API-01 core test) ───────────────────
echo ""
echo "--- Gate 1: Billing Gate Rejection ---"
# Attempt launch on pod-99 (non-existent pod, definitely no billing)
# Assert that the server rejects it with a billing gate error
REJECT_RESP=$(curl -s --max-time 10 -X POST \
    -H "Content-Type: application/json" \
    -d '{"pod_id":"pod-99","sim_type":"f1_25"}' \
    "${BASE_URL}/games/launch" 2>/dev/null)

if echo "$REJECT_RESP" | grep -qi "no active billing"; then
    pass "Launch correctly rejected on pod-99: no active billing"
elif echo "$REJECT_RESP" | grep -qi "error"; then
    ERR_MSG=$(echo "$REJECT_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin).get('error','?'))" 2>/dev/null || echo "$REJECT_RESP")
    pass "Launch rejected on pod-99 with error: ${ERR_MSG}"
else
    fail "Expected billing rejection for pod-99, got: ${REJECT_RESP}"
fi

# ─── Gate 2: Create billing session on pod-8 ─────────────────────────────
echo ""
echo "--- Gate 2: Create Billing Session ---"
BILL_RESP=$(curl -s --max-time 10 -X POST \
    -H "Content-Type: application/json" \
    -d "{\"pod_id\": \"${POD_ID}\", \"driver_id\": \"driver_test_trial\", \"pricing_tier_id\": \"tier_trial\"}" \
    "${BASE_URL}/billing/start" 2>/dev/null)

SESSION_ID=$(echo "$BILL_RESP" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    print(d.get('billing_session_id') or d.get('session_id') or '')
except: print('')
" 2>/dev/null)

if [ -n "$SESSION_ID" ]; then
    pass "Billing session created: ${SESSION_ID}"
else
    BILL_ERR=$(echo "$BILL_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin).get('error','?'))" 2>/dev/null || echo "$BILL_RESP")
    if echo "$BILL_ERR" | grep -qi "already has an active"; then
        pass "Billing already active on ${POD_ID} (idempotent)"
        # Try to extract session ID from active sessions for later use
        ACTIVE_RESP=$(curl -s --max-time 10 "${BASE_URL}/billing/active" 2>/dev/null)
        SESSION_ID=$(echo "$ACTIVE_RESP" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    sessions = data if isinstance(data, list) else data.get('sessions', [])
    for s in sessions:
        if s.get('pod_id','') == '${POD_ID}':
            print(s.get('id') or s.get('billing_session_id') or s.get('session_id') or '')
            break
except: print('')
" 2>/dev/null)
        if [ -n "$SESSION_ID" ]; then
            info "Recovered existing session ID: ${SESSION_ID}"
        fi
    else
        fail "Could not create test billing: ${BILL_ERR}"
    fi
fi

# ─── Gate 3: Verify active session appears ───────────────────────────────
echo ""
echo "--- Gate 3: Verify Active Session ---"
ACTIVE=$(curl -s --max-time 10 "${BASE_URL}/billing/active" 2>/dev/null)
POD_IN_ACTIVE=$(echo "$ACTIVE" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    sessions = data if isinstance(data, list) else data.get('sessions', [])
    for s in sessions:
        if s.get('pod_id','') == '${POD_ID}':
            print('FOUND')
            break
    else:
        print('NOT_FOUND')
except: print('PARSE_ERROR')
" 2>/dev/null)

if [ "$POD_IN_ACTIVE" = "FOUND" ]; then
    pass "${POD_ID} appears in active billing sessions"
elif [ "$POD_IN_ACTIVE" = "NOT_FOUND" ]; then
    fail "${POD_ID} not found in active billing sessions"
    info "Active sessions response: ${ACTIVE}"
else
    fail "Could not parse active sessions response: ${ACTIVE}"
fi

# ─── Gate 4: End billing session ─────────────────────────────────────────
echo ""
echo "--- Gate 4: End Billing Session ---"
if [ -n "$SESSION_ID" ]; then
    STOP_RESP=$(curl -s --max-time 10 -X POST \
        "${BASE_URL}/billing/${SESSION_ID}/stop" 2>/dev/null)
    if echo "$STOP_RESP" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    sys.exit(0 if d.get('ok') or d.get('stopped') or d.get('success') else 1)
except: sys.exit(1)
" 2>/dev/null; then
        pass "Billing session ${SESSION_ID} ended cleanly"
    else
        # Some implementations return empty body or different shape on success
        HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" --max-time 10 -X POST \
            "${BASE_URL}/billing/${SESSION_ID}/stop" 2>/dev/null)
        if [ "$HTTP_CODE" = "200" ] || [ "$HTTP_CODE" = "204" ]; then
            pass "Billing session stop returned HTTP ${HTTP_CODE}"
        else
            fail "Could not stop billing session: ${STOP_RESP}"
        fi
    fi
else
    skip "No SESSION_ID captured — skipping end session test"
fi

# ─── Gate 5: Verify session ended ────────────────────────────────────────
echo ""
echo "--- Gate 5: Verify Session Ended ---"
# Poll up to 3 attempts with 2s sleep since session end may be async
FOUND_AFTER_STOP=false
for attempt in 1 2 3; do
    if [ "$attempt" -gt 1 ]; then
        sleep 2
    fi
    ACTIVE_CHECK=$(curl -s --max-time 10 "${BASE_URL}/billing/active" 2>/dev/null)
    STILL_ACTIVE=$(echo "$ACTIVE_CHECK" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    sessions = data if isinstance(data, list) else data.get('sessions', [])
    for s in sessions:
        if s.get('pod_id','') == '${POD_ID}':
            print('STILL_ACTIVE')
            break
    else:
        print('GONE')
except: print('PARSE_ERROR')
" 2>/dev/null)
    if [ "$STILL_ACTIVE" = "GONE" ]; then
        FOUND_AFTER_STOP=false
        break
    elif [ "$STILL_ACTIVE" = "STILL_ACTIVE" ]; then
        FOUND_AFTER_STOP=true
        info "Attempt ${attempt}: session still active, retrying..."
    fi
done

if [ "$FOUND_AFTER_STOP" = "false" ]; then
    pass "${POD_ID} billing session no longer in active list"
else
    # Gate 4 may have been skipped (no SESSION_ID) — if so, this is expected
    if [ -z "$SESSION_ID" ]; then
        skip "Session was not stopped (no SESSION_ID) — active state expected"
    else
        fail "${POD_ID} billing session still active after stop command"
    fi
fi

# ─── Summary ─────────────────────────────────────────────────────────────
echo ""
summary_exit
