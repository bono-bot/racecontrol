---
phase: 339-api-endpoints
plan: 01
subsystem: api
tags: [serde, wallet, json-schema, axum, sqlite]

requires:
  - phase: 338-wallet-core-logic
    provides: WalletInfo struct with rupee/bonus columns, currency_type on transactions, cash_refund function
provides:
  - WalletInfo serde renames producing unified JSON field names (balance_credits, total_spent, etc.)
  - transactions_count field on WalletInfo
  - Topup response with new_balance_credits, bonus_credits_granted, rupee_amount, max_cash_refund
  - Webhook response with new_balance_credits
  - all_wallet_transactions with currency_type and rupee/bonus/cash-refund summary counters
affects: [340-admin-dashboard, 341-pos-kiosk-display, 342-cloud-sync]

tech-stack:
  added: []
  patterns: [serde rename for API field naming without changing Rust internals]

key-files:
  created: []
  modified:
    - crates/rc-common/src/types.rs
    - crates/racecontrol/src/wallet.rs
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/tests/integration.rs

key-decisions:
  - "Rust field names stay as _paise (values ARE in paise) -- only JSON serialization renames via serde"
  - "transactions_count uses separate COUNT query with proper error propagation (map_err + ?), not unwrap_or"
  - "total_credited serde rename is intentional extension beyond SC-1 for admin dashboard Phase 340"
  - "gateway_topup counted in total_rupee_deposits via OR condition (starts_with topup OR exact gateway_topup)"

patterns-established:
  - "Serde rename pattern: keep Rust field names stable, rename JSON output only"
  - "Summary counters pattern: compute aggregates in-memory during transaction iteration"

requirements-completed: [SC-1, SC-2, SC-4, SC-5]

duration: 18min
completed: 2026-04-07
---

# Phase 339 Plan 01: API Endpoints - Response Schema Renames Summary

**WalletInfo serde renames producing unified credits JSON schema, plus transactions_count field and rupee/bonus/cash-refund summary counters on all_wallet_transactions**

## Performance

- **Duration:** 18 min
- **Started:** 2026-04-07T15:06:59Z
- **Completed:** 2026-04-07T15:24:27Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- WalletInfo serializes with renamed JSON keys: balance_credits, total_credited, total_spent, rupee_deposited, rupee_refunded, bonus_credited
- transactions_count field populated via COUNT query with proper error handling (no unwrap)
- Topup handler returns new_balance_credits, bonus_credits_granted, rupee_amount, max_cash_refund
- Payment webhook returns new_balance_credits in both duplicate and success paths
- all_wallet_transactions includes currency_type per transaction and summary with total_rupee_deposits, total_bonus_credits, total_cash_refunds
- Customer wallet fallback uses new field names
- Cloud sync verified unaffected (uses SQL json_object, not serde)

## Task Commits

Each task was committed atomically:

1. **Task 1: WalletInfo serde renames + transactions_count field** - `5b40e0ca` (feat)
2. **Task 2: Update handler responses -- topup, webhook, all_wallet_transactions** - `d3edddde` (feat)

## Files Created/Modified
- `crates/rc-common/src/types.rs` - Added serde rename attributes to WalletInfo, added transactions_count field
- `crates/racecontrol/src/wallet.rs` - Added COUNT query for transactions_count in get_wallet_info()
- `crates/racecontrol/src/api/routes.rs` - Updated topup, webhook, all_wallet_transactions, customer_wallet responses
- `crates/racecontrol/tests/integration.rs` - Fixed test schema to include Phase 338 columns

## Decisions Made
- Rust field names stay as `_paise` (values ARE in paise) -- only JSON serialization changes via serde rename
- transactions_count uses `map_err()?` instead of `unwrap_or(0)` per CLAUDE.md no-unwrap rule
- `total_credited` (serde rename of total_credited_paise) is an intentional extension beyond SC-1 for admin dashboard Phase 340
- Idempotent replay omits max_cash_refund (it didn't change anything -- no DB call needed)
- gateway_topup counted in total_rupee_deposits via `starts_with("topup") || == "gateway_topup"`

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed test schema missing Phase 338 columns**
- **Found during:** Task 1 (verification step)
- **Issue:** Test database schema in integration.rs lacked rupee_deposited_paise, rupee_refunded_paise, bonus_credited_paise columns on wallets table and currency_type on wallet_transactions. Pre-existing gap from Phase 338 not updating test fixtures.
- **Fix:** Added missing columns to CREATE TABLE statements and added gateway_topup/refund_cash to txn_type CHECK constraint
- **Files modified:** crates/racecontrol/tests/integration.rs
- **Verification:** All 21 wallet/financial/billing tests pass (62 passed before fix, 70 after -- 8 wallet tests unblocked)
- **Committed in:** 5b40e0ca (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Test schema fix was essential for verification. No scope creep.

## Issues Encountered
- 9 pre-existing test failures in lap suspect/notification tests (unrelated to wallet changes) -- not addressed per scope boundary rule

## Known Stubs
None -- all API responses are wired to live DB queries with no placeholder data.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All wallet endpoint response schemas now use unified "credits" naming
- Ready for Plan 339-02 (cash-refund endpoint + refund response updates)
- Ready for Phase 340 (admin dashboard can consume these field names directly)

---
*Phase: 339-api-endpoints*
*Completed: 2026-04-07*
