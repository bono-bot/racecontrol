---
phase: 154-ordering-core
plan: 03
subsystem: ui
tags: [react, nextjs, typescript, cafe, ordering, pos, wallet]

requires:
  - phase: 154-01
    provides: POST /api/v1/cafe/orders endpoint, stock fields in GET /api/v1/cafe/menu

provides:
  - CafeMenuPanel transformed into POS order builder with customer selection and checkout
  - CafeOrderItem, CafeOrderItemDetail, CafeOrderResponse types in kiosk/src/lib/types.ts
  - api.placeCafeOrder(driverId, items) method in kiosk/src/lib/api.ts
  - Staff can add items, select a customer by name search, and submit orders that debit wallet

affects: [154-02, cafe-kiosk, pos-ui]

tech-stack:
  added: []
  patterns:
    - Two-column POS layout (menu 60% / order sidebar 40%) with controlled React state
    - Inline qty controls on item cards after first add (no separate cart step)
    - Customer autocomplete by filtering drivers list client-side (max 8 results)
    - Error-preserving order flow — sidebar keeps items on API error so staff can retry

key-files:
  created: []
  modified:
    - kiosk/src/lib/types.ts
    - kiosk/src/lib/api.ts
    - kiosk/src/components/CafeMenuPanel.tsx

key-decisions:
  - "Cart state is React-only (no localStorage) per plan decision"
  - "Out-of-stock items show overlay on card and Add button is absent — not just disabled"
  - "Driver search filters client-side from preloaded drivers list — no per-keystroke API calls"
  - "On API error response with error field, order items are preserved so staff can retry or adjust"

patterns-established:
  - "CafeMenuPanel two-column POS pattern: menu left, order sidebar right"
  - "fetchApi<CafeOrderResponse | { error: string }> pattern for endpoints returning typed errors"

requirements-completed: [ORD-02]

duration: 18min
completed: 2026-03-22
---

# Phase 154 Plan 03: POS Cafe Order Builder Summary

**Staff POS order builder in CafeMenuPanel: item add/qty controls with out-of-stock overlays, live customer name search, and wallet-debiting checkout via POST /api/v1/cafe/orders**

## Performance

- **Duration:** 18 min
- **Started:** 2026-03-22T17:30:00+05:30
- **Completed:** 2026-03-22T17:48:00+05:30
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- Added `is_countable`, `stock_quantity`, `out_of_stock` fields to `CafeMenuItem` and new `CafeOrderItem`, `CafeOrderItemDetail`, `CafeOrderResponse` types
- Added `api.placeCafeOrder(driverId, items)` using staff auth `fetchApi` — POST to `/cafe/orders`
- Rewrote `CafeMenuPanel` from a browse-only display into a full POS order builder: two-column layout, item add buttons, inline qty controls, out-of-stock overlays, customer autocomplete, running order total, and checkout with success/error states

## Task Commits

Each task was committed atomically:

1. **Task 1: Update kiosk types and add order API method** - `0b5ee831` (feat)
2. **Task 2: Add order builder and customer selection to CafeMenuPanel** - `13156b26` (feat)

**Plan metadata:** (docs commit below)

## Files Created/Modified

- `kiosk/src/lib/types.ts` - Added stock fields to CafeMenuItem; added CafeOrderItem, CafeOrderItemDetail, CafeOrderResponse
- `kiosk/src/lib/api.ts` - Added CafeOrderItem/CafeOrderResponse imports; added api.placeCafeOrder method
- `kiosk/src/components/CafeMenuPanel.tsx` - Full rewrite: two-column POS layout, order builder state, customer search, checkout flow

## Decisions Made

- Cart kept in React state only (no localStorage) — per plan decision, keeps the flow simple for staff
- Out-of-stock items render an overlay (not a disabled button) to make the status visually unambiguous
- Driver autocomplete filters the preloaded `drivers` list client-side — avoids per-keystroke API round trips
- On API error (including insufficient balance), order items are preserved so staff can adjust or retry without rebuilding the cart

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- POS staff-assisted ordering (ORD-02) is complete and builds successfully
- Customer self-service ordering (ORD-03, Plan 02) can proceed independently — shares the same backend endpoint
- Manual verification: open kiosk /control, navigate to cafe panel, confirm order builder UI and place a test order

---
*Phase: 154-ordering-core*
*Completed: 2026-03-22*
