#!/usr/bin/env bash
# Seed a TEST_ONLY driver + funded wallet for E2E tests.
#
# Consumed by tests/e2e/api/dpdp-erasure.sh and billing-flow.sh, which
# both SKIP unless TEST_DRIVER_ID is set. This script is the minimum-viable
# pre-seed that produces that id, so CI can chain:
#
#     TEST_DRIVER_ID="$(bash seed-test-driver.sh --dpdp-footprint)" \
#     TEST_MODE_ACTIVE=1 \
#     bash tests/e2e/api/dpdp-erasure.sh
#
# By design it does NOT go through /customer/login phone+OTP or
# /customer/register (which updates an existing driver, not creates one).
# There is no public "create driver" endpoint — customer flow creates the
# row during OTP verify. For a harness we skip that entirely and insert
# directly, gated by the TEST_ONLY prefix invariant the rest of the stack
# already enforces (/customer/test-mint-jwt rejects non-TEST_ONLY ids,
# cleanup-test-drivers.mjs deletes only TEST_ONLY rows).
#
# Usage:
#     DB_PATH=/path/to/racecontrol.db bash tests/e2e/api/seed-test-driver.sh \
#         [--wallet-paise N]        # default 100000 (Rs.1000)
#         [--dpdp-footprint]        # seed rows in a handful of ERASE_TABLES
#                                     so dpdp-erasure.sh has non-trivial
#                                     count-is-zero assertions
#         [--prefix TEST_ONLY_xxx]  # default TEST_ONLY_HARNESS
#
# Prints driver_id on stdout. Logs everything else to stderr.
#
# Exit codes:
#     0 — seeded OK
#     2 — harness error (DB not found, wrong prefix, sqlite3 failure)

set -euo pipefail

DB_PATH="${DB_PATH:-C:/RacingPoint/data/racecontrol.db}"
WALLET_PAISE="100000"
DPDP_FOOTPRINT=0
PREFIX_BASE="TEST_ONLY_HARNESS"

while [ $# -gt 0 ]; do
    case "$1" in
        --wallet-paise) WALLET_PAISE="$2"; shift 2 ;;
        --dpdp-footprint) DPDP_FOOTPRINT=1; shift ;;
        --prefix) PREFIX_BASE="$2"; shift 2 ;;
        *)
            echo "[seed-test-driver] unknown arg: $1" >&2
            exit 2
            ;;
    esac
done

log() { echo "[seed-test-driver] $*" >&2; }

# Hard invariant — prevents this script from ever touching a real id.
case "$PREFIX_BASE" in
    TEST_ONLY*) ;;
    *)
        log "FAIL: --prefix must start with TEST_ONLY (got: $PREFIX_BASE)"
        exit 2
        ;;
esac

if ! command -v sqlite3 >/dev/null 2>&1; then
    log "FAIL: sqlite3 not in PATH"
    exit 2
fi
if [ ! -f "$DB_PATH" ]; then
    log "FAIL: DB not found at $DB_PATH"
    exit 2
fi

# Unique per invocation so parallel runs don't collide on PRIMARY KEY.
TS="$(python3 -c 'import time; print(int(time.time()))')"
DRIVER_ID="${PREFIX_BASE}_${TS}_$$"

log "db: $DB_PATH"
log "seeding driver_id: $DRIVER_ID"
log "wallet_paise: $WALLET_PAISE"
log "dpdp_footprint: $DPDP_FOOTPRINT"

# Driver row — minimum columns. Later migrations add columns but all
# have defaults (per grep of migrate_*.rs ALTER TABLE drivers). If a
# future migration adds a NOT NULL column without a default, this
# insert will fail loudly with a SQLite constraint error — that's the
# signal to update this helper.
sqlite3 "$DB_PATH" <<SQL
BEGIN;
INSERT INTO drivers (id, name) VALUES ('$DRIVER_ID', 'TEST_ONLY harness $DRIVER_ID');
INSERT INTO wallets (driver_id, balance_paise, total_credited_paise)
VALUES ('$DRIVER_ID', $WALLET_PAISE, $WALLET_PAISE);
COMMIT;
SQL

if [ "$DPDP_FOOTPRINT" = "1" ]; then
    # Seed a handful of rows across ERASE_TABLES so dpdp-erasure.sh's
    # post-delete COUNT(*) checks are non-trivial. We pick tables that
    # have a well-known driver_id FK and few other required columns.
    # If any of these fail, the whole footprint step is skipped (|| true)
    # so the driver+wallet seed remains usable for billing-flow.sh.
    log "seeding DPDP footprint (best-effort, non-fatal)..."
    sqlite3 "$DB_PATH" <<SQL || log "WARN: some footprint inserts failed (table absent or schema drift)"
INSERT OR IGNORE INTO notifications (id, driver_id, kind, payload_json, created_at)
VALUES ('${DRIVER_ID}-notif', '$DRIVER_ID', 'test', '{}', datetime('now'));
INSERT OR IGNORE INTO customer_preferences (driver_id, preferences_json, updated_at)
VALUES ('$DRIVER_ID', '{}', datetime('now'));
INSERT OR IGNORE INTO driver_achievements (id, driver_id, achievement_key, earned_at)
VALUES ('${DRIVER_ID}-ach', '$DRIVER_ID', 'TEST_ONLY', datetime('now'));
SQL
fi

# stdout: driver_id (for piping).
echo "$DRIVER_ID"
log "seeded OK — pipe: TEST_DRIVER_ID=\"\$(bash $0)\""
exit 0
