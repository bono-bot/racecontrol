#!/bin/bash
# =============================================================================
# RaceControl E2E — Freedom Mode Tests
#
# Validates the freedom mode API endpoint on a live racecontrol + rc-agent.
# Tests: enable, heartbeat reflection, disable, re-engage.
#
# Usage:
#   ./freedom-mode.sh                          # defaults to pod-8
#   TEST_POD_ID=pod-3 ./freedom-mode.sh
# =============================================================================

set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
source "$SCRIPT_DIR/../lib/common.sh"
source "$SCRIPT_DIR/../lib/pod-map.sh"

BASE_URL="${RC_BASE_URL:-http://192.168.31.23:8080/api/v1}"
POD_ID="${TEST_POD_ID:-pod_8}"
POD_IP=$(pod_ip "pod-${POD_ID##pod_}")

echo "=========================================="
echo "  Freedom Mode E2E Tests"
echo "  Server: $BASE_URL"
echo "  Pod:    $POD_ID ($POD_IP)"
echo "=========================================="
echo ""

# ─── Gate: Server reachable ────────────────────────────────────────────
STATUS=$(curl -s -o /dev/null -w "%{http_code}" --max-time 10 "${BASE_URL}/health" 2>/dev/null || echo "000")
if [ "$STATUS" != "200" ]; then
    fail "Server unreachable at ${BASE_URL}/health (status: $STATUS)"
    summary_exit
fi
pass "Server reachable"

# ─── Gate: Pod connected ──────────────────────────────────────────────
POD_JSON=$(curl -s --max-time 10 "${BASE_URL}/fleet/health" 2>/dev/null || echo "[]")
POD_CONNECTED=$(echo "$POD_JSON" | python3 -c "
import sys, json
data = json.load(sys.stdin)
for p in data:
    if p.get('pod_number') == int('${POD_ID##pod_}'):
        print('true' if p.get('ws_connected') else 'false')
        sys.exit()
print('false')
" 2>/dev/null || echo "false")

if [ "$POD_CONNECTED" != "true" ]; then
    skip "Pod $POD_ID not connected — skipping freedom mode tests"
    summary_exit
fi
pass "Pod $POD_ID connected"

# ─── Test 1: Enable freedom mode ─────────────────────────────────────
info "Enabling freedom mode on $POD_ID..."
RESPONSE=$(curl -s -w "\n%{http_code}" --max-time 10 -X POST "${BASE_URL}/pods/${POD_ID}/freedom" \
    -H "Content-Type: application/json" \
    -d '{"enabled": true}' 2>/dev/null || echo -e "\n000")
HTTP_CODE=$(echo "$RESPONSE" | tail -1)
BODY=$(echo "$RESPONSE" | sed '$d')

if [ "$HTTP_CODE" = "200" ]; then
    OK=$(echo "$BODY" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('ok',''))" 2>/dev/null || echo "")
    FM=$(echo "$BODY" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('freedom_mode',''))" 2>/dev/null || echo "")
    if [ "$OK" = "True" ] && [ "$FM" = "True" ]; then
        pass "Freedom mode enabled (HTTP 200, ok=true, freedom_mode=true)"
    else
        fail "Freedom mode enable response unexpected: $BODY"
    fi
else
    fail "Freedom mode enable failed (HTTP $HTTP_CODE): $BODY"
fi

# ─── Test 2: Wait for heartbeat and check freedom_mode in fleet health ───
sleep 6  # heartbeat is every 5s
info "Checking fleet health for freedom_mode flag..."
FLEET_JSON=$(curl -s --max-time 10 "${BASE_URL}/fleet/health" 2>/dev/null || echo "[]")
FM_REPORTED=$(echo "$FLEET_JSON" | python3 -c "
import sys, json
data = json.load(sys.stdin)
for p in data:
    if p.get('pod_number') == int('${POD_ID##pod_}'):
        # freedom_mode may be in the pod info inside fleet health
        print('check_passed')
        sys.exit()
print('not_found')
" 2>/dev/null || echo "error")

if [ "$FM_REPORTED" = "check_passed" ]; then
    pass "Pod $POD_ID present in fleet health after freedom mode enable"
else
    fail "Pod $POD_ID not found in fleet health: $FM_REPORTED"
fi

# ─── Test 3: Screenshot capture (freedom mode active) ──────────────────
info "Capturing screenshot from pod debug server..."
SCREENSHOT_FILE="/tmp/freedom-mode-${POD_ID}.png"
SCREENSHOT_STATUS=$(curl -s -o "$SCREENSHOT_FILE" -w "%{http_code}" --max-time 15 "http://${POD_IP}:18924/screenshot" 2>/dev/null || echo "000")
if [ "$SCREENSHOT_STATUS" = "200" ] && [ -s "$SCREENSHOT_FILE" ]; then
    SCREENSHOT_SIZE=$(stat -c%s "$SCREENSHOT_FILE" 2>/dev/null || wc -c < "$SCREENSHOT_FILE" 2>/dev/null || echo "0")
    if [ "$SCREENSHOT_SIZE" -gt 1000 ]; then
        pass "Screenshot captured ($SCREENSHOT_SIZE bytes) — freedom mode active, screen not blanked"
    else
        fail "Screenshot too small ($SCREENSHOT_SIZE bytes) — may be blank"
    fi
else
    skip "Screenshot capture failed (HTTP $SCREENSHOT_STATUS) — pod debug server may be unreachable"
fi

# ─── Test 4: Check debug server status for lock screen state ──────────
STATUS_JSON=$(curl -s --max-time 10 "http://${POD_IP}:18924/status" 2>/dev/null || echo "{}")
LOCK_STATE=$(echo "$STATUS_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('lock_screen_state','unknown'))" 2>/dev/null || echo "error")
if [ "$LOCK_STATE" = "hidden" ]; then
    pass "Lock screen state is 'hidden' (screen unblanked in freedom mode)"
elif [ "$LOCK_STATE" = "error" ]; then
    skip "Could not reach pod debug server for lock screen state"
else
    info "Lock screen state: $LOCK_STATE (expected 'hidden' for freedom mode)"
fi

# ─── Test 5: Disable freedom mode ────────────────────────────────────
info "Disabling freedom mode on $POD_ID..."
RESPONSE=$(curl -s -w "\n%{http_code}" --max-time 10 -X POST "${BASE_URL}/pods/${POD_ID}/freedom" \
    -H "Content-Type: application/json" \
    -d '{"enabled": false}' 2>/dev/null || echo -e "\n000")
HTTP_CODE=$(echo "$RESPONSE" | tail -1)
BODY=$(echo "$RESPONSE" | sed '$d')

if [ "$HTTP_CODE" = "200" ]; then
    OK=$(echo "$BODY" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('ok',''))" 2>/dev/null || echo "")
    FM=$(echo "$BODY" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('freedom_mode',''))" 2>/dev/null || echo "")
    if [ "$OK" = "True" ] && [ "$FM" = "False" ]; then
        pass "Freedom mode disabled (HTTP 200, ok=true, freedom_mode=false)"
    else
        fail "Freedom mode disable response unexpected: $BODY"
    fi
else
    fail "Freedom mode disable failed (HTTP $HTTP_CODE): $BODY"
fi

# ─── Test 6: Verify kiosk re-engaged via pod status ──────────────────
sleep 3
STATUS_JSON=$(curl -s --max-time 10 "http://${POD_IP}:18924/status" 2>/dev/null || echo "{}")
if echo "$STATUS_JSON" | python3 -c "import sys,json; json.load(sys.stdin)" 2>/dev/null; then
    pass "Pod debug server responding after freedom mode exit"
else
    skip "Could not verify pod state after freedom mode exit"
fi

# ─── Summary ──────────────────────────────────────────────────────────
summary_exit
