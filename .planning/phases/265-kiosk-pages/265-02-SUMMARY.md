---
phase: 265-kiosk-pages
plan: 02
subsystem: kiosk-ui
tags: [touch-optimization, booking-wizard, countdown-ring, pinpad, kiosk]
dependency_graph:
  requires: []
  provides: [KS-02, KS-03, KS-04]
  affects: [kiosk-booking, kiosk-fleet, staff-pin-auth]
tech_stack:
  added: []
  patterns: [inline-svg-countdown-ring, on-screen-numpad, touch-stepper-buttons]
key_files:
  created: []
  modified:
    - kiosk/src/app/book/page.tsx
    - kiosk/src/app/fleet/page.tsx
decisions:
  - Replaced AI count range slider with +/- stepper buttons for touch usability
  - Used inline SVG for CountdownRing (no external component library needed)
  - Changed PIN from 4-digit keyboard input to 6-digit on-screen numpad
  - Used billing sessions polling (5s interval) rather than WS for countdown data
metrics:
  duration: 8m
  completed: 2026-03-30T16:46:08+05:30
---

# Phase 265 Plan 02: Touch-Optimized Booking Wizard + Fleet Billing Summary

Touch-optimized kiosk booking wizard with tap-only interaction, radial countdown ring for active billing sessions, and 6-digit on-screen PinPad replacing keyboard input.

## Tasks Completed

### Task 1: Touch-optimize book/page.tsx wizard steps
**Commit:** `8ed00cb7`

- Added `active:scale-[0.97]` press feedback to 30 interactive elements across all wizard steps
- Added `min-h-[44px]` or `min-h-[60px]` touch targets to all buttons and selection cards
- Replaced the AI count `<input type="range">` slider with `+`/`-` stepper buttons (each 44x44px minimum)
- Verified no `onMouseEnter`/`onMouseLeave` handlers exist (already clean)
- Verified no hardcoded `/kiosk/` paths exist (already clean)
- All wizard phases (phone, otp, wizard, booking, success, error) already had `overflow-hidden h-screen`

### Task 2: Billing countdown ring and 6-digit PinPad in fleet/page.tsx
**Commit:** `39221b34`

- Added `CountdownRing` inline SVG component with radial progress ring
  - `stroke` color switches to `#E10600` and `animate-pulse` activates when `remaining < 300s`
  - Displays `MM:SS` text below the ring
- Added `useEffect` polling `api.activeBillingSessions()` every 5s, stored as `Map<podId, BillingSession>`
- CountdownRing renders inside pod cards when an active billing session exists for that pod
- Replaced the `<input type="password" inputMode="numeric">` keyboard PIN input with a full 6-digit on-screen numpad:
  - 6 dot boxes showing filled/empty state
  - 3x4 grid: digits 1-9, Clear, 0, Backspace
  - Verify button enabled only when `pin.length === 6`
  - All numpad buttons have `min-h-[44px]` and `active:scale-[0.97]`
- Added `min-h-[44px]` to Maintenance, Clear Maintenance, and Close buttons
- Page root changed to `h-screen overflow-hidden` to prevent scroll

## Deviations from Plan

None - plan executed exactly as written.

## Verification Results

- Build: 0 TypeScript errors (verified with `npx next build`)
- No `onMouseEnter`/`onMouseLeave` in either file (0 hits)
- No hardcoded `/kiosk/` paths in either file (0 hits)
- No `type="password"` or `inputMode` in fleet/page.tsx (0 hits - keyboard input removed)
- `CountdownRing`/`remaining_seconds` present in fleet/page.tsx (2 hits)
- `animate-pulse` present in fleet/page.tsx CountdownRing (2 hits)
- All buttons have `min-h-[44px]` (8 occurrences in fleet/page.tsx, 30+ in book/page.tsx)

## Known Stubs

None - all data sources are wired to real API endpoints.

## Self-Check: PASSED

- FOUND: kiosk/src/app/book/page.tsx
- FOUND: kiosk/src/app/fleet/page.tsx
- FOUND: .planning/phases/265-kiosk-pages/265-02-SUMMARY.md
- FOUND: 8ed00cb7
- FOUND: 39221b34
