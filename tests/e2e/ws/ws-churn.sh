#!/bin/bash
# tests/e2e/ws/ws-churn.sh — WebSocket Churn Detection Test
#
# Queries the fleet health endpoint for dashboard_ws_churn metrics.
# High churn (>10 connects/disconnects per minute) indicates a frontend
# in a connect/disconnect loop — usually from a stale build.
#
# Tests:
#   1. dashboard_ws_churn.healthy == true
#   2. connects_per_min < 10
#   3. disconnects_per_min < 10
#   4. dashboard_clients count is reasonable (0-20)
#
# Usage:
#   bash tests/e2e/ws/ws-churn.sh

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
source "$SCRIPT_DIR/../lib/common.sh"

RC_BASE_URL="${RC_BASE_URL:-http://192.168.31.23:8080/api/v1}"
CHURN_THRESHOLD=10

info "WebSocket Churn Detection"
info "Base URL: ${RC_BASE_URL}"
info "Churn threshold: ${CHURN_THRESHOLD} connects/min"
echo ""

# Fetch fleet health
FLEET_RESP=$(curl -s --max-time 10 "${RC_BASE_URL}/fleet/health" 2>/dev/null)

if [ -z "$FLEET_RESP" ]; then
    fail "Fleet health endpoint unreachable"
    summary_exit
fi

# Parse churn metrics
CHURN_DATA=$(echo "$FLEET_RESP" | node --no-warnings -e "
    const data = JSON.parse(require('fs').readFileSync(0, 'utf8'));
    const churn = data.dashboard_ws_churn || {};
    console.log(JSON.stringify({
        connects: churn.connects_per_min || 0,
        disconnects: churn.disconnects_per_min || 0,
        healthy: churn.healthy !== false,
        clients: data.dashboard_clients || 0,
    }));
" 2>/dev/null)

if [ -z "$CHURN_DATA" ]; then
    fail "Failed to parse dashboard_ws_churn from fleet health"
    summary_exit
fi

CONNECTS=$(echo "$CHURN_DATA" | node --no-warnings -e "console.log(JSON.parse(require('fs').readFileSync(0,'utf8')).connects)")
DISCONNECTS=$(echo "$CHURN_DATA" | node --no-warnings -e "console.log(JSON.parse(require('fs').readFileSync(0,'utf8')).disconnects)")
HEALTHY=$(echo "$CHURN_DATA" | node --no-warnings -e "console.log(JSON.parse(require('fs').readFileSync(0,'utf8')).healthy)")
CLIENTS=$(echo "$CHURN_DATA" | node --no-warnings -e "console.log(JSON.parse(require('fs').readFileSync(0,'utf8')).clients)")

# ─── Test 1: Churn healthy flag ───────────────────────────────────────────────

if [ "$HEALTHY" = "true" ]; then
    pass "Dashboard WS churn: healthy=true"
else
    fail "Dashboard WS churn: healthy=false (stale frontend build suspected)"
fi

# ─── Test 2: Connect rate ─────────────────────────────────────────────────────

if [ "$CONNECTS" -lt "$CHURN_THRESHOLD" ] 2>/dev/null; then
    pass "Dashboard WS connects: ${CONNECTS}/min (threshold: ${CHURN_THRESHOLD})"
else
    fail "Dashboard WS connects: ${CONNECTS}/min EXCEEDS threshold ${CHURN_THRESHOLD} — frontend in reconnect loop"
fi

# ─── Test 3: Disconnect rate ──────────────────────────────────────────────────

if [ "$DISCONNECTS" -lt "$CHURN_THRESHOLD" ] 2>/dev/null; then
    pass "Dashboard WS disconnects: ${DISCONNECTS}/min (threshold: ${CHURN_THRESHOLD})"
else
    fail "Dashboard WS disconnects: ${DISCONNECTS}/min EXCEEDS threshold ${CHURN_THRESHOLD} — frontend in disconnect loop"
fi

# ─── Test 4: Client count sanity ──────────────────────────────────────────────

if [ "$CLIENTS" -ge 0 ] && [ "$CLIENTS" -le 20 ] 2>/dev/null; then
    pass "Dashboard WS clients: ${CLIENTS} (sane range 0-20)"
else
    fail "Dashboard WS clients: ${CLIENTS} (outside expected range 0-20)"
fi

echo ""
summary_exit
