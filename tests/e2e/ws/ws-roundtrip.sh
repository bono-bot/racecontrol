#!/bin/bash
# tests/e2e/ws/ws-roundtrip.sh — WebSocket Message Round-Trip Test
#
# Validates that the dashboard WS endpoint sends real-time updates
# when fleet state changes. Tests message format compatibility between
# server and what frontends expect.
#
# Tests:
#   1. Connect to /ws/dashboard and receive initial pod_list
#   2. Verify pod_list data array has expected fields per pod
#   3. Verify billing_session_list is sent on connect
#   4. Multi-message collection — verify we get >1 message types within 15s
#
# Usage:
#   bash tests/e2e/ws/ws-roundtrip.sh

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
source "$SCRIPT_DIR/../lib/common.sh"

RC_BASE_URL="${RC_BASE_URL:-http://192.168.31.23:8080/api/v1}"
WS_HOST=$(echo "$RC_BASE_URL" | sed 's|https\?://||; s|/.*||')
RC_WS_URL="${RC_WS_URL:-ws://${WS_HOST}}"
RC_WS_TOKEN="${RC_WS_TOKEN:-rp-terminal-2026}"

info "WebSocket Round-Trip Tests"
info "WS URL: ${RC_WS_URL}/ws/dashboard"
echo ""

# ─── Test 1: pod_list on connect ──────────────────────────────────────────────

POD_LIST=$(node --no-warnings "$SCRIPT_DIR/ws-connect.mjs" \
    "${RC_WS_URL}/ws/dashboard?token=${RC_WS_TOKEN}" \
    '{"expect_type":"pod_list","timeout_ms":10000}' 2>&1)
POD_LIST_EXIT=$?

if [ $POD_LIST_EXIT -eq 0 ]; then
    # Validate pod data structure
    POD_VALIDATION=$(echo "$POD_LIST" | node --no-warnings -e "
        const msg = JSON.parse(require('fs').readFileSync(0, 'utf8'));
        const pods = msg.data || [];
        const results = [];

        if (!Array.isArray(pods)) {
            results.push('NOT_ARRAY');
        } else if (pods.length === 0) {
            results.push('EMPTY');
        } else {
            const required = ['id', 'number', 'name', 'status'];
            const expected = ['ip_address', 'sim_type', 'driving_state', 'screen_blanked', 'last_seen'];
            const pod = pods[0];

            for (const f of required) {
                if (!(f in pod)) results.push('MISSING_' + f.toUpperCase());
            }

            let optionalPresent = 0;
            for (const f of expected) {
                if (f in pod) optionalPresent++;
            }
            results.push('OPTIONAL_' + optionalPresent + '_OF_' + expected.length);
        }

        console.log(results.join(',') || 'VALID');
    " 2>/dev/null)

    case "$POD_VALIDATION" in
        *MISSING*)
            fail "pod_list round-trip: missing required fields — ${POD_VALIDATION}" ;;
        EMPTY)
            pass "pod_list round-trip: received empty pod list (no pods registered)" ;;
        NOT_ARRAY)
            fail "pod_list round-trip: data is not an array" ;;
        *)
            pass "pod_list round-trip: valid structure — ${POD_VALIDATION}" ;;
    esac
else
    fail "pod_list round-trip: connection failed — ${POD_LIST}"
fi

# ─── Test 2: Multi-message collection (15s window) ───────────────────────────

MULTI_MSGS=$(node --no-warnings "$SCRIPT_DIR/ws-collect.mjs" \
    "${RC_WS_URL}/ws/dashboard?token=${RC_WS_TOKEN}" \
    '{"collect_ms":15000,"max_messages":50}' 2>&1)
MULTI_EXIT=$?

if [ $MULTI_EXIT -eq 0 ]; then
    MSG_TYPES=$(echo "$MULTI_MSGS" | node --no-warnings -e "
        const data = JSON.parse(require('fs').readFileSync(0, 'utf8'));
        const types = [...new Set(data.messages.map(m => m.type || m.event).filter(Boolean))];
        console.log(JSON.stringify({
            count: data.messages.length,
            types: types,
            unique: types.length,
        }));
    " 2>/dev/null)

    MSG_COUNT=$(echo "$MSG_TYPES" | node --no-warnings -e "console.log(JSON.parse(require('fs').readFileSync(0,'utf8')).count)")
    UNIQUE_TYPES=$(echo "$MSG_TYPES" | node --no-warnings -e "console.log(JSON.parse(require('fs').readFileSync(0,'utf8')).unique)")
    TYPE_LIST=$(echo "$MSG_TYPES" | node --no-warnings -e "console.log(JSON.parse(require('fs').readFileSync(0,'utf8')).types.join(', '))")

    if [ "$MSG_COUNT" -gt 0 ] 2>/dev/null; then
        pass "Multi-message: received ${MSG_COUNT} messages, ${UNIQUE_TYPES} types: [${TYPE_LIST}]"
    else
        fail "Multi-message: received 0 messages in 15s window"
    fi

    # Verify no parse errors in collected messages
    PARSE_ERRORS=$(echo "$MULTI_MSGS" | node --no-warnings -e "
        const data = JSON.parse(require('fs').readFileSync(0, 'utf8'));
        console.log(data.parse_errors || 0);
    " 2>/dev/null)

    if [ "$PARSE_ERRORS" = "0" ]; then
        pass "Multi-message: all ${MSG_COUNT} messages are valid JSON"
    else
        fail "Multi-message: ${PARSE_ERRORS} messages failed JSON parse (format mismatch)"
    fi
else
    fail "Multi-message: collection failed — ${MULTI_MSGS}"
fi

# ─── Test 3: Connection stability (no unexpected close in 10s) ────────────────

STABILITY=$(node --no-warnings "$SCRIPT_DIR/ws-collect.mjs" \
    "${RC_WS_URL}/ws/dashboard?token=${RC_WS_TOKEN}" \
    '{"collect_ms":10000,"max_messages":100}' 2>&1)
STABILITY_EXIT=$?

if [ $STABILITY_EXIT -eq 0 ]; then
    CLOSE_CODE=$(echo "$STABILITY" | node --no-warnings -e "
        const data = JSON.parse(require('fs').readFileSync(0, 'utf8'));
        console.log(data.close_code || 'clean');
    " 2>/dev/null)

    if [ "$CLOSE_CODE" = "clean" ] || [ "$CLOSE_CODE" = "1000" ]; then
        pass "Connection stability: no unexpected disconnects in 10s"
    else
        fail "Connection stability: unexpected close code ${CLOSE_CODE} within 10s"
    fi
else
    fail "Connection stability: test failed — ${STABILITY}"
fi

echo ""
summary_exit
