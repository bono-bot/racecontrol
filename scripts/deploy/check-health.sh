#!/usr/bin/env bash
# check-health.sh — poll all Racing Point services, print pass/fail, exit non-zero on any failure
# Usage: bash check-health.sh
# Run from James (.27). comms-link checked on localhost, all others on server .23

set -euo pipefail

SERVER="192.168.31.23"
TIMEOUT=5
PASS=0
FAIL=0

check_service() {
  local name="$1"
  local url="$2"
  local response
  response=$(curl -sf --max-time "$TIMEOUT" "$url" 2>/dev/null || echo "")
  local status
  status=$(echo "$response" | grep -o '"status":"ok"' | head -1)
  if [ -n "$status" ]; then
    echo "  PASS  $name ($url)"
    PASS=$((PASS + 1))
  else
    echo "  FAIL  $name ($url)"
    FAIL=$((FAIL + 1))
  fi
}

echo "=== Racing Point Health Check $(date '+%Y-%m-%d %H:%M IST') ==="
echo ""

check_service "racecontrol   :8080" "http://${SERVER}:8080/api/v1/health"
check_service "kiosk         :3300" "http://${SERVER}:3300/kiosk/api/health"
check_service "web-dashboard :3200" "http://${SERVER}:3200/api/health"
check_service "comms-link    :8766" "http://localhost:8766/health"
check_service "rc-sentry     :8091" "http://${SERVER}:8091/health"

echo ""
echo "Results: ${PASS} passed, ${FAIL} failed"

if [ "$FAIL" -gt 0 ]; then
  echo "HEALTH CHECK FAILED — ${FAIL} service(s) down"
  exit 1
fi

echo "HEALTH CHECK PASSED — all services healthy"
exit 0
