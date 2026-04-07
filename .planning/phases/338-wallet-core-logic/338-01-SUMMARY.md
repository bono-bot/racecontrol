---
phase: 338-wallet-core-logic
plan: 01
subsystem: wallet
tags: [wallet, rupee-credit-separation, currency-tracking, types]
dependency_graph:
  requires: [337-01]
  provides: [WAL-02]
  affects: [wallet.rs, cloud_sync.rs, rc-common/types.rs]
tech_stack:
  added: []
  patterns: [conditional-column-increment, currency-type-routing, computed-field-derivation]
key_files:
  created: []
  modified:
    - crates/rc-common/src/types.rs
    - crates/racecontrol/src/wallet.rs
    - crates/racecontrol/src/cloud_sync.rs
decisions:
  - "adjustment txn_type now routes to post_bonus (not post_topup) to match bonus_credited_paise column tracking (D-02)"
  - "max_cash_refund computed in get_wallet_info: rupee_deposited - rupee_refunded - total_debited, clamped to [0, balance] (D-14)"
  - "credit_wallet documents intentional lack of rupee_deposited_paise tracking — it is only for incentive bonuses, never topup_*"
metrics:
  duration_minutes: 8
  completed_date: "2026-04-07T14:11:11Z"
  tasks_completed: 2
  tasks_total: 2
  files_modified: 3
---

# Phase 338 Plan 01: Wallet Core Logic — Types and Functions Summary

**One-liner:** Extended WalletInfo/WalletTransaction structs with rupee/bonus tracking columns and updated all wallet functions to conditionally increment rupee_deposited_paise, bonus_credited_paise, and include currency_type on every wallet_transactions INSERT.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Update WalletInfo and WalletTransaction structs in rc-common | `1ec42704` | crates/rc-common/src/types.rs |
| 2 | Update credit_in_tx, credit_wallet, debit_in_tx, get_wallet_info, get_transactions, cloud_sync.rs | `d464a08e` | crates/racecontrol/src/wallet.rs, crates/racecontrol/src/cloud_sync.rs |

## What Was Built

### Struct Changes (rc-common/types.rs)

`WalletInfo` gained 4 new fields:
- `rupee_deposited_paise: i64` — running total of rupee deposits (topup_* txns)
- `rupee_refunded_paise: i64` — running total of cash refunds issued
- `bonus_credited_paise: i64` — running total of bonus/adjustment credits
- `max_cash_refund: i64` — computed field: `rupee_deposited - rupee_refunded - total_debited`, clamped to `[0, balance]`

`WalletTransaction` gained 1 new field:
- `currency_type: String` — 'rupee' or 'credit' per D-04, D-06, D-21

### Function Changes (wallet.rs)

**`currency_type_for()` helper:**
- topup_* → "rupee"
- refund_cash → "rupee"
- everything else (bonus, adjustment, debit_*, refund_session, refund_manual) → "credit"

**`credit_in_tx`:**
- UPDATE wallets now conditionally increments `rupee_deposited_paise` (for topup_*) and `bonus_credited_paise` (for bonus/adjustment)
- refund_session/refund_manual: neither column touched (D-03)
- INSERT wallet_transactions now includes `currency_type` column

**`credit()` adjustment arm:**
- Changed from `post_topup` to `post_bonus` to match bonus_credited_paise column tracking (D-02)

**`credit_wallet`:**
- UPDATE wallets now increments `bonus_credited_paise` for known bonus types
- INSERT wallet_transactions now includes `currency_type` column
- Added inline comment documenting intentional lack of `rupee_deposited_paise` tracking

**`debit_in_tx`:**
- INSERT wallet_transactions now includes `currency_type = 'credit'` (literal, per D-06)

**`get_wallet_info`:**
- SELECT now fetches rupee_deposited_paise, rupee_refunded_paise, bonus_credited_paise
- Computes max_cash_refund per D-14 formula

**`get_transactions`:**
- SELECT now fetches currency_type
- Tuple type updated from 9 to 10 fields
- Struct mapping includes currency_type

### cloud_sync.rs Change

`process_debit_intents` INSERT now includes `currency_type = 'credit'` (D-06, D-20).

## Verification

```
cargo check (racecontrol workspace): PASS — 0 errors
cargo test --lib -p racecontrol-crate: PASS — 832 tests, 0 failures
grep -c "currency_type" wallet.rs: 11 (minimum required: 5)
grep "currency_type" cloud_sync.rs: found in process_debit_intents INSERT
grep "rupee_deposited_paise = rupee_deposited_paise + ?" wallet.rs: FOUND
grep "bonus_credited_paise = bonus_credited_paise + ?" wallet.rs: FOUND
grep "max_cash_refund" wallet.rs: FOUND
grep "credit_wallet is used for incentive bonuses" wallet.rs: FOUND
```

## Deviations from Plan

None — plan executed exactly as written.

## Known Stubs

None — all data flows are fully wired. The new columns have DEFAULT 0 (from Phase 337 migration) so existing rows are valid without backfill.

## Self-Check: PASSED

- `1ec42704` exists: FOUND
- `d464a08e` exists: FOUND
- crates/rc-common/src/types.rs modified: FOUND (rupee_deposited_paise field present)
- crates/racecontrol/src/wallet.rs modified: FOUND (currency_type_for helper, 11 matches)
- crates/racecontrol/src/cloud_sync.rs modified: FOUND (currency_type in INSERT)
