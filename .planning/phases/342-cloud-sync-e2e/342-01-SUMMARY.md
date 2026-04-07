---
phase: 342-cloud-sync-e2e
plan: 01
subsystem: database
tags: [cloud-sync, wallet, sqlite, sqlx, rust]

# Dependency graph
requires:
  - phase: 337-db-schema-migration
    provides: "rupee_deposited_paise, rupee_refunded_paise, bonus_credited_paise columns in wallets table"
  - phase: 338-wallet-core-logic
    provides: "Wallet logic that writes to the 3 new columns (topup, refund, bonus)"
  - phase: 339-api-endpoints
    provides: "API endpoints that expose wallet data with new columns"
provides:
  - "Cloud sync push includes 3 new wallet tracking columns in json_object payload"
  - "Cloud sync pull/upsert extracts, SELECTs, UPDATEs, and INSERTs all 3 new columns"
  - "Backward-compatible .unwrap_or(0) for old-format cloud data"
affects: [deploy, cloud-parity]

# Tech tracking
tech-stack:
  added: []
  patterns: [".unwrap_or(0) for backward-compatible JSON field extraction"]

key-files:
  created: []
  modified:
    - crates/racecontrol/src/cloud_sync.rs

key-decisions:
  - "Placed new columns after updated_at but before phone/email in push json_object for logical grouping"
  - "Used underscore-prefixed locals in SELECT destructuring since new columns exist only for tuple type correctness"
  - "process_debit_intents confirmed unchanged per D-07 -- only touches balance_paise and total_debited_paise"

patterns-established:
  - "Cloud sync column addition: push json_object + extract + SELECT + UPDATE + INSERT -- 5 edit points per column"

requirements-completed: [SC-1, SC-2, SC-3, SC-4]

# Metrics
duration: 8min
completed: 2026-04-07
---

# Phase 342 Plan 01: Cloud Sync E2E Summary

**Cloud sync push/pull updated with rupee_deposited_paise, rupee_refunded_paise, bonus_credited_paise across 5 edit points in cloud_sync.rs**

## Performance

- **Duration:** 8 min
- **Started:** 2026-04-07T16:45:57Z
- **Completed:** 2026-04-07T16:54:00Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- Push json_object query includes all 3 new wallet tracking columns for cloud sync
- Upsert wallet function extracts, queries, updates, and inserts all 3 new columns with backward-compatible defaults
- Verified process_debit_intents remains unchanged (zero references to new columns)
- cargo check passes with zero errors

## Task Commits

Each task was committed atomically:

1. **Task 1: Add 3 wallet columns to cloud sync push and pull** - `0ccc321c` (feat)
2. **Task 2: Compile verification and E2E test checklist** - verification-only, no code changes

**Plan metadata:** (pending docs commit)

## Files Created/Modified
- `crates/racecontrol/src/cloud_sync.rs` - Added 3 wallet tracking columns to push json_object, upsert field extraction, SELECT, UPDATE, and INSERT queries

## Decisions Made
- Placed new columns after `updated_at` but before `phone`/`email` in push json_object for logical grouping with other wallet fields
- Used underscore-prefixed locals (`_local_rupee_dep`, `_local_rupee_ref`, `_local_bonus_cr`) in SELECT destructuring since they exist only for tuple type correctness
- Confirmed process_debit_intents needs no changes per D-07 -- it only touches balance_paise and total_debited_paise

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- 9 pre-existing test failures in integration tests (BILL-08 refund calc, lap suspect, notification tests) -- none related to cloud_sync changes, all out of scope

## User Setup Required
None - no external service configuration required.

## E2E Test Checklist (for deploy session)

The following manual E2E tests should be run after deploying the updated binary to verify cloud sync of new wallet columns:

1. **Topup:** POST /api/v1/wallet/topup with 100000 paise (Rs 1000) for a test driver
2. **Verify wallet:** balance_paise=100000, rupee_deposited_paise=100000, bonus_credited_paise=X (per bonus rules)
3. **Spend:** POST /api/v1/billing/start a session, spend 200 credits (20000 paise)
4. **Verify debit:** balance_paise=80000+bonus, total_debited_paise=20000
5. **Refund request:** POST /api/v1/wallet/refund/request for the test driver
6. **Verify max refundable:** rupee_deposited_paise - rupee_refunded_paise - total_debited_paise = 100000 - 0 - 20000 = 80000 (Rs 800)
7. **Verify bonus exclusion:** bonus is NOT included in refundable amount

## Next Phase Readiness
- Cloud sync code complete, ready for deploy
- Deploy parity required: venue (.23) AND cloud (Bono VPS) must both receive the updated binary
- E2E test checklist above should be executed post-deploy

---
*Phase: 342-cloud-sync-e2e*
*Completed: 2026-04-07*
