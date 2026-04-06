#!/usr/bin/env bash
# =============================================================================
# deploy-nextjs.sh -- Frontend deploy + post-deploy verification
#
# Builds Next.js apps (web, kiosk, or both) and runs deploy-verify.sh
# for build hash table, page crawler, and visual regression checks.
#
# Usage:
#   bash scripts/deploy-nextjs.sh                     # build all + verify
#   bash scripts/deploy-nextjs.sh --app web            # build web only + verify
#   bash scripts/deploy-nextjs.sh --app kiosk           # build kiosk only + verify
#   bash scripts/deploy-nextjs.sh --verify-only         # skip build, run verification
#   bash scripts/deploy-nextjs.sh --skip-vr             # build + verify without VR
#   bash scripts/deploy-nextjs.sh --verify-only --skip-vr  # quick hash table + crawler
#
# Apps:
#   web   = Web dashboard (:3200) + Admin (:3201) -- same codebase
#   kiosk = Kiosk app (:3300)
#   all   = Both (default)
#
# Exit codes:
#   0 = deploy + verification passed
#   1 = build failure or verification failure (visual regression / crawler)
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

# --- Colors & helpers (match deploy-server.sh) ---
GREEN='\033[0;32m'; RED='\033[0;31m'; YELLOW='\033[0;33m'; CYAN='\033[0;36m'; NC='\033[0m'
pass() { echo -e "  ${GREEN}OK${NC}    $1"; }
fail() { echo -e "  ${RED}FAIL${NC}  $1"; exit 1; }
warn() { echo -e "  ${YELLOW}WARN${NC}  $1"; }
info() { echo -e "  ${CYAN}...${NC}   $1"; }

# --- Parse arguments ---
APP="all"
SKIP_VR=false
VERIFY_ONLY=false
VERIFY_ARGS=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        --app)
            APP="$2"
            shift 2
            ;;
        --skip-vr)
            SKIP_VR=true
            VERIFY_ARGS+=("--skip-vr")
            shift
            ;;
        --skip-crawler)
            VERIFY_ARGS+=("--skip-crawler")
            shift
            ;;
        --verify-only)
            VERIFY_ONLY=true
            shift
            ;;
        *)
            echo "Unknown argument: $1"
            echo "Usage: $0 [--app web|kiosk|all] [--verify-only] [--skip-vr] [--skip-crawler]"
            exit 1
            ;;
    esac
done

# Validate --app
case "$APP" in
    web|kiosk|all) ;;
    *) fail "Invalid app: ${APP}. Must be web, kiosk, or all." ;;
esac

DEPLOY_START=$(date +%s)
SERVER_IP="${SERVER_IP:-192.168.31.23}"

echo ""
echo "=========================================="
echo "  Frontend Deploy + Verification"
echo "  App: ${APP}"
echo "  Mode: $(if $VERIFY_ONLY; then echo 'verify-only'; else echo 'build + verify'; fi)"
echo "=========================================="
echo ""

# =========================================================================
# Step 1: Pre-deploy snapshot
# =========================================================================

GIT_HASH=$(git -C "$REPO_DIR" rev-parse --short HEAD 2>/dev/null || echo "unknown")
info "Git HEAD: ${GIT_HASH}"

if ! $VERIFY_ONLY; then
    echo ""
    echo -e "${CYAN}--- Pre-Deploy Status ---${NC}"
    echo ""

    # Check if apps are currently running
    WEB_CHECK=$(curl -s -o /dev/null -w "%{http_code}" --max-time 3 "http://${SERVER_IP}:3200" 2>/dev/null || echo "000")
    KIOSK_CHECK=$(curl -s -o /dev/null -w "%{http_code}" --max-time 3 "http://${SERVER_IP}:3300/kiosk" 2>/dev/null || echo "000")

    info "Web :3200 status: HTTP ${WEB_CHECK}"
    info "Kiosk :3300 status: HTTP ${KIOSK_CHECK}"
    echo ""

    # =========================================================================
    # Step 2: Build selected app(s)
    # =========================================================================

    echo -e "${CYAN}--- Building Apps ---${NC}"
    echo ""

    build_app() {
        local app_name="$1"
        local app_dir="$2"
        local app_path="${REPO_DIR}/${app_dir}"

        if [ ! -d "$app_path" ]; then
            fail "${app_name}: directory not found at ${app_path}"
        fi

        info "Building ${app_name} (${app_dir})..."
        local build_start
        build_start=$(date +%s)

        cd "$app_path"
        npm run build 2>&1

        local build_exit=$?
        local build_end
        build_end=$(date +%s)
        local build_duration=$((build_end - build_start))

        if [ "$build_exit" -ne 0 ]; then
            fail "${app_name} build failed (exit code: ${build_exit})"
        fi

        pass "${app_name} built successfully (${build_duration}s)"
        echo ""
    }

    if [ "$APP" = "web" ] || [ "$APP" = "all" ]; then
        build_app "Web/Admin" "web"
    fi

    if [ "$APP" = "kiosk" ] || [ "$APP" = "all" ]; then
        build_app "Kiosk" "kiosk"
    fi

    # =========================================================================
    # Step 3: Restart app processes
    # =========================================================================

    echo -e "${CYAN}--- Restart Notice ---${NC}"
    echo ""
    warn "Restart the app process on server .23 if needed"
    info "Web/Admin: restart the Next.js process serving :3200/:3201"
    info "Kiosk: restart the Next.js process serving :3300"
    info "Method depends on deployment (schtasks, pm2, manual restart)"
    echo ""

    # Wait briefly for processes to stabilize after any restart
    info "Waiting 5s for processes to stabilize..."
    sleep 5
fi

# =========================================================================
# Step 4: Post-deploy verification
# =========================================================================

echo -e "${CYAN}--- Post-Deploy Verification ---${NC}"
echo ""

VERIFY_EXIT=0
bash "${SCRIPT_DIR}/deploy-verify.sh" --expected-hash "$GIT_HASH" "${VERIFY_ARGS[@]}" || VERIFY_EXIT=$?

echo ""

if [ "$VERIFY_EXIT" -ne 0 ]; then
    echo -e "${RED}=========================================="
    echo "  DEPLOY VERIFICATION FAILED"
    echo "==========================================${NC}"
    echo ""
    echo -e "  ${YELLOW}Rollback options:${NC}"
    echo "    1. Rebuild from previous commit: git checkout HEAD~1 -- web/ kiosk/ && npm run build"
    echo "    2. Restore from backup if available"
    echo "    3. Review test failures above and fix"
    echo ""
fi

# =========================================================================
# Summary
# =========================================================================

DEPLOY_END=$(date +%s)
DEPLOY_DURATION=$((DEPLOY_END - DEPLOY_START))

echo "=========================================="
if [ "$VERIFY_EXIT" -eq 0 ]; then
    echo -e "  ${GREEN}FRONTEND DEPLOY COMPLETE${NC} (${DEPLOY_DURATION}s)"
else
    echo -e "  ${RED}FRONTEND DEPLOY FAILED${NC} (${DEPLOY_DURATION}s)"
fi
echo "  Hash: ${GIT_HASH}"
echo "  App: ${APP}"
echo "=========================================="

exit $VERIFY_EXIT
