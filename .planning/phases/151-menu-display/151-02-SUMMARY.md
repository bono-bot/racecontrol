---
phase: 151-menu-display
plan: "02"
subsystem: ui
tags: [next.js, pwa, cafe, menu, react]

# Dependency graph
requires:
  - phase: 151-menu-display
    provides: "Backend /api/v1/cafe/menu endpoint with CafeMenuItem data"
provides:
  - "PWA /cafe route: category-grouped menu with card grid, image fallback, price formatting"
  - "publicApi.cafeMenu() and CafeMenuItem/CafeMenuResponse types in api.ts"
  - "getImageBaseUrl() helper for static image path resolution"
  - "Cafe tab in BottomNav replacing Stats tab"
affects: [pwa, bottom-nav, cafe]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "publicApi pattern for unauthenticated endpoints (same as leaderboard, time-trial)"
    - "React useState imgError pattern for graceful image fallback (no innerHTML manipulation)"
    - "getImageBaseUrl() strips /api/v1 suffix to build static file base URL"

key-files:
  created:
    - pwa/src/app/cafe/layout.tsx
    - pwa/src/app/cafe/page.tsx
  modified:
    - pwa/src/lib/api.ts
    - pwa/src/components/BottomNav.tsx

key-decisions:
  - "Replaced Stats tab with Cafe in BottomNav (7-tab max for mobile; stats accessible from profile)"
  - "Image error fallback uses React useState imgError to avoid innerHTML (XSS-safe)"
  - "getImageBaseUrl() strips /api/v1 suffix since NEXT_PUBLIC_API_URL includes it but static files are served at root"

patterns-established:
  - "React image error fallback: useState(false) + onError={() => setImgError(true)} + conditional render"

requirements-completed: [MENU-08]

# Metrics
duration: 15min
completed: 2026-03-22
---

# Phase 151 Plan 02: Cafe Menu PWA Display Summary

**Next.js PWA /cafe page with 2-column card grid grouped by category, image fallback, paise-to-rupees price formatting, and Cafe tab replacing Stats in BottomNav**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-03-22T14:45:00+05:30
- **Completed:** 2026-03-22T15:00:00+05:30
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Added CafeMenuItem + CafeMenuResponse types and publicApi.cafeMenu() to api.ts
- Added getImageBaseUrl() helper to strip /api/v1 suffix for static image serving
- Replaced Stats tab with Cafe tab (coffee cup SVG) in BottomNav
- Created auth-gated /cafe layout (same pattern as /dashboard)
- Created /cafe page (214 lines): category filter pills, 2-column item cards, loading skeletons, empty state, image fallback via React state, Rs. price formatting

## Task Commits

Each task was committed atomically:

1. **Task 1: Add cafe menu types and API method, add Cafe tab to BottomNav** - `06278ab5` (feat)
2. **Task 2: Build PWA cafe menu page with category sections and item cards** - `501612e1` (feat)

## Files Created/Modified
- `pwa/src/lib/api.ts` - Added CafeMenuItem, CafeMenuResponse interfaces; getImageBaseUrl() helper; publicApi.cafeMenu() method
- `pwa/src/components/BottomNav.tsx` - Replaced Stats tab with Cafe tab (href=/cafe, coffee cup icon)
- `pwa/src/app/cafe/layout.tsx` - Auth-gated layout with BottomNav, same pattern as dashboard
- `pwa/src/app/cafe/page.tsx` - Full cafe menu page: category filter, 2-col grid, item cards with image/placeholder, description, price

## Decisions Made
- Replaced Stats tab with Cafe in BottomNav — 7 tabs is max for mobile bottom nav; stats are still accessible via /stats direct URL or from profile page
- Image error fallback uses React useState(imgError) + onError handler instead of DOM innerHTML manipulation — XSS-safe, idiomatic React
- getImageBaseUrl() strips /api/v1 suffix — NEXT_PUBLIC_API_URL always includes /api/v1 but static files (cafe-images/) are served at the host root

## Deviations from Plan

None - plan executed exactly as written. The one deviation worth noting: the security hook flagged an initial implementation that used innerHTML for image error fallback. Refactored to React useState pattern (cleaner, XSS-safe, idiomatic) before the file was committed — no committed deviation.

## Issues Encountered
- Security hook flagged innerHTML usage in first image error fallback attempt. Refactored to useState(imgError) pattern immediately — not committed.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- /cafe page is live and connects to /api/v1/cafe/menu
- MENU-08 complete: customers can browse cafe menu from PWA while at venue
- Ready for any follow-on phases (cart, ordering, etc.)

---
*Phase: 151-menu-display*
*Completed: 2026-03-22*
