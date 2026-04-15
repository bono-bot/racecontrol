#!/bin/bash
# deploy-preflight.sh — Pre-flight checks before pod deployment
# MMA consensus: runs key parity + staging server + pod readiness in one gate
#
# Usage:
#   SENTRY_KEY="auto" bash scripts/deploy-preflight.sh <hash>
#   SENTRY_KEY="478a..." bash scripts/deploy-preflight.sh <hash>
#
# "auto" reads key from server's racecontrol.toml via SSH

set -euo pipefail

HASH="${1:?Usage: deploy-preflight.sh <build_hash>}"
STAGING_PORT="${STAGING_PORT:-18889}"
TOML_HOST="${TOML_HOST:-ADMIN@100.125.108.37}"

# Pod IPs
PODS=(
  "192.168.31.89"
  "192.168.31.33"
  "192.168.31.28"
  "192.168.31.88"
  "192.168.31.86"
  "192.168.31.87"
  "192.168.31.38"
  "192.168.31.91"
)

echo "=== Deploy Pre-Flight: ${HASH} ==="
FAIL=0

# ── Check 1: Sentry key parity ──────────────────────────────────
echo ""
echo "--- Check 1: Sentry Key Parity ---"

if [ "${SENTRY_KEY:-}" = "auto" ] || [ -z "${SENTRY_KEY:-}" ]; then
  echo "Reading key from server racecontrol.toml..."
  SENTRY_KEY=$(ssh -o ConnectTimeout=5 "$TOML_HOST" \
    "findstr sentry_service_key C:\\RacingPoint\\racecontrol.toml" 2>/dev/null | \
    sed -n 's/.*"\([^"]*\)".*/\1/p' | head -1)
  if [ -z "$SENTRY_KEY" ]; then
    echo "FAIL: Could not read sentry_service_key from server"
    FAIL=$((FAIL + 1))
  else
    echo "Server key: ${SENTRY_KEY:0:12}..."
  fi
fi

KEY_MISMATCHES=0
for pod_ip in "${PODS[@]}"; do
  # Bug E fix: /ping is public (always 200) — use /exec with benign command to test key
  AUTH_RESULT=$(curl -s --connect-timeout 3 --max-time 5 \
    -H "X-Service-Key: ${SENTRY_KEY}" \
    -H "Content-Type: application/json" \
    -d '{"cmd":"echo AUTH_OK"}' \
    "http://${pod_ip}:8091/exec" 2>/dev/null || echo "")

  if echo "$AUTH_RESULT" | grep -q "AUTH_OK"; then
    echo "  ${pod_ip}: KEY_OK (authed exec)"
  elif echo "$AUTH_RESULT" | grep -qi "blocked\|unauthorized\|forbidden"; then
    echo "  ${pod_ip}: KEY_MISMATCH"
    KEY_MISMATCHES=$((KEY_MISMATCHES + 1))
  elif [ -z "$AUTH_RESULT" ]; then
    echo "  ${pod_ip}: UNREACHABLE"
    # Not a key failure — pod may not be booted yet
  else
    echo "  ${pod_ip}: KEY_MISMATCH (unexpected: ${AUTH_RESULT:0:60})"
    KEY_MISMATCHES=$((KEY_MISMATCHES + 1))
  fi
done

if [ "$KEY_MISMATCHES" -gt 0 ]; then
  echo "FAIL: ${KEY_MISMATCHES} pod(s) have mismatched sentry keys"
  echo "Fix: Update RCSENTRY_SERVICE_KEY env var on those pods"
  FAIL=$((FAIL + 1))
else
  echo "PASS: All reachable pods accept the server key"
fi

# ── Check 2: Staging server binary available ─────────────────────
echo ""
echo "--- Check 2: Staging Server Binary ---"

BINARY_NAME="rc-agent-${HASH}.exe"
ACTUAL_SIZE=$(curl -sI "http://localhost:${STAGING_PORT}/${BINARY_NAME}" 2>/dev/null | \
  grep -i content-length | awk '{print $2}' | tr -d '\r')

if [ -z "$ACTUAL_SIZE" ] || [ "${ACTUAL_SIZE:-0}" -lt 1000000 ] 2>/dev/null; then
  echo "FAIL: ${BINARY_NAME} not available or too small (${ACTUAL_SIZE:-0} bytes)"
  echo "  Start staging server: bash scripts/start-staging-server.sh"
  FAIL=$((FAIL + 1))
else
  echo "PASS: ${BINARY_NAME} = ${ACTUAL_SIZE} bytes"
fi

# ── Check 3: Server health ──────────────────────────────────────
echo ""
echo "--- Check 3: Server Health ---"

SERVER_BUILD=$(curl -sf --connect-timeout 5 "http://192.168.31.23:8080/api/v1/health" 2>/dev/null | \
  python3 -c "import sys,json; print(json.load(sys.stdin).get('build_id','UNKNOWN'))" 2>/dev/null || echo "UNREACHABLE")

if [ "$SERVER_BUILD" = "UNREACHABLE" ]; then
  echo "FAIL: Server is not reachable"
  FAIL=$((FAIL + 1))
else
  echo "PASS: Server running build ${SERVER_BUILD}"
fi

# ── Summary ─────────────────────────────────────────────────────
echo ""
echo "=== Pre-Flight Summary ==="
if [ "$FAIL" -gt 0 ]; then
  echo "BLOCKED: ${FAIL} check(s) failed. Fix before deploying."
  exit 1
else
  echo "ALL CLEAR: Ready to deploy ${HASH}"
  echo ""
  echo "Sentry key for deploy commands:"
  echo "  SENTRY_KEY=\"${SENTRY_KEY}\""
  exit 0
fi
