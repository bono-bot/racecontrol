---
phase: 81
plan: 02
subsystem: kiosk-ui
tags: [multi-game, game-picker, pwa-requests, kiosk, frontend]
dependency_graph:
  requires: [81-01]
  provides: [GamePickerPanel, GameLaunchRequestBanner, game-logos]
  affects: [kiosk/src/app/staff/page.tsx, kiosk/src/components/KioskPodCard.tsx, kiosk/src/hooks/useKioskSocket.ts]
tech_stack:
  added: []
  patterns: [onError-img-fallback, setTimeout-auto-expire, shared-display-mapping]
key_files:
  created:
    - kiosk/src/lib/gameDisplayInfo.ts
    - kiosk/src/components/GamePickerPanel.tsx
    - kiosk/src/components/GameLaunchRequestBanner.tsx
    - kiosk/public/game-logos/assetto-corsa.png
    - kiosk/public/game-logos/assetto-corsa-evo.png
    - kiosk/public/game-logos/assetto-corsa-rally.png
    - kiosk/public/game-logos/iracing.png
    - kiosk/public/game-logos/f1-25.png
    - kiosk/public/game-logos/le-mans-ultimate.png
  modified:
    - kiosk/src/app/staff/page.tsx
    - kiosk/src/components/KioskPodCard.tsx
    - kiosk/src/hooks/useKioskSocket.ts
    - kiosk/src/lib/types.ts
decisions:
  - gameDisplayInfo.ts shared utility avoids duplicating GAME_DISPLAY in GamePickerPanel and KioskPodCard
  - Placeholder 1x1 transparent PNGs for game logos -- real logos to be added by Uday later; onError fallback to abbreviation chip works immediately
  - AC included in GamePickerPanel list per locked decision; onLaunch callback routes AC to wizard, non-AC to api.launchGame directly
  - GameLaunchRequested WS event uses PascalCase (matches backend event name convention)
  - Auto-expire 60s timeout fires per-request using request_id for precise cleanup
metrics:
  duration_minutes: 5
  completed_date: "2026-03-21T07:02:10+05:30"
  tasks_completed: 2
  files_changed: 14
---

# Phase 81 Plan 02: Kiosk Multi-Game UI Summary

**One-liner:** GamePickerPanel + GameLaunchRequestBanner with direct non-AC launch, PWA request confirm/dismiss, and per-pod running game logo display

## What Was Built

### Task 1: GamePickerPanel + Game Logo Display on Pod Card

Created `kiosk/src/lib/gameDisplayInfo.ts` as a shared GAME_DISPLAY mapping (6 games: assetto_corsa, assetto_corsa_evo, assetto_corsa_rally, iracing, f1_25, le_mans_ultimate) with name, logo path, and abbreviation.

Created `kiosk/src/components/GamePickerPanel.tsx` — a panel rendered inside SidePanel when staff taps "Launch Game" on a pod card. Shows all installed games with 40x40px logos (or abbreviation chip fallback). Clicking AC opens the existing SetupWizard; clicking any other game calls `api.launchGame` directly.

Modified `kiosk/src/app/staff/page.tsx`:
- Added `"game_picker"` to `PanelMode` type
- Changed `onLaunchGame` handler on KioskPodCard to `setPanelMode("game_picker")` instead of always opening wizard
- Added GamePickerPanel rendering in SidePanel with onLaunch routing logic

Modified `kiosk/src/components/KioskPodCard.tsx`:
- Added `GameLogo` inline component (40x40px img with onError fallback to abbreviation chip)
- Shows game logo in "selecting" (launching) state next to game state text
- Shows game logo in "on_track" state next to driver name

### Task 2: GameLaunchRequestBanner + WebSocket Event Handling

Modified `kiosk/src/hooks/useKioskSocket.ts`:
- Added `GameLaunchRequest` interface (pod_id, sim_type, driver_name, request_id)
- Added `gameLaunchRequests` state array
- Added `"GameLaunchRequested"` case in WS message handler -- appends to array, sets 60s auto-expire timeout
- Added `dismissGameRequest(requestId)` callback
- Both exported from the hook

Created `kiosk/src/components/GameLaunchRequestBanner.tsx`:
- Fixed-position banner rendered above pod grid (z-index 50)
- Each request: yellow left border, "{driver} wants to play {game}" body, "Confirm Launch" red button, "Dismiss" text button
- Multiple requests stack vertically (newest first)
- 60s auto-expire shows "Request expired" text before parent removes the request

Modified `kiosk/src/app/staff/page.tsx`:
- Destructures `gameLaunchRequests` and `dismissGameRequest` from useKioskSocket
- Renders GameLaunchRequestBanner above pod grid
- onConfirm: calls `api.launchGame` + dismisses; onDismiss: dismisses immediately

Created 6 placeholder PNG files (1x1 transparent) in `kiosk/public/game-logos/` — real logos to be placed by Uday; onError fallback to abbreviation chips is fully functional.

## Deviations from Plan

None -- plan executed exactly as written.

## Commits

| Task | Hash | Description |
|------|------|-------------|
| 1 | 73244a9 | feat(81-02): GamePickerPanel + game logo display on pod card |
| 2 | 5270be2 | feat(81-02): GameLaunchRequestBanner + PWA request WebSocket handling |

## Self-Check: PASSED

All files exist, both commits verified in git log.
