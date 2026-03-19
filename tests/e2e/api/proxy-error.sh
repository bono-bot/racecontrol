#!/bin/bash
# =============================================================================
# Proxy Error Page E2E Test
#
# Validates that the racecontrol reverse proxy returns a branded HTML error
# page (not plain text) when the kiosk or dashboard backend is unreachable.
#
# How it works:
#   Hits the proxy paths (/kiosk, /billing) on the racecontrol server.
#   If the backend is down, the proxy MUST return:
#     - HTTP 502
#     - Content-Type: text/html
#     - Body containing Racing Point branding (#E10600, RACING)
#     - Body containing auto-retry JS (location.reload)
#   If the backend is up (200), the test passes with a skip note.
#
# Usage:
#   bash tests/e2e/api/proxy-error.sh
#
# Exit code = number of failures (0 = all pass)
# =============================================================================

set -uo pipefail

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=../lib/common.sh
source "$SCRIPT_DIR/../lib/common.sh"

# Use the server root (no /api/v1 suffix) since proxy routes are at the root
RC_SERVER="${RC_BASE_URL:-http://192.168.31.23:8080/api/v1}"
# Strip /api/v1 suffix to get the server root
SERVER_ROOT=$(echo "$RC_SERVER" | sed 's|/api/v1$||')

echo "========================================"
echo "Proxy Error Page E2E Test"
echo "Server: ${SERVER_ROOT}"
echo "========================================"
echo ""

# ─── Helper: test proxy error page ──────────────────────────────────────────
# Tests that a given proxy path returns a branded error page when backend is down.
# If backend is up (200), skips with a note.
check_proxy_error() {
    local path="$1"
    local label="$2"

    RESP=$(curl -s -w "\n%{http_code}" --max-time 10 "${SERVER_ROOT}${path}" 2>/dev/null || echo -e "\n000")
    CODE=$(echo "$RESP" | tail -1)
    BODY=$(echo "$RESP" | sed '$d')

    if [ "$CODE" = "200" ] || [ "$CODE" = "304" ]; then
        skip "${label}: backend is running (${CODE}) — cannot test error page"
        return
    fi

    if [ "$CODE" = "000" ]; then
        fail "${label}: racecontrol unreachable at ${SERVER_ROOT}${path}"
        return
    fi

    # Gate 1: Must be 502 Bad Gateway
    if [ "$CODE" = "502" ]; then
        pass "${label}: returns 502 when backend is down"
    else
        fail "${label}: expected 502, got ${CODE}"
    fi

    # Gate 2: Must be HTML, not plain text
    if echo "$BODY" | grep -qi "<!DOCTYPE html>"; then
        pass "${label}: response is HTML (not plain text)"
    else
        fail "${label}: response is plain text (missing <!DOCTYPE html>)"
    fi

    # Gate 3: Must contain Racing Point branding
    if echo "$BODY" | grep -q "#E10600"; then
        pass "${label}: contains Racing Point red (#E10600)"
    else
        fail "${label}: missing Racing Point branding (#E10600)"
    fi

    if echo "$BODY" | grep -q "RACING"; then
        pass "${label}: contains RACING wordmark"
    else
        fail "${label}: missing RACING wordmark"
    fi

    # Gate 4: Must contain auto-retry JS
    if echo "$BODY" | grep -q "location.reload"; then
        pass "${label}: contains auto-retry (location.reload)"
    else
        fail "${label}: missing auto-retry JS (location.reload)"
    fi

    # Gate 5: Must show contextual service name
    if echo "$BODY" | grep -qi "STARTING UP"; then
        pass "${label}: shows 'STARTING UP' message"
    else
        fail "${label}: missing 'STARTING UP' message"
    fi
}

# ─── Preflight: Is racecontrol itself reachable? ────────────────────────────
echo "--- Preflight ---"
HEALTH=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 "${SERVER_ROOT}/api/v1/health" 2>/dev/null || echo "000")
if [ "$HEALTH" != "200" ]; then
    fail "racecontrol not reachable at ${SERVER_ROOT}/api/v1/health (got ${HEALTH})"
    summary_exit
fi
pass "racecontrol is reachable"
echo ""

# ─── Test kiosk proxy error page ────────────────────────────────────────────
echo "--- Kiosk Proxy (/kiosk) ---"
check_proxy_error "/kiosk" "Kiosk proxy error page"
echo ""

# ─── Test dashboard proxy error page ────────────────────────────────────────
echo "--- Dashboard Proxy (/billing) ---"
check_proxy_error "/billing" "Dashboard proxy error page"
echo ""

# ─── Summary ────────────────────────────────────────────────────────────────
summary_exit
