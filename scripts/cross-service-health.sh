#!/bin/bash

# cross-service-health.sh — Multi-service health check with cross-proxy verification
#
# Checks each service directly and verifies cross-service proxy chains.
# Reports pass/fail for all running services with a summary.
#
# Usage:
#   bash scripts/cross-service-health.sh
#
# Exit code = number of failures (0 = all healthy)

# ANSI colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BOLD='\033[1m'
RESET='\033[0m'

PASSED=0
FAILED=0
SKIPPED=0

# Helper: check a service endpoint
check_service() {
  local label=$1
  local url=$2
  local required=${3:-true}  # true = failure counts, false = skip if down

  if curl -sf --max-time 5 "$url" > /dev/null 2>&1; then
    echo -e "  ${GREEN}PASS${RESET}  ${label}  (${url})"
    PASSED=$((PASSED + 1))
  else
    if [ "$required" = "true" ]; then
      echo -e "  ${RED}FAIL${RESET}  ${label}  (${url})"
      FAILED=$((FAILED + 1))
    else
      echo -e "  ${YELLOW}SKIP${RESET}  ${label}  (${url}) — not running"
      SKIPPED=$((SKIPPED + 1))
    fi
  fi
}

echo ""
echo -e "${BOLD}Cross-Service Health Check${RESET}"
echo -e "${BOLD}=========================${RESET}"
echo ""

# --- Direct service checks ---
echo -e "${BOLD}Direct Services:${RESET}"

check_service "rc-core" "http://localhost:8080/api/v1/health"
check_service "api-gateway" "http://localhost:3100/api/health"
check_service "pwa" "http://localhost:3500" "false"
check_service "admin" "http://localhost:3200" "false"
check_service "dashboard" "http://localhost:3400" "false"

echo ""

# --- Cross-service proxy chain checks ---
echo -e "${BOLD}Proxy Chains:${RESET}"

# Admin -> rc-core proxy
check_service "admin -> rc-core" "http://localhost:3200/api/rc/health" "false"

# Dashboard -> rc-core proxy
check_service "dashboard -> rc-core" "http://localhost:3400/api/rc/health" "false"

echo ""

# --- Summary ---
TOTAL=$((PASSED + FAILED + SKIPPED))
echo -e "${BOLD}Results:${RESET} ${GREEN}${PASSED} passed${RESET}, ${RED}${FAILED} failed${RESET}, ${YELLOW}${SKIPPED} skipped${RESET} (${TOTAL} total)"

if [ "$FAILED" -gt 0 ]; then
  echo -e "${RED}${BOLD}Some services are unhealthy!${RESET}"
fi

exit "$FAILED"
