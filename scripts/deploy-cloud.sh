#!/bin/bash
# =============================================================================
# deploy-cloud.sh — Deploy racecontrol to Bono VPS (cloud mirror)
#
# The cloud VPS runs a Linux binary managed by pm2, NOT a Windows .exe.
# Deploy sequence: git pull → cargo build → pm2 restart → health verify.
#
# Standing rule enforcement:
#   "Deploy all targets — enumerate from MEMORY.md, not code."
#   This script closes the structural gap where cloud was missing from ALL
#   deploy scripts, causing persistent build drift.
#
# Usage:
#   ./deploy-cloud.sh                # default: SSH to Bono VPS
#   CLOUD_HOST=x.x.x.x ./deploy-cloud.sh
#   SKIP_BUILD=1 ./deploy-cloud.sh   # restart only (binary already built)
# =============================================================================

set -euo pipefail

CLOUD_HOST="${CLOUD_HOST:-100.70.177.44}"
CLOUD_USER="${CLOUD_USER:-root}"
CLOUD_SSH="${CLOUD_USER}@${CLOUD_HOST}"
HEALTH_URL="http://localhost:8080/api/v1/health"
REPO_DIR="/root/racingpoint/racecontrol"
CARGO_BIN="/root/.cargo/bin"

GREEN='\033[0;32m'; RED='\033[0;31m'; YELLOW='\033[0;33m'; CYAN='\033[0;36m'; NC='\033[0m'
pass() { echo -e "  ${GREEN}OK${NC}    $1"; }
fail() { echo -e "  ${RED}FAIL${NC}  $1"; exit 1; }
info() { echo -e "  ${YELLOW}...${NC}   $1"; }

echo "=========================================="
echo "  Cloud Deploy (Bono VPS)"
echo "  Host:  ${CLOUD_SSH}"
echo "  Repo:  ${REPO_DIR}"
echo "=========================================="
echo ""

# ─── Gate 0: SSH reachable ───────────────────────────────────────────
info "Checking SSH connectivity..."
if ! ssh -o ConnectTimeout=5 -o BatchMode=yes "${CLOUD_SSH}" "echo ok" > /dev/null 2>&1; then
    fail "Cannot SSH to ${CLOUD_SSH}. Check Tailscale / network."
fi
pass "SSH reachable"

# ─── Gate 1: Record pre-deploy state ────────────────────────────────
info "Recording pre-deploy state..."
PRE_BUILD=$(ssh "${CLOUD_SSH}" "curl -s --max-time 5 ${HEALTH_URL} 2>/dev/null" | grep -oP '"build_id":"\K[^"]+' || echo "unknown")
echo -e "  ${CYAN}>>>${NC}   Pre-deploy build_id: ${PRE_BUILD}"

# ─── Gate 2: Get expected build_id from local HEAD ──────────────────
SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
REPO_ROOT=$(cd "${SCRIPT_DIR}/.." 2>/dev/null && pwd || echo "")
EXPECTED_BUILD_ID=""
if [ -d "${REPO_ROOT}/.git" ]; then
    EXPECTED_BUILD_ID=$(git -C "$REPO_ROOT" rev-parse --short HEAD 2>/dev/null || echo "")
    echo -e "  ${CYAN}>>>${NC}   Expected build_id: ${EXPECTED_BUILD_ID}"
fi

# ─── Step 1: Git pull ───────────────────────────────────────────────
info "Pulling latest code on VPS..."
PULL_OUTPUT=$(ssh "${CLOUD_SSH}" "cd ${REPO_DIR} && git pull 2>&1")
echo "    ${PULL_OUTPUT}"

# Verify VPS HEAD matches local HEAD
VPS_HEAD=$(ssh "${CLOUD_SSH}" "cd ${REPO_DIR} && git rev-parse --short HEAD 2>/dev/null")
if [ -n "$EXPECTED_BUILD_ID" ] && [ "$VPS_HEAD" != "$EXPECTED_BUILD_ID" ]; then
    echo -e "  ${RED}!!${NC}    VPS HEAD (${VPS_HEAD}) != local HEAD (${EXPECTED_BUILD_ID})"
    echo -e "  ${RED}!!${NC}    Push local commits first: git push"
    fail "Code not in sync — push first"
fi
pass "VPS at ${VPS_HEAD}"

# ─── Step 2: Build ──────────────────────────────────────────────────
if [ "${SKIP_BUILD:-}" = "1" ]; then
    info "SKIP_BUILD=1 — skipping cargo build"
else
    info "Building racecontrol (release)... this takes 2-5 minutes"
    # Standing rule: touch build.rs to force GIT_HASH refresh
    BUILD_OUTPUT=$(ssh "${CLOUD_SSH}" "export PATH=${CARGO_BIN}:\$PATH && cd ${REPO_DIR} && touch crates/racecontrol/build.rs && cargo build --release -p racecontrol-crate 2>&1 | tail -5")
    echo "    ${BUILD_OUTPUT}"

    if echo "$BUILD_OUTPUT" | grep -q "^error"; then
        fail "Cargo build failed — see output above"
    fi
    pass "Build complete"
fi

# ─── Step 3: Restart via pm2 ────────────────────────────────────────
info "Restarting racecontrol via pm2..."
ssh "${CLOUD_SSH}" "pm2 restart racecontrol 2>&1" > /dev/null
sleep 5
pass "pm2 restart issued"

# ─── Step 4: Health check with build_id verification ────────────────
info "Verifying health..."
for i in $(seq 1 6); do
    HEALTH_BODY=$(ssh "${CLOUD_SSH}" "curl -s --max-time 5 ${HEALTH_URL} 2>/dev/null" || echo "")

    if echo "$HEALTH_BODY" | grep -q '"status":"ok"'; then
        ACTUAL_BUILD=$(echo "$HEALTH_BODY" | grep -oP '"build_id":"\K[^"]+' || echo "unknown")

        if [ -n "$EXPECTED_BUILD_ID" ] && [ "$ACTUAL_BUILD" != "$EXPECTED_BUILD_ID" ]; then
            echo -e "  ${RED}!!${NC}    build_id MISMATCH: expected ${EXPECTED_BUILD_ID}, got ${ACTUAL_BUILD}"
            echo -e "  ${RED}!!${NC}    Try: cargo clean on VPS + rebuild"
            fail "build_id mismatch after deploy"
        fi

        pass "Cloud healthy — build_id: ${ACTUAL_BUILD}"
        if [ "$ACTUAL_BUILD" != "$PRE_BUILD" ]; then
            pass "Updated from ${PRE_BUILD} → ${ACTUAL_BUILD}"
        elif [ "$ACTUAL_BUILD" = "$PRE_BUILD" ]; then
            echo -e "  ${YELLOW}>>>${NC}   build_id unchanged (${ACTUAL_BUILD}) — code changes may be docs-only"
        fi
        echo ""
        echo -e "${GREEN}=== Cloud deploy successful! ===${NC}"
        exit 0
    fi
    echo "    Attempt $i/6 — waiting..."
    sleep 5
done

echo ""
echo -e "${RED}Cloud server did not come up after 30 seconds.${NC}"
echo -e "${RED}Manual recovery: ssh ${CLOUD_SSH}${NC}"
echo -e "${RED}  pm2 logs racecontrol --lines 20${NC}"
echo -e "${RED}  pm2 restart racecontrol${NC}"
exit 1
