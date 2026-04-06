#!/usr/bin/env bash
# =============================================================================
# deploy-verify.sh -- Post-deploy verification: hash table + page crawler + VR
#
# Standalone script for verifying frontend deployments. Can be called
# independently or from deploy-nextjs.sh after a frontend deploy.
#
# Usage:
#   bash scripts/deploy-verify.sh                      # full verification
#   bash scripts/deploy-verify.sh --expected-hash abc1234
#   bash scripts/deploy-verify.sh --skip-crawler        # hash table only + VR
#   bash scripts/deploy-verify.sh --skip-vr             # hash table + crawler only
#   bash scripts/deploy-verify.sh --skip-crawler --skip-vr  # hash table only
#
# Exit codes:
#   0 = all checks passed (or informational mismatches only)
#   1 = visual regression detected or crawler failure
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

# --- Colors & helpers (match deploy-server.sh) ---
GREEN='\033[0;32m'; RED='\033[0;31m'; YELLOW='\033[0;33m'; CYAN='\033[0;36m'; NC='\033[0m'
pass() { echo -e "  ${GREEN}OK${NC}    $1"; }
fail() { echo -e "  ${RED}FAIL${NC}  $1"; }
warn() { echo -e "  ${YELLOW}WARN${NC}  $1"; }
info() { echo -e "  ${CYAN}...${NC}   $1"; }

# --- Parse arguments ---
EXPECTED_HASH=""
SKIP_CRAWLER=false
SKIP_VR=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --expected-hash)
            EXPECTED_HASH="$2"
            shift 2
            ;;
        --skip-crawler)
            SKIP_CRAWLER=true
            shift
            ;;
        --skip-vr)
            SKIP_VR=true
            shift
            ;;
        *)
            echo "Unknown argument: $1"
            echo "Usage: $0 [--expected-hash HASH] [--skip-crawler] [--skip-vr]"
            exit 1
            ;;
    esac
done

# --- Default expected hash from git HEAD ---
if [ -z "$EXPECTED_HASH" ]; then
    if [ -d "${REPO_DIR}/.git" ]; then
        EXPECTED_HASH=$(git -C "$REPO_DIR" rev-parse --short HEAD 2>/dev/null || echo "unknown")
    else
        EXPECTED_HASH="unknown"
    fi
fi

VERIFY_START=$(date +%s)
EXIT_CODE=0

echo ""
echo "=========================================="
echo "  Post-Deploy Verification"
echo "  Expected hash: ${EXPECTED_HASH}"
echo "  Crawler: $(if $SKIP_CRAWLER; then echo 'SKIP'; else echo 'ON'; fi)"
echo "  Visual regression: $(if $SKIP_VR; then echo 'SKIP'; else echo 'ON'; fi)"
echo "=========================================="
echo ""

# =========================================================================
# Section 1: Build Hash Verification Table
# =========================================================================

SERVER_IP="${SERVER_IP:-192.168.31.23}"
CLOUD_HOST="${CLOUD_HOST:-srv1422716.hstgr.cloud}"

echo -e "${CYAN}--- Build Hash Verification Table ---${NC}"
echo ""

# Header
printf "  %-14s %-12s %-12s %-12s\n" "Target" "Expected" "Running" "Status"
printf "  %-14s %-12s %-12s %-12s\n" "--------------" "------------" "------------" "------------"

HASH_MISMATCHES=0

# --- Server (.23:8080) ---
SERVER_HEALTH=$(curl -s --max-time 5 "http://${SERVER_IP}:8080/api/v1/health" 2>/dev/null || echo "")
if [ -n "$SERVER_HEALTH" ]; then
    SERVER_BUILD=$(echo "$SERVER_HEALTH" | grep -oP '"build_id":"\K[^"]+' 2>/dev/null || echo "unknown")
    if [ "$SERVER_BUILD" = "$EXPECTED_HASH" ]; then
        printf "  %-14s %-12s %-12s ${GREEN}%-12s${NC}\n" "Server :8080" "$EXPECTED_HASH" "$SERVER_BUILD" "MATCH"
    else
        printf "  %-14s %-12s %-12s ${YELLOW}%-12s${NC}\n" "Server :8080" "$EXPECTED_HASH" "$SERVER_BUILD" "MISMATCH"
        HASH_MISMATCHES=$((HASH_MISMATCHES + 1))
    fi
else
    printf "  %-14s %-12s %-12s ${RED}%-12s${NC}\n" "Server :8080" "$EXPECTED_HASH" "-" "DOWN"
fi

# --- Web (.23:3200) ---
WEB_STATUS=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 "http://${SERVER_IP}:3200" 2>/dev/null || echo "000")
if [ "$WEB_STATUS" = "200" ] || [ "$WEB_STATUS" = "307" ] || [ "$WEB_STATUS" = "302" ]; then
    printf "  %-14s %-12s %-12s ${GREEN}%-12s${NC}\n" "Web :3200" "-" "HTTP ${WEB_STATUS}" "OK"
else
    printf "  %-14s %-12s %-12s ${RED}%-12s${NC}\n" "Web :3200" "-" "HTTP ${WEB_STATUS}" "DOWN"
fi

# --- Admin (.23:3201) ---
ADMIN_STATUS=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 "http://${SERVER_IP}:3201" 2>/dev/null || echo "000")
if [ "$ADMIN_STATUS" = "200" ] || [ "$ADMIN_STATUS" = "307" ] || [ "$ADMIN_STATUS" = "302" ]; then
    printf "  %-14s %-12s %-12s ${GREEN}%-12s${NC}\n" "Admin :3201" "-" "HTTP ${ADMIN_STATUS}" "OK"
else
    printf "  %-14s %-12s %-12s ${RED}%-12s${NC}\n" "Admin :3201" "-" "HTTP ${ADMIN_STATUS}" "DOWN"
fi

# --- Kiosk (.23:3300) ---
KIOSK_STATUS=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 "http://${SERVER_IP}:3300/kiosk" 2>/dev/null || echo "000")
if [ "$KIOSK_STATUS" = "200" ] || [ "$KIOSK_STATUS" = "307" ] || [ "$KIOSK_STATUS" = "302" ]; then
    printf "  %-14s %-12s %-12s ${GREEN}%-12s${NC}\n" "Kiosk :3300" "-" "HTTP ${KIOSK_STATUS}" "OK"
else
    printf "  %-14s %-12s %-12s ${RED}%-12s${NC}\n" "Kiosk :3300" "-" "HTTP ${KIOSK_STATUS}" "DOWN"
fi

# --- Cloud (Bono VPS) ---
CLOUD_HEALTH=$(curl -s --max-time 10 "http://${CLOUD_HOST}:8080/api/v1/health" 2>/dev/null || echo "")
if [ -n "$CLOUD_HEALTH" ]; then
    CLOUD_BUILD=$(echo "$CLOUD_HEALTH" | grep -oP '"build_id":"\K[^"]+' 2>/dev/null || echo "unknown")
    if [ "$CLOUD_BUILD" = "$EXPECTED_HASH" ]; then
        printf "  %-14s %-12s %-12s ${GREEN}%-12s${NC}\n" "Cloud" "$EXPECTED_HASH" "$CLOUD_BUILD" "MATCH"
    else
        printf "  %-14s %-12s %-12s ${YELLOW}%-12s${NC}\n" "Cloud" "$EXPECTED_HASH" "$CLOUD_BUILD" "MISMATCH"
        HASH_MISMATCHES=$((HASH_MISMATCHES + 1))
    fi
else
    printf "  %-14s %-12s %-12s ${YELLOW}%-12s${NC}\n" "Cloud" "$EXPECTED_HASH" "-" "TIMEOUT"
fi

echo ""
if [ "$HASH_MISMATCHES" -gt 0 ]; then
    warn "${HASH_MISMATCHES} hash mismatch(es) detected (informational -- does not fail deploy)"
else
    pass "All reachable targets verified"
fi
echo ""

# =========================================================================
# Section 2: Page Crawler Auto-Run
# =========================================================================

if $SKIP_CRAWLER; then
    info "Page crawler: SKIPPED (--skip-crawler)"
else
    echo -e "${CYAN}--- Page Crawler ---${NC}"
    echo ""
    info "Running page crawler..."

    CRAWLER_EXIT=0
    cd "$REPO_DIR"
    npx playwright test --config tests/page-crawler/playwright.config.ts 2>&1 || CRAWLER_EXIT=$?

    if [ "$CRAWLER_EXIT" -ne 0 ]; then
        fail "Page crawler detected failures (exit code: ${CRAWLER_EXIT})"
        warn "Review test output above for failed pages"
        EXIT_CODE=1
    else
        pass "Page crawler: all pages accessible"
    fi
    echo ""
fi

# =========================================================================
# Section 3: Visual Regression Check
# =========================================================================

if $SKIP_VR; then
    info "Visual regression: SKIPPED (--skip-vr)"
else
    echo -e "${CYAN}--- Visual Regression Check ---${NC}"
    echo ""

    SCREENSHOTS_DIR="${REPO_DIR}/tests/visual-regression/__screenshots__"

    # Check if baselines exist
    if [ ! -d "$SCREENSHOTS_DIR" ] || [ -z "$(ls -A "$SCREENSHOTS_DIR" 2>/dev/null)" ]; then
        warn "No baseline screenshots found in tests/visual-regression/__screenshots__/"
        warn "First run -- treating as success. Run with --update-snapshots to create baselines:"
        info "  npx playwright test --config tests/visual-regression/playwright.config.ts --update-snapshots"
    else
        info "Running visual regression comparison..."

        VR_EXIT=0
        cd "$REPO_DIR"
        npx playwright test --config tests/visual-regression/playwright.config.ts 2>&1 || VR_EXIT=$?

        if [ "$VR_EXIT" -ne 0 ]; then
            fail "VISUAL REGRESSION DETECTED"
            echo ""
            echo -e "  ${RED}Review diffs in: tests/visual-regression/test-results/${NC}"
            echo -e "  ${YELLOW}To update baselines:${NC}"
            echo -e "  ${YELLOW}  npx playwright test --config tests/visual-regression/playwright.config.ts --update-snapshots${NC}"
            echo ""
            EXIT_CODE=1
        else
            pass "Visual regression: no regressions detected"
        fi
    fi
    echo ""
fi

# =========================================================================
# Summary
# =========================================================================

VERIFY_END=$(date +%s)
VERIFY_DURATION=$((VERIFY_END - VERIFY_START))

echo "=========================================="
if [ "$EXIT_CODE" -eq 0 ]; then
    echo -e "  ${GREEN}VERIFICATION PASSED${NC} (${VERIFY_DURATION}s)"
else
    echo -e "  ${RED}VERIFICATION FAILED${NC} (${VERIFY_DURATION}s)"
fi
echo "=========================================="

exit $EXIT_CODE
