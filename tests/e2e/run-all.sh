#!/bin/bash
# tests/e2e/run-all.sh -- Master E2E test runner
# Single entry point for the full RaceControl E2E test suite.
# Runs phases in sequence, aborts on preflight failure, writes summary.json.
# Usage:
#   bash tests/e2e/run-all.sh                  # run all phases
#   bash tests/e2e/run-all.sh --skip-deploy    # skip Phase 4 (deploy verification)
#   bash tests/e2e/run-all.sh --skip-browser   # skip Phase 3 (Playwright)
#
# Environment variables:
#   RC_BASE_URL      default: http://192.168.31.23:8080/api/v1
#   TEST_POD_ID      default: pod-8
#   KIOSK_BASE_URL   default: http://192.168.31.23:3300 (for Playwright)
#
# Exit code: total failure count (0 = all pass)
# DO NOT use set -e here -- we capture per-phase exit codes manually.

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
REPO_ROOT=$(cd "$SCRIPT_DIR/../.." && pwd)

# ─── Flag parsing ────────────────────────────────────────────────────────────
SKIP_DEPLOY=false
SKIP_BROWSER=false
for arg in "$@"; do
    case "$arg" in
        --skip-deploy)  SKIP_DEPLOY=true  ;;
        --skip-browser) SKIP_BROWSER=true ;;
    esac
done

# ─── Results directory ───────────────────────────────────────────────────────
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
RESULTS_DIR="$SCRIPT_DIR/results/run-${TIMESTAMP}"
mkdir -p "$RESULTS_DIR"

# Export so child scripts (deploy/verify.sh) can write their AI debugger log here
export RESULTS_DIR

# Export environment vars so child scripts inherit them
export RC_BASE_URL="${RC_BASE_URL:-http://192.168.31.23:8080/api/v1}"
export TEST_POD_ID="${TEST_POD_ID:-pod-8}"
export KIOSK_BASE_URL="${KIOSK_BASE_URL:-http://192.168.31.23:3300}"

# ─── Phase status tracking (bash 3 compatible -- no associative arrays) ──────
PREFLIGHT_EXIT=0
API_EXIT=0
WS_EXIT=0
BROWSER_EXIT=0
DEPLOY_EXIT=0
FLEET_HEALTH_EXIT=0

PREFLIGHT_STATUS="SKIP"
API_STATUS="SKIP"
WS_STATUS="SKIP"
BROWSER_STATUS="SKIP"
DEPLOY_STATUS="SKIP"
FLEET_HEALTH_STATUS="SKIP"

TOTAL_FAIL=0

# ─── Helper: run_phase ───────────────────────────────────────────────────────
# Usage: run_phase <name> <cmd> [args...]
# Runs the command, tees output to results/<name>.log, returns exit code.
# Uses PIPESTATUS[0] to get the command's exit code through tee.
run_phase() {
    local name="$1"; shift
    local logfile="$RESULTS_DIR/${name}.log"
    echo ""
    echo "========================================"
    echo "  Phase: ${name}"
    echo "========================================"
    "$@" 2>&1 | tee "$logfile"
    return "${PIPESTATUS[0]}"
}

# ─── Suite header ────────────────────────────────────────────────────────────
echo ""
echo "============================================================"
echo "  RaceControl E2E Test Suite"
echo "  $(date '+%Y-%m-%d %H:%M:%S IST')"
echo "  Base URL  : ${RC_BASE_URL}"
echo "  Pod ID    : ${TEST_POD_ID}"
echo "  Skip flags: deploy=${SKIP_DEPLOY} browser=${SKIP_BROWSER}"
echo "  Results   : ${RESULTS_DIR}"
echo "============================================================"

# ─── Phase 1: Preflight ──────────────────────────────────────────────────────
# Phase 1a: Smoke tests (API reachability + JSON responses)
run_phase "smoke" bash "$SCRIPT_DIR/smoke.sh"
SMOKE_EXIT="${PIPESTATUS[0]}"

if [ "$SMOKE_EXIT" -ne 0 ]; then
    echo ""
    echo "PREFLIGHT FAILED -- smoke tests returned ${SMOKE_EXIT} failures. Aborting remaining phases."
    PREFLIGHT_EXIT=$SMOKE_EXIT
    PREFLIGHT_STATUS="FAIL"
    TOTAL_FAIL=$((TOTAL_FAIL + SMOKE_EXIT))
else
    # Phase 1b: Cross-process integration checks (non-blocking — needs local DB + localhost)
    run_phase "cross-process" bash "$SCRIPT_DIR/cross-process.sh"
    CROSS_EXIT="${PIPESTATUS[0]}"

    if [ "$CROSS_EXIT" -ne 0 ]; then
        echo ""
        echo "NOTE: cross-process had ${CROSS_EXIT} failures (needs local DB + localhost — expected when running against remote server)"
        # Cross-process failures are non-blocking — it's a legacy script that checks localhost + local SQLite
        PREFLIGHT_STATUS="PASS"
    else
        PREFLIGHT_STATUS="PASS"
    fi
fi

# ─── Phase 2: API Tests (only if preflight passed) ───────────────────────────
if [ "$PREFLIGHT_STATUS" = "PASS" ]; then
    # Phase 2a: Billing lifecycle (API-01)
    run_phase "api-billing" bash "$SCRIPT_DIR/api/billing.sh"
    BILLING_EXIT="${PIPESTATUS[0]}"

    # Phase 2b: Game launch -- comprehensive version
    run_phase "api-launch" bash "$SCRIPT_DIR/game-launch.sh"
    LAUNCH_EXIT="${PIPESTATUS[0]}"

    # Phase 2c: Per-game launch/state lifecycle (API-02/03/04/05)
    run_phase "api-launch-pergame" bash "$SCRIPT_DIR/api/launch.sh"
    PERGAME_EXIT="${PIPESTATUS[0]}"

    # Phase 2d: Proxy error page (branded 502 when kiosk/dashboard is down)
    run_phase "api-proxy-error" bash "$SCRIPT_DIR/api/proxy-error.sh"
    PROXY_ERROR_EXIT="${PIPESTATUS[0]}"

    API_EXIT=$((BILLING_EXIT + LAUNCH_EXIT + PERGAME_EXIT + PROXY_ERROR_EXIT))
    TOTAL_FAIL=$((TOTAL_FAIL + API_EXIT))
    if [ "$API_EXIT" -eq 0 ]; then
        API_STATUS="PASS"
    else
        API_STATUS="FAIL"
    fi
fi

# ─── Phase 2.5: WebSocket Tests (only if preflight passed) ──────────────────
if [ "$PREFLIGHT_STATUS" = "PASS" ]; then
    run_phase "ws-smoke" bash "$SCRIPT_DIR/ws/ws-smoke.sh"
    WS_SMOKE_EXIT="${PIPESTATUS[0]}"

    run_phase "ws-churn" bash "$SCRIPT_DIR/ws/ws-churn.sh"
    WS_CHURN_EXIT="${PIPESTATUS[0]}"

    run_phase "ws-roundtrip" bash "$SCRIPT_DIR/ws/ws-roundtrip.sh"
    WS_ROUNDTRIP_EXIT="${PIPESTATUS[0]}"

    run_phase "ws-frontend-staleness" bash "$SCRIPT_DIR/ws/frontend-staleness.sh"
    WS_STALENESS_EXIT="${PIPESTATUS[0]}"

    WS_EXIT=$((WS_SMOKE_EXIT + WS_CHURN_EXIT + WS_ROUNDTRIP_EXIT + WS_STALENESS_EXIT))
    TOTAL_FAIL=$((TOTAL_FAIL + WS_EXIT))
    if [ "$WS_EXIT" -eq 0 ]; then
        WS_STATUS="PASS"
    else
        WS_STATUS="FAIL"
    fi
fi

# ─── Phase 3: Browser Tests (only if preflight passed, unless --skip-browser) ─
if [ "$PREFLIGHT_STATUS" = "PASS" ] && [ "$SKIP_BROWSER" = "false" ]; then
    run_phase "browser" npx playwright test --config "$REPO_ROOT/playwright.config.ts"
    BROWSER_EXIT="${PIPESTATUS[0]}"
    TOTAL_FAIL=$((TOTAL_FAIL + BROWSER_EXIT))
    if [ "$BROWSER_EXIT" -eq 0 ]; then
        BROWSER_STATUS="PASS"
    else
        BROWSER_STATUS="FAIL"
    fi
elif [ "$SKIP_BROWSER" = "true" ]; then
    BROWSER_STATUS="SKIP"
    echo ""
    echo "  Phase: browser -- SKIPPED (--skip-browser)"
fi

# ─── Phase 4: Deploy Verification (only if preflight passed, unless --skip-deploy) ─
if [ "$PREFLIGHT_STATUS" = "PASS" ] && [ "$SKIP_DEPLOY" = "false" ]; then
    run_phase "deploy" bash "$SCRIPT_DIR/deploy/verify.sh"
    DEPLOY_EXIT="${PIPESTATUS[0]}"
    TOTAL_FAIL=$((TOTAL_FAIL + DEPLOY_EXIT))
    if [ "$DEPLOY_EXIT" -eq 0 ]; then
        DEPLOY_STATUS="PASS"
    else
        DEPLOY_STATUS="FAIL"
    fi
elif [ "$SKIP_DEPLOY" = "true" ]; then
    DEPLOY_STATUS="SKIP"
    echo ""
    echo "  Phase: deploy -- SKIPPED (--skip-deploy)"
fi

# ─── Phase 5: Fleet Health (final gate -- Phase 50 self-test on all pods) ────
if [ "$PREFLIGHT_STATUS" = "PASS" ] && [ "$SKIP_DEPLOY" = "false" ]; then
    run_phase "fleet-health" bash "$SCRIPT_DIR/fleet/pod-health.sh"
    FLEET_HEALTH_EXIT="${PIPESTATUS[0]}"
    TOTAL_FAIL=$((TOTAL_FAIL + FLEET_HEALTH_EXIT))
    if [ "$FLEET_HEALTH_EXIT" -eq 0 ]; then
        FLEET_HEALTH_STATUS="PASS"
    else
        FLEET_HEALTH_STATUS="FAIL"
    fi
else
    FLEET_HEALTH_STATUS="SKIP"
    echo ""
    echo "  Phase: fleet-health -- SKIPPED"
fi

# ─── Summary Table ───────────────────────────────────────────────────────────
echo ""
echo "============================================================"
echo "  E2E Test Suite Summary"
echo "============================================================"
echo ""
printf "  %-20s %s\n" "Phase" "Status"
printf "  %-20s %s\n" "--------------------" "------"
printf "  %-20s %s\n" "Preflight" "$PREFLIGHT_STATUS"
printf "  %-20s %s\n" "API Tests" "$API_STATUS"
printf "  %-20s %s\n" "WebSocket Tests" "$WS_STATUS"
printf "  %-20s %s\n" "Browser Tests" "$BROWSER_STATUS"
printf "  %-20s %s\n" "Deploy Verify" "$DEPLOY_STATUS"
printf "  %-20s %s\n" "Fleet Health" "$FLEET_HEALTH_STATUS"
echo ""
echo "  Total failures: ${TOTAL_FAIL}"
echo "  Results dir:    ${RESULTS_DIR}"
echo "============================================================"

# ─── Write summary.json (DEPL-03) ────────────────────────────────────────────
mkdir -p "$RESULTS_DIR" 2>/dev/null
# Convert MSYS /c/ path to C:/ for Python on Windows
RESULTS_DIR_PY=$(echo "$RESULTS_DIR" | sed 's|^/c/|C:/|; s|^/d/|D:/|')
python3 -c "
import json, os
summary = {
    'timestamp': '${TIMESTAMP}',
    'phases': {
        'preflight': {'status': '${PREFLIGHT_STATUS}', 'exit_code': ${PREFLIGHT_EXIT}},
        'api': {'status': '${API_STATUS}', 'exit_code': ${API_EXIT}},
        'websocket': {'status': '${WS_STATUS}', 'exit_code': ${WS_EXIT}},
        'browser': {'status': '${BROWSER_STATUS}', 'exit_code': ${BROWSER_EXIT}},
        'deploy': {'status': '${DEPLOY_STATUS}', 'exit_code': ${DEPLOY_EXIT}},
        'fleet_health': {'status': '${FLEET_HEALTH_STATUS}', 'exit_code': ${FLEET_HEALTH_EXIT}}
    },
    'total_fail': ${TOTAL_FAIL}
}
outdir = '${RESULTS_DIR_PY}'
os.makedirs(outdir, exist_ok=True)
with open(os.path.join(outdir, 'summary.json'), 'w') as f:
    json.dump(summary, f, indent=2)
print('  Summary written to: ' + outdir + '/summary.json')
"

# ─── Final exit ──────────────────────────────────────────────────────────────
if [ "$TOTAL_FAIL" -eq 0 ]; then
    echo ""
    echo "  ALL PHASES PASSED"
fi
exit "$TOTAL_FAIL"
