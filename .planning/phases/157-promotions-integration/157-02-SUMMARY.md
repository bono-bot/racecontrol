---
phase: 157-promotions-integration
plan: 02
subsystem: ui
tags: [react, nextjs, typescript, promotions, cafe, pwa, kiosk]

requires:
  - phase: 157-01
    provides: "GET /cafe/promos/active endpoint returning ActivePromo[], PlaceOrderResponse enriched with discount_paise and applied_promo_name"

provides:
  - "ActivePromo type exported from pwa/src/lib/api.ts and kiosk/src/lib/types.ts"
  - "publicApi.activePromos() in PWA calling GET /cafe/promos/active"
  - "api.publicCafePromos() in kiosk calling GET /cafe/promos/active"
  - "PromoBanner component in PWA cafe page and kiosk CafeMenuPanel"
  - "Discount + promo name shown in PWA OrderConfirmation and kiosk order success"
  - "CafeOrderResponse extended with discount_paise, applied_promo_id, applied_promo_name"

affects:
  - cafe-ordering
  - promotions
  - pwa
  - kiosk

tech-stack:
  added: []
  patterns:
    - "Promo fetch is non-fatal: .catch(() => setActivePromos([])) — promo errors never break the page"
    - "No client-side discount math: all discounts read from PlaceOrderResponse.discount_paise"
    - "Array.isArray guard before setState for API responses that may return error objects"

key-files:
  created: []
  modified:
    - pwa/src/lib/api.ts
    - pwa/src/app/cafe/page.tsx
    - kiosk/src/lib/api.ts
    - kiosk/src/lib/types.ts
    - kiosk/src/components/CafeMenuPanel.tsx

key-decisions:
  - "ActivePromo placed in pwa/src/lib/api.ts (inline with other api types) and kiosk/src/lib/types.ts (kiosk type file) — consistent with each project's existing patterns"
  - "PromoBanner renders null when promos array is empty — no empty div in DOM when no active promos"
  - "discount_paise conditionally rendered only when > 0 — clean receipt when no promo applied"

patterns-established:
  - "PromoBanner: reusable display pattern for promos in both PWA and kiosk frontends"
  - "Promo fetch on mount, non-fatal, Array.isArray guard — established pattern for optional enrichment fetches"

requirements-completed: [PROMO-05, PROMO-06]

duration: 12min
completed: 2026-03-22
---

# Phase 157 Plan 02: Promotions Frontend Integration Summary

**PromoBanner + discount receipt display added to PWA cafe page and kiosk CafeMenuPanel, fetching active promos from /cafe/promos/active on mount with non-fatal error handling**

## Performance

- **Duration:** 12 min
- **Started:** 2026-03-22T23:35:00+05:30
- **Completed:** 2026-03-22T23:47:00+05:30
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments

- ActivePromo type exported from both frontends, CafeOrderResponse extended with discount fields
- PromoBanner component renders promo name + time_label strip in PWA cafe page and kiosk CafeMenuPanel
- Applied discount and promo name shown in PWA OrderConfirmation and kiosk order success screen
- All promo fetch failures are non-fatal — page works normally when no promos are active

## Task Commits

Each task was committed atomically:

1. **Task 1: Add ActivePromo type and activePromos API methods to PWA and kiosk** - `ebd626d1` (feat)
2. **Task 2: Promo banner + discount display in PWA cafe page and POS CafeMenuPanel** - `a1d05ce7` (feat)

## Files Created/Modified

- `pwa/src/lib/api.ts` - Added ActivePromo interface, extended CafeOrderResponse, added publicApi.activePromos()
- `pwa/src/app/cafe/page.tsx` - Added PromoBanner component, activePromos state+fetch, discount in OrderConfirmation
- `kiosk/src/lib/types.ts` - Added ActivePromo interface, extended CafeOrderResponse with discount fields
- `kiosk/src/lib/api.ts` - Re-exported ActivePromo, added api.publicCafePromos()
- `kiosk/src/components/CafeMenuPanel.tsx` - Added PromoBanner component, activePromos state+fetch, discount in order success

## Decisions Made

- ActivePromo placed in `pwa/src/lib/api.ts` inline with other cafe types (consistent with existing PWA pattern) and in `kiosk/src/lib/types.ts` (consistent with kiosk's type-separation pattern)
- PromoBanner renders `null` when promos array is empty — no DOM noise when no promos active
- Discount line only rendered when `discount_paise > 0` — clean checkout receipt when no promo applied

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- PROMO-05 and PROMO-06 requirements fulfilled: customers see active promos during their time window, discount visibly applied at checkout
- Ready for Phase 157 verification / QA pass
- No outstanding frontend promo work

---
*Phase: 157-promotions-integration*
*Completed: 2026-03-22*
