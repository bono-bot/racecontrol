---
phase: 339-api-endpoints
plan: 02
subsystem: api
tags: [axum, wallet, cash-refund, json-schema, rest-api]

requires:
  - phase: 338-wallet-core-logic
    provides: wallet::cash_refund() and wallet::get_max_cash_refund() functions
  - phase: 339-api-endpoints plan 01
    provides: WalletInfo serde renames, unified credits JSON schema
provides:
  - POST /wallet/{driver_id}/cash-refund endpoint returning typed cash_refund response
  - Credit refund response with type=credit_refund and max_cash_refund field
  - ROADMAP SC-3 aligned with two-endpoint refund design
affects: [340-admin-dashboard, 341-pos-kiosk-display]

tech-stack:
  added: []
  patterns: [separate endpoints for distinct refund types to isolate MMA-203 security caps]

key-files:
  created: []
  modified:
    - crates/racecontrol/src/api/routes.rs
    - .planning/ROADMAP.md

key-decisions:
  - "Two-endpoint design: /refund for credits, /cash-refund for real money -- isolates MMA-203 security caps from new cash refund logic (D-11/D-17)"
  - "Pre-check via get_max_cash_refund before calling cash_refund -- shows cap in error message while actual enforcement is TOCTOU-safe inside wallet::cash_refund"
  - "Credit refund response includes max_cash_refund so admin UI can show 'or refund X as cash' cross-sell"

patterns-established:
  - "Refund type differentiation: every refund response includes a 'type' field (credit_refund or cash_refund)"
  - "Cash refund remaining cap: success responses include max_cash_refund_remaining for UI state updates"

requirements-completed: [SC-3]

duration: 9min
completed: 2026-04-07
---

# Phase 339 Plan 02: Cash Refund Endpoint + Credit Refund Type Differentiation Summary

**New POST /wallet/{driver_id}/cash-refund endpoint with TOCTOU-safe cap enforcement, plus type field differentiation on credit refund responses for admin UI**

## Performance

- **Duration:** 9 min
- **Started:** 2026-04-07T15:26:40Z
- **Completed:** 2026-04-07T15:36:01Z
- **Tasks:** 3
- **Files modified:** 2

## Accomplishments
- New POST /wallet/{driver_id}/cash-refund endpoint registered in staff routes, calls wallet::cash_refund()
- Cash refund response includes type=cash_refund, amount, new_balance_credits, max_cash_refund_remaining
- Both credit refund paths (referenced and non-referenced) return type=credit_refund with max_cash_refund for admin UI cross-sell
- ROADMAP SC-3 updated to reflect the intentional two-endpoint design matching CONTEXT.md D-11/D-17

## Task Commits

Each task was committed atomically:

1. **Task 1: Add cash refund endpoint + register route** - `19cf78ab` (feat)
2. **Task 2: Update credit refund response with type differentiation** - `5d0a639b` (feat)
3. **Task 3: Update ROADMAP SC-3 to reflect two-endpoint design** - `2d440644` (docs)

## Files Created/Modified
- `crates/racecontrol/src/api/routes.rs` - Added CashRefundRequest struct, cash_refund_wallet handler, route registration; updated both refund_wallet response paths with type=credit_refund and max_cash_refund
- `.planning/ROADMAP.md` - SC-3 now describes both /refund and /cash-refund endpoints; 339-02 marked complete

## Decisions Made
- Two-endpoint design: /refund stays credit-only, /cash-refund is new -- isolates MMA-203 security caps from cash refund logic
- Pre-check calls get_max_cash_refund for user-friendly error message; actual enforcement is TOCTOU-safe inside cash_refund()
- Credit refund includes max_cash_refund field so admin UI can show "or refund X as cash" option
- debit_wallet_manual still uses new_balance_paise (not renamed) -- out of scope for this plan, only refund responses updated

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## Known Stubs
None -- all API responses are wired to live wallet functions with no placeholder data.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 339 complete (2/2 plans executed) -- all 5 success criteria met
- Ready for Phase 340 (admin dashboard) -- can consume balance_credits, type=credit_refund/cash_refund, max_cash_refund fields
- Ready for Phase 341 (POS/kiosk) -- same unified JSON schema on port 8080

---
*Phase: 339-api-endpoints*
*Completed: 2026-04-07*
