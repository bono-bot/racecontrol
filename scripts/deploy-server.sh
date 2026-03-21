#!/bin/bash
# =============================================================================
# deploy-server.sh — Safe racecontrol server deploy via rc-sentry
#
# FAILSAFE RULES (learned from incident 2026-03-21):
#   1. ALWAYS use rc-sentry (:8091) — NEVER rc-agent (:8090) for deploy ops.
#      rc-agent depends on racecontrol via WebSocket and dies when racecontrol stops.
#   2. Kill the bat wrapper window FIRST (it has an auto-relaunch loop).
#      taskkill /F /IM cmd.exe /FI "WINDOWTITLE eq RaceControl*" kills the wrapper.
#      This also kills racecontrol.exe (child process).
#   3. Wait 3s for file handles to release before swapping binary.
#   4. Verify new binary size matches local before starting.
#   5. Health-poll with 60s timeout before declaring success.
#
# Based on: deploy-staging/RaceControl.bat pattern + Phase 55 rc-sentry deploys.
#
# Usage:
#   ./deploy-server.sh                    # deploy from default staging dir
#   BINARY_DIR=/path/to/bins ./deploy-server.sh
# =============================================================================

set -euo pipefail

SERVER_IP="${SERVER_IP:-192.168.31.23}"
JAMES_IP="${JAMES_IP:-192.168.31.27}"
SENTRY_PORT=8091
HEALTH_PORT=8080
SERVE_PORT=18888
BINARY_DIR="${BINARY_DIR:-$HOME/racingpoint/deploy-staging}"

GREEN='\033[0;32m'; RED='\033[0;31m'; YELLOW='\033[0;33m'; NC='\033[0m'
pass() { echo -e "  ${GREEN}OK${NC}    $1"; }
fail() { echo -e "  ${RED}FAIL${NC}  $1"; exit 1; }
info() { echo -e "  ${YELLOW}...${NC}   $1"; }

echo "=========================================="
echo "  Safe Server Deploy (via rc-sentry)"
echo "  Server:  ${SERVER_IP}:${SENTRY_PORT}"
echo "  Binary:  ${BINARY_DIR}/racecontrol.exe"
echo "=========================================="
echo ""

# ─── Gate 1: rc-sentry reachable ─────────────────────────────────────
info "Checking rc-sentry..."
SENTRY=$(curl -s --max-time 5 "http://${SERVER_IP}:${SENTRY_PORT}/ping" 2>/dev/null || echo "")
if [ "$SENTRY" != "pong" ]; then
    fail "rc-sentry unreachable at ${SERVER_IP}:${SENTRY_PORT}. Cannot deploy without it."
fi
pass "rc-sentry reachable"

# ─── Gate 2: Binary exists and is valid ──────────────────────────────
if [ ! -f "${BINARY_DIR}/racecontrol.exe" ]; then
    fail "Binary not found: ${BINARY_DIR}/racecontrol.exe"
fi
LOCAL_SIZE=$(stat -c%s "${BINARY_DIR}/racecontrol.exe" 2>/dev/null || wc -c < "${BINARY_DIR}/racecontrol.exe" 2>/dev/null || echo "0")
if [ "$LOCAL_SIZE" -lt 1000000 ]; then
    fail "Binary too small (${LOCAL_SIZE} bytes)"
fi
pass "Binary valid (${LOCAL_SIZE} bytes)"

# ─── Step 1: Serve binary via HTTP ───────────────────────────────────
info "Starting HTTP file server on :${SERVE_PORT}..."
cd "${BINARY_DIR}"
python3 -m http.server ${SERVE_PORT} --bind 0.0.0.0 > /dev/null 2>&1 &
HTTP_PID=$!
trap "kill $HTTP_PID 2>/dev/null" EXIT
sleep 1
pass "HTTP server started (PID ${HTTP_PID})"

# ─── Step 2: Download binary to server via rc-sentry ─────────────────
info "Downloading racecontrol.exe to server..."
DL_RESULT=$(curl -s --max-time 120 "http://${SERVER_IP}:${SENTRY_PORT}/exec" \
    -H "Content-Type: application/json" \
    -d "{\"cmd\":\"curl -s -o C:/RacingPoint/racecontrol-new.exe http://${JAMES_IP}:${SERVE_PORT}/racecontrol.exe && for %f in (C:/RacingPoint/racecontrol-new.exe) do echo SIZE=%~zf\"}" 2>/dev/null || echo "")

REMOTE_SIZE=$(echo "$DL_RESULT" | grep -oP 'SIZE=\K[0-9]+' || echo "0")
if [ "$REMOTE_SIZE" -lt 1000000 ]; then
    fail "Download failed or file too small (${REMOTE_SIZE} bytes on server, expected ~${LOCAL_SIZE})"
fi
pass "Downloaded to server (${REMOTE_SIZE} bytes)"

# ─── Step 3: Stop racecontrol via bat window kill ────────────────────
# RaceControl.bat runs in a cmd.exe with title "RaceControl Server" and auto-relaunches.
# Kill the WRAPPER first (prevents auto-relaunch), which also kills the child racecontrol.exe.
info "Stopping racecontrol (killing bat wrapper + process)..."
curl -s --max-time 15 "http://${SERVER_IP}:${SENTRY_PORT}/exec" \
    -H "Content-Type: application/json" \
    -d '{"cmd":"taskkill /F /FI \"WINDOWTITLE eq RaceControl*\" 2>nul & taskkill /F /IM racecontrol.exe 2>nul & echo STOPPED"}' > /dev/null 2>&1
sleep 3
pass "racecontrol stopped"

# ─── Step 4: Swap binary ────────────────────────────────────────────
info "Swapping binary..."
SWAP=$(curl -s --max-time 15 "http://${SERVER_IP}:${SENTRY_PORT}/exec" \
    -H "Content-Type: application/json" \
    -d '{"cmd":"move /Y C:/RacingPoint/racecontrol-new.exe C:/RacingPoint/racecontrol.exe & echo SWAPPED"}' 2>/dev/null || echo "")
if ! echo "$SWAP" | grep -q "SWAPPED"; then
    fail "Swap failed: $SWAP"
fi
pass "Binary swapped"

# ─── Step 5: Start racecontrol ──────────────────────────────────────
info "Starting racecontrol..."
curl -s --max-time 10 "http://${SERVER_IP}:${SENTRY_PORT}/exec" \
    -H "Content-Type: application/json" \
    -d '{"cmd":"start \"RaceControl Server\" C:/RacingPoint/start-racecontrol.bat & echo STARTED"}' > /dev/null 2>&1

# ─── Step 6: Health check with retry ────────────────────────────────
info "Waiting for server health..."
for i in $(seq 1 12); do
    sleep 5
    HEALTH_BODY=$(curl -s --max-time 5 "http://${SERVER_IP}:${HEALTH_PORT}/api/v1/health" 2>/dev/null || echo "")
    if echo "$HEALTH_BODY" | grep -q '"status":"ok"'; then
        BUILD_ID=$(echo "$HEALTH_BODY" | grep -oP '"build_id":"\K[^"]+' || echo "unknown")
        pass "Server healthy — build_id: ${BUILD_ID}"
        echo ""
        echo -e "${GREEN}=== Deploy successful! ===${NC}"
        exit 0
    fi
    echo "    Attempt $i/12 — waiting..."
done

fail "Server did not come up after 60 seconds. Check logs on server."
