#!/bin/bash
# verify-frontend-deploy.sh — Phase 262 post-deploy verification gate
# Usage: bash verify-frontend-deploy.sh [repo_root] [server_ip]
#   repo_root: path to racecontrol repo (default: /c/Users/bono/racingpoint/racecontrol)
#   server_ip: server IP (default: 192.168.31.23)
# Exits 0 if all checks pass. Exits 1 if any check fails.
# Run after every frontend deploy to confirm pipeline is correct.
set -uo pipefail

REPO_ROOT="${1:-/c/Users/bono/racingpoint/racecontrol}"
SERVER_IP="${2:-192.168.31.23}"
FAIL=0
PASS_COUNT=0
TOTAL=5

echo "=============================="
echo "Phase 262 Deploy Gate"
echo "=============================="
echo "  Repo:   $REPO_ROOT"
echo "  Server: $SERVER_IP"
echo ""

# =========================================================================
# [CHECK 1/5] Web static files served (HTTP 200)
# =========================================================================
echo "[CHECK 1/5] Web static files served (HTTP 200)"

WEB_CSS=$(ls "$REPO_ROOT/web/.next/static/css/"*.css 2>/dev/null | head -1 | xargs basename 2>/dev/null || echo "")
if [ -z "$WEB_CSS" ]; then
  WEB_CSS="app.css"
  echo "  (No local CSS found, using default name: $WEB_CSS)"
fi

WEB_STATIC_URL="http://$SERVER_IP:3200/_next/static/css/$WEB_CSS"
echo "  Testing: $WEB_STATIC_URL"
WEB_STATIC_CODE=$(curl -s -o /dev/null -w "%{http_code}" --max-time 10 "$WEB_STATIC_URL" 2>/dev/null || echo "000")

if [ "$WEB_STATIC_CODE" = "200" ]; then
  echo "  PASS: HTTP $WEB_STATIC_CODE"
  PASS_COUNT=$((PASS_COUNT + 1))
else
  echo "  FAIL: HTTP $WEB_STATIC_CODE (expected 200)"
  echo "  FIX: cp -r web/.next/static web/.next/standalone/.next/static && restart web"
  FAIL=1
fi
echo ""

# =========================================================================
# [CHECK 2/5] Kiosk static files served (HTTP 200)
# =========================================================================
echo "[CHECK 2/5] Kiosk static files served (HTTP 200)"

KIOSK_CSS=$(ls "$REPO_ROOT/kiosk/.next/static/css/"*.css 2>/dev/null | head -1 | xargs basename 2>/dev/null || echo "")
if [ -z "$KIOSK_CSS" ]; then
  KIOSK_CSS="app.css"
  echo "  (No local CSS found, using default name: $KIOSK_CSS)"
fi

KIOSK_STATIC_URL="http://$SERVER_IP:3300/kiosk/_next/static/css/$KIOSK_CSS"
echo "  Testing: $KIOSK_STATIC_URL"
KIOSK_STATIC_CODE=$(curl -s -o /dev/null -w "%{http_code}" --max-time 10 "$KIOSK_STATIC_URL" 2>/dev/null || echo "000")

if [ "$KIOSK_STATIC_CODE" = "200" ]; then
  echo "  PASS: HTTP $KIOSK_STATIC_CODE"
  PASS_COUNT=$((PASS_COUNT + 1))
else
  echo "  FAIL: HTTP $KIOSK_STATIC_CODE (expected 200)"
  echo "  FIX: cp -r kiosk/.next/static kiosk/.next/standalone/.next/static && restart kiosk"
  FAIL=1
fi
echo ""

# =========================================================================
# [CHECK 3/5] NEXT_PUBLIC_ env vars have LAN IPs (not localhost)
# =========================================================================
echo "[CHECK 3/5] NEXT_PUBLIC_ env vars have LAN IPs (not localhost)"

CHECK3_FAIL=0

# Check web
WEB_ENV_OUTPUT=$(bash "$REPO_ROOT/scripts/deploy/check-frontend-env.sh" "$REPO_ROOT/web" 2>&1) || true
WEB_ENV_RC=$?
if [ "$WEB_ENV_RC" -eq 0 ]; then
  echo "  web: $WEB_ENV_OUTPUT"
else
  echo "  web: FAIL"
  echo "  $WEB_ENV_OUTPUT"
  CHECK3_FAIL=1
fi

# Check kiosk
KIOSK_ENV_OUTPUT=$(bash "$REPO_ROOT/scripts/deploy/check-frontend-env.sh" "$REPO_ROOT/kiosk" 2>&1) || true
KIOSK_ENV_RC=$?
if [ "$KIOSK_ENV_RC" -eq 0 ]; then
  echo "  kiosk: $KIOSK_ENV_OUTPUT"
else
  echo "  kiosk: FAIL"
  echo "  $KIOSK_ENV_OUTPUT"
  CHECK3_FAIL=1
fi

if [ "$CHECK3_FAIL" -eq 0 ]; then
  echo "  PASS: Both apps have NEXT_PUBLIC_ vars with LAN IPs"
  PASS_COUNT=$((PASS_COUNT + 1))
else
  echo "  FAIL: One or both apps have missing/localhost NEXT_PUBLIC_ vars"
  FAIL=1
fi
echo ""

# =========================================================================
# [CHECK 4/5] outputFileTracingRoot set in both next.config.ts
# =========================================================================
echo "[CHECK 4/5] outputFileTracingRoot set in both next.config.ts"

TRACE_COUNT=0
if grep -q "outputFileTracingRoot" "$REPO_ROOT/web/next.config.ts" 2>/dev/null; then
  echo "  web/next.config.ts: found"
  TRACE_COUNT=$((TRACE_COUNT + 1))
else
  echo "  web/next.config.ts: MISSING"
fi

if grep -q "outputFileTracingRoot" "$REPO_ROOT/kiosk/next.config.ts" 2>/dev/null; then
  echo "  kiosk/next.config.ts: found"
  TRACE_COUNT=$((TRACE_COUNT + 1))
else
  echo "  kiosk/next.config.ts: MISSING"
fi

if [ "$TRACE_COUNT" -eq 2 ]; then
  echo "  PASS: Both next.config.ts files have outputFileTracingRoot"
  PASS_COUNT=$((PASS_COUNT + 1))
else
  echo "  FAIL: $TRACE_COUNT/2 files have outputFileTracingRoot"
  echo "  FIX: Add this to the missing next.config.ts:"
  echo "    outputFileTracingRoot: path.join(__dirname)"
  FAIL=1
fi
echo ""

# =========================================================================
# [CHECK 5/5] /leaderboard-display is unauthenticated (200 not 302)
# =========================================================================
echo "[CHECK 5/5] /leaderboard-display is unauthenticated (200 not 302)"

LB_URL="http://$SERVER_IP:3200/leaderboard-display"
echo "  Testing: $LB_URL"
LB_CODE=$(curl -s -o /dev/null -w "%{http_code}" --max-time 10 --max-redirs 0 "$LB_URL" 2>/dev/null || echo "000")

if [ "$LB_CODE" = "200" ]; then
  echo "  PASS: HTTP $LB_CODE (unauthenticated, no redirect)"
  PASS_COUNT=$((PASS_COUNT + 1))
elif [ "$LB_CODE" = "301" ] || [ "$LB_CODE" = "302" ] || [ "$LB_CODE" = "307" ] || [ "$LB_CODE" = "308" ]; then
  echo "  FAIL: HTTP $LB_CODE (redirecting to login)"
  echo "  FIX: Remove AuthGate wrapper from leaderboard-display page. Do NOT wrap this route."
  FAIL=1
else
  echo "  FAIL: HTTP $LB_CODE (app may not be running)"
  echo "  FIX: Ensure web app is running on port 3200. Then re-run this check."
  FAIL=1
fi
echo ""

# =========================================================================
# Summary
# =========================================================================
echo "=============================="
echo "Phase 262 Deploy Gate Results"
echo "=============================="
echo "  Passed: $PASS_COUNT/$TOTAL"
if [ "$FAIL" -eq 0 ]; then
  echo "  Status: PASS — deploy pipeline fully verified"
  echo "=============================="
  exit 0
else
  FAIL_COUNT=$(( TOTAL - PASS_COUNT ))
  echo "  Status: FAIL — $FAIL_COUNT check(s) failed"
  echo "  Fix the issues above and re-run: bash scripts/deploy/verify-frontend-deploy.sh"
  echo "=============================="
  exit 1
fi
