---
phase: 153-inventory-alerts
plan: 02
subsystem: ui
tags: [nextjs, react, typescript, low-stock, polling, useEffect, tailwind]

# Dependency graph
requires:
  - phase: 153-01
    provides: /api/v1/cafe/items/low-stock endpoint returning LowStockItem[]

provides:
  - LowStockItem TypeScript interface exported from web/src/lib/api.ts
  - api.listLowStockItems() typed method in api object
  - Low-stock red warning banner in CafePage above tab bar with 60s polling

affects: [cafe-ui, admin-dashboard, inventory-alerts]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Polling useEffect pattern: fetch on mount + setInterval(60s) + cleanup (cancelled flag + clearInterval)"
    - "Best-effort banner: catch swallowed silently, banner absent on fetch failure"

key-files:
  created: []
  modified:
    - web/src/lib/api.ts
    - web/src/app/cafe/page.tsx

key-decisions:
  - "Separate useEffect for low-stock polling (not merged into loadData useEffect) for clean separation of concerns"
  - "Banner uses cancelled flag to prevent setState after unmount"
  - "Banner is best-effort: fetch failures silently suppress the banner rather than showing an error"

patterns-established:
  - "Polling pattern: useEffect with cancelled flag + setInterval(fn, 60_000) + cleanup returns cancellation"

requirements-completed: [INV-07]

# Metrics
duration: 8min
completed: 2026-03-22
---

# Phase 153 Plan 02: Inventory Alerts Summary

**Red low-stock warning banner in CafePage with 60s polling via api.listLowStockItems(), listing each breached item's name, stock quantity, and threshold**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-22T14:55:00Z
- **Completed:** 2026-03-22T15:03:00Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Added `LowStockItem` TypeScript interface exported from api.ts alongside `CafeItem`
- Added `api.listLowStockItems()` method that fetches `/cafe/items/low-stock` with auth headers, fully typed, no `any`
- Added low-stock warning banner to CafePage above the tab bar, rendered conditionally when items are below threshold
- Banner polls every 60 seconds via a separate useEffect with cleanup (cancelled flag + clearInterval)
- Hydration-safe: no localStorage/sessionStorage in useState initializer

## Task Commits

Each task was committed atomically:

1. **Task 1: TypeScript type + api.ts method** - `899b5f73` (feat)
2. **Task 2: Low-stock banner in CafePage** - `b1d336d3` (feat)

**Plan metadata:** (docs commit follows)

## Files Created/Modified

- `web/src/lib/api.ts` - Added `LowStockItem` interface and `listLowStockItems()` method
- `web/src/app/cafe/page.tsx` - Added `LowStockItem` import, `lowStockItems` state, polling useEffect, and warning banner JSX above Tab Navigation

## Decisions Made

- Kept polling useEffect separate from the existing `loadData` useEffect for clean separation of concerns and independent lifecycles
- Banner is best-effort: fetch failures are caught and swallowed silently, banner simply doesn't appear rather than showing an error state
- Used `cancelled` flag pattern to prevent `setState` after component unmount

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- INV-07 complete: admin sees low-stock banner on cafe page at a glance
- Phase 153 complete (both plans shipped)
- TypeScript type check passes with zero errors across the web project

---
*Phase: 153-inventory-alerts*
*Completed: 2026-03-22*
