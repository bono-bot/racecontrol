---
phase: 338-wallet-core-logic
plan: 02
subsystem: payments
tags: [rust, wallet, accounting, double-entry, cash-refund, sqlite, axum]

# Dependency graph
requires:
  - phase: 338-01
    provides: "WalletInfo with rupee_deposited/refunded columns, currency_type_for() helper"
  - phase: 337-db-schema-migration
    provides: "rupee_deposited_paise, rupee_refunded_paise, bonus_credited_paise columns on wallets table"
provides:
  - "wallet::cash_refund(state, driver_id, amount_paise, staff_id, notes) — real money return operation"
  - "wallet::get_max_cash_refund(state, driver_id) — cap calculation for UI/API queries"
  - "accounting::post_cash_refund(state, driver_id, amount_paise, method, staff_id, txn_id) — journal entry"
affects: [339-wallet-api, billing, staff-dashboard, admin-portal]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "TOCTOU-safe refund: cap check via SELECT inside same SQLite transaction as UPDATE"
    - "state.db.begin() pattern for standalone transactions (not acquire+begin to avoid borrow issues)"
    - "Negative amount_paise in wallet_transactions for money-out operations (refund/debit)"
    - "D-07 signature convention: no refund_method in core function; API layer can extend"

key-files:
  created: []
  modified:
    - "crates/racecontrol/src/wallet.rs — cash_refund + get_max_cash_refund added after refund()"
    - "crates/racecontrol/src/accounting.rs — post_cash_refund added after post_refund()"

key-decisions:
  - "Cash refund method defaults to 'cash' internally — Phase 339 API layer determines actual method"
  - "Cap check runs INSIDE the SQL transaction (not before tx.begin()) to prevent TOCTOU race"
  - "rupee_refunded_paise incremented in same UPDATE as balance_paise decrement for atomicity"
  - "Existing refund() function unchanged — it remains the credit-only game-reset refund path (D-16)"

patterns-established:
  - "Cash-out operations: Dr. acc_wallet (liability decreases) Cr. acc_cash/acc_bank (asset decreases)"
  - "All money-out wallet_transactions use negative amount_paise"
  - "Refund cap formula: rupee_deposited - rupee_refunded - total_debited, clamped to [0, balance]"

requirements-completed: [WAL-02]

# Metrics
duration: 10min
completed: 2026-04-07
---

# Phase 338 Plan 02: Wallet Core Logic — Cash Refund Summary

**TOCTOU-safe cash_refund() with atomic cap enforcement, Dr. wallet Cr. cash double-entry journal, and standalone get_max_cash_refund() query helper**

## Performance

- **Duration:** 10 min
- **Started:** 2026-04-07T14:35:57Z
- **Completed:** 2026-04-07T14:46:38Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- `cash_refund()` in wallet.rs: signature exactly per D-07, TOCTOU-safe cap check inside tx, decrements `balance_paise` AND increments `rupee_refunded_paise` atomically, records `refund_cash`/`rupee` transaction, posts Dr. acc_wallet Cr. acc_cash journal
- `get_max_cash_refund()` in wallet.rs: read-only formula (`rupee_deposited - rupee_refunded - total_debited`, clamped to `[0, balance]`) for UI/API pre-flight queries
- `post_cash_refund()` in accounting.rs: reverse of `post_topup` — Dr. acc_wallet Cr. acc_cash (cash) or acc_bank (bank/card/upi/online)
- All existing wallet functions and `refund()` remain unchanged

## Task Commits

1. **Task 1: Add post_cash_refund to accounting.rs** - `307ee0d8` (feat)
2. **Task 2: Add get_max_cash_refund and cash_refund to wallet.rs** - `3e3779eb` (feat)

## Files Created/Modified

- `crates/racecontrol/src/accounting.rs` - Added `post_cash_refund()` after `post_refund()`, lines 551-593
- `crates/racecontrol/src/wallet.rs` - Added `get_max_cash_refund()` and `cash_refund()` after `refund()`, lines 501-641

## Decisions Made

- **Cash refund method defaults to "cash":** Core function has no `refund_method` parameter per D-07. The default "cash" maps to `acc_cash` in the journal. Phase 339 API layer can accept a method param and extend if needed.
- **Cap check inside transaction:** SELECT with `&mut *tx` before UPDATE prevents TOCTOU race — no concurrent refund can over-refund between read and write.
- **state.db.begin() not acquire+begin:** Matches `debit_wallet` pattern (line 258). The acquire+begin borrow pattern used in `credit()` and `debit()` causes lifetime conflicts in this context.

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered

- `cargo test -p racecontrol-crate` shows 17 pre-existing failures: `no such column: rupee_deposited_paise` in integration test DB setup. These are pre-existing from 337-db-schema-migration — the test DB schema doesn't include the new columns yet. Confirmed by stashing our changes and re-running: same 17 failures existed before plan 338-02. Not caused by this plan.

## Known Stubs

None — no hardcoded values or placeholder data. All functions operate on real DB data.

## Next Phase Readiness

- `cash_refund()` and `get_max_cash_refund()` are ready for Phase 339 to wire into HTTP API endpoints
- `post_cash_refund()` is ready — accounting journal posts automatically on every successful cash refund
- The refund method can be passed from the API layer as "cash", "bank", "upi", etc. to route to acc_cash or acc_bank

## Self-Check

---
*Phase: 338-wallet-core-logic*
*Completed: 2026-04-07*
