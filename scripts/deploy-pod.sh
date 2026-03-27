#!/bin/bash
# =============================================================================
# deploy-pod.sh — Safe rc-agent deploy to a single pod via rc-sentry
#
# FAILSAFE: Uses rc-sentry (:8091) for all deploy operations.
# rc-sentry is independent of both racecontrol and rc-agent — it never
# loses connectivity during deploys.
#
# VERIFICATION (v2): Records expected build_id before deploy, verifies
# post-deploy that the pod reports the correct build_id. Preserves
# rollback binary. Compares SHA256 of staged vs downloaded binary.
#
# Usage:
#   ./deploy-pod.sh pod-8                 # deploy to Pod 8 only
#   ./deploy-pod.sh all                   # deploy to all 8 pods
#   BINARY_DIR=/path ./deploy-pod.sh pod-3
# =============================================================================

set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
JAMES_IP="${JAMES_IP:-192.168.31.27}"
SENTRY_PORT=8091
SERVE_PORT=18889
BINARY_DIR="${BINARY_DIR:-$(cd "${SCRIPT_DIR}/../racingpoint/deploy-staging" 2>/dev/null && pwd || echo "$HOME/racingpoint/deploy-staging")}"
BINARY_DIR="${BINARY_DIR:-$HOME/racingpoint/deploy-staging}"

GREEN='\033[0;32m'; RED='\033[0;31m'; YELLOW='\033[0;33m'; CYAN='\033[0;36m'; NC='\033[0m'
pass() { echo -e "  ${GREEN}OK${NC}    $1"; }
fail() { echo -e "  ${RED}FAIL${NC}  $1"; FAILURES=$((FAILURES+1)); }
info() { echo -e "  ${YELLOW}...${NC}   $1"; }
FAILURES=0

# ─── Deploy lock (F49: prevent concurrent deploys) ────────────────────
LOCK_FILE="/tmp/deploy-pod.lock"
if [ -f "$LOCK_FILE" ]; then
    LOCK_PID=$(cat "$LOCK_FILE" 2>/dev/null || echo "0")
    LOCK_AGE=$(( $(date +%s) - $(stat -c%Y "$LOCK_FILE" 2>/dev/null || echo "0") ))
    if kill -0 "$LOCK_PID" 2>/dev/null && [ "$LOCK_AGE" -lt 600 ]; then
        fail "Another deploy (PID $LOCK_PID) is in progress (${LOCK_AGE}s). Wait or remove $LOCK_FILE"
        exit 1
    fi
    echo -e "  ${YELLOW}!!${NC}    Stale lock (PID $LOCK_PID dead or age ${LOCK_AGE}s) — removing"
fi
echo "$$" > "$LOCK_FILE"
trap "rm -f '$LOCK_FILE'" EXIT

# Source pod map if available
POD_MAP="${SCRIPT_DIR}/../tests/e2e/lib/pod-map.sh"
if [ -f "$POD_MAP" ]; then
    source "$POD_MAP"
else
    pod_ip() {
        case "$1" in
            pod-1) echo "192.168.31.89" ;; pod-2) echo "192.168.31.33" ;;
            pod-3) echo "192.168.31.28" ;; pod-4) echo "192.168.31.88" ;;
            pod-5) echo "192.168.31.86" ;; pod-6) echo "192.168.31.87" ;;
            pod-7) echo "192.168.31.38" ;; pod-8) echo "192.168.31.91" ;;
            pos) echo "192.168.31.20" ;;
            *) echo "" ;;
        esac
    }
fi

# ─── Gate: Verify security + manifest before deploying ────────────────
MANIFEST="${BINARY_DIR}/release-manifest.toml"
if [ ! -f "$MANIFEST" ]; then
    fail "release-manifest.toml not found — run ./scripts/stage-release.sh first"
    exit 1
fi
MANIFEST_HASH=$(grep 'git_commit' "$MANIFEST" 2>/dev/null | head -1 | cut -d'"' -f2)
HEAD_HASH=$(cd "${SCRIPT_DIR}/.." 2>/dev/null && git rev-parse --short HEAD 2>/dev/null || echo "unknown")
if [ "$MANIFEST_HASH" != "$HEAD_HASH" ]; then
    echo -e "  ${YELLOW}!!${NC}    Manifest git_commit=${MANIFEST_HASH} vs HEAD=${HEAD_HASH} — staged binary may be stale"
fi

# Run security check (fast, static — no daemon needed)
COMMS_ROOT="${SCRIPT_DIR}/../../comms-link"
if [ -f "${COMMS_ROOT}/test/security-check.js" ]; then
    info "Running security gate (SEC-GATE-01)..."
    if node "${COMMS_ROOT}/test/security-check.js" > /dev/null 2>&1; then
        pass "Security gate passed"
    else
        fail "Security gate FAILED — run: node ${COMMS_ROOT}/test/security-check.js"
        exit 1
    fi
fi

deploy_pod() {
    local POD_NAME="$1"
    local POD_IP=$(pod_ip "$POD_NAME")
    if [ -z "$POD_IP" ]; then
        fail "Unknown pod: $POD_NAME"
        return
    fi

    echo ""
    echo "─── Deploying to $POD_NAME ($POD_IP) ───"

    # Gate: rc-sentry reachable
    SENTRY=$(curl -s --max-time 5 "http://${POD_IP}:${SENTRY_PORT}/ping" 2>/dev/null || echo "")
    if [ "$SENTRY" != "pong" ]; then
        fail "$POD_NAME: rc-sentry unreachable"
        return
    fi

    # Download
    info "$POD_NAME: Downloading rc-agent.exe..."
    DL=$(curl -s --max-time 120 "http://${POD_IP}:${SENTRY_PORT}/exec" \
        -H "Content-Type: application/json" \
        -d "{\"cmd\":\"curl -s -o C:/RacingPoint/rc-agent-new.exe http://${JAMES_IP}:${SERVE_PORT}/rc-agent.exe && for %f in (C:/RacingPoint/rc-agent-new.exe) do echo SIZE=%~zf\"}" 2>/dev/null || echo "")
    DL_SIZE=$(echo "$DL" | grep -oP 'SIZE=\K[0-9]+' || echo "0")
    if [ "$DL_SIZE" -lt 500000 ]; then
        fail "$POD_NAME: Download failed (${DL_SIZE} bytes)"
        return
    fi
    pass "$POD_NAME: Downloaded (${DL_SIZE} bytes)"

    # SHA256 verification: compare staged binary hash with downloaded binary on pod
    info "$POD_NAME: Verifying SHA256..."
    REMOTE_HASH=$(curl -s --max-time 30 "http://${POD_IP}:${SENTRY_PORT}/exec" \
        -H "Content-Type: application/json" \
        -d '{"cmd":"certutil -hashfile C:/RacingPoint/rc-agent-new.exe SHA256 | findstr /v hash | findstr /v Cert"}' 2>/dev/null || echo "")
    REMOTE_HASH=$(echo "$REMOTE_HASH" | tr -d '[:space:]' | head -c 64)
    if [ -n "$LOCAL_SHA256" ] && [ -n "$REMOTE_HASH" ] && [ "$LOCAL_SHA256" != "$REMOTE_HASH" ]; then
        fail "$POD_NAME: SHA256 mismatch! Local=${LOCAL_SHA256:0:12}... Remote=${REMOTE_HASH:0:12}..."
        # Clean up bad download
        curl -s --max-time 10 "http://${POD_IP}:${SENTRY_PORT}/exec" \
            -H "Content-Type: application/json" \
            -d '{"cmd":"del C:/RacingPoint/rc-agent-new.exe 2>nul"}' > /dev/null 2>&1
        return
    elif [ -n "$LOCAL_SHA256" ] && [ -n "$REMOTE_HASH" ]; then
        pass "$POD_NAME: SHA256 verified (${REMOTE_HASH:0:12}...)"
    else
        info "$POD_NAME: SHA256 check skipped (hash unavailable)"
    fi

    # Clear sentinel files before deploy (PP-06: MAINTENANCE_MODE blocks pods silently)
    info "$POD_NAME: Clearing sentinel files..."
    curl -s --max-time 10 "http://${POD_IP}:${SENTRY_PORT}/exec" \
        -H "Content-Type: application/json" \
        -d '{"cmd":"del /Q C:\\RacingPoint\\MAINTENANCE_MODE C:\\RacingPoint\\GRACEFUL_RELAUNCH C:\\RacingPoint\\rcagent-restart-sentinel.txt 2>nul & echo CLEARED"}' > /dev/null 2>&1

    # Set OTA_DEPLOYING sentinel with Unix epoch for TTL enforcement (F15+N5)
    # Sentinel auto-expires after 10 min — watchdogs must check file age, not just existence
    OTA_EPOCH=$(date +%s)
    curl -s --max-time 10 "http://${POD_IP}:${SENTRY_PORT}/exec" \
        -H "Content-Type: application/json" \
        -d "{\"cmd\":\"echo ${OTA_EPOCH} > C:\\\\RacingPoint\\\\OTA_DEPLOYING & echo SENTINEL_SET\"}" > /dev/null 2>&1

    # Stop rc-agent (kill bat wrapper first to prevent auto-relaunch, then kill process)
    info "$POD_NAME: Stopping rc-agent..."
    curl -s --max-time 15 "http://${POD_IP}:${SENTRY_PORT}/exec" \
        -H "Content-Type: application/json" \
        -d '{"cmd":"taskkill /F /FI \"WINDOWTITLE eq RC*Agent*\" 2>nul & taskkill /F /IM rc-agent.exe 2>nul & echo KILLED"}' > /dev/null 2>&1
    sleep 3

    # Preserve rollback binary before swap
    info "$POD_NAME: Preserving rollback binary..."
    curl -s --max-time 10 "http://${POD_IP}:${SENTRY_PORT}/exec" \
        -H "Content-Type: application/json" \
        -d '{"cmd":"if exist C:\\RacingPoint\\rc-agent.exe (copy /Y C:\\RacingPoint\\rc-agent.exe C:\\RacingPoint\\rc-agent-prev.exe >nul & echo PRESERVED) else (echo SKIP)"}' > /dev/null 2>&1

    # Swap
    SWAP=$(curl -s --max-time 15 "http://${POD_IP}:${SENTRY_PORT}/exec" \
        -H "Content-Type: application/json" \
        -d '{"cmd":"move /Y C:/RacingPoint/rc-agent-new.exe C:/RacingPoint/rc-agent.exe & echo SWAPPED"}' 2>/dev/null || echo "")
    if ! echo "$SWAP" | grep -q "SWAPPED"; then
        fail "$POD_NAME: Swap failed"
        return
    fi

    # Sync canonical bat files (BAT-04: deploy-pod.sh includes bat sync step)
    info "$POD_NAME: Syncing bat files..."
    if [ -f "${BINARY_DIR}/start-rcagent.bat" ]; then
        local BAT_DL
        BAT_DL=$(curl -s --max-time 30 "http://${POD_IP}:${SENTRY_PORT}/exec" \
            -H "Content-Type: application/json" \
            -d "{\"cmd\":\"curl -s -o C:/RacingPoint/start-rcagent.bat http://${JAMES_IP}:${SERVE_PORT}/start-rcagent.bat & echo BAT_SYNCED\"}" 2>/dev/null || echo "")
        if echo "$BAT_DL" | grep -q "BAT_SYNCED"; then
            pass "$POD_NAME: start-rcagent.bat synced"
        else
            info "$POD_NAME: start-rcagent.bat sync failed (non-fatal)"
        fi
    fi
    if [ -f "${BINARY_DIR}/start-rcsentry.bat" ]; then
        BAT_DL=$(curl -s --max-time 30 "http://${POD_IP}:${SENTRY_PORT}/exec" \
            -H "Content-Type: application/json" \
            -d "{\"cmd\":\"curl -s -o C:/RacingPoint/start-rcsentry.bat http://${JAMES_IP}:${SERVE_PORT}/start-rcsentry.bat & echo BAT_SYNCED\"}" 2>/dev/null || echo "")
        if echo "$BAT_DL" | grep -q "BAT_SYNCED"; then
            pass "$POD_NAME: start-rcsentry.bat synced"
        else
            info "$POD_NAME: start-rcsentry.bat sync failed (non-fatal)"
        fi
    fi

    # Start
    curl -s --max-time 10 "http://${POD_IP}:${SENTRY_PORT}/exec" \
        -H "Content-Type: application/json" \
        -d '{"cmd":"start \"rc-agent\" C:/RacingPoint/start-rcagent.bat & echo STARTED"}' > /dev/null 2>&1

    # Verify: ping + build_id
    sleep 5
    HEALTH_BODY=$(curl -s --max-time 5 "http://${POD_IP}:8090/health" 2>/dev/null || echo "")
    ACTUAL_BUILD=$(echo "$HEALTH_BODY" | grep -oP '"build_id":"\K[^"]+' || echo "")

    if [ -z "$ACTUAL_BUILD" ]; then
        info "$POD_NAME: rc-agent not yet responding (may need more time)"
    elif [ -n "$EXPECTED_BUILD_ID" ] && [ "$ACTUAL_BUILD" != "$EXPECTED_BUILD_ID" ]; then
        fail "$POD_NAME: build_id MISMATCH — expected ${EXPECTED_BUILD_ID}, got ${ACTUAL_BUILD}. Rollback: rename rc-agent-prev.exe back."
    elif [ -n "$EXPECTED_BUILD_ID" ]; then
        pass "$POD_NAME: rc-agent UP — build_id ${ACTUAL_BUILD} matches expected"
    else
        pass "$POD_NAME: rc-agent UP — build_id ${ACTUAL_BUILD} (no expected build_id to compare)"
    fi

    # Clear OTA_DEPLOYING sentinel (F15)
    curl -s --max-time 10 "http://${POD_IP}:${SENTRY_PORT}/exec" \
        -H "Content-Type: application/json" \
        -d '{"cmd":"del /Q C:\\RacingPoint\\OTA_DEPLOYING 2>nul & echo OTA_CLEARED"}' > /dev/null 2>&1

    # Verify Session 1 context (PP-01: Session 0 kills all GUI)
    SESSION_CHECK=$(curl -s --max-time 10 "http://${POD_IP}:${SENTRY_PORT}/exec" \
        -H "Content-Type: application/json" \
        -d '{"cmd":"tasklist /V /FO CSV | findstr rc-agent"}' 2>/dev/null || echo "")
    if echo "$SESSION_CHECK" | grep -qi "services"; then
        fail "$POD_NAME: rc-agent running in Session 0 — GUI will NOT work. Kill + let RCWatchdog restart."
    elif echo "$SESSION_CHECK" | grep -qi "console"; then
        pass "$POD_NAME: rc-agent in Session 1 (Console) — GUI OK"
    else
        info "$POD_NAME: Session context check inconclusive"
    fi

    # Verify bat sync (post-deploy)
    if [ -f "${SCRIPT_DIR}/bat-scanner.sh" ]; then
        source "${SCRIPT_DIR}/bat-scanner.sh" 2>/dev/null
        local POD_NUM="${POD_NAME#pod-}"
        if bat_scan_pod "$POD_NUM" "start-rcagent.bat" "${SCRIPT_DIR}/deploy/start-rcagent.bat" >/dev/null 2>&1; then
            pass "$POD_NAME: bat file verified (post-deploy match)"
        else
            info "$POD_NAME: bat file verification skipped or drift detected"
        fi
    fi
}

# ─── Main ─────────────────────────────────────────────────────────────

TARGET="${1:-}"
if [ -z "$TARGET" ]; then
    echo "Usage: $0 <pod-N|all>"
    exit 1
fi

# Gate: Binary exists
if [ ! -f "${BINARY_DIR}/rc-agent.exe" ]; then
    fail "Binary not found: ${BINARY_DIR}/rc-agent.exe"
    exit 1
fi
BINARY_SIZE=$(stat -c%s "${BINARY_DIR}/rc-agent.exe" 2>/dev/null || wc -c < "${BINARY_DIR}/rc-agent.exe" 2>/dev/null || echo "0")
pass "rc-agent.exe binary valid (${BINARY_SIZE} bytes)"

# Compute local SHA256 for verification
LOCAL_SHA256=$(sha256sum "${BINARY_DIR}/rc-agent.exe" 2>/dev/null | awk '{print $1}' || certutil -hashfile "${BINARY_DIR}/rc-agent.exe" SHA256 2>/dev/null | grep -v hash | grep -v Cert | tr -d '[:space:]' || echo "")
if [ -n "$LOCAL_SHA256" ]; then
    pass "Local SHA256: ${LOCAL_SHA256:0:16}..."
else
    fail "Cannot compute SHA256 of local binary — deploy integrity cannot be verified"
    exit 1
fi

# Record expected build_id from git HEAD (if inside a git repo)
EXPECTED_BUILD_ID=""
REPO_DIR=$(cd "${SCRIPT_DIR}/.." 2>/dev/null && pwd || echo "")
if [ -d "${REPO_DIR}/.git" ]; then
    EXPECTED_BUILD_ID=$(git -C "$REPO_DIR" rev-parse --short HEAD 2>/dev/null || echo "")
    if [ -n "$EXPECTED_BUILD_ID" ]; then
        echo -e "  ${CYAN}>>>${NC}   Expected build_id: ${EXPECTED_BUILD_ID}"
    fi

    # Staleness check: compare binary mtime vs latest commit time
    BINARY_MTIME=$(stat -c%Y "${BINARY_DIR}/rc-agent.exe" 2>/dev/null || echo "0")
    LATEST_COMMIT_TIME=$(git -C "$REPO_DIR" log -1 --format=%ct -- crates/rc-agent/ 2>/dev/null || echo "0")
    if [ "$LATEST_COMMIT_TIME" -gt "$BINARY_MTIME" ] 2>/dev/null; then
        echo -e "  ${RED}!!${NC}    ${RED}WARNING: Staged binary is OLDER than latest rc-agent commit!${NC}"
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

# Copy canonical bat files to serve directory for bat sync during deploy
cp "${SCRIPT_DIR}/deploy/start-rcagent.bat" "${BINARY_DIR}/" 2>/dev/null || true
cp "${SCRIPT_DIR}/deploy/start-rcsentry.bat" "${BINARY_DIR}/" 2>/dev/null || true

# Start HTTP server (N11: add auto-kill timeout to prevent hanging)
info "Starting HTTP file server on :${SERVE_PORT}..."
cd "${BINARY_DIR}"
python3 -m http.server ${SERVE_PORT} --bind 0.0.0.0 > /dev/null 2>&1 &
HTTP_PID=$!
trap "kill $HTTP_PID 2>/dev/null; rm -f '$LOCK_FILE'" EXIT
# Auto-kill HTTP server after 10 minutes to prevent stale server
( sleep 600 && kill $HTTP_PID 2>/dev/null ) &
sleep 1

if [ "$TARGET" = "all" ]; then
    for i in $(seq 1 8); do
        deploy_pod "pod-$i"
    done
else
    deploy_pod "$TARGET"
fi

echo ""
echo "=========================================="
if [ "$FAILURES" -eq 0 ]; then
    echo -e "${GREEN}Deploy complete — 0 failures${NC}"
else
    echo -e "${RED}Deploy complete — ${FAILURES} failure(s)${NC}"
    echo -e "${YELLOW}Rollback: On failed pods, rename rc-agent-prev.exe → rc-agent.exe via rc-sentry${NC}"
fi
echo "=========================================="
exit $FAILURES
