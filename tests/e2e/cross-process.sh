#!/bin/bash
# =============================================================================
# Cross-Process Integration Test Suite
#
# Validates inter-process dependencies: schema compatibility, service health,
# sync table coverage, and API endpoint spot checks.
#
# Usage:
#   bash tests/e2e/cross-process.sh
#
# Exit code = number of failures (0 = all pass)
# =============================================================================

set -uo pipefail

# ANSI colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Counters
PASS=0
FAIL=0
SKIP=0

pass() {
  PASS=$((PASS + 1))
  echo -e "  ${GREEN}PASS${NC}  $1"
}

fail() {
  FAIL=$((FAIL + 1))
  echo -e "  ${RED}FAIL${NC}  $1"
}

skip() {
  SKIP=$((SKIP + 1))
  echo -e "  ${YELLOW}SKIP${NC}  $1"
}

# Resolve paths
SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
ROOT_DIR=$(cd "$SCRIPT_DIR/../.." && pwd)

echo ""
echo "=== Cross-Process Integration Tests ==="
echo "Root: $ROOT_DIR"
echo ""

# ─── Section 1: Schema Compatibility ───────────────────────────────────────
echo "--- Section 1: Schema Compatibility ---"

if node "$ROOT_DIR/scripts/check-schema-compat.js" > /dev/null 2>&1; then
  pass "Schema compatibility check passed (all reader tables exist)"
else
  fail "Schema compatibility check failed (reader tables missing from database)"
fi

echo ""

# ─── Section 2: Service Health and Proxy Chains ───────────────────────────
echo "--- Section 2: Service Health & Proxy Chains ---"

HEALTH_SCRIPT="$ROOT_DIR/scripts/cross-service-health.sh"
if [ -f "$HEALTH_SCRIPT" ]; then
  HEALTH_EXIT=0
  bash "$HEALTH_SCRIPT" > /dev/null 2>&1 || HEALTH_EXIT=$?

  if [ "$HEALTH_EXIT" -eq 0 ]; then
    pass "All service health and proxy checks passed"
  else
    fail "cross-service-health.sh reported $HEALTH_EXIT failure(s)"
  fi
else
  skip "cross-service-health.sh not found at $HEALTH_SCRIPT"
fi

echo ""

# ─── Section 3: Sync Table Coverage ───────────────────────────────────────
echo "--- Section 3: Sync Table Coverage ---"

DB_PATH="$ROOT_DIR/data/racecontrol.db"

if [ ! -f "$DB_PATH" ]; then
  skip "racecontrol.db not found at $DB_PATH (may be venue-only)"
else
  # Get actual tables from the database
  ACTUAL_TABLES=$(sqlite3 "$DB_PATH" ".tables" 2>/dev/null | tr -s ' ' '\n' | sort)

  if [ -z "$ACTUAL_TABLES" ]; then
    fail "Could not read tables from racecontrol.db"
  else
    # Get sync boundary tables from DEPENDENCIES.json
    SYNC_BOUNDARY=$(node -e "const d=require('$ROOT_DIR/DEPENDENCIES.json'); console.log(JSON.stringify(d.sync_boundary))" 2>/dev/null)

    if [ -z "$SYNC_BOUNDARY" ]; then
      fail "Could not read sync_boundary from DEPENDENCIES.json"
    else
      # Check cloud_to_venue (tables_pulled)
      PULLED=$(echo "$SYNC_BOUNDARY" | node -e "
        let buf=''; process.stdin.on('data',d=>buf+=d); process.stdin.on('end',()=>{
          const sb=JSON.parse(buf);
          (sb.cloud_to_venue.tables_pulled||[]).forEach(t=>console.log(t));
        })")

      PULLED_FAIL=0
      while IFS= read -r table; do
        [ -z "$table" ] && continue
        if echo "$ACTUAL_TABLES" | grep -qw "$table"; then
          pass "Sync pull table exists: $table"
        else
          fail "Sync pull table MISSING from racecontrol.db: $table"
          PULLED_FAIL=$((PULLED_FAIL + 1))
        fi
      done <<< "$PULLED"

      # Check venue_to_cloud (tables_pushed)
      PUSHED=$(echo "$SYNC_BOUNDARY" | node -e "
        let buf=''; process.stdin.on('data',d=>buf+=d); process.stdin.on('end',()=>{
          const sb=JSON.parse(buf);
          (sb.venue_to_cloud.tables_pushed||[]).forEach(t=>console.log(t));
        })")

      PUSHED_FAIL=0
      while IFS= read -r table; do
        [ -z "$table" ] && continue
        if echo "$ACTUAL_TABLES" | grep -qw "$table"; then
          pass "Sync push table exists: $table"
        else
          fail "Sync push table MISSING from racecontrol.db: $table"
          PUSHED_FAIL=$((PUSHED_FAIL + 1))
        fi
      done <<< "$PUSHED"
    fi
  fi
fi

echo ""

# ─── Section 4: API Endpoint Spot Checks ──────────────────────────────────
echo "--- Section 4: API Endpoint Spot Checks ---"

RC_BASE="http://localhost:8080/api/v1"

# Check if racecontrol is reachable at all
if ! curl -sf --max-time 5 "$RC_BASE/health" > /dev/null 2>&1; then
  skip "racecontrol not running at $RC_BASE (cannot perform API spot checks)"
else
  # Health endpoint
  HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 "$RC_BASE/health" 2>/dev/null || echo "000")
  if [ "$HTTP_CODE" = "200" ]; then
    pass "GET /api/v1/health -> $HTTP_CODE"
  else
    fail "GET /api/v1/health -> $HTTP_CODE (expected 200)"
  fi

  # Pods endpoint (200 or 401 are fine, just not 404/500)
  HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 "$RC_BASE/pods" 2>/dev/null || echo "000")
  if [ "$HTTP_CODE" = "200" ] || [ "$HTTP_CODE" = "401" ]; then
    pass "GET /api/v1/pods -> $HTTP_CODE"
  elif [ "$HTTP_CODE" = "404" ] || [ "$HTTP_CODE" = "500" ] || [ "$HTTP_CODE" = "000" ]; then
    fail "GET /api/v1/pods -> $HTTP_CODE (expected 200 or 401)"
  else
    pass "GET /api/v1/pods -> $HTTP_CODE"
  fi
fi

echo ""

# ─── Summary ──────────────────────────────────────────────────────────────
TOTAL=$((PASS + FAIL + SKIP))
echo "=== Cross-Process Integration Tests ==="
echo -e "Results: ${GREEN}${PASS} passed${NC}, ${RED}${FAIL} failed${NC}, ${YELLOW}${SKIP} skipped${NC} (${TOTAL} total)"
echo ""

exit "$FAIL"
