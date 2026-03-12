#!/bin/bash
# =============================================================================
# RaceControl E2E Smoke Test
#
# Validates that all core API endpoints are reachable and returning expected
# status codes. Run against a live rc-core instance.
#
# Usage:
#   ./smoke.sh                          # defaults to localhost:8080/api/v1
#   RC_BASE_URL=https://rc.racingpoint.cloud/api/v1 ./smoke.sh
# =============================================================================

set -euo pipefail

BASE_URL="${RC_BASE_URL:-http://localhost:8080/api/v1}"
PASS=0
FAIL=0
TOTAL=0

# Colors (disable if not a terminal)
if [ -t 1 ]; then
    GREEN='\033[0;32m'
    RED='\033[0;31m'
    YELLOW='\033[0;33m'
    NC='\033[0m'
else
    GREEN=''
    RED=''
    YELLOW=''
    NC=''
fi

check() {
    local endpoint="$1"
    local expected_status="$2"
    local description="${3:-$endpoint}"
    TOTAL=$((TOTAL + 1))

    STATUS=$(curl -s -o /dev/null -w "%{http_code}" --max-time 10 "${BASE_URL}${endpoint}" 2>/dev/null || echo "000")

    if [ "$STATUS" = "$expected_status" ]; then
        echo -e "${GREEN}PASS${NC}: ${description} -> ${STATUS}"
        PASS=$((PASS + 1))
    else
        echo -e "${RED}FAIL${NC}: ${description} -> ${STATUS} (expected ${expected_status})"
        FAIL=$((FAIL + 1))
    fi
}

check_json() {
    local endpoint="$1"
    local expected_status="$2"
    local description="${3:-$endpoint}"
    TOTAL=$((TOTAL + 1))

    RESPONSE=$(curl -s -w "\n%{http_code}" --max-time 10 "${BASE_URL}${endpoint}" 2>/dev/null || echo -e "\n000")
    STATUS=$(echo "$RESPONSE" | tail -1)
    BODY=$(echo "$RESPONSE" | sed '$d')

    if [ "$STATUS" = "$expected_status" ]; then
        # Verify response is valid JSON
        if echo "$BODY" | python3 -m json.tool > /dev/null 2>&1; then
            echo -e "${GREEN}PASS${NC}: ${description} -> ${STATUS} (valid JSON)"
            PASS=$((PASS + 1))
        else
            echo -e "${YELLOW}WARN${NC}: ${description} -> ${STATUS} (not valid JSON)"
            PASS=$((PASS + 1))  # Status correct, just warn about JSON
        fi
    else
        echo -e "${RED}FAIL${NC}: ${description} -> ${STATUS} (expected ${expected_status})"
        FAIL=$((FAIL + 1))
    fi
}

echo "========================================"
echo "RaceControl E2E Smoke Test"
echo "Base URL: ${BASE_URL}"
echo "========================================"
echo ""

# ─── Health & System ─────────────────────────────────────────────────────────
echo "--- Health & System ---"
check "/health" "200" "Health check"

# ─── Public Endpoints (no auth) ─────────────────────────────────────────────
echo ""
echo "--- Public Endpoints ---"
check_json "/public/leaderboard" "200" "Public leaderboard"
check_json "/public/time-trial" "200" "Public time trials"

# ─── Pod Management ─────────────────────────────────────────────────────────
echo ""
echo "--- Pod Management ---"
check_json "/pods" "200" "Pod list"

# ─── Billing (read-only) ────────────────────────────────────────────────────
echo ""
echo "--- Billing ---"
check_json "/billing/sessions/active" "200" "Active billing sessions"
check_json "/pricing" "200" "Pricing tiers"

# ─── Customer ───────────────────────────────────────────────────────────────
echo ""
echo "--- Customer ---"
check_json "/customer/packages" "200" "Customer packages"

# ─── Kiosk ───────────────────────────────────────────────────────────────────
echo ""
echo "--- Kiosk ---"
check_json "/kiosk/experiences" "200" "Kiosk experiences"

# ─── Summary ─────────────────────────────────────────────────────────────────
echo ""
echo "========================================"
echo -e "Results: ${GREEN}${PASS} passed${NC}, ${RED}${FAIL} failed${NC} (${TOTAL} total)"
echo "========================================"

if [ "$FAIL" -gt 0 ]; then
    echo -e "${RED}SMOKE TEST FAILED${NC}"
    exit 1
else
    echo -e "${GREEN}SMOKE TEST PASSED${NC}"
    exit 0
fi
