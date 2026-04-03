#!/bin/bash
# tests/e2e/ws/frontend-staleness.sh — Frontend Build Staleness Check
#
# Compares the server's Rust build timestamp against frontend app build dates.
# A stale frontend that can't parse new WS message formats enters a
# connect/disconnect loop (800+ events/min) that's invisible to health checks.
#
# Tests:
#   1. Server build_id matches git HEAD (or at least exists)
#   2. Each frontend app (kiosk, web, admin) has a .next directory
#   3. Frontend .next/BUILD_ID files are not more than 48h older than server deploy
#   4. Cross-check: if dashboard_ws_churn is unhealthy, flag staleness as root cause
#
# Usage:
#   bash tests/e2e/ws/frontend-staleness.sh

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
source "$SCRIPT_DIR/../lib/common.sh"

RC_BASE_URL="${RC_BASE_URL:-http://192.168.31.23:8080/api/v1}"
SERVER_IP=$(echo "$RC_BASE_URL" | sed 's|https\?://||; s|:.*||')
STALENESS_THRESHOLD_HOURS=48

info "Frontend Build Staleness Check"
info "Server: ${SERVER_IP}"
info "Staleness threshold: ${STALENESS_THRESHOLD_HOURS}h"
echo ""

# ─── Test 1: Server build_id exists and is valid ──────────────────────────────

HEALTH_RESP=$(curl -s --max-time 10 "${RC_BASE_URL}/health" 2>/dev/null)

if [ -z "$HEALTH_RESP" ]; then
    fail "Server health unreachable — cannot determine build freshness"
    summary_exit
fi

BUILD_ID=$(echo "$HEALTH_RESP" | node --no-warnings -e "
    try { const d = JSON.parse(require('fs').readFileSync(0,'utf8')); console.log(d.build_id || 'none'); }
    catch { console.log('parse_error'); }
" 2>/dev/null)

if [ -n "$BUILD_ID" ] && [ "$BUILD_ID" != "none" ] && [ "$BUILD_ID" != "parse_error" ]; then
    pass "Server build_id: ${BUILD_ID}"
else
    fail "Server build_id: missing or unparseable"
fi

# ─── Test 2: Check each frontend app staleness via SSH ────────────────────────
# This checks local filesystem paths since tests run on James (.27) or server (.23)

FRONTEND_APPS=("kiosk" "web" "apps/admin")
STALE_COUNT=0

for APP in "${FRONTEND_APPS[@]}"; do
    APP_NAME=$(basename "$APP")
    NEXT_DIR="/c/Users/bono/racingpoint/racecontrol/${APP}/.next"

    if [ ! -d "$NEXT_DIR" ]; then
        # Try server path
        NEXT_DIR="/c/RacingPoint/racecontrol-apps/${APP_NAME}/.next"
    fi

    if [ -d "$NEXT_DIR" ]; then
        # Check BUILD_ID file age
        BUILD_FILE="${NEXT_DIR}/BUILD_ID"
        if [ -f "$BUILD_FILE" ]; then
            FILE_AGE_SECS=$(( $(date +%s) - $(date -r "$BUILD_FILE" +%s 2>/dev/null || echo 0) ))
            FILE_AGE_HOURS=$((FILE_AGE_SECS / 3600))

            if [ "$FILE_AGE_HOURS" -lt "$STALENESS_THRESHOLD_HOURS" ]; then
                pass "${APP_NAME} frontend: built ${FILE_AGE_HOURS}h ago (threshold: ${STALENESS_THRESHOLD_HOURS}h)"
            else
                fail "${APP_NAME} frontend: built ${FILE_AGE_HOURS}h ago — STALE (threshold: ${STALENESS_THRESHOLD_HOURS}h)"
                STALE_COUNT=$((STALE_COUNT + 1))
            fi
        else
            skip "${APP_NAME} frontend: no BUILD_ID file (may use different build system)"
        fi
    else
        skip "${APP_NAME} frontend: .next directory not found locally"
    fi
done

# ─── Test 3: Cross-check with WS churn ───────────────────────────────────────

FLEET_RESP=$(curl -s --max-time 10 "${RC_BASE_URL}/fleet/health" 2>/dev/null)

if [ -n "$FLEET_RESP" ]; then
    CHURN_HEALTHY=$(echo "$FLEET_RESP" | node --no-warnings -e "
        try {
            const d = JSON.parse(require('fs').readFileSync(0,'utf8'));
            const churn = d.dashboard_ws_churn || {};
            console.log(churn.healthy !== false ? 'true' : 'false');
        } catch { console.log('unknown'); }
    " 2>/dev/null)

    if [ "$CHURN_HEALTHY" = "false" ] && [ "$STALE_COUNT" -gt 0 ]; then
        fail "CORRELATION: WS churn unhealthy + ${STALE_COUNT} stale frontend(s) — rebuild frontends immediately"
    elif [ "$CHURN_HEALTHY" = "false" ]; then
        fail "WS churn unhealthy but frontends appear fresh — investigate WS message format changes"
    elif [ "$STALE_COUNT" -gt 0 ]; then
        fail "${STALE_COUNT} stale frontend(s) detected — WS churn currently OK but at risk"
    else
        pass "Cross-check: WS churn healthy + frontends fresh"
    fi
else
    skip "Cross-check: fleet health unreachable"
fi

echo ""
summary_exit
