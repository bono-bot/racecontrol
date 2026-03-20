#!/bin/bash
# =============================================================================
# Auto-Start Liveness Verification
#
# Checks that the two auto-start services on James's machine (.27) are
# accepting connections after boot or after Task Scheduler triggers them.
#
# Usage:
#   bash tests/e2e/deploy/auto-start.sh
#
# Services verified:
#   :9998  — Staging HTTP server (serves deploy-staging/ for pod binary downloads)
#   :9999  — Web terminal (Uday's phone terminal access)
#
# Exit code: number of failed gates (0 = all passed)
# =============================================================================

set -uo pipefail

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)

# shellcheck source=../lib/common.sh
source "$SCRIPT_DIR/../lib/common.sh"

JAMES_IP="192.168.31.27"

echo "========================================"
echo "Auto-Start Liveness Check"
echo "James machine: ${JAMES_IP}"
echo "========================================"
echo ""

# ─── Gate 1: Staging HTTP Server (:9998) ──────────────────────────────────
echo "--- Gate 1: Staging HTTP Server (:9998) ---"

if curl -sf --max-time 5 "http://${JAMES_IP}:9998/" > /dev/null 2>&1; then
    pass "Staging HTTP server responding on :9998 (deploy-staging/ directory served)"
else
    fail "Staging HTTP server NOT responding on :9998 — Task RacingPoint-StagingHTTP may not have started"
fi

# ─── Gate 2: Web Terminal (:9999) ─────────────────────────────────────────
echo ""
echo "--- Gate 2: Web Terminal (:9999) ---"

if curl -sf --max-time 5 "http://${JAMES_IP}:9999/" > /dev/null 2>&1; then
    pass "Web terminal responding on :9999 (Uday phone access available)"
else
    fail "Web terminal NOT responding on :9999 — Task RacingPoint-WebTerm may not have started"
fi

# ─── Summary ───────────────────────────────────────────────────────────────
echo ""
summary_exit "Auto-Start" 2
