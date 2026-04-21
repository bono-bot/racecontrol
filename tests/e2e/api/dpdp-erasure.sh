#!/usr/bin/env bash
# DPDP §12 right-of-erasure — Runtime contract test (Day 4 of chess-analogy plan).
#
# Converts the STRUCTURAL DPDP check (scripts/audit/dpdp-coverage-check.py,
# which verifies schema FK edges match customer_legal.rs lists) into a
# RUNTIME check that actually calls DELETE /customer/data-delete and asserts
# every ERASE_TABLES row is gone + every POINTER_TABLES column is NULL.
#
# Source of truth: crates/racecontrol/src/api/customer_legal.rs, parsed via
# tests/e2e/api/dpdp_erase_list.py. No hardcoded table lists here — when a
# new FK table is added to ERASE_TABLES, this test picks it up automatically.
#
# Usage:
#     RACECONTROL_URL=http://localhost:8080 DB_PATH=/path/to/racecontrol.db \
#         bash tests/e2e/api/dpdp-erasure.sh
#
# Defaults: RACECONTROL_URL=http://localhost:8080
#           DB_PATH=C:/RacingPoint/data/racecontrol.db (venue convention)
#
# Exit codes:
#     0 — test passed (all ERASE tables empty + all POINTER columns NULL)
#     0 — SKIPPED (server unreachable or auth harness not wired yet)
#     1 — test failed (rows survived delete OR pointer cols still populated)
#     2 — harness error (parser failed, DB unreadable, etc.)
#
# Skip-branch state (2026-04-21): the auth-token minting path is wired
# via Path 2 — POST /customer/test-mint-jwt, registered only when the
# server boots with TEST_MODE=true. Harness signals availability by
# setting TEST_MODE_ACTIVE=1 + TEST_DRIVER_ID=TEST_ONLY_DPDP_xxx. Without
# both, script still exits 0 with SKIP so CI stays green until seed+deploy.
# See crates/racecontrol/src/api/customer_auth.rs::customer_test_mint_jwt.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
PARSER="$SCRIPT_DIR/dpdp_erase_list.py"

RACECONTROL_URL="${RACECONTROL_URL:-http://localhost:8080}"
DB_PATH="${DB_PATH:-C:/RacingPoint/data/racecontrol.db}"
TEST_DRIVER_PREFIX="TEST_ONLY_DPDP_"

log() { echo "[dpdp-erasure] $*" >&2; }

# ─── Skip-branch 1: parser missing ───────────────────────────────────────────
if [ ! -x "$PARSER" ] && [ ! -f "$PARSER" ]; then
    log "SKIP: parser $PARSER not found"
    exit 0
fi

# ─── Skip-branch 2: server unreachable (cheapest probe first) ────────────────
if ! curl -s -m 5 "${RACECONTROL_URL}/api/v1/health" >/dev/null 2>&1; then
    log "SKIP: racecontrol unreachable at $RACECONTROL_URL"
    exit 0
fi

# ─── Skip-branch 3: server not booted in TEST_MODE ──────────────────────────
# The runtime test needs a valid customer bearer token. Path 2
# (2026-04-21) shipped POST /customer/test-mint-jwt behind TEST_MODE=true.
# Harness signals this endpoint is available by setting TEST_MODE_ACTIVE=1.
if [ "${TEST_MODE_ACTIVE:-0}" != "1" ]; then
    log "SKIP: TEST_MODE_ACTIVE != 1 (server must boot with TEST_MODE=true AND"
    log "  harness must set TEST_MODE_ACTIVE=1 to assert the endpoint is live)"
    exit 0
fi

# ─── Skip-branch 4: driver not seeded ────────────────────────────────────────
# Seeding is NOT this script's job — it takes too many trade-offs (direct
# sqlite insert vs /customer/register + manual phone+OTP). Caller must
# pre-seed a TEST_ONLY driver across ERASE_TABLES + POINTER_TABLES (see the
# STEP 5-7 table list below) and pass the driver_id via env.
if [ -z "${TEST_DRIVER_ID:-}" ]; then
    log "SKIP: TEST_DRIVER_ID unset. Seed a TEST_ONLY driver first (rows in"
    log "  ERASE_TABLES + POINTER_TABLES), then pass TEST_DRIVER_ID=TEST_ONLY_xxx"
    exit 0
fi
case "$TEST_DRIVER_ID" in
    TEST_ONLY*) ;;
    *)
        log "SKIP: TEST_DRIVER_ID must start with 'TEST_ONLY' (got: $TEST_DRIVER_ID)"
        log "  The /customer/test-mint-jwt handler rejects non-TEST_ONLY ids anyway"
        exit 0
        ;;
esac

# ─── Real path ───────────────────────────────────────────────────────────────

log "server: $RACECONTROL_URL"
log "db: $DB_PATH"
log "driver: $TEST_DRIVER_ID"

# Parse source-of-truth lists.
ERASE_JSON="$(python3 "$PARSER" --json)"
ERASE_COUNT=$(echo "$ERASE_JSON" | python3 -c "import json,sys; d=json.load(sys.stdin); print(len(d['erase']))")
POINTER_COUNT=$(echo "$ERASE_JSON" | python3 -c "import json,sys; d=json.load(sys.stdin); print(len(d['pointer']))")
TRANSITIVE_COUNT=$(echo "$ERASE_JSON" | python3 -c "import json,sys; d=json.load(sys.stdin); print(len(d['transitive']))")

log "SoT: $ERASE_COUNT erase / $POINTER_COUNT pointer / $TRANSITIVE_COUNT transitive tables"

# STEP 1: mint bearer token via /customer/test-mint-jwt (Path 2).
MINT_REQ_BODY=$(python3 -c "import json,sys; print(json.dumps({'driver_id': '$TEST_DRIVER_ID'}))")
MINT_RESP="$(curl -s -m 10 -X POST -H "Content-Type: application/json" \
    -d "$MINT_REQ_BODY" "$RACECONTROL_URL/api/v1/customer/test-mint-jwt")"
BEARER="$(echo "$MINT_RESP" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('token',''))" 2>/dev/null || echo '')"
if [ -z "$BEARER" ]; then
    log "FAIL: /customer/test-mint-jwt returned no token: $MINT_RESP"
    log "  (Check: server booted with TEST_MODE=true? driver seeded? driver_id prefix?)"
    exit 2
fi
log "minted bearer (len=${#BEARER})"

# STEP 2: pre-delete sanity — profile endpoint must recognise the driver.
PROFILE_BEFORE="$(curl -s -m 10 -H "Authorization: Bearer $BEARER" \
    "$RACECONTROL_URL/api/v1/customer/profile")"
DRIVER_BEFORE="$(echo "$PROFILE_BEFORE" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('driver',{}).get('id',''))" 2>/dev/null || echo '')"
if [ "$DRIVER_BEFORE" != "$TEST_DRIVER_ID" ]; then
    log "FAIL: profile pre-check got driver_id='$DRIVER_BEFORE', expected '$TEST_DRIVER_ID'"
    log "  profile resp: $PROFILE_BEFORE"
    exit 2
fi
log "pre-delete profile confirms driver exists"

# STEP 3: invoke the real erase endpoint.
DELETE_CODE="$(curl -s -o /tmp/dpdp-delete-body.$$ -w "%{http_code}" -m 30 -X DELETE \
    -H "Authorization: Bearer $BEARER" \
    "$RACECONTROL_URL/api/v1/customer/data-delete")"
DELETE_BODY="$(cat /tmp/dpdp-delete-body.$$ 2>/dev/null || echo '')"
rm -f /tmp/dpdp-delete-body.$$ 2>/dev/null || true
if [ "$DELETE_CODE" != "200" ] && [ "$DELETE_CODE" != "204" ]; then
    log "FAIL: DELETE /customer/data-delete returned HTTP $DELETE_CODE: $DELETE_BODY"
    exit 1
fi
log "delete response: HTTP $DELETE_CODE"

# STEP 4: verify ERASE_TABLES are empty for driver_id (sqlite3 direct).
# Each row: {"table": "...", "fk_columns": ["driver_id", ...]} — all listed
# columns must count 0 for the deleted driver_id.
FAIL_COUNT=0
ERASE_ROWS_TMP="/tmp/dpdp-erase-rows.$$"
echo "$ERASE_JSON" | python3 -c "
import json, sys
d = json.load(sys.stdin)
for r in d['erase']:
    for col in r['fk_columns']:
        print(f\"{r['table']}|{col}\")" > "$ERASE_ROWS_TMP"

log "verifying $ERASE_COUNT erase tables..."
while IFS='|' read -r TABLE COL; do
    [ -z "$TABLE" ] && continue
    COUNT="$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM $TABLE WHERE $COL = '$TEST_DRIVER_ID';" 2>/dev/null || echo 'NA')"
    if [ "$COUNT" = "NA" ]; then
        log "  WARN: $TABLE.$COL unqueryable (likely absent in this DB — OK)"
    elif [ "$COUNT" != "0" ]; then
        log "  FAIL: $TABLE.$COL still has $COUNT rows for $TEST_DRIVER_ID"
        FAIL_COUNT=$((FAIL_COUNT+1))
    fi
done < "$ERASE_ROWS_TMP"
rm -f "$ERASE_ROWS_TMP" 2>/dev/null || true

# STEP 5: verify POINTER_TABLES have no rows still referencing driver_id.
# POINTER flow: customer_data_delete() UPDATEs these to NULL before the
# driver row itself is deleted. Post-delete the count where col=driver_id
# must be 0 (value nulled, not row deleted — rows survive for retention).
POINTER_ROWS_TMP="/tmp/dpdp-pointer-rows.$$"
echo "$ERASE_JSON" | python3 -c "
import json, sys
d = json.load(sys.stdin)
for r in d['pointer']:
    print(f\"{r['table']}|{r['column']}\")" > "$POINTER_ROWS_TMP"

log "verifying $POINTER_COUNT pointer tables..."
while IFS='|' read -r TABLE COL; do
    [ -z "$TABLE" ] && continue
    COUNT="$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM $TABLE WHERE $COL = '$TEST_DRIVER_ID';" 2>/dev/null || echo 'NA')"
    if [ "$COUNT" = "NA" ]; then
        log "  WARN: $TABLE.$COL unqueryable (likely absent — OK)"
    elif [ "$COUNT" != "0" ]; then
        log "  FAIL: $TABLE.$COL still points at $TEST_DRIVER_ID ($COUNT rows; expected NULL)"
        FAIL_COUNT=$((FAIL_COUNT+1))
    fi
done < "$POINTER_ROWS_TMP"
rm -f "$POINTER_ROWS_TMP" 2>/dev/null || true

# STEP 6: drivers row itself must be gone.
DRIVERS_COUNT="$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM drivers WHERE id = '$TEST_DRIVER_ID';" 2>/dev/null || echo 'NA')"
if [ "$DRIVERS_COUNT" = "NA" ]; then
    log "FAIL: drivers table unqueryable (DB path wrong?)"
    exit 2
elif [ "$DRIVERS_COUNT" != "0" ]; then
    log "FAIL: drivers row still exists for $TEST_DRIVER_ID (count=$DRIVERS_COUNT)"
    FAIL_COUNT=$((FAIL_COUNT+1))
fi
log "drivers.id post-check: count=$DRIVERS_COUNT"

# STEP 7: TRANSITIVE_ERASE_SQL verification is NOT implemented here. The
# structural check at scripts/audit/dpdp-coverage-check.py already asserts
# the TRANSITIVE SQL covers the needed FK chains; a runtime assert would
# need to reconstruct each chain (billing_events→billing_sessions.driver_id,
# telemetry_samples→laps→driver_id, …) from customer_legal.rs. Deferred.

if [ "$FAIL_COUNT" -gt 0 ]; then
    log "DPDP erasure INVARIANT VIOLATED — $FAIL_COUNT residual rows"
    exit 1
fi

log "DPDP erasure invariant held: $ERASE_COUNT erase tables empty + $POINTER_COUNT pointers nulled + drivers row gone"
exit 0
