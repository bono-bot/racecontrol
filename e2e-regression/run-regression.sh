#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════
# E2E Regression Test Orchestrator
# Runs from James (.27) — deploys to POS (.20) and Server (.23)
# ═══════════════════════════════════════════════════════════════

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
POS_IP="192.168.31.20"
POS_USER="POS"
SERVER_IP="192.168.31.23"
SERVER_USER="ADMIN"
REMOTE_DIR="C:/Users/\$USER/e2e-regression"
RESULTS_DIR="$SCRIPT_DIR/results-$(date +%Y%m%d-%H%M%S)"

echo "═══════════════════════════════════════════════════════"
echo " Racing Point E2E Regression Test Suite"
echo " $(date '+%Y-%m-%d %H:%M IST')"
echo "═══════════════════════════════════════════════════════"

# ─── Step 0: Install dependencies locally ────────────────
echo ""
echo "Step 0: Installing dependencies..."
cd "$SCRIPT_DIR"
npm install 2>/dev/null || true
npx playwright install chromium 2>/dev/null || true

# ─── Step 1: Pre-flight checks ──────────────────────────
echo ""
echo "Step 1: Pre-flight checks..."

# Check server health
echo "  Checking server health..."
SERVER_HEALTH=$(curl -s --connect-timeout 5 "http://$SERVER_IP:8080/api/v1/health" 2>/dev/null || echo '{"ok":false}')
echo "  Server: $SERVER_HEALTH"

# Check POS reachability
echo "  Checking POS PC..."
POS_PING=$(ping -c 1 -W 2 "$POS_IP" 2>/dev/null && echo "OK" || echo "UNREACHABLE")
echo "  POS: $POS_PING"

# Check fleet
echo "  Checking fleet..."
FLEET=$(curl -s --connect-timeout 5 "http://$SERVER_IP:8080/api/v1/fleet/health" 2>/dev/null || echo '[]')
POD_COUNT=$(echo "$FLEET" | python3 -c "import sys,json; d=json.load(sys.stdin); print(len([p for p in d if p.get('ws_connected')]))" 2>/dev/null || echo "0")
echo "  Connected pods: $POD_COUNT"

# ─── Step 2: Deploy test suite to POS and Server ────────
echo ""
echo "Step 2: Deploying test suite..."

# Deploy to POS
echo "  Deploying to POS ($POS_IP)..."
ssh -o ConnectTimeout=5 "$POS_USER@$POS_IP" "mkdir -p C:/Users/POS/e2e-regression" 2>/dev/null || true
scp -r "$SCRIPT_DIR"/{package.json,tsconfig.json,playwright.config.ts,fixtures,lib,tests} "$POS_USER@$POS_IP:C:/Users/POS/e2e-regression/" 2>/dev/null || echo "  WARNING: POS deploy failed"

# Deploy to Server
echo "  Deploying to Server ($SERVER_IP)..."
ssh -o ConnectTimeout=5 "$SERVER_USER@$SERVER_IP" "mkdir -p C:/Users/ADMIN/e2e-regression" 2>/dev/null || true
scp -r "$SCRIPT_DIR"/{package.json,tsconfig.json,playwright.config.ts,fixtures,lib,tests} "$SERVER_USER@$SERVER_IP:C:/Users/ADMIN/e2e-regression/" 2>/dev/null || echo "  WARNING: Server deploy failed"

# Install deps on remote machines
echo "  Installing deps on POS..."
ssh "$POS_USER@$POS_IP" "cd C:/Users/POS/e2e-regression && npm install && npx playwright install chromium" 2>/dev/null || echo "  WARNING: POS npm install failed"

echo "  Installing deps on Server..."
ssh "$SERVER_USER@$SERVER_IP" "cd C:/Users/ADMIN/e2e-regression && npm install && npx playwright install chromium" 2>/dev/null || echo "  WARNING: Server npm install failed"

# ─── Step 3: Run tests ──────────────────────────────────
echo ""
echo "Step 3: Running tests..."
mkdir -p "$RESULTS_DIR"

# Option A: Run locally from James (simpler — hits same URLs)
echo "  Running full suite from James..."
cd "$SCRIPT_DIR"
npx playwright test --reporter=list,html 2>&1 | tee "$RESULTS_DIR/test-output.log"

# ─── Step 4: Collect results ────────────────────────────
echo ""
echo "Step 4: Collecting results..."

# Copy local results
cp -r "$SCRIPT_DIR/test-results" "$RESULTS_DIR/" 2>/dev/null || true
cp -r "$SCRIPT_DIR/evidence" "$RESULTS_DIR/" 2>/dev/null || true

# Collect from POS
scp -r "$POS_USER@$POS_IP:C:/Users/POS/e2e-regression/test-results" "$RESULTS_DIR/pos-results/" 2>/dev/null || true

# Collect from Server
scp -r "$SERVER_USER@$SERVER_IP:C:/Users/ADMIN/e2e-regression/test-results" "$RESULTS_DIR/server-results/" 2>/dev/null || true

# ─── Step 5: Summary ────────────────────────────────────
echo ""
echo "═══════════════════════════════════════════════════════"
echo " TEST RUN COMPLETE"
echo " Results: $RESULTS_DIR"
echo " HTML Report: $RESULTS_DIR/test-results/html/index.html"
echo " Evidence: $RESULTS_DIR/evidence/"
echo "═══════════════════════════════════════════════════════"
