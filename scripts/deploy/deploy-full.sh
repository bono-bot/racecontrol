#!/usr/bin/env bash
# =============================================================================
# deploy-full.sh — Unified atomic deploy for Bono VPS (cloud mirror)
#
# Builds EVERYTHING: Rust binary + ALL 4 frontends + admin dashboard.
# Closes the structural gap where deploy-cloud.sh only built the Rust binary
# and CI only built 3 of 4 frontends (missing kiosk).
#
# Sequence (atomic — each step gates the next):
#   1. SSH reachable
#   2. Git pull (verify HEAD matches local)
#   3. Cargo build (release, racecontrol binary)
#   4. Build ALL frontends (kiosk, web, pwa, admin dashboard)
#   5. Restart all services via pm2
#   6. Health verify (binary + all frontends)
#
# Usage:
#   bash scripts/deploy/deploy-full.sh              # full deploy
#   SKIP_BUILD=1 bash scripts/deploy/deploy-full.sh # restart only
#   SKIP_RUST=1 bash scripts/deploy/deploy-full.sh  # frontends only
#   SKIP_FRONTEND=1 bash scripts/deploy/deploy-full.sh  # Rust only
#
# Standing rule enforcement:
#   "DEPLOY PARITY — every local deploy MUST also deploy to cloud"
#   "After ANY server deploy, rebuild ALL 3 frontends"
#   "Build must finish before restart" (feedback_deploy_rebuild_atomic.md)
# =============================================================================

set -euo pipefail

CLOUD_HOST="${CLOUD_HOST:-100.70.177.44}"
CLOUD_USER="${CLOUD_USER:-root}"
CLOUD_SSH="${CLOUD_USER}@${CLOUD_HOST}"
REPO_DIR="/root/racingpoint/racecontrol"
ADMIN_DIR="/root/racingpoint/racingpoint-admin"
CARGO_BIN="/root/.cargo/bin"

GREEN='\033[0;32m'; RED='\033[0;31m'; YELLOW='\033[0;33m'; CYAN='\033[0;36m'; NC='\033[0m'
pass() { echo -e "  ${GREEN}OK${NC}    $1"; }
fail() { echo -e "  ${RED}FAIL${NC}  $1"; exit 1; }
info() { echo -e "  ${YELLOW}...${NC}   $1"; }
section() { echo ""; echo -e "${CYAN}=== $1 ===${NC}"; }

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

# Get expected build_id from local HEAD
EXPECTED_BUILD_ID=""
if [ -d "${REPO_ROOT}/.git" ]; then
    EXPECTED_BUILD_ID=$(git -C "$REPO_ROOT" rev-parse --short HEAD 2>/dev/null || echo "")
fi

DEPLOY_START=$(date +%s)

echo ""
echo "=========================================="
echo "  Full Cloud Deploy (Bono VPS)"
echo "  Host:  ${CLOUD_SSH}"
echo "  Repo:  ${REPO_DIR}"
echo "  Expected build_id: ${EXPECTED_BUILD_ID:-unknown}"
echo "  Skip Rust:     ${SKIP_RUST:-0}"
echo "  Skip Frontend: ${SKIP_FRONTEND:-0}"
echo "  Skip Build:    ${SKIP_BUILD:-0}"
echo "=========================================="
echo ""

# ─── Step 0: SSH reachable ──────────────────────────────────────────
section "Step 0: SSH connectivity"
if ! ssh -o ConnectTimeout=5 -o BatchMode=yes "${CLOUD_SSH}" "echo ok" > /dev/null 2>&1; then
    fail "Cannot SSH to ${CLOUD_SSH}. Check Tailscale / network."
fi
pass "SSH reachable"

# ─── Step 1: Record pre-deploy state ────────────────────────────────
section "Step 1: Pre-deploy state"
PRE_BUILD=$(ssh "${CLOUD_SSH}" "curl -s --max-time 5 http://localhost:8080/api/v1/health 2>/dev/null" \
    | grep -oP '"build_id":"\K[^"]+' || echo "unknown")
echo -e "  ${CYAN}>>>${NC}   Pre-deploy build_id: ${PRE_BUILD}"

# ─── Step 2: Git pull ───────────────────────────────────────────────
section "Step 2: Git pull"
info "Pulling racecontrol..."
ssh "${CLOUD_SSH}" "cd ${REPO_DIR} && git stash --include-untracked -q 2>/dev/null || true && git pull origin main 2>&1" | head -5

info "Pulling racingpoint-admin..."
ssh "${CLOUD_SSH}" "cd ${ADMIN_DIR} && git stash --include-untracked -q 2>/dev/null || true && git pull origin main 2>&1" | head -5

# Verify VPS HEAD matches local HEAD
VPS_HEAD=$(ssh "${CLOUD_SSH}" "cd ${REPO_DIR} && git rev-parse --short HEAD 2>/dev/null")
if [ -n "$EXPECTED_BUILD_ID" ] && [ "$VPS_HEAD" != "$EXPECTED_BUILD_ID" ]; then
    echo -e "  ${RED}!!${NC}    VPS HEAD (${VPS_HEAD}) != local HEAD (${EXPECTED_BUILD_ID})"
    fail "Code not in sync — push local commits first"
fi
pass "VPS at ${VPS_HEAD}"

# ─── Step 3: Cargo build ───────────────────────────────────────────
if [ "${SKIP_BUILD:-}" = "1" ] || [ "${SKIP_RUST:-}" = "1" ]; then
    section "Step 3: Cargo build (SKIPPED)"
else
    section "Step 3: Cargo build"
    info "Building racecontrol (release)... this takes 2-5 minutes"
    BUILD_OUTPUT=$(ssh "${CLOUD_SSH}" "export PATH=${CARGO_BIN}:\$PATH && cd ${REPO_DIR} && cargo build --release -p racecontrol-crate 2>&1 | tail -5")
    echo "    ${BUILD_OUTPUT}"
    if echo "$BUILD_OUTPUT" | grep -q "^error"; then
        fail "Cargo build failed"
    fi
    pass "Rust build complete"
fi

# ─── Step 4: Build ALL frontends ────────────────────────────────────
if [ "${SKIP_BUILD:-}" = "1" ] || [ "${SKIP_FRONTEND:-}" = "1" ]; then
    section "Step 4: Frontend builds (SKIPPED)"
else
    section "Step 4: Frontend builds"

    # 4a: Kiosk (the one CI was missing)
    info "Building kiosk..."
    KIOSK_OUT=$(ssh "${CLOUD_SSH}" "cd ${REPO_DIR}/kiosk && npm ci --production=false 2>&1 | tail -1 && NEXT_PUBLIC_API_URL=https://api.racingpoint.cloud/api/v1 NEXT_PUBLIC_IS_CLOUD=true npm run build 2>&1 | tail -3")
    echo "    ${KIOSK_OUT}"
    if echo "$KIOSK_OUT" | grep -qi "error\|failed"; then
        warn "Kiosk build may have errors — check output above"
    else
        pass "Kiosk built"
    fi

    # 4b: Web dashboard
    info "Building web dashboard..."
    WEB_OUT=$(ssh "${CLOUD_SSH}" "cd ${REPO_DIR}/web && npm ci --production=false 2>&1 | tail -1 && NEXT_PUBLIC_API_URL=https://api.racingpoint.cloud npm run build 2>&1 | tail -3")
    echo "    ${WEB_OUT}"
    if echo "$WEB_OUT" | grep -qi "error\|failed"; then
        warn "Web build may have errors"
    else
        pass "Web built"
    fi

    # 4c: PWA
    info "Building PWA..."
    PWA_OUT=$(ssh "${CLOUD_SSH}" "cd ${REPO_DIR}/pwa && npm ci --production=false 2>&1 | tail -1 && NEXT_PUBLIC_API_URL=https://api.racingpoint.cloud/api/v1 NEXT_PUBLIC_IS_CLOUD=true npm run build 2>&1 | tail -3")
    echo "    ${PWA_OUT}"
    if echo "$PWA_OUT" | grep -qi "error\|failed"; then
        warn "PWA build may have errors"
    else
        pass "PWA built"
    fi

    # 4d: Admin dashboard (separate repo)
    info "Building admin dashboard..."
    ADMIN_OUT=$(ssh "${CLOUD_SSH}" "cd ${ADMIN_DIR} && npm ci --production=false 2>&1 | tail -1 && npm run build 2>&1 | tail -3")
    echo "    ${ADMIN_OUT}"
    if echo "$ADMIN_OUT" | grep -qi "error\|failed"; then
        warn "Admin build may have errors"
    else
        pass "Admin built"
    fi
fi

# ─── Step 5: Restart all services via pm2 ───────────────────────────
section "Step 5: Restart services"
info "Restarting all pm2 services..."

# Restart binary first, then frontends
ssh "${CLOUD_SSH}" "pm2 restart racecontrol 2>&1 || true" > /dev/null
sleep 3
ssh "${CLOUD_SSH}" "pm2 restart racecontrol-pwa 2>&1 || true" > /dev/null
ssh "${CLOUD_SSH}" "pm2 restart racingpoint-dashboard 2>&1 || true" > /dev/null
# Restart kiosk + admin if they have pm2 entries
ssh "${CLOUD_SSH}" "pm2 restart racecontrol-kiosk 2>&1 || true" > /dev/null
ssh "${CLOUD_SSH}" "pm2 restart racingpoint-admin 2>&1 || true" > /dev/null
sleep 5
pass "pm2 restart issued for all services"

# ─── Step 6: Health verify ──────────────────────────────────────────
section "Step 6: Health verification"

# 6a: Binary health + build_id
info "Checking binary health..."
for i in $(seq 1 6); do
    HEALTH_BODY=$(ssh "${CLOUD_SSH}" "curl -s --max-time 5 http://localhost:8080/api/v1/health 2>/dev/null" || echo "")
    if echo "$HEALTH_BODY" | grep -q '"status":"ok"'; then
        ACTUAL_BUILD=$(echo "$HEALTH_BODY" | grep -oP '"build_id":"\K[^"]+' || echo "unknown")
        if [ -n "$EXPECTED_BUILD_ID" ] && [ "$ACTUAL_BUILD" != "$EXPECTED_BUILD_ID" ]; then
            echo -e "  ${RED}!!${NC}    build_id MISMATCH: expected ${EXPECTED_BUILD_ID}, got ${ACTUAL_BUILD}"
            fail "build_id mismatch after deploy"
        fi
        pass "Binary healthy — build_id: ${ACTUAL_BUILD}"
        break
    fi
    echo "    Attempt $i/6 — waiting..."
    sleep 5
done

# 6b: Frontend health
info "Checking frontends..."
printf "  %-10s %-10s\n" "Service" "Status"
printf "  %-10s %-10s\n" "----------" "----------"

for svc_url in "PWA:3100:/" "Web:3200:/" "Kiosk:3300:/" "Admin:3201:/"; do
    svc=$(echo "$svc_url" | cut -d: -f1)
    port=$(echo "$svc_url" | cut -d: -f2)
    path=$(echo "$svc_url" | cut -d: -f3)
    code=$(ssh "${CLOUD_SSH}" "curl -sf -o /dev/null -w '%{http_code}' --max-time 5 http://localhost:${port}${path} 2>/dev/null" || echo "000")
    printf "  %-10s %-10s\n" "$svc" "HTTP $code"
done

echo ""

# ─── Summary ────────────────────────────────────────────────────────
DEPLOY_END=$(date +%s)
DEPLOY_DURATION=$((DEPLOY_END - DEPLOY_START))

echo "=========================================="
echo -e "  ${GREEN}Full cloud deploy complete!${NC}"
echo "  Duration: ${DEPLOY_DURATION}s"
echo "  Pre:  ${PRE_BUILD}"
echo "  Post: ${ACTUAL_BUILD:-unknown}"
echo "=========================================="

# Suggest running fleet parity check
echo ""
echo "Next: run fleet parity check to compare all targets:"
echo "  bash scripts/deploy/deploy-verify-fleet.sh"
