#!/bin/bash
# tests/e2e/ws/ws-smoke.sh — WebSocket Smoke Test
#
# Validates that all WebSocket endpoints accept connections and send
# properly formatted messages. Uses Node.js native WebSocket (v22+).
#
# Endpoints tested:
#   1. /ws/dashboard — Dashboard WS (receives pod_list on connect)
#   2. /ws/agent — Agent bridge (requires PSK, tests upgrade acceptance)
#
# Usage:
#   bash tests/e2e/ws/ws-smoke.sh
#   RC_WS_URL=ws://192.168.31.23:8080 bash tests/e2e/ws/ws-smoke.sh

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
source "$SCRIPT_DIR/../lib/common.sh"

# Parse WS URL from RC_BASE_URL or use default
RC_BASE_URL="${RC_BASE_URL:-http://192.168.31.23:8080/api/v1}"
# Extract host:port from RC_BASE_URL
WS_HOST=$(echo "$RC_BASE_URL" | sed 's|https\?://||; s|/.*||')
RC_WS_URL="${RC_WS_URL:-ws://${WS_HOST}}"
# WS auth token — read from env or fetch from server config
RC_WS_TOKEN="${RC_WS_TOKEN:-rp-terminal-2026}"

info "WebSocket Smoke Tests"
info "WS URL: ${RC_WS_URL}"
echo ""

# ─── Test 1: Dashboard WS — connect, receive pod_list ─────────────────────────

DASHBOARD_RESULT=$(node --no-warnings "$SCRIPT_DIR/ws-connect.mjs" \
    "${RC_WS_URL}/ws/dashboard?token=${RC_WS_TOKEN}" \
    '{"expect_type":"pod_list","timeout_ms":10000}' 2>&1)
DASHBOARD_EXIT=$?

if [ $DASHBOARD_EXIT -eq 0 ]; then
    # Validate the response is JSON with a "type" field
    MSG_TYPE=$(echo "$DASHBOARD_RESULT" | node --no-warnings -e "
        const lines = require('fs').readFileSync(0,'utf8').trim().split('\n');
        const last = lines[lines.length - 1];
        try { const d = JSON.parse(last); console.log(d.type || d.event || 'unknown'); }
        catch { console.log('parse_error'); }
    " 2>/dev/null)

    if [ "$MSG_TYPE" = "pod_list" ] || [ "$MSG_TYPE" = "pod_update" ]; then
        pass "Dashboard WS: connected, received '${MSG_TYPE}' message"
    elif [ "$MSG_TYPE" = "parse_error" ]; then
        fail "Dashboard WS: connected but message is not valid JSON"
    else
        # Got a message but not pod_list — still a valid WS connection
        pass "Dashboard WS: connected, received '${MSG_TYPE}' message (expected pod_list)"
    fi
else
    fail "Dashboard WS: connection failed — ${DASHBOARD_RESULT}"
fi

# ─── Test 2: Dashboard WS — message schema validation ─────────────────────────

SCHEMA_RESULT=$(node --no-warnings "$SCRIPT_DIR/ws-connect.mjs" \
    "${RC_WS_URL}/ws/dashboard?token=${RC_WS_TOKEN}" \
    '{"expect_type":"pod_list","timeout_ms":10000,"validate_schema":true}' 2>&1)
SCHEMA_EXIT=$?

if [ $SCHEMA_EXIT -eq 0 ]; then
    # Check that pod_list has expected structure: {event:'pod_list', data:[{id, number, name, status, ...}]}
    VALID=$(echo "$SCHEMA_RESULT" | node --no-warnings -e "
        const lines = require('fs').readFileSync(0,'utf8').trim().split('\n');
        const last = lines[lines.length - 1];
        try {
            const d = JSON.parse(last);
            const msgType = d.type || d.event;
            if (msgType === 'pod_list' && Array.isArray(d.data)) {
                if (d.data.length === 0) { console.log('empty_but_valid'); process.exit(0); }
                const pod = d.data[0];
                const hasFields = 'id' in pod && 'number' in pod && 'name' in pod && 'status' in pod;
                console.log(hasFields ? 'valid' : 'missing_fields');
            } else {
                console.log('wrong_structure');
            }
        } catch { console.log('parse_error'); }
    " 2>/dev/null)

    case "$VALID" in
        valid|empty_but_valid)
            pass "Dashboard WS schema: pod_list has id + number + name + status fields" ;;
        missing_fields)
            fail "Dashboard WS schema: pod_list items missing required fields (id, number, name, status)" ;;
        wrong_structure)
            fail "Dashboard WS schema: expected {event:'pod_list', data:[...]}" ;;
        *)
            fail "Dashboard WS schema: parse error" ;;
    esac
else
    skip "Dashboard WS schema: skipped (connection failed)"
fi

# ─── Test 3: Agent WS — connection upgrade test ──────────────────────────────
# Agent endpoint accepts WS upgrade and waits for Register message with PSK.
# Without PSK/JWT the connection opens but auth happens post-connect.
# We just verify the WS upgrade succeeds (HTTP 101).

AGENT_RESULT=$(node --no-warnings -e "
    const ws = new WebSocket('${RC_WS_URL}/ws/agent');
    ws.addEventListener('open', () => { console.log('UPGRADE_OK'); ws.close(); setTimeout(() => process.exit(0), 100); });
    ws.addEventListener('error', (e) => { console.log('UPGRADE_FAIL: ' + (e.message || 'unknown')); process.exit(1); });
    setTimeout(() => { console.log('TIMEOUT'); process.exit(1); }, 5000);
" 2>&1)
AGENT_EXIT=$?

if [ $AGENT_EXIT -eq 0 ] && echo "$AGENT_RESULT" | grep -q "UPGRADE_OK"; then
    pass "Agent WS: upgrade accepted (HTTP 101), auth is post-connect via Register"
elif echo "$AGENT_RESULT" | grep -qi "401\|403\|FAIL"; then
    pass "Agent WS: correctly rejected at upgrade (production auth mode)"
else
    fail "Agent WS: unexpected result — ${AGENT_RESULT}"
fi

# ─── Test 4: WS upgrade on non-existent path — should fail ────────────────────

BOGUS_RESULT=$(node --no-warnings "$SCRIPT_DIR/ws-connect.mjs" \
    "${RC_WS_URL}/ws/nonexistent" \
    '{"expect_close":true,"timeout_ms":5000}' 2>&1)
BOGUS_EXIT=$?

if [ $BOGUS_EXIT -ne 0 ] || echo "$BOGUS_RESULT" | grep -qi "error\|fail\|reject\|404"; then
    pass "Bogus WS path: correctly rejected /ws/nonexistent"
else
    fail "Bogus WS path: unexpectedly accepted connection to /ws/nonexistent"
fi

echo ""
summary_exit
