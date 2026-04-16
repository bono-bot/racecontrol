#!/usr/bin/env bash
# =============================================================================
# deploy-verify-fleet.sh — Comprehensive fleet parity checker
#
# Checks across ALL targets:
#   1. Binary build_id (server, pods, POS, cloud)
#   2. Bat file SHA256 parity across all pods
#   3. Frontend BUILD_ID / staleness (server + cloud)
#   4. DB state (pricing_tiers is_active)
#
# Usage:
#   bash scripts/deploy/deploy-verify-fleet.sh
#   bash scripts/deploy/deploy-verify-fleet.sh --expected-hash abc1234
#   bash scripts/deploy/deploy-verify-fleet.sh --skip-db
#   bash scripts/deploy/deploy-verify-fleet.sh --skip-frontend
#
# Run from James (.27). Requires SSH access to pods, server, cloud.
# =============================================================================

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"

GREEN='\033[0;32m'; RED='\033[0;31m'; YELLOW='\033[0;33m'; CYAN='\033[0;36m'; NC='\033[0m'
pass() { echo -e "  ${GREEN}OK${NC}    $1"; }
fail() { echo -e "  ${RED}FAIL${NC}  $1"; }
warn() { echo -e "  ${YELLOW}WARN${NC}  $1"; }
info() { echo -e "  ${CYAN}...${NC}   $1"; }

# --- Parse arguments ---
EXPECTED_HASH=""
SKIP_DB=false
SKIP_FRONTEND=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --expected-hash) EXPECTED_HASH="$2"; shift 2 ;;
        --skip-db) SKIP_DB=true; shift ;;
        --skip-frontend) SKIP_FRONTEND=true; shift ;;
        *) echo "Unknown arg: $1"; exit 1 ;;
    esac
done

# Default expected hash from git HEAD
if [ -z "$EXPECTED_HASH" ] && [ -d "${REPO_DIR}/.git" ]; then
    EXPECTED_HASH=$(git -C "$REPO_DIR" rev-parse --short HEAD 2>/dev/null || echo "unknown")
fi

# --- Canonical bat SHA256 from repo ---
CANONICAL_BAT="${REPO_DIR}/scripts/deploy/start-rcagent.bat"
CANONICAL_BAT_SHA=""
if [ -f "$CANONICAL_BAT" ]; then
    CANONICAL_BAT_SHA=$(sha256sum "$CANONICAL_BAT" 2>/dev/null | awk '{print $1}')
fi

# --- Network map ---
SERVER_IP="192.168.31.23"
POS_IP="192.168.31.130"
CLOUD_HOST="100.70.177.44"
CLOUD_PUBLIC="srv1422716.hstgr.cloud"

declare -A POD_IPS=(
    [1]="192.168.31.89" [2]="192.168.31.33" [3]="192.168.31.28" [4]="192.168.31.88"
    [5]="192.168.31.86" [6]="192.168.31.87" [7]="192.168.31.38" [8]="192.168.31.91"
)

TOTAL_CHECKS=0
TOTAL_PASS=0
TOTAL_FAIL=0
TOTAL_WARN=0

record() {
    TOTAL_CHECKS=$((TOTAL_CHECKS + 1))
    case "$1" in
        pass) TOTAL_PASS=$((TOTAL_PASS + 1)) ;;
        fail) TOTAL_FAIL=$((TOTAL_FAIL + 1)) ;;
        warn) TOTAL_WARN=$((TOTAL_WARN + 1)) ;;
    esac
}

echo ""
echo "=========================================="
echo "  Racing Point Fleet Parity Checker"
echo "  $(date '+%Y-%m-%d %H:%M') IST"
echo "  Expected hash: ${EXPECTED_HASH}"
echo "  Canonical bat: ${CANONICAL_BAT_SHA:0:16}..."
echo "=========================================="
echo ""

# =========================================================================
# Section 1: Binary build_id
# =========================================================================

echo -e "${CYAN}--- 1. Binary build_id ---${NC}"
echo ""

printf "  %-14s %-14s %-14s %-10s\n" "Target" "Expected" "Actual" "Status"
printf "  %-14s %-14s %-14s %-10s\n" "--------------" "--------------" "--------------" "----------"

check_build_id() {
    local target="$1"
    local url="$2"
    local resp
    resp=$(curl -sf --max-time 5 "$url" 2>/dev/null || echo "")
    if [ -z "$resp" ]; then
        printf "  %-14s %-14s %-14s %-10s\n" "$target" "$EXPECTED_HASH" "-" "UNREACH"
        record warn
        return
    fi
    local bid
    bid=$(echo "$resp" | grep -o '"build_id":"[^"]*"' | head -1 | cut -d'"' -f4)
    if [ -z "$bid" ]; then
        bid="(missing)"
    fi
    if [ "$bid" = "$EXPECTED_HASH" ]; then
        printf "  %-14s %-14s %-14s %-10s\n" "$target" "$EXPECTED_HASH" "$bid" "MATCH"
        record pass
    else
        printf "  %-14s %-14s %-14s %-10s\n" "$target" "$EXPECTED_HASH" "$bid" "MISMATCH"
        record fail
    fi
}

check_build_id "Server .23" "http://${SERVER_IP}:8080/api/v1/health"

for n in 1 2 3 4 5 6 7 8; do
    check_build_id "Pod $n" "http://${POD_IPS[$n]}:8090/health"
done

check_build_id "POS .130" "http://${POS_IP}:8090/health"
check_build_id "Cloud VPS" "http://${CLOUD_HOST}:8080/api/v1/health"

echo ""

# =========================================================================
# Section 2: Bat file SHA256 parity (pods only)
# =========================================================================

echo -e "${CYAN}--- 2. Bat file parity (start-rcagent.bat) ---${NC}"
echo ""

if [ -z "$CANONICAL_BAT_SHA" ]; then
    warn "Cannot compute canonical bat SHA — ${CANONICAL_BAT} not found"
else
    printf "  %-8s %-18s %-10s\n" "Pod" "SHA256 (first 16)" "Status"
    printf "  %-8s %-18s %-10s\n" "--------" "------------------" "----------"

    for n in 1 2 3 4 5 6 7 8; do
        pod_sha=$(ssh -o ConnectTimeout=5 -o BatchMode=yes "pod${n}" \
            "certutil -hashfile C:\\RacingPoint\\start-rcagent.bat SHA256 2>nul" 2>/dev/null \
            | grep -v "SHA256\|CertUtil" | tr -d '[:space:]')
        if [ -z "$pod_sha" ]; then
            printf "  %-8s %-18s %-10s\n" "Pod $n" "-" "UNREACH"
            record warn
        elif [ "$pod_sha" = "$CANONICAL_BAT_SHA" ]; then
            printf "  %-8s %-18s %-10s\n" "Pod $n" "${pod_sha:0:16}" "MATCH"
            record pass
        else
            printf "  %-8s %-18s %-10s\n" "Pod $n" "${pod_sha:0:16}" "MISMATCH"
            record fail
        fi
    done
fi

echo ""

# =========================================================================
# Section 3: Frontend BUILD_ID / staleness
# =========================================================================

if ! $SKIP_FRONTEND; then
    echo -e "${CYAN}--- 3. Frontend staleness ---${NC}"
    echo ""

    # Check server frontends via HTTP status
    printf "  %-20s %-10s %-10s\n" "Frontend" "Server" "Cloud"
    printf "  %-20s %-10s %-10s\n" "--------------------" "----------" "----------"

    check_frontend() {
        local name="$1"
        local server_url="$2"
        local cloud_url="$3"

        local s_code c_code
        s_code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 "$server_url" 2>/dev/null)
        [ -z "$s_code" ] && s_code="000"
        c_code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 "$cloud_url" 2>/dev/null)
        [ -z "$c_code" ] && c_code="000"

        local s_status c_status
        if [ "$s_code" = "200" ] || [ "$s_code" = "307" ] || [ "$s_code" = "302" ]; then
            s_status="HTTP $s_code"
            record pass
        else
            s_status="HTTP $s_code"
            record fail
        fi
        if [ "$c_code" = "200" ] || [ "$c_code" = "307" ] || [ "$c_code" = "302" ]; then
            c_status="HTTP $c_code"
            record pass
        else
            c_status="HTTP $c_code"
            record warn
        fi

        printf "  %-20s %-10s %-10s\n" "$name" "$s_status" "$c_status"
    }

    check_frontend "Web (:3200)" \
        "http://${SERVER_IP}:3200" \
        "http://${CLOUD_HOST}:3200"

    check_frontend "Kiosk (:3300)" \
        "http://${SERVER_IP}:3300/kiosk" \
        "http://${CLOUD_HOST}:3300"

    check_frontend "Admin (:3201)" \
        "http://${SERVER_IP}:3201" \
        "http://${CLOUD_HOST}:3201"

    check_frontend "PWA (:3100)" \
        "http://${SERVER_IP}:3100" \
        "http://${CLOUD_HOST}:3100"

    echo ""
fi

# =========================================================================
# Section 4: DB state (pricing_tiers is_active)
# =========================================================================

if ! $SKIP_DB; then
    echo -e "${CYAN}--- 4. DB state (pricing_tiers) ---${NC}"
    echo ""

    # Server DB — query via the billing API endpoint (avoids sqlite3 dependency)
    info "Checking server DB via API..."
    SERVER_PRICING=$(curl -sf --max-time 5 "http://${SERVER_IP}:8080/api/v1/billing/pricing" 2>/dev/null || echo "")
    if [ -n "$SERVER_PRICING" ]; then
        # Count pricing entries to verify structure
        SERVER_TIER_COUNT=$(echo "$SERVER_PRICING" | grep -o '"duration_minutes"' | wc -l)
        echo "  Server: $SERVER_TIER_COUNT pricing tier(s) in API response"
        record pass
    else
        warn "Server: could not query pricing API"
        record warn
    fi

    info "Checking cloud DB..."
    CLOUD_DB_RESULT=$(ssh -o ConnectTimeout=5 -o BatchMode=yes root@100.70.177.44 \
        "sqlite3 /root/racingpoint/racecontrol/racecontrol.db 'SELECT COUNT(*) FROM pricing_tiers WHERE is_active = 1' 2>/dev/null" 2>/dev/null || echo "")
    if [ -n "$CLOUD_DB_RESULT" ]; then
        echo "  Cloud: $CLOUD_DB_RESULT active pricing tier(s)"
        if [ "$CLOUD_DB_RESULT" = "0" ]; then
            pass "Cloud: legacy pricing (tiers hidden)"
            record pass
        else
            warn "Cloud: $CLOUD_DB_RESULT tier(s) active — verify intended"
            record warn
        fi
    else
        warn "Cloud: could not query DB"
        record warn
    fi

    echo ""
fi

# =========================================================================
# Summary
# =========================================================================

echo "=========================================="
echo "  Fleet Parity Summary"
echo "=========================================="
echo ""
echo "  Total checks: $TOTAL_CHECKS"
echo -e "  ${GREEN}Pass:${NC}    $TOTAL_PASS"
echo -e "  ${RED}Fail:${NC}    $TOTAL_FAIL"
echo -e "  ${YELLOW}Warn:${NC}    $TOTAL_WARN"
echo ""

if [ "$TOTAL_FAIL" -gt 0 ]; then
    echo -e "  ${RED}FLEET PARITY: FAILED${NC} — $TOTAL_FAIL issue(s) need attention"
    exit 1
elif [ "$TOTAL_WARN" -gt 0 ]; then
    echo -e "  ${YELLOW}FLEET PARITY: WARNING${NC} — $TOTAL_WARN item(s) to review"
    exit 2
else
    echo -e "  ${GREEN}FLEET PARITY: PASSED${NC}"
    exit 0
fi
