---
phase: 13-leaderboard-core
plan: 05
subsystem: ui
tags: [next.js, react, pwa, leaderboard, driver-profile, mobile-responsive]

# Dependency graph
requires:
  - phase: 13-leaderboard-core
    provides: "Public API endpoints for leaderboard, circuit/vehicle records, driver search and profile (Plans 02 + 04)"
provides:
  - "PWA leaderboard page with sim_type filter and show_invalid toggle"
  - "PWA circuit/vehicle records page with car filter"
  - "PWA driver search page with debounced search"
  - "PWA driver profile page with stats, personal bests, lap history"
  - "publicApi client methods for all 5 new endpoint types"
affects: [14-events-championships, 15-telemetry-driver-rating]

# Tech tracking
tech-stack:
  added: []
  patterns: ["use client SPA pattern with publicApi fetch layer", "mobile-first card layout below 640px, table on desktop", "monospace font for lap times at 14px minimum"]

key-files:
  created:
    - "pwa/src/app/records/page.tsx"
    - "pwa/src/app/drivers/page.tsx"
    - "pwa/src/app/drivers/[id]/page.tsx"
  modified:
    - "pwa/src/lib/api.ts"
    - "pwa/src/app/leaderboard/public/page.tsx"

key-decisions:
  - "Inline time formatting utility (formatLapTime) in each page rather than shared module — minimal duplication, avoids premature abstraction"
  - "Debounced driver search at 300ms with 2-char minimum to avoid excessive API calls"
  - "class_badge conditionally rendered only when non-null — ready for Phase 15 RAT-01 without placeholder UI"

patterns-established:
  - "Mobile card layout: position + driver name row 1, car + time row 2 below 640px breakpoint"
  - "Brand color consistency: #E10600 red accent, #222222 cards, #333333 borders, monospace times"
  - "404 handling on driver profile: graceful 'Driver not found' with link back to search"

requirements-completed: [PUB-01, PUB-02]

# Metrics
duration: 12min
completed: 2026-03-15
---

# Phase 13 Plan 05: PWA Pages Summary

**Public leaderboard with sim_type/invalid filter, circuit records with car filter, driver search with debounced input, and driver profile with stats/personal bests/lap history -- all mobile-first at 375px**

## Performance

- **Duration:** 12 min
- **Started:** 2026-03-14T21:55:00Z
- **Completed:** 2026-03-14T22:06:08Z
- **Tasks:** 3 (2 auto + 1 checkpoint)
- **Files modified:** 5

## Accomplishments
- Leaderboard page updated with sim_type dropdown (Assetto Corsa / F1 25) and show_invalid toggle, re-fetching on filter change
- Records page built with circuit records grouped by track, car filter dropdown, and vehicle records view
- Driver search page with debounced 300ms search input, avatar/initials display, and grid layout
- Driver profile page showing stats cards, personal bests table, and full lap history with sector times and validity badges
- All pages mobile-responsive at 375px with brand colors (#E10600, #222222) and minimum font sizes enforced

## Task Commits

Each task was committed atomically:

1. **Task 1: Add publicApi methods and build leaderboard + records pages** - `d880959` (feat)
2. **Task 2: Build driver search and profile pages** - `abff4b4` (feat)
3. **Task 3: Visual verification checkpoint** - no commit (human approval)

**Plan metadata:** pending (docs: complete plan)

## Files Created/Modified
- `pwa/src/lib/api.ts` - Added 5 publicApi methods: trackLeaderboard (updated with params), circuitRecords, vehicleRecords, searchDrivers, driverProfile
- `pwa/src/app/leaderboard/public/page.tsx` - Updated with sim_type dropdown, show_invalid toggle, mobile card layout
- `pwa/src/app/records/page.tsx` - New page: circuit records grouped by track with car filter and vehicle records view
- `pwa/src/app/drivers/page.tsx` - New page: driver search with debounced input, result cards in responsive grid
- `pwa/src/app/drivers/[id]/page.tsx` - New page: driver profile with stats, personal bests, lap history, sector times

## Decisions Made
- Inline time formatting utility (formatLapTime) in each page rather than shared module -- minimal duplication, avoids premature abstraction
- Debounced driver search at 300ms with 2-char minimum to avoid excessive API calls
- class_badge conditionally rendered only when non-null -- ready for Phase 15 RAT-01 without placeholder UI

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- All Phase 13 plans (01-05) are complete -- Phase 13 Leaderboard Core is finished
- Public PWA pages consume all backend endpoints built in Plans 02 and 04
- Phase 14 (Events and Championships) can build on the established PWA page patterns and publicApi layer
- Phase 15 driver rating will populate the class_badge field already wired into the driver profile page

## Self-Check: PASSED

All 5 source files verified on disk. Both task commits (d880959, abff4b4) found in git history. Summary file exists.

---
*Phase: 13-leaderboard-core*
*Completed: 2026-03-15*
