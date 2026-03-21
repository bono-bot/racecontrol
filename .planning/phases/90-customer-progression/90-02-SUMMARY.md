---
phase: 90-customer-progression
plan: 90-02
status: complete
started: 2026-03-21
completed: 2026-03-21
duration_minutes: 15
---

# Plan 90-02 Summary

## Objective
Build the PWA passport page and badge showcase on the profile page.

## What Was Built

### Task 1: API methods + Passport page + Profile badge showcase
- **pwa/src/lib/api.ts**: Added `passport()` and `badges()` methods calling `/customer/passport` and `/customer/badges`
- **pwa/src/app/passport/page.tsx**: New page with:
  - 4-stat summary card (Tracks Driven, Cars Driven, Total Laps, Week Streak)
  - Circuits section with Starter/Explorer/Legend tiers, progress bars, 3-column collection grid
  - Cars section with Starter Garage/Explorer Garage/Legend Garage tiers
  - Driven items at full opacity, undriven at opacity-30
  - Loading spinner, empty state, error state with "Pull to refresh"
- **pwa/src/app/profile/page.tsx**: Added:
  - Badge showcase card (5 badges in grid, earned = rp-red tint, locked = opacity-30)
  - Driving Passport link row with "N circuits · N cars driven" subtitle
  - BadgeIcon component with inline SVG icons (flag, map, trophy, car, zap)

### Task 2: Visual Verification (Checkpoint)
- Verified via Playwright screenshots
- Passport page renders all tiers correctly with track/car names and categories
- Profile page shows badges section (0/5 locked) and passport link row
- All UI follows the approved UI-SPEC.md design contract

## Commits
- `fd2e76c`: feat(90-02): passport page, badge showcase, and profile passport link

## Key Files
- `pwa/src/app/passport/page.tsx` (new)
- `pwa/src/app/profile/page.tsx` (modified)
- `pwa/src/lib/api.ts` (modified)

## Deviations
None — implementation follows UI-SPEC.md exactly.

## Self-Check: PASSED
- Passport page loads with tiered collections
- Profile badges section shows 5 badges with icons
- Profile passport link shows "N circuits · N cars driven"
- PWA builds without errors
- All existing pages unaffected
