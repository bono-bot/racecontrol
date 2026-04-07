---
phase: 337-db-schema-migration
plan: 01
subsystem: database
tags: [sqlite, schema-migration, wallet, rupee-credit-separation, alter-table, backfill]

# Dependency graph
requires: []
provides:
  - "wallets.rupee_deposited_paise column (total real-money deposits per driver)"
  - "wallets.rupee_refunded_paise column (total cash refunds per driver)"
  - "wallets.bonus_credited_paise column (total promotional credits per driver)"
  - "wallet_transactions.currency_type column (rupee or credit per transaction)"
  - "Backfilled currency_type for all existing topup transactions"
  - "Backfilled rupee_deposited_paise and bonus_credited_paise for all existing wallets"
affects:
  - "338-wallet-core-logic"
  - "339-wallet-api"
  - "340-341-frontend"
  - "342-e2e-verification"

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "let _ = sqlx::query(...).execute(pool).await for idempotent ALTER TABLE (suppresses duplicate column errors)"
    - "UPDATE WHERE clause guards for idempotent backfill (WHERE rupee_deposited_paise = 0)"
    - "COALESCE(..., 0) subquery pattern for safe SUM aggregation"

key-files:
  created: []
  modified:
    - "crates/racecontrol/src/db/mod.rs"

key-decisions:
  - "Used let _ = pattern to suppress SQLite duplicate column errors — same pattern as all prior migrations in db/mod.rs"
  - "Default 'credit' for currency_type makes existing wallet_transactions rows valid without explicit backfill"
  - "rupee_refunded_paise stays 0 for all existing wallets — no cash refund feature existed before v45.0"
  - "balance_paise untouched per D-04 — remains the single spendable credits pool"
  - "No CHECK constraint on currency_type — SQLite limitation, app-level enforcement in phase 338"

patterns-established:
  - "Phase 337 backfill pattern: UPDATE WHERE column = 0 guards idempotency on re-run"
  - "Wallet separation columns: rupee_deposited_paise and bonus_credited_paise are accounting totals, not spendable balances"

requirements-completed:
  - WAL-01

# Metrics
duration: 20min
completed: 2026-04-07
---

# Phase 337 Plan 01: DB Schema Migration Summary

**4 ALTER TABLE statements and 3 idempotent backfill UPDATEs in migrate() establish rupee/credit column separation for wallets and wallet_transactions tables**

## Performance

- **Duration:** 20 min
- **Started:** 2026-04-07T13:39:00Z
- **Completed:** 2026-04-07T13:59:00Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- Added rupee_deposited_paise, rupee_refunded_paise, bonus_credited_paise columns to wallets table with idempotent ALTER TABLE
- Added currency_type column to wallet_transactions table with DEFAULT 'credit' (makes existing rows valid)
- Backfilled currency_type = 'rupee' for all topup_cash/topup_card/topup_upi/topup_online transactions
- Backfilled rupee_deposited_paise from historical topup sums per driver
- Backfilled bonus_credited_paise from historical bonus/adjustment sums per driver
- All cargo tests pass: racecontrol (4/4), rc-common (245/245 + 1 doc test)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add ALTER TABLE columns and backfill queries to migrate()** - `1dc0ec9b` (feat)
2. **Task 2: Verify migration idempotency and correctness with cargo test** - No code changes; verification-only task

## Files Created/Modified
- `crates/racecontrol/src/db/mod.rs` - Added 59 lines in migrate(): 4 ALTER TABLE + 3 backfill UPDATE + info log

## Decisions Made
- Inserted migration block before the existing `tracing::info!("Database migrations complete")` line per plan instructions
- Preserved the "Database migrations complete" final log line after the new v45.0 block
- Did not add rupee_refunded_paise backfill — D-09 confirms 0 is correct for all existing wallets

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- `cargo test -p rc-agent` package ID mismatch — crate is named `rc-agent-crate` not `rc-agent`. Used `--no-run` to confirm test binary compiles cleanly. Pre-existing environment limitation prevents running interactive-session tests; not caused by our changes.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 338 (Wallet Core Logic) can now read/write rupee_deposited_paise, rupee_refunded_paise, bonus_credited_paise on wallets
- Phase 338 can now read/write currency_type on wallet_transactions
- Cloud database gets migration automatically via cloud_sync.rs (D-14: 'wallets' in SYNC_TABLES)
- Migration will run on first server restart on both venue (.23) and cloud (Bono VPS)

## Known Stubs
None — this is a pure schema migration. No UI or logic stubs introduced.

## Self-Check: PASSED
- `1dc0ec9b` exists in git log
- `crates/racecontrol/src/db/mod.rs` modified with all 4 ALTER TABLE + 3 UPDATE statements
- `grep -c "rupee_deposited_paise"` returns 4 (≥3 required)
- `grep -c "currency_type"` returns 4 (≥3 required)
- `cargo check --bin racecontrol` finished with 0 errors
- `cargo test --bin racecontrol` result: 4 passed, 0 failed
- `cargo test -p rc-common` result: 246 passed, 0 failed

---
*Phase: 337-db-schema-migration*
*Completed: 2026-04-07*
