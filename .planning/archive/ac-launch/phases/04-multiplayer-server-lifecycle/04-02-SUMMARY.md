---
phase: 04-multiplayer-server-lifecycle
plan: 02
subsystem: kiosk
tags: [multiplayer, kiosk, booking-wizard, UI]
dependency_graph:
  requires: [04-01]
  provides: [kiosk-multiplayer-booking-flow]
  affects: [kiosk/src/app/book/page.tsx, kiosk/src/lib/api.ts, kiosk/src/lib/types.ts]
tech_stack:
  added: []
  patterns: [conditional-booking-handler, multi-success-screen, pod-count-selector]
key_files:
  created: []
  modified:
    - kiosk/src/lib/types.ts
    - kiosk/src/lib/api.ts
    - kiosk/src/app/book/page.tsx
decisions:
  - Replaced old multiplayer_lobby (join/create server UI) with pod count selector for kiosk self-serve flow
  - Success screen branches on multiAssignments.length for multiplayer vs single display
  - Review button text changes to "BOOK N RIGS" in multi mode for clarity
metrics:
  duration: 3min
  completed: 2026-03-15
---

# Phase 4 Plan 02: Kiosk Multiplayer Booking Wizard Summary

Kiosk "Play with Friends" flow: pod count selector (2-8), multiplayer booking via POST /kiosk/book-multiplayer, success screen showing per-friend PIN + pod number cards.

## Tasks Completed

| # | Task | Commit | Key Changes |
|---|------|--------|-------------|
| 1 | TypeScript types + API client | f42260a | KioskMultiplayerAssignment + KioskMultiplayerResult interfaces, api.kioskBookMultiplayer() |
| 2 | Play with Friends flow + multi-success screen | 3122183 | Pod count selector, handleBookMultiplayer(), conditional success screen, state reset |

## What Was Built

### Task 1: TypeScript Types + API Client
- Added `KioskMultiplayerAssignment` interface (pin, pod_id, pod_number, role) to types.ts
- Added `KioskMultiplayerResult` interface (group_session_id, experience_name, tier_name, allocated_seconds, assignments[]) to types.ts
- Added `api.kioskBookMultiplayer()` to api.ts -- sends POST to `/kiosk/book-multiplayer` with Bearer auth token
- Types match the Plan 04-01 API response structure exactly

### Task 2: Booking Wizard Multiplayer Flow
- **multiplayer_lobby step**: Replaced old join/create server UI with pod count selector grid (2-8 rigs)
- **handleBookMultiplayer()**: New booking handler that calls `api.kioskBookMultiplayer()` with pricing_tier_id, pod_count, and experience/custom config
- **Review step**: Shows "Rigs: N rigs" row when in multi mode; button text changes to "BOOK N RIGS"
- **Success screen**: Branches on `multiAssignments.length > 0`:
  - Multi: shows card per friend with Rig number + PIN digits + "You" / "Friend N" label
  - Single: unchanged (big pod number + PIN display)
- **State reset**: podCount, multiAssignments, multiExperienceName cleared on cancel/back-to-phone

## Decisions Made

1. **Replaced multiplayer_lobby content** -- The old join/create server UI was for a different multiplayer model. The kiosk self-serve "Play with Friends" flow only needs a pod count selector (the server is auto-managed by racecontrol).
2. **Success screen conditional branch** -- Used `multiAssignments.length > 0` as the discriminator rather than `playerMode === "multi"` to ensure the multi display only shows when the API actually returned assignments.
3. **Button text differentiation** -- Changed "BOOK SESSION" to "BOOK N RIGS" in multi mode so customers see their pod count reflected in the CTA.

## Deviations from Plan

None -- plan executed exactly as written.

## Verification Results

1. `npx tsc --noEmit` -- clean (exit 0), no type errors
2. types.ts has KioskMultiplayerAssignment and KioskMultiplayerResult interfaces
3. api.ts has kioskBookMultiplayer function with Bearer auth
4. page.tsx multiplayer_lobby step shows pod count grid (2-8)
5. page.tsx success screen has isMulti branch showing per-friend assignments
6. page.tsx handleBook() unchanged for single-player flow
7. Single-player booking flow completely unmodified
