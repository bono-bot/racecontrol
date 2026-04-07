---
phase: 340-admin-dashboard
plan: 01
subsystem: ui
tags: [nextjs, typescript, swr, tailwind, wallet, admin-dashboard]

# Dependency graph
requires:
  - phase: 339-api-endpoints
    provides: "Wallet API endpoints with currency_type field and summary breakdowns"
provides:
  - "Wallet API client module (walletApi) with typed interfaces"
  - "Wallet summary cards on billing reports page"
  - "currency_type badge on wallet transactions page"
  - "currency_type badge on billing history page"
affects: [340-02, 340-03]

# Tech tracking
tech-stack:
  added: []
  patterns: ["walletApi module following billingApi pattern", "currencyBadge helper for rupee/credit display"]

key-files:
  created:
    - racingpoint-admin/src/lib/api/wallet.ts
  modified:
    - racingpoint-admin/src/app/(dashboard)/billing/reports/page.tsx
    - racingpoint-admin/src/app/(dashboard)/wallet-transactions/page.tsx
    - racingpoint-admin/src/app/(dashboard)/billing/history/page.tsx

key-decisions:
  - "SessionWithCurrency local interface extension for billing history (avoids modifying shared BillingSession type until Phase 339 types propagate)"
  - "currencyBadge duplicated in two pages (wallet-transactions and billing/history) rather than extracting shared component -- minimal code, keeps pages self-contained"

patterns-established:
  - "walletApi module: rcFetch-based API client with typed interfaces matching Phase 339 contract"
  - "currencyBadge pattern: green for rupee, blue for credit, dash for null"

requirements-completed: [SC-1, SC-2]

# Metrics
duration: 4min
completed: 2026-04-07
---

# Phase 340 Plan 01: Wallet UI + Currency Badges Summary

**Wallet API module, 4 wallet summary cards on reports page, and currency_type badges on both wallet transactions and billing history pages**

## Performance

- **Duration:** 4 min
- **Started:** 2026-04-07T16:04:01Z
- **Completed:** 2026-04-07T16:07:33Z
- **Tasks:** 4
- **Files modified:** 4

## Accomplishments
- Created typed wallet API module with getInfo, getTransactions, topup, cashRefund, debit methods
- Added 4 wallet summary cards (Rupee Deposits, Bonus Credits, Credits Spent, Cash Refunds) to billing reports page
- Added currency_type badge (green=rupee, blue=credit) to wallet transactions table
- Added currency_type badge to billing history table with SessionWithCurrency interface extension

## Task Commits

Each task was committed atomically:

1. **Task 1: Create wallet API module** - `1849e68` (feat)
2. **Task 2: Add wallet summary cards to billing reports page** - `d7db1e5` (feat)
3. **Task 3: Add currency_type badge to wallet transactions page** - `7b175ca` (feat)
4. **Task 4: Add currency_type badge to billing history page** - `4f3b9dd` (feat)

## Files Created/Modified
- `racingpoint-admin/src/lib/api/wallet.ts` - Wallet API client module with typed interfaces (WalletInfo, WalletTransaction, WalletTransactionsSummary, WalletTransactionsReport)
- `racingpoint-admin/src/app/(dashboard)/billing/reports/page.tsx` - Added walletApi import, useSWR call, and 4 wallet summary cards
- `racingpoint-admin/src/app/(dashboard)/wallet-transactions/page.tsx` - Added currency_type to WalletTxn, Summary interfaces; currencyBadge helper; Currency column
- `racingpoint-admin/src/app/(dashboard)/billing/history/page.tsx` - Added SessionWithCurrency interface; currencyBadge helper; Currency column; updated colSpan

## Decisions Made
- Used SessionWithCurrency local interface extension in billing history to safely cast the session type rather than modifying the shared ActiveSession/BillingSession types. The API returns currency_type but the TypeScript types from billing.ts don't include it yet.
- Duplicated currencyBadge helper in two page files rather than extracting a shared component. The function is 12 lines and keeping it local avoids cross-file dependencies for a simple badge.

## Deviations from Plan

None - plan executed exactly as written.

## Known Stubs

None - all 4 wallet summary cards are wired to live walletApi.getTransactions() data, and currency badges read from the API response currency_type field.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Wallet API module is ready for Plan 02 (cash refund button, credit adjustment button)
- Reports page, wallet transactions page, and billing history page are updated and TypeScript-clean
- Deploy handled in Plan 03

## Self-Check: PASSED

- [x] wallet.ts exists
- [x] 340-01-SUMMARY.md exists
- [x] Commit 1849e68 found
- [x] Commit d7db1e5 found
- [x] Commit 7b175ca found
- [x] Commit 4f3b9dd found

---
*Phase: 340-admin-dashboard*
*Completed: 2026-04-07*
