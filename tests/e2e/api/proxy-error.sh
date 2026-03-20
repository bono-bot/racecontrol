#!/bin/bash
# =============================================================================
# Proxy Error Page + Retry E2E Test
#
# Validates two behaviors of the racecontrol reverse proxy:
#
# 1. RETRY LOGIC: When backend is down, the proxy retries 3 times with 1s
#    backoff before giving up. This absorbs the typical 3-5s Node.js startup
#    window so customers never see the error page during normal boot.
#
# 2. FALLBACK PAGE: After retries are exhausted, returns a branded HTML error
#    page (not plain text) with:
#      - HTTP 502
#      - Content-Type: text/html
#      - Racing Point branding (#E10600, RACING)
#      - Auto-retry JS (location.reload every 5s)
#      - Contextual service name ("Kiosk STARTING UP" / "Dashboard STARTING UP")
#
# If the backend is up (200), most gates skip gracefully.
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
echo "Proxy Error Page + Retry E2E Test"
echo "Server: ${SERVER_ROOT}"
echo "========================================"
echo ""

# ─── Helper: test proxy error page ──────────────────────────────────────────
# Tests that a given proxy path returns a branded error page when backend is down.
# If backend is up (200), skips with a note.
check_proxy_error() {
    local path="$1"
    local label="$2"

    # Measure response time to verify retry backoff is happening
    local START_TIME
    START_TIME=$(date +%s)

    RESP=$(curl -s -w "\n%{http_code}" --max-time 30 "${SERVER_ROOT}${path}" 2>/dev/null || echo -e "\n000")
    CODE=$(echo "$RESP" | tail -1)
    BODY=$(echo "$RESP" | sed '$d')

    local END_TIME
    END_TIME=$(date +%s)
    local ELAPSED=$((END_TIME - START_TIME))

    if [ "$CODE" = "200" ] || [ "$CODE" = "304" ]; then
        pass "${label}: backend is running (${CODE}) — proxy forwarded successfully"

        # Gate R1: When backend is up, verify proxy returns actual content (not error page)
        if echo "$BODY" | grep -q "STARTING UP"; then
            fail "${label}: backend returned 200 but body contains error page"
        else
            pass "${label}: response is real content (not error page)"
        fi
        return
    fi

    if [ "$CODE" = "000" ]; then
        fail "${label}: racecontrol unreachable at ${SERVER_ROOT}${path}"
        return
    fi

    # ─── Retry behavior gates ───────────────────────────────────────────────
    # Gate R2: Proxy should take >=2s when backend is down (3 attempts x 1s backoff)
    info "${label}: response took ${ELAPSED}s (expected >=2s from retry backoff)"
    if [ "$ELAPSED" -ge 2 ]; then
        pass "${label}: retry backoff observed (${ELAPSED}s >= 2s)"
    else
        fail "${label}: response too fast (${ELAPSED}s) — retry logic may not be working"
    fi

    # ─── Fallback page gates ────────────────────────────────────────────────
    # Gate F1: Must be 502 Bad Gateway
    if [ "$CODE" = "502" ]; then
        pass "${label}: returns 502 after retries exhausted"
    else
        fail "${label}: expected 502, got ${CODE}"
    fi

    # Gate F2: Must be HTML, not plain text
    if echo "$BODY" | grep -qi "<!DOCTYPE html>"; then
        pass "${label}: response is HTML (not plain text)"
    else
        fail "${label}: response is plain text (missing <!DOCTYPE html>)"
    fi

    # Gate F3: Must contain Racing Point branding
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

    # Gate F4: Must contain auto-retry JS
    if echo "$BODY" | grep -q "location.reload"; then
        pass "${label}: contains auto-retry (location.reload)"
    else
        fail "${label}: missing auto-retry JS (location.reload)"
    fi

    # Gate F5: Must show contextual service name
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

# ─── Test kiosk proxy ──────────────────────────────────────────────────────
echo "--- Kiosk Proxy (/kiosk) ---"
check_proxy_error "/kiosk" "Kiosk proxy"
echo ""

# ─── Test dashboard proxy ──────────────────────────────────────────────────
echo "--- Dashboard Proxy (/billing) ---"
check_proxy_error "/billing" "Dashboard proxy"
echo ""

# ─── Summary ────────────────────────────────────────────────────────────────
summary_exit
