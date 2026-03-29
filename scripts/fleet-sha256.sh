#!/usr/bin/env bash
# fleet-sha256.sh — SHA256 fleet binary verification (GAP-10)
#
# Verifies that all deployed binaries match expected SHA256 hashes.
# Computes local hash from staging, then checks each pod + server.
#
# Usage:
#   bash scripts/fleet-sha256.sh                    # verify all
#   bash scripts/fleet-sha256.sh --binary rc-agent   # verify specific binary

set -e

# Configuration
STAGING_DIR="C:/Users/bono/racingpoint/deploy-staging"
SERVER_IP="192.168.31.23"
SERVER_TS="100.125.108.37"
SENTRY_PORT=8091
POD_IPS=(192.168.31.89 192.168.31.33 192.168.31.28 192.168.31.88 192.168.31.86 192.168.31.87 192.168.31.38 192.168.31.91)
POD_NAMES=(Pod-1 Pod-2 Pod-3 Pod-4 Pod-5 Pod-6 Pod-7 Pod-8)

BINARY="${2:-rc-agent}"
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
RESET='\033[0m'

echo "============================================================"
echo "SHA256 Fleet Verification — ${BINARY}"
echo "============================================================"
echo ""

# Step 1: Compute expected hash from staging
STAGING_BINARY="${STAGING_DIR}/${BINARY}.exe"
if [ ! -f "$STAGING_BINARY" ]; then
  echo -e "${RED}ERROR: Staging binary not found: ${STAGING_BINARY}${RESET}"
  exit 1
fi

EXPECTED_HASH=$(sha256sum "$STAGING_BINARY" | awk '{print $1}')
EXPECTED_SIZE=$(wc -c < "$STAGING_BINARY")
echo "Expected SHA256: ${EXPECTED_HASH}"
echo "Expected size:   ${EXPECTED_SIZE} bytes"
echo ""

PASS=0
FAIL=0
SKIP=0

# Step 2: Verify server
echo "--- Server (.23) ---"
SERVER_HASH=$(ssh -o ConnectTimeout=5 ADMIN@${SERVER_TS} "certutil -hashfile C:\\RacingPoint\\${BINARY}.exe SHA256 2>nul | findstr /v hash | findstr /v Cert" 2>/dev/null | tr -d '\r\n ' || echo "UNREACHABLE")
if [ "$SERVER_HASH" = "UNREACHABLE" ]; then
  echo -e "  ${YELLOW}SKIP: Server unreachable${RESET}"
  ((SKIP++))
elif [ "$SERVER_HASH" = "$EXPECTED_HASH" ]; then
  echo -e "  ${GREEN}PASS: Hash matches${RESET}"
  ((PASS++))
else
  echo -e "  ${RED}FAIL: Hash mismatch${RESET}"
  echo "    Got:      ${SERVER_HASH}"
  echo "    Expected: ${EXPECTED_HASH}"
  ((FAIL++))
fi

# Step 3: Verify pods via rc-sentry exec
echo ""
for i in "${!POD_IPS[@]}"; do
  pod_ip="${POD_IPS[$i]}"
  pod_name="${POD_NAMES[$i]}"
  echo "--- ${pod_name} (${pod_ip}) ---"

  # Use rc-sentry exec to get hash
  RESULT=$(curl -s --connect-timeout 3 -X POST "http://${pod_ip}:${SENTRY_PORT}/exec" \
    -H "Content-Type: application/json" \
    -d "{\"cmd\":\"certutil -hashfile C:\\\\RacingPoint\\\\${BINARY}.exe SHA256 2>nul | findstr /v hash | findstr /v Cert\"}" \
    2>/dev/null || echo '{"stdout":"UNREACHABLE"}')

  POD_HASH=$(echo "$RESULT" | grep -oP '"stdout"\s*:\s*"([^"]*)"' | head -1 | sed 's/.*"stdout"\s*:\s*"//;s/".*//' | tr -d '\r\n ')

  if [ -z "$POD_HASH" ] || [ "$POD_HASH" = "UNREACHABLE" ]; then
    echo -e "  ${YELLOW}SKIP: Pod unreachable or rc-sentry down${RESET}"
    ((SKIP++))
  elif [ "$POD_HASH" = "$EXPECTED_HASH" ]; then
    echo -e "  ${GREEN}PASS: Hash matches${RESET}"
    ((PASS++))
  else
    echo -e "  ${RED}FAIL: Hash mismatch${RESET}"
    echo "    Got:      ${POD_HASH}"
    echo "    Expected: ${EXPECTED_HASH}"
    ((FAIL++))
  fi
done

# Summary
echo ""
echo "============================================================"
echo "RESULTS: ${PASS} PASS | ${FAIL} FAIL | ${SKIP} SKIP"
echo "============================================================"

if [ $FAIL -gt 0 ]; then
  echo -e "${RED}VERIFICATION FAILED — ${FAIL} machine(s) have mismatched binaries${RESET}"
  exit 1
fi

echo -e "${GREEN}All reachable machines verified${RESET}"
exit 0
