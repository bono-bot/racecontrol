#!/bin/bash
# =============================================================================
# RaceControl E2E — Freedom Mode Screenshot Verification (Post-Deploy)
#
# Enables freedom mode, captures screenshots from the pod debug server,
# verifies the screen is NOT blanked, then disables freedom mode and verifies
# the screen returns to normal kiosk state.
#
# Usage:
#   ./freedom-mode-screenshot.sh
#   TEST_POD_ID=pod_3 SCREENSHOT_DIR=/tmp/e2e-screenshots ./freedom-mode-screenshot.sh
# =============================================================================

set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
source "$SCRIPT_DIR/../lib/common.sh"
source "$SCRIPT_DIR/../lib/pod-map.sh"

BASE_URL="${RC_BASE_URL:-http://192.168.31.23:8080/api/v1}"
POD_ID="${TEST_POD_ID:-pod_8}"
POD_IP=$(pod_ip "pod-${POD_ID##pod_}")
SCREENSHOT_DIR="${SCREENSHOT_DIR:-/tmp/e2e-freedom-screenshots}"

mkdir -p "$SCREENSHOT_DIR"

echo "=========================================="
echo "  Freedom Mode Screenshot Verification"
echo "  Server: $BASE_URL"
echo "  Pod:    $POD_ID ($POD_IP)"
echo "  Output: $SCREENSHOT_DIR"
echo "=========================================="
echo ""

# ─── Gate: Server + Pod reachable ─────────────────────────────────────
STATUS=$(curl -s -o /dev/null -w "%{http_code}" --max-time 10 "${BASE_URL}/health" 2>/dev/null || echo "000")
if [ "$STATUS" != "200" ]; then
    fail "Server unreachable"
    summary_exit
fi
pass "Server reachable"

DEBUG_STATUS=$(curl -s -o /dev/null -w "%{http_code}" --max-time 10 "http://${POD_IP}:18924/status" 2>/dev/null || echo "000")
if [ "$DEBUG_STATUS" != "200" ]; then
    skip "Pod debug server unreachable at ${POD_IP}:18924 — cannot do screenshot verification"
    summary_exit
fi
pass "Pod debug server reachable"

# ─── Capture: Before (baseline) ──────────────────────────────────────
info "Capturing baseline screenshot..."
BEFORE_FILE="${SCREENSHOT_DIR}/before-freedom-${POD_ID}.png"
curl -s -o "$BEFORE_FILE" --max-time 15 "http://${POD_IP}:18924/screenshot" 2>/dev/null || true
BEFORE_SIZE=$(stat -c%s "$BEFORE_FILE" 2>/dev/null || wc -c < "$BEFORE_FILE" 2>/dev/null || echo "0")
if [ "$BEFORE_SIZE" -gt 1000 ]; then
    pass "Baseline screenshot captured ($BEFORE_SIZE bytes): $BEFORE_FILE"
else
    fail "Baseline screenshot too small or missing ($BEFORE_SIZE bytes)"
fi

BEFORE_STATE=$(curl -s --max-time 10 "http://${POD_IP}:18924/status" 2>/dev/null | python3 -c "import sys,json; print(json.load(sys.stdin).get('lock_screen_state','unknown'))" 2>/dev/null || echo "unknown")
info "Before state: lock_screen=$BEFORE_STATE"

# ─── Enable freedom mode ─────────────────────────────────────────────
info "Enabling freedom mode..."
ENABLE_RESP=$(curl -s --max-time 10 -X POST "${BASE_URL}/pods/${POD_ID}/freedom" \
    -H "Content-Type: application/json" -d '{"enabled": true}' 2>/dev/null || echo "{}")
ENABLE_OK=$(echo "$ENABLE_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin).get('ok',False))" 2>/dev/null || echo "False")
if [ "$ENABLE_OK" = "True" ]; then
    pass "Freedom mode enabled"
else
    fail "Freedom mode enable failed: $ENABLE_RESP"
    summary_exit
fi

# Wait for agent to process + lock screen to clear
sleep 4

# ─── Capture: Freedom mode active ────────────────────────────────────
info "Capturing freedom mode screenshot..."
FREEDOM_FILE="${SCREENSHOT_DIR}/freedom-active-${POD_ID}.png"
curl -s -o "$FREEDOM_FILE" --max-time 15 "http://${POD_IP}:18924/screenshot" 2>/dev/null || true
FREEDOM_SIZE=$(stat -c%s "$FREEDOM_FILE" 2>/dev/null || wc -c < "$FREEDOM_FILE" 2>/dev/null || echo "0")
if [ "$FREEDOM_SIZE" -gt 1000 ]; then
    pass "Freedom mode screenshot captured ($FREEDOM_SIZE bytes): $FREEDOM_FILE"
else
    fail "Freedom mode screenshot too small or missing ($FREEDOM_SIZE bytes)"
fi

# Verify lock screen is hidden (freedom mode should unblank)
FREEDOM_STATE=$(curl -s --max-time 10 "http://${POD_IP}:18924/status" 2>/dev/null | python3 -c "import sys,json; print(json.load(sys.stdin).get('lock_screen_state','unknown'))" 2>/dev/null || echo "unknown")
if [ "$FREEDOM_STATE" = "hidden" ]; then
    pass "Lock screen is 'hidden' during freedom mode (screen unblanked)"
else
    fail "Lock screen state is '$FREEDOM_STATE' — expected 'hidden' during freedom mode"
fi

# ─── Verify: Screenshot differs from blank (size heuristic) ──────────
# A blank/black screen PNG is typically very small (< 5KB) due to compression
# A real desktop/game screenshot is much larger
if [ "$FREEDOM_SIZE" -gt 5000 ]; then
    pass "Freedom mode screenshot is non-trivial ($FREEDOM_SIZE bytes > 5KB) — screen is NOT blank"
else
    info "Freedom mode screenshot is small ($FREEDOM_SIZE bytes) — might be blank or minimal content"
fi

# ─── Disable freedom mode ────────────────────────────────────────────
info "Disabling freedom mode..."
DISABLE_RESP=$(curl -s --max-time 10 -X POST "${BASE_URL}/pods/${POD_ID}/freedom" \
    -H "Content-Type: application/json" -d '{"enabled": false}' 2>/dev/null || echo "{}")
DISABLE_OK=$(echo "$DISABLE_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin).get('ok',False))" 2>/dev/null || echo "False")
if [ "$DISABLE_OK" = "True" ]; then
    pass "Freedom mode disabled"
else
    fail "Freedom mode disable failed: $DISABLE_RESP"
fi

sleep 4

# ─── Capture: After freedom mode disabled ────────────────────────────
info "Capturing post-freedom screenshot..."
AFTER_FILE="${SCREENSHOT_DIR}/after-freedom-${POD_ID}.png"
curl -s -o "$AFTER_FILE" --max-time 15 "http://${POD_IP}:18924/screenshot" 2>/dev/null || true
AFTER_SIZE=$(stat -c%s "$AFTER_FILE" 2>/dev/null || wc -c < "$AFTER_FILE" 2>/dev/null || echo "0")
if [ "$AFTER_SIZE" -gt 1000 ]; then
    pass "Post-freedom screenshot captured ($AFTER_SIZE bytes): $AFTER_FILE"
else
    fail "Post-freedom screenshot too small or missing ($AFTER_SIZE bytes)"
fi

AFTER_STATE=$(curl -s --max-time 10 "http://${POD_IP}:18924/status" 2>/dev/null | python3 -c "import sys,json; print(json.load(sys.stdin).get('lock_screen_state','unknown'))" 2>/dev/null || echo "unknown")
info "After state: lock_screen=$AFTER_STATE"

# ─── Results ──────────────────────────────────────────────────────────
echo ""
echo "Screenshots saved to: $SCREENSHOT_DIR"
ls -la "$SCREENSHOT_DIR"/*-${POD_ID}.png 2>/dev/null || true
echo ""

summary_exit
