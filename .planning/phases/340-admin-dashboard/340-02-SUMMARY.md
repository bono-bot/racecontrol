---
phase: 340-admin-dashboard
plan: 02
subsystem: ui
tags: [nextjs, typescript, tailwind, wallet, admin-dashboard, cash-refund, credit-adjustment]

# Dependency graph
requires:
  - phase: 340-01
    provides: "Wallet API module (walletApi) with getInfo, cashRefund, topup, debit methods"
provides:
  - "Cash Refund button with modal, ConfirmDialog confirmation, role-gated visibility"
  - "Credit Adjustment button with add/remove toggle, reason dropdown, dual API routing"
affects: [340-03]

# Tech tracking
tech-stack:
  added: []
  patterns: ["ConfirmDialog for destructive actions", "AuthContext isAdmin role gate", "walletApi.getInfo for max_cash_refund display"]

key-files:
  created: []
  modified:
    - racingpoint-admin/src/app/(dashboard)/wallet-transactions/page.tsx

key-decisions:
  - "Cash Refund button role-gated via AuthContext isAdmin (D-14) -- cashier role cannot see refund button"
  - "Credit Adjustment button NOT role-gated -- available to all staff when driver selected"
  - "ADJUST_REASONS constant defined outside component for reusability"

patterns-established:
  - "ConfirmDialog for cash refund confirmation with null guard on selectedDriverId"
  - "walletApi.getInfo useEffect on selectedDriverId change for wallet info fetch"

requirements-completed: [SC-3, SC-4]

# Metrics
duration: 3min
completed: 2026-04-07
---

# Phase 340 Plan 02: Cash Refund + Credit Adjustment Summary

**Cash refund modal with max cap display and ConfirmDialog, credit adjustment modal with add/remove toggle and reason dropdown, both integrated via walletApi**

## Performance

- **Duration:** 3 min
- **Started:** 2026-04-07T16:09:55Z
- **Completed:** 2026-04-07T16:12:51Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- Added Cash Refund button (role-gated to admin/manager via AuthContext isAdmin) with modal showing driver balance, max_cash_refund from walletApi.getInfo, amount validation, and ConfirmDialog confirmation
- Added Credit Adjustment button with add/remove toggle, reason dropdown (bonus/correction/penalty/other), dual API routing (topup for add, debit for remove)
- Both modals include null guards on selectedDriverId, success/error toasts via sonner, transaction refresh after success

## Task Commits

Each task was committed atomically:

1. **Task 1: Add Cash Refund button and modal** - `a43f494` (feat)
2. **Task 2: Add Credit Adjustment button and modal** - `05bca6a` (feat)

## Files Created/Modified
- `racingpoint-admin/src/app/(dashboard)/wallet-transactions/page.tsx` - Added imports (walletApi, WalletInfo, toast, ConfirmDialog, AuthContext), auth context, cash refund state/modal/confirm dialog, credit adjustment state/modal with add/remove toggle, ADJUST_REASONS constant, walletInfo useEffect

## Decisions Made
- Cash Refund button is role-gated via `isAdmin` from AuthContext (per D-14) -- only superadmin/staff can see it, cashier role cannot
- Credit Adjustment button is available to all authenticated staff when a driver is selected (not role-gated per plan spec)
- ADJUST_REASONS constant placed outside component (after TOPUP_METHODS) for consistency with existing pattern

## Deviations from Plan

None - plan executed exactly as written.

## Known Stubs

None - Cash Refund modal reads max_cash_refund from live walletApi.getInfo() call, Credit Adjustment routes to live walletApi.topup() and walletApi.debit() endpoints.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Wallet transactions page now has all 4 action capabilities: Top Up, Cash Refund, Adjust Credits (add/remove)
- Deploy handled in Plan 03

## Self-Check: PASSED

- [x] 340-02-SUMMARY.md exists
- [x] Commit a43f494 found
- [x] Commit 05bca6a found
