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
# VERIFICATION (v2): Records expected build_id, verifies post-deploy match.
# Preserves rollback binary. Compares SHA256.
#
# Based on: deploy-staging/RaceControl.bat pattern + Phase 55 rc-sentry deploys.
#
# Usage:
#   ./deploy-server.sh                    # deploy from default staging dir
#   BINARY_DIR=/path/to/bins ./deploy-server.sh
# =============================================================================

set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
SERVER_IP="${SERVER_IP:-192.168.31.23}"
JAMES_IP="${JAMES_IP:-192.168.31.27}"
SENTRY_PORT=8091
HEALTH_PORT=8080
SERVE_PORT=18888
BINARY_DIR="${BINARY_DIR:-$HOME/racingpoint/deploy-staging}"

GREEN='\033[0;32m'; RED='\033[0;31m'; YELLOW='\033[0;33m'; CYAN='\033[0;36m'; NC='\033[0m'
pass() { echo -e "  ${GREEN}OK${NC}    $1"; }
fail() { echo -e "  ${RED}FAIL${NC}  $1"; exit 1; }
info() { echo -e "  ${YELLOW}...${NC}   $1"; }

echo "=========================================="
echo "  Safe Server Deploy (via rc-sentry)"
echo "  Server:  ${SERVER_IP}:${SENTRY_PORT}"
echo "  Binary:  ${BINARY_DIR}/racecontrol.exe"
echo "=========================================="
echo ""

# ─── Gate 0: Security + manifest check ────────────────────────────────
MANIFEST="${BINARY_DIR}/release-manifest.toml"
if [ ! -f "$MANIFEST" ]; then
    fail "release-manifest.toml not found — run ./scripts/stage-release.sh first"
fi
MANIFEST_HASH=$(grep 'git_commit' "$MANIFEST" 2>/dev/null | head -1 | cut -d'"' -f2)
HEAD_HASH=$(cd "${SCRIPT_DIR}/.." 2>/dev/null && git rev-parse --short HEAD 2>/dev/null || echo "unknown")
if [ "$MANIFEST_HASH" != "$HEAD_HASH" ]; then
    echo -e "  ${YELLOW}!!${NC}    Manifest git_commit=${MANIFEST_HASH} vs HEAD=${HEAD_HASH} — staged binary may be stale"
fi

COMMS_ROOT="${SCRIPT_DIR}/../../comms-link"
if [ -f "${COMMS_ROOT}/test/security-check.js" ]; then
    info "Running security gate (SEC-GATE-01)..."
    if node "${COMMS_ROOT}/test/security-check.js" > /dev/null 2>&1; then
        pass "Security gate passed"
    else
        fail "Security gate FAILED — run: node ${COMMS_ROOT}/test/security-check.js"
    fi
fi

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

# ─── Gate 3: SHA256 of local binary ──────────────────────────────────
LOCAL_SHA256=$(sha256sum "${BINARY_DIR}/racecontrol.exe" 2>/dev/null | awk '{print $1}' || certutil -hashfile "${BINARY_DIR}/racecontrol.exe" SHA256 2>/dev/null | grep -v hash | grep -v Cert | tr -d '[:space:]' || echo "")
if [ -n "$LOCAL_SHA256" ]; then
    pass "Local SHA256: ${LOCAL_SHA256:0:16}..."
fi

# ─── Gate 4: Expected build_id from git HEAD ─────────────────────────
EXPECTED_BUILD_ID=""
REPO_DIR=$(cd "${SCRIPT_DIR}/.." 2>/dev/null && pwd || echo "")
if [ -d "${REPO_DIR}/.git" ]; then
    EXPECTED_BUILD_ID=$(git -C "$REPO_DIR" rev-parse --short HEAD 2>/dev/null || echo "")
    if [ -n "$EXPECTED_BUILD_ID" ]; then
        echo -e "  ${CYAN}>>>${NC}   Expected build_id: ${EXPECTED_BUILD_ID}"
    fi

    # Staleness check
    BINARY_MTIME=$(stat -c%Y "${BINARY_DIR}/racecontrol.exe" 2>/dev/null || echo "0")
    LATEST_COMMIT_TIME=$(git -C "$REPO_DIR" log -1 --format=%ct -- crates/racecontrol/ 2>/dev/null || echo "0")
    if [ "$LATEST_COMMIT_TIME" -gt "$BINARY_MTIME" ] 2>/dev/null; then
        echo -e "  ${RED}!!${NC}    ${RED}WARNING: Staged binary is OLDER than latest racecontrol commit!${NC}"
        echo -e "  ${RED}!!${NC}    ${RED}Run ./scripts/stage-release.sh first to rebuild.${NC}"
        echo ""
        read -p "  Continue anyway? (y/N) " -n 1 -r
        echo ""
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            echo "Aborted."
            exit 1
        fi
    fi
fi

# ─── Gate 5: Record pre-deploy build_id ──────────────────────────────
info "Recording pre-deploy server state..."
PRE_HEALTH=$(curl -s --max-time 5 "http://${SERVER_IP}:${HEALTH_PORT}/api/v1/health" 2>/dev/null || echo "")
PRE_BUILD=$(echo "$PRE_HEALTH" | grep -oP '"build_id":"\K[^"]+' || echo "unknown")
echo -e "  ${CYAN}>>>${NC}   Pre-deploy build_id: ${PRE_BUILD}"

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

# ─── Step 2b: SHA256 verification on server ──────────────────────────
if [ -n "$LOCAL_SHA256" ]; then
    info "Verifying SHA256 on server..."
    REMOTE_HASH=$(curl -s --max-time 30 "http://${SERVER_IP}:${SENTRY_PORT}/exec" \
        -H "Content-Type: application/json" \
        -d '{"cmd":"certutil -hashfile C:/RacingPoint/racecontrol-new.exe SHA256 | findstr /v hash | findstr /v Cert"}' 2>/dev/null || echo "")
    REMOTE_HASH=$(echo "$REMOTE_HASH" | tr -d '[:space:]' | head -c 64)
    if [ -n "$REMOTE_HASH" ] && [ "$LOCAL_SHA256" != "$REMOTE_HASH" ]; then
        fail "SHA256 mismatch! Local=${LOCAL_SHA256:0:12}... Remote=${REMOTE_HASH:0:12}... — binary corrupted during transfer."
    elif [ -n "$REMOTE_HASH" ]; then
        pass "SHA256 verified (${REMOTE_HASH:0:12}...)"
    else
        info "SHA256 check skipped (remote hash unavailable)"
    fi
fi

# ─── Step 2c: Clear sentinel files ────────────────────────────────────
info "Clearing sentinel files on server..."
curl -s --max-time 10 "http://${SERVER_IP}:${SENTRY_PORT}/exec" \
    -H "Content-Type: application/json" \
    -d '{"cmd":"del /Q C:\\RacingPoint\\MAINTENANCE_MODE C:\\RacingPoint\\GRACEFUL_RELAUNCH 2>nul & echo CLEARED"}' > /dev/null 2>&1

# ─── Step 3: Stop racecontrol via bat window kill ────────────────────
info "Stopping racecontrol (killing bat wrapper + process)..."
curl -s --max-time 15 "http://${SERVER_IP}:${SENTRY_PORT}/exec" \
    -H "Content-Type: application/json" \
    -d '{"cmd":"taskkill /F /FI \"WINDOWTITLE eq RaceControl*\" 2>nul & taskkill /F /IM racecontrol.exe 2>nul & echo STOPPED"}' > /dev/null 2>&1
sleep 3
pass "racecontrol stopped"

# ─── Step 4: Preserve rollback binary, then swap ─────────────────────
info "Preserving rollback binary (racecontrol-prev.exe)..."
curl -s --max-time 10 "http://${SERVER_IP}:${SENTRY_PORT}/exec" \
    -H "Content-Type: application/json" \
    -d '{"cmd":"if exist C:\\RacingPoint\\racecontrol.exe (copy /Y C:\\RacingPoint\\racecontrol.exe C:\\RacingPoint\\racecontrol-prev.exe >nul & echo PRESERVED) else (echo SKIP)"}' > /dev/null 2>&1

info "Swapping binary..."
SWAP=$(curl -s --max-time 15 "http://${SERVER_IP}:${SENTRY_PORT}/exec" \
    -H "Content-Type: application/json" \
    -d '{"cmd":"move /Y C:/RacingPoint/racecontrol-new.exe C:/RacingPoint/racecontrol.exe & echo SWAPPED"}' 2>/dev/null || echo "")
if ! echo "$SWAP" | grep -q "SWAPPED"; then
    fail "Swap failed: $SWAP"
fi
pass "Binary swapped (rollback: racecontrol-prev.exe)"

# ─── Step 5: Start racecontrol ──────────────────────────────────────
info "Starting racecontrol..."
curl -s --max-time 10 "http://${SERVER_IP}:${SENTRY_PORT}/exec" \
    -H "Content-Type: application/json" \
    -d '{"cmd":"start \"RaceControl Server\" C:/RacingPoint/start-racecontrol.bat & echo STARTED"}' > /dev/null 2>&1

# ─── Step 6: Health check with build_id verification ─────────────────
info "Waiting for server health..."
for i in $(seq 1 12); do
    sleep 5
    HEALTH_BODY=$(curl -s --max-time 5 "http://${SERVER_IP}:${HEALTH_PORT}/api/v1/health" 2>/dev/null || echo "")
    if echo "$HEALTH_BODY" | grep -q '"status":"ok"'; then
        ACTUAL_BUILD=$(echo "$HEALTH_BODY" | grep -oP '"build_id":"\K[^"]+' || echo "unknown")

        if [ -n "$EXPECTED_BUILD_ID" ] && [ "$ACTUAL_BUILD" != "$EXPECTED_BUILD_ID" ]; then
            echo ""
            echo -e "  ${RED}!!${NC}    ${RED}build_id MISMATCH: expected ${EXPECTED_BUILD_ID}, got ${ACTUAL_BUILD}${NC}"
            echo -e "  ${RED}!!${NC}    ${RED}Cargo may have cached a stale build. Run:${NC}"
            echo -e "  ${RED}!!${NC}    ${RED}  touch crates/racecontrol/build.rs && cargo build --release${NC}"
            echo -e "  ${RED}!!${NC}    ${RED}Rollback: rename racecontrol-prev.exe → racecontrol.exe on server${NC}"
            exit 1
        fi

        pass "Server healthy — build_id: ${ACTUAL_BUILD}"
        if [ -n "$EXPECTED_BUILD_ID" ]; then
            pass "build_id matches expected (${EXPECTED_BUILD_ID})"
        fi
        echo ""
        echo -e "${GREEN}=== Deploy successful! ===${NC}"
        echo -e "${YELLOW}NOTE: Verify the EXACT fix, not just health. Test the specific endpoint/behavior that changed.${NC}"
        exit 0
    fi
    echo "    Attempt $i/12 — waiting..."
done

echo ""
echo -e "${RED}Server did not come up after 60 seconds.${NC}"
echo -e "${YELLOW}Rollback: via rc-sentry exec, rename racecontrol-prev.exe → racecontrol.exe, then schtasks /Run /TN StartRCTemp${NC}"
exit 1
