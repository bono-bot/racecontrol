---
phase: 265-kiosk-pages
plan: 01
subsystem: ui
tags: [kiosk, touch, tailwind, nextjs, accessibility]

requires:
  - phase: 261-design-system-foundation
    provides: shared tokens (rp-red, rp-card, rp-border, rp-grey, rp-black)
provides:
  - Touch-optimized kiosk home page with press feedback and offline count
  - Remaining-time timer on active pod cards (replaces elapsed)
affects: [265-kiosk-pages, 266-quality-gate]

tech-stack:
  added: []
  patterns:
    - "active:scale-[0.97] for touch press feedback on tappable cards"
    - "No group-hover: classes on content — touch devices get full visibility"
    - "remaining_seconds from WS billing timer drives countdown (no local elapsed state)"

key-files:
  created: []
  modified:
    - kiosk/src/app/page.tsx

key-decisions:
  - "Removed all group-hover: classes instead of adding touch equivalents — simpler, no hidden content"
  - "Used remaining_seconds directly from WS billing data instead of computing locally — single source of truth"
  - "Added focus-visible:border-rp-red for keyboard accessibility on idle pod cards"

patterns-established:
  - "Touch-first kiosk pattern: active:scale for press, no hover-only content, 44px+ targets"

requirements-completed: [KS-01]

duration: 8min
completed: 2026-03-30
---

# Phase 265 Plan 01: Kiosk Home Page Summary

**Touch-optimized pod selection grid with offline count header, active:scale press feedback, remaining-time countdown, and zero hover-only content**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-30T11:10:00Z
- **Completed:** 2026-03-30T11:18:00Z
- **Tasks:** 1 (auto) + 1 (checkpoint)
- **Files modified:** 1

## Accomplishments
- Added Offline pod count pill to KioskHeader (Available/Racing/Offline trio)
- Added active:scale-[0.97] press feedback on idle pod cards for touch devices
- Removed all group-hover: classes from pod cards (no hover-only content)
- Switched ActivePodCard from elapsed timer to remaining_seconds countdown with red pulse <5min warning
- Enforced 44px+ touch targets on Staff Login link (min-h-[44px] min-w-[44px])
- Added focus-visible:border-rp-red for keyboard accessibility

## Task Commits

Each task was committed atomically:

1. **Task 1: Add offline count, touch press feedback, remaining timer** - PENDING COMMIT (feat)

**Plan metadata:** PENDING COMMIT (docs: complete plan)

## Files Created/Modified
- `kiosk/src/app/page.tsx` - Redesigned CustomerLanding with touch-optimized pod grid, offline count header, remaining timer

## Decisions Made
- Removed hover-only classes entirely rather than duplicating with touch equivalents -- simpler code, same UX
- Used `remaining_seconds` directly from WS billing data rather than computing elapsed locally -- eliminates client-server clock drift
- Added `focus-visible:border-rp-red` on idle cards for keyboard users without affecting touch flow

## Deviations from Plan

None - plan executed exactly as written.

## Known Stubs

None - all data sources are wired to live WS data from useKioskSocket.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- KS-01 code changes complete, awaiting checkpoint:human-verify on actual pod touchscreen
- Ready for KS-02 (game launch flow) after verification

---
*Phase: 265-kiosk-pages*
*Completed: 2026-03-30*
