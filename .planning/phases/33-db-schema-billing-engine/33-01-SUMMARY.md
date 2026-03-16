---
phase: 33-db-schema-billing-engine
plan: 01
subsystem: database
tags: [sqlite, billing, serde, sqlx, test-migrations]

# Dependency graph
requires:
  - phase: 14-events-and-championships
    provides: integration test infrastructure (run_test_migrations, test_db_setup pattern)
provides:
  - Title Case billing_rates seed data matching default_billing_rate_tiers() in billing.rs
  - billing_rates CREATE TABLE + INSERT OR IGNORE in test migrations
  - assert_eq!(3) seed count assertion in test_db_setup
  - test_billing_tick_old_field_alias covering PROTOC-01 serde alias round-trip
affects: [34-billing-rates-crud, 35-billing-frontend]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "assert_eq!(count, N) for deterministic seed tables (vs assert!(>= N) for non-deterministic)"
    - "serde alias test: deserialize old key -> verify new field -> verify re-serialization uses canonical name"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/tests/integration.rs
    - crates/rc-common/src/protocol.rs

key-decisions:
  - "Use assert_eq!(3) not assert!(>= 3) for billing_rates seed — INSERT OR IGNORE with deterministic IDs means exactly 3 always"
  - "Test migration schema mirrors production CREATE TABLE exactly — same columns, same defaults, same order"

patterns-established:
  - "Phase N schema goes at end of run_test_migrations() with a Phase N comment header block"
  - "Serde alias tests verify both directions: old key -> new field (deserialization) AND new field name in re-serialized output (not alias)"

requirements-completed: [RATE-01, RATE-02, RATE-03, BILLC-02, BILLC-03, BILLC-04, BILLC-05, PROTOC-01, PROTOC-02]

# Metrics
duration: 2min
completed: 2026-03-17
---

# Phase 33 Plan 01: DB Schema + Billing Engine Summary

**Capitalization bug fixed in billing_rates seed (lowercase -> Title Case), test migrations extended with billing_rates table + seed assertion (exactly 3 rows), and PROTOC-01 serde alias round-trip test added — all 9 Phase 33 requirements have automated verification with 331+113 tests green**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-17T06:14:54Z
- **Completed:** 2026-03-17T06:17:08Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- Fixed `billing_rates` seed SQL in `db/mod.rs`: tier_name values changed from `'standard'/'extended'/'marathon'` to `'Standard'/'Extended'/'Marathon'` — now matches `default_billing_rate_tiers()` which returns Title Case names
- Extended `run_test_migrations()` in `integration.rs` with Phase 33 billing_rates CREATE TABLE + INSERT OR IGNORE seed, enabling integration tests to query billing_rates
- Added `assert_eq!(billing_rate_count.0, 3)` assertion to `test_db_setup()` — confirms exactly 3 seeded tiers after migration
- Added `test_billing_tick_old_field_alias()` to `protocol.rs` — tests that JSON with `"minutes_to_value_tier": 15` deserializes to `minutes_to_next_tier == Some(15)` via serde alias, and re-serialization emits canonical field name (completing PROTOC-01)

## Task Commits

Each task was committed atomically:

1. **Task 1: Fix seed capitalization + add billing_rates to test migrations and assertions** - `8a5adf0` (fix)
2. **Task 2: Add serde alias round-trip test and run full suite green** - `d4dcbe5` (feat)

**Plan metadata:** `[pending]` (docs: complete plan)

## Files Created/Modified

- `crates/racecontrol/src/db/mod.rs` - Fixed tier_name casing in INSERT OR IGNORE billing_rates seed (3 values)
- `crates/racecontrol/tests/integration.rs` - Added Phase 33 billing_rates table to test migrations + assert_eq!(3) assertion in test_db_setup
- `crates/rc-common/src/protocol.rs` - Added test_billing_tick_old_field_alias() covering PROTOC-01 serde alias deserialization and canonical re-serialization

## Decisions Made

- `assert_eq!(3)` used (not `assert!(>= 3)`) for billing_rates — INSERT OR IGNORE with deterministic IDs guarantees exactly 3 rows, making an exact assertion stronger and more correct
- Test migration schema mirrors production `db/mod.rs` CREATE TABLE column-for-column to avoid hidden discrepancies between test and prod DBs

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- All 9 Phase 33 requirements have automated verification
- Phase 34 (billing rates CRUD routes: GET/POST/PUT/DELETE /billing/rates) can proceed — billing_rates table exists in both production DB and test migrations
- Phase 34 cache invalidation pattern is documented in STATE.md: synchronous invalidate() call before returning 200/204

---
*Phase: 33-db-schema-billing-engine*
*Completed: 2026-03-17*
