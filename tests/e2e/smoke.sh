#!/bin/bash
# =============================================================================
# RaceControl E2E Smoke Test
#
# Validates that all core API endpoints are reachable and returning expected
# status codes. Run against a live racecontrol instance.
#
# Usage:
#   ./smoke.sh                          # defaults to localhost:8080/api/v1
#   RC_BASE_URL=https://rc.racingpoint.cloud/api/v1 ./smoke.sh
# =============================================================================

set -euo pipefail

BASE_URL="${RC_BASE_URL:-http://localhost:8080/api/v1}"
SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/common.sh
source "$SCRIPT_DIR/lib/common.sh"
TOTAL=0

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

# ─── Pod Management (auth-protected) ─────────────────────────────────────────
echo ""
echo "--- Pod Management (auth required) ---"
check "/pods" "401" "Pod list (requires auth → 401)"

# ─── Billing (auth-protected) ────────────────────────────────────────────────
echo ""
echo "--- Billing (auth required) ---"
check "/billing/sessions/active" "401" "Active billing sessions (requires auth → 401)"
check "/pricing" "401" "Pricing tiers (requires auth → 401)"

# ─── Customer ───────────────────────────────────────────────────────────────
echo ""
echo "--- Customer ---"
check_json "/customer/packages" "200" "Customer packages"

# ─── Kiosk (auth-protected) ───────────────────────────────────────────────────
echo ""
echo "--- Kiosk (auth required) ---"
check "/kiosk/experiences" "401" "Kiosk experiences (requires auth → 401)"

# ─── Summary ─────────────────────────────────────────────────────────────────
summary_exit
