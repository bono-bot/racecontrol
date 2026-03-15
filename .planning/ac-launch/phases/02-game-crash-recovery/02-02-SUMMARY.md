---
phase: 02-game-crash-recovery
plan: 02
subsystem: kiosk
tags: [nextjs, typescript, kiosk-ui, crash-badge, relaunch-button]

requires: [02-01]
provides:
  - "Game Crashed badge on KioskPodCard (red border + CRASHED label)"
  - "Relaunch Game button on KioskPodCard and LiveSessionPanel"
  - "relaunchGame API method in api.ts"
affects: []

key-files:
  modified:
    - kiosk/src/lib/types.ts
    - kiosk/src/lib/api.ts
    - kiosk/src/components/KioskPodCard.tsx
    - kiosk/src/components/LiveSessionPanel.tsx
    - kiosk/src/app/staff/page.tsx

requirements-completed: [CRASH-03, CRASH-04]

duration: 3min
completed: 2026-03-15
---

# Phase 2 Plan 02: Kiosk Game Crashed UI

## Accomplishments
- KioskPodCard shows "Crashed" state label with red border when game_state === "error"
- Red "Game Crashed" banner with error message on KioskPodCard and LiveSessionPanel
- "Relaunch Game" button (red) calls POST /games/relaunch/{pod_id}
- "crashed" added to KioskPodState union type
- relaunchGame API method added to api.ts
- Wired in staff/page.tsx for both card and panel views
- TypeScript compiles cleanly

## Commits
- `6e94eb0` feat(02-02): kiosk Game Crashed badge + Relaunch button

## Files Modified
- `kiosk/src/lib/types.ts` — Added "crashed" to KioskPodState
- `kiosk/src/lib/api.ts` — Added relaunchGame method
- `kiosk/src/components/KioskPodCard.tsx` — Crash state in derivePodState, red badge, relaunch button
- `kiosk/src/components/LiveSessionPanel.tsx` — Crash banner + relaunch button
- `kiosk/src/app/staff/page.tsx` — Wired onRelaunchGame prop

---
*Phase: 02-game-crash-recovery*
*Completed: 2026-03-15*
