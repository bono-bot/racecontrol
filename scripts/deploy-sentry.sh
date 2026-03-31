#!/bin/bash
# =============================================================================
# deploy-sentry.sh — Safe rc-sentry deploy to pods via SCP + schtask
#
# rc-sentry is the independent watchdog/exec endpoint (:8091). Unlike rc-agent
# deploys, we cannot use rc-sentry to deploy itself — so we use SSH/SCP
# directly and restart via the StartRCSentry scheduled task.
#
# Hash-based naming: binary staged as rc-sentry-<hash>.exe on pod.
# start-rcsentry.bat auto-swaps hash-named → rc-sentry.exe on restart.
#
# Pod 1 exception: has RCSentry Windows Service — needs Stop-Service first.
#
# MMA Protocol v3.0: deploy lock, canary (Pod 8), fleet, verification.
#
# Usage:
#   ./deploy-sentry.sh                    # build + deploy all 8 pods
#   ./deploy-sentry.sh --skip-build       # deploy pre-built binary
#   ./deploy-sentry.sh pod-8              # single pod (canary)
#   ./deploy-sentry.sh all                # explicit all pods
# =============================================================================

set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
REPO_ROOT="${SCRIPT_DIR}/.."
BINARY="$REPO_ROOT/target/release/rc-sentry.exe"

GREEN='\033[0;32m'; RED='\033[0;31m'; YELLOW='\033[0;33m'; CYAN='\033[0;36m'; NC='\033[0m'
pass() { echo -e "  ${GREEN}OK${NC}    $1"; }
fail() { echo -e "  ${RED}FAIL${NC}  $1"; FAILURES=$((FAILURES+1)); }
warn() { echo -e "  ${YELLOW}!!${NC}    $1"; }
info() { echo -e "  ${CYAN}...${NC}   $1"; }
FAILURES=0

# ─── Deploy lock ─────────────────────────────────────────────────────
LOCK_FILE="/tmp/deploy-sentry.lock"
if [ -f "$LOCK_FILE" ]; then
    LOCK_PID=$(cat "$LOCK_FILE" 2>/dev/null || echo "0")
    LOCK_AGE=$(( $(date +%s) - $(stat -c%Y "$LOCK_FILE" 2>/dev/null || echo "0") ))
    if kill -0 "$LOCK_PID" 2>/dev/null && [ "$LOCK_AGE" -lt 600 ]; then
        fail "Another deploy (PID $LOCK_PID) in progress (${LOCK_AGE}s). Wait or remove $LOCK_FILE"
        exit 1
    fi
    warn "Stale lock (PID $LOCK_PID dead or age ${LOCK_AGE}s) — removing"
fi
echo "$$" > "$LOCK_FILE"
trap "rm -f '$LOCK_FILE'" EXIT

# ─── Parse args ──────────────────────────────────────────────────────
SKIP_BUILD=false
TARGET="all"
for arg in "$@"; do
    case "$arg" in
        --skip-build) SKIP_BUILD=true ;;
        pod-*|all) TARGET="$arg" ;;
        *) echo "Usage: $0 [--skip-build] [pod-N|all]"; exit 1 ;;
    esac
done

# ─── Pod map (SSH aliases: pod1..pod8) ────────────────────────────────
declare -A POD_IPS=(
    [1]="192.168.31.89" [2]="192.168.31.33" [3]="192.168.31.28" [4]="192.168.31.88"
    [5]="192.168.31.86" [6]="192.168.31.87" [7]="192.168.31.38" [8]="192.168.31.91"
)

if [ "$TARGET" = "all" ]; then
    PODS=(1 2 3 4 5 6 7 8)
else
    POD_NUM="${TARGET#pod-}"
    if [ -z "${POD_IPS[$POD_NUM]+x}" ]; then
        fail "Unknown pod: $TARGET"
        exit 1
    fi
    PODS=("$POD_NUM")
fi

echo "============================================================"
echo "  rc-sentry deploy — target: ${TARGET}"
echo "============================================================"

# ─── Step 1: Build ───────────────────────────────────────────────────
if [ "$SKIP_BUILD" = false ]; then
    info "Building rc-sentry (release)..."
    cd "$REPO_ROOT"
    touch crates/rc-sentry/build.rs 2>/dev/null || true
    cargo build --release -p rc-sentry 2>&1 | tail -3
    pass "Build complete"
fi

if [ ! -f "$BINARY" ]; then
    fail "Binary not found: $BINARY"
    exit 1
fi

HASH=$(cd "$REPO_ROOT" && git rev-parse --short HEAD)
SIZE=$(stat -c%s "$BINARY")
echo ""
echo "  Binary: rc-sentry.exe"
echo "  Hash:   $HASH"
echo "  Size:   $SIZE bytes"
echo ""

# ─── Step 2: Stage binary on pods ───────────────────────────────────
echo "--- Stage ---"
STAGE_OK=()
STAGE_FAIL=()

for pod in "${PODS[@]}"; do
    info "Pod $pod: uploading rc-sentry-${HASH}.exe ..."
    if scp -q "$BINARY" "pod${pod}:C:/RacingPoint/rc-sentry-${HASH}.exe" 2>/dev/null; then
        # Verify size on remote
        REMOTE_SIZE=$(ssh "pod${pod}" "powershell -Command \"(Get-Item C:\\RacingPoint\\rc-sentry-${HASH}.exe).Length\"" 2>/dev/null | tr -d '\r\n')
        if [ "$REMOTE_SIZE" = "$SIZE" ]; then
            pass "Pod $pod: staged ($SIZE bytes)"
            STAGE_OK+=("$pod")
        else
            fail "Pod $pod: size mismatch (local=$SIZE remote=$REMOTE_SIZE)"
            STAGE_FAIL+=("$pod")
        fi
    else
        fail "Pod $pod: SCP failed"
        STAGE_FAIL+=("$pod")
    fi
done

if [ ${#STAGE_OK[@]} -eq 0 ]; then
    fail "No pods staged successfully — aborting"
    exit 1
fi

echo ""

# ─── Step 3: Restart (kill + schtask swap + start) ──────────────────
echo "--- Restart ---"
DEPLOY_OK=()

for pod in "${STAGE_OK[@]}"; do
    info "Pod $pod: stopping + swapping + restarting..."

    # Pod 1 has a Windows Service — stop it first
    SERVICE_CMD=""
    if [ "$pod" = "1" ]; then
        SERVICE_CMD="Stop-Service RCSentry -Force -ErrorAction SilentlyContinue;"
    fi

    # PowerShell: stop process, wait for file handle release, force swap, restart via schtask
    RESULT=$(ssh "pod${pod}" "powershell -Command \"${SERVICE_CMD} Stop-Process -Name rc-sentry -Force -ErrorAction SilentlyContinue; Start-Sleep 3; Remove-Item C:\\RacingPoint\\rc-sentry-prev.exe -Force -ErrorAction SilentlyContinue; Rename-Item C:\\RacingPoint\\rc-sentry.exe C:\\RacingPoint\\rc-sentry-prev.exe -Force -ErrorAction SilentlyContinue; Rename-Item C:\\RacingPoint\\rc-sentry-${HASH}.exe C:\\RacingPoint\\rc-sentry.exe -Force; (Get-Item C:\\RacingPoint\\rc-sentry.exe).Length\"" 2>/dev/null | tr -d '\r\n')

    if [ "$RESULT" = "$SIZE" ]; then
        # Start via scheduled task
        ssh "pod${pod}" "schtasks /run /tn \"StartRCSentry\"" >/dev/null 2>&1
        pass "Pod $pod: swapped ($RESULT bytes), schtask triggered"
        DEPLOY_OK+=("$pod")
    else
        fail "Pod $pod: swap failed (got size=$RESULT, expected=$SIZE)"
        # Try to restore
        ssh "pod${pod}" "powershell -Command \"if(Test-Path C:\\RacingPoint\\rc-sentry-prev.exe){ Rename-Item C:\\RacingPoint\\rc-sentry-prev.exe C:\\RacingPoint\\rc-sentry.exe -Force -ErrorAction SilentlyContinue }; schtasks /run /tn StartRCSentry\"" >/dev/null 2>&1
        warn "Pod $pod: attempted rollback to previous binary"
    fi
done

echo ""

# ─── Step 4: Verify (wait for process, check size) ──────────────────
echo "--- Verify (waiting 8s for processes to start) ---"
sleep 8

VERIFY_OK=0
VERIFY_FAIL=0

for pod in "${DEPLOY_OK[@]}"; do
    # Check process is running
    PROC=$(ssh "pod${pod}" "tasklist" 2>/dev/null | grep -c "rc-sentry" || true)
    # Check binary size
    LIVE_SIZE=$(ssh "pod${pod}" "powershell -Command \"(Get-Item C:\\RacingPoint\\rc-sentry.exe).Length\"" 2>/dev/null | tr -d '\r\n')

    if [ "$PROC" -ge 1 ] && [ "$LIVE_SIZE" = "$SIZE" ]; then
        pass "Pod $pod: running (size=$LIVE_SIZE)"
        VERIFY_OK=$((VERIFY_OK+1))
    else
        fail "Pod $pod: NOT running or wrong size (proc=$PROC size=$LIVE_SIZE)"
        VERIFY_FAIL=$((VERIFY_FAIL+1))
    fi
done

echo ""

# ─── Summary ─────────────────────────────────────────────────────────
echo "============================================================"
echo "  rc-sentry deploy summary"
echo "============================================================"
echo "  Build:    $HASH ($SIZE bytes)"
echo "  Targeted: ${#PODS[@]} pods"
echo "  Staged:   ${#STAGE_OK[@]} OK, ${#STAGE_FAIL[@]} failed"
echo "  Deployed: ${#DEPLOY_OK[@]} swapped"
echo "  Verified: $VERIFY_OK running, $VERIFY_FAIL failed"

if [ "$VERIFY_FAIL" -gt 0 ] || [ ${#STAGE_FAIL[@]} -gt 0 ]; then
    echo -e "  ${RED}RESULT: PARTIAL DEPLOY${NC}"
    echo "  Failed pods need manual investigation via SSH"
    exit 1
else
    echo -e "  ${GREEN}RESULT: ALL PODS DEPLOYED${NC}"
fi
echo "============================================================"
