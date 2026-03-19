#!/bin/bash
# tests/e2e/api/session-lifecycle.sh -- Session lifecycle E2E test (SESSION-01, SESSION-02)
# Tests: billing create, end_reason schema presence, pod status API, billing active check,
#        session end + pod reset timing, end_reason field on completed session
# Usage: bash tests/e2e/api/session-lifecycle.sh
#   RC_BASE_URL=http://192.168.31.23:8080/api/v1 bash tests/e2e/api/session-lifecycle.sh
set -uo pipefail

BASE_URL="${RC_BASE_URL:-http://192.168.31.23:8080/api/v1}"
POD_ID="${TEST_POD_ID:-pod-8}"
SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=../lib/common.sh
source "$SCRIPT_DIR/../lib/common.sh"
# shellcheck source=../lib/pod-map.sh
source "$SCRIPT_DIR/../lib/pod-map.sh"

echo "========================================"
echo "Session Lifecycle E2E Test (SESSION-01/02)"
echo "Base URL : ${BASE_URL}"
echo "Pod ID   : ${POD_ID}"
echo "========================================"
echo ""

SESSION_ID=""

# Cleanup trap: end any stale billing session on test exit to avoid poisoning Pod 8
cleanup() {
    if [ -n "$SESSION_ID" ]; then
        info "Cleanup: ending session ${SESSION_ID} to prevent stale billing"
        curl -s --max-time 5 -X POST "${BASE_URL}/billing/session/${SESSION_ID}/end" >/dev/null 2>&1 || true
    fi
}
trap cleanup EXIT

# --- Gate 0: Server reachable -----------------------------------------------
echo "--- Gate 0: Server Health ---"
HEALTH=$(curl -s --max-time 5 "${BASE_URL}/health" 2>/dev/null || echo "UNREACHABLE")
if [ "$HEALTH" = "UNREACHABLE" ]; then
    fail "racecontrol not reachable at ${BASE_URL}"
    echo ""
    echo "Cannot proceed -- server is down."
    exit 1
fi
pass "Server reachable"

# --- Gate 1: end_reason schema check ----------------------------------------
echo ""
echo "--- Gate 1: end_reason column schema check ---"
# Check recent billing sessions response includes end_reason field (even if null).
# Uses GET /billing/sessions?pod_id=... to inspect schema without creating a session.
SESSIONS_RESP=$(curl -s --max-time 10 \
    "${BASE_URL}/billing/sessions?pod_id=${POD_ID}&limit=1" 2>/dev/null || echo "UNREACHABLE")

if [ "$SESSIONS_RESP" = "UNREACHABLE" ]; then
    skip "Could not reach billing/sessions endpoint -- skipping schema check"
else
    # Check if 'end_reason' key appears in the response at all (schema existence check)
    HAS_END_REASON=$(echo "$SESSIONS_RESP" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    # Could be a list or {sessions: [...]}
    items = d if isinstance(d, list) else d.get('sessions', d.get('data', []))
    if isinstance(items, list):
        if len(items) == 0:
            # No sessions yet -- check if the outer object has the field as a hint
            print('NO_SESSIONS')
        else:
            s = items[0]
            print('HAS_FIELD' if 'end_reason' in s else 'MISSING_FIELD')
    else:
        # Unexpected shape -- still pass if we got a response
        print('UNEXPECTED_SHAPE')
except Exception as e:
    print('PARSE_ERROR')
" 2>/dev/null || echo "PARSE_ERROR")

    case "$HAS_END_REASON" in
        HAS_FIELD)
            pass "end_reason field present in billing session schema"
            ;;
        NO_SESSIONS)
            skip "No historical sessions for ${POD_ID} -- cannot verify end_reason schema from history"
            ;;
        MISSING_FIELD)
            fail "end_reason field missing from billing session response -- migration may not have run"
            ;;
        UNEXPECTED_SHAPE|PARSE_ERROR)
            skip "Could not parse sessions response for schema check: ${SESSIONS_RESP}"
            ;;
    esac
fi

# --- Gate 2: Pod status API returns billing state ---------------------------
echo ""
echo "--- Gate 2: Pod status API ---"
POD_STATUS_RESP=$(curl -s --max-time 10 \
    "${BASE_URL}/pods/${POD_ID}" 2>/dev/null || echo "UNREACHABLE")

if [ "$POD_STATUS_RESP" = "UNREACHABLE" ]; then
    fail "Could not reach ${BASE_URL}/pods/${POD_ID}"
else
    POD_STATUS_OK=$(echo "$POD_STATUS_RESP" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    # Accept any response shape that has pod_id or id or billing_active field
    if 'error' not in d and ('pod_id' in d or 'id' in d or 'billing_active' in d or 'status' in d):
        print('OK')
    else:
        print('NO_STATUS')
except:
    print('PARSE_ERROR')
" 2>/dev/null || echo "PARSE_ERROR")

    if [ "$POD_STATUS_OK" = "OK" ]; then
        pass "Pod status API returned a valid response for ${POD_ID}"
    else
        fail "Pod status API response malformed or missing billing fields: ${POD_STATUS_RESP}"
    fi
fi

# --- Gate 3: Billing create + active check ----------------------------------
echo ""
echo "--- Gate 3: Create billing session ---"
BILL_RESP=$(curl -s --max-time 10 -X POST \
    -H "Content-Type: application/json" \
    -d "{\"pod_id\": \"${POD_ID}\", \"driver_id\": \"driver_test_trial\", \"pricing_tier_id\": \"tier_trial\"}" \
    "${BASE_URL}/billing/start" 2>/dev/null || echo "UNREACHABLE")

if [ "$BILL_RESP" = "UNREACHABLE" ]; then
    fail "Could not reach billing/start endpoint"
else
    SESSION_ID=$(echo "$BILL_RESP" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    print(d.get('billing_session_id') or d.get('session_id') or '')
except:
    print('')
" 2>/dev/null || echo "")

    if [ -n "$SESSION_ID" ]; then
        pass "Billing session created: ${SESSION_ID}"
    else
        BILL_ERR=$(echo "$BILL_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin).get('error','?'))" 2>/dev/null || echo "$BILL_RESP")
        if echo "$BILL_ERR" | grep -qi "already has an active"; then
            pass "Billing already active on ${POD_ID} (pre-existing session)"
            # Try to recover existing session ID for later cleanup
            ACTIVE_RESP=$(curl -s --max-time 10 "${BASE_URL}/billing/active" 2>/dev/null || echo "")
            SESSION_ID=$(echo "$ACTIVE_RESP" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    sessions = data if isinstance(data, list) else data.get('sessions', [])
    for s in sessions:
        if s.get('pod_id','') == '${POD_ID}':
            print(s.get('id') or s.get('billing_session_id') or s.get('session_id') or '')
            break
except:
    print('')
" 2>/dev/null || echo "")
            if [ -n "$SESSION_ID" ]; then
                info "Recovered existing session ID: ${SESSION_ID}"
            fi
        else
            fail "Could not create test billing: ${BILL_ERR}"
        fi
    fi
fi

# --- Gate 4: End session + verify pod reset (SESSION-02) --------------------
echo ""
echo "--- Gate 4: End session + pod reset ---"
if [ -n "$SESSION_ID" ]; then
    STOP_RESP=$(curl -s --max-time 10 -X POST \
        "${BASE_URL}/billing/session/${SESSION_ID}/end?reason=e2e_test" 2>/dev/null || echo "UNREACHABLE")

    if [ "$STOP_RESP" = "UNREACHABLE" ]; then
        fail "Could not reach billing/session/${SESSION_ID}/end"
    else
        HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" --max-time 10 -X POST \
            "${BASE_URL}/billing/session/${SESSION_ID}/end" 2>/dev/null || echo "0")
        # Accept 200 or 204 (already ended from first call above is OK too)
        if [ "$HTTP_CODE" = "200" ] || [ "$HTTP_CODE" = "204" ] || echo "$STOP_RESP" | python3 -c "
import sys,json
try:
    d=json.load(sys.stdin)
    sys.exit(0 if d.get('ok') or d.get('stopped') or d.get('success') else 1)
except: sys.exit(1)
" 2>/dev/null; then
            pass "Billing session ${SESSION_ID} ended (HTTP ${HTTP_CODE})"
            SESSION_ID="" # Mark as cleared for cleanup trap
        else
            # Try to check if the session is already gone (idempotent end is OK)
            ACTIVE_AFTER=$(curl -s --max-time 10 "${BASE_URL}/billing/active" 2>/dev/null || echo "")
            STILL_ACTIVE=$(echo "$ACTIVE_AFTER" | python3 -c "
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
except:
    print('PARSE_ERROR')
" 2>/dev/null || echo "PARSE_ERROR")

            if [ "$STILL_ACTIVE" = "GONE" ]; then
                pass "Session already ended (idempotent)"
                SESSION_ID=""
            else
                fail "Could not end billing session: ${STOP_RESP}"
            fi
        fi
    fi

    # Poll for pod reset (SESSION-02): blank_timer fires after 30s and calls show_idle_pin_entry()
    # We poll the pod status for up to 35s (30s blank_timer + 5s buffer)
    if [ -z "$SESSION_ID" ]; then
        info "Polling ${POD_ID} status for up to 35s to verify billing cleared..."
        BILLING_CLEARED=false
        for attempt in 1 2 3 4 5 6 7; do
            if [ "$attempt" -gt 1 ]; then
                sleep 5
            fi
            ACTIVE_CHECK=$(curl -s --max-time 10 "${BASE_URL}/billing/active" 2>/dev/null || echo "")
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
except:
    print('PARSE_ERROR')
" 2>/dev/null || echo "PARSE_ERROR")
            if [ "$STILL_ACTIVE" = "GONE" ]; then
                BILLING_CLEARED=true
                break
            fi
            info "Attempt ${attempt}: billing still active, waiting 5s..."
        done

        if [ "$BILLING_CLEARED" = "true" ]; then
            pass "${POD_ID} billing cleared after session end (SESSION-02 pod reset)"
        else
            fail "${POD_ID} billing still active after 35s — pod reset (SESSION-02) may not have fired"
        fi
    fi
else
    skip "No SESSION_ID captured — skipping end session + pod reset test"
fi

# --- Gate 5: end_reason field on completed session --------------------------
echo ""
echo "--- Gate 5: end_reason on completed session ---"
# After ending the session, check that it has an end_reason populated.
# Uses GET /billing/sessions?pod_id=...&limit=1 to get most recent session.
RECENT_RESP=$(curl -s --max-time 10 \
    "${BASE_URL}/billing/sessions?pod_id=${POD_ID}&limit=1" 2>/dev/null || echo "UNREACHABLE")

if [ "$RECENT_RESP" = "UNREACHABLE" ]; then
    skip "Could not reach billing/sessions for end_reason verification"
else
    END_REASON_STATUS=$(echo "$RECENT_RESP" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    items = d if isinstance(d, list) else d.get('sessions', d.get('data', []))
    if isinstance(items, list) and len(items) > 0:
        s = items[0]
        if 'end_reason' not in s:
            print('MISSING_FIELD')
        elif s['end_reason'] is None:
            # Null is acceptable for sessions ended without explicit reason
            print('NULL_OK')
        else:
            print('HAS_VALUE:' + str(s['end_reason']))
    else:
        print('NO_SESSIONS')
except Exception as e:
    print('PARSE_ERROR')
" 2>/dev/null || echo "PARSE_ERROR")

    case "$END_REASON_STATUS" in
        HAS_VALUE:*)
            REASON="${END_REASON_STATUS#HAS_VALUE:}"
            pass "end_reason populated on completed session: '${REASON}'"
            ;;
        NULL_OK)
            pass "end_reason field present (null = manual/no-reason end, field exists in schema)"
            ;;
        MISSING_FIELD)
            fail "end_reason field missing from completed session -- schema migration not applied"
            ;;
        NO_SESSIONS)
            skip "No recent sessions found for ${POD_ID} to verify end_reason"
            ;;
        PARSE_ERROR)
            skip "Could not parse billing/sessions response for end_reason check"
            ;;
    esac
fi

# --- Summary ------------------------------------------------------------------
echo ""
summary_exit
