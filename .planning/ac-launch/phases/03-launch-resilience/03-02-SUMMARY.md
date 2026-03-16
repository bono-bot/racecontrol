---
phase: 03-launch-resilience
plan: 02
subsystem: billing, ui
tags: [rust, axum, react, typescript, billing, kiosk, diagnostics]

# Dependency graph
requires:
  - phase: 03-launch-resilience/01
    provides: "LaunchDiagnostics struct in rc-common, diagnostics field on GameLaunchInfo, agent threading"
  - phase: 02-game-crash-recovery
    provides: "PausedGamePause billing status, Race Engineer auto-relaunch logic"
provides:
  - "Billing auto-pause (PausedGamePause) when all auto-relaunch attempts exhausted"
  - "Kiosk structured diagnostics display (CM status, exit code, log errors, fallback)"
  - "Context-aware 'Launch Failed' vs 'Crashed' labels on kiosk dashboard"
affects: [04-multiplayer-server-lifecycle, kiosk-dashboard]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Billing pause on launch exhaustion reuses existing PausedGamePause status from Phase 2"
    - "serde(default) on LaunchDiagnostics fields for rolling deploy compatibility"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/game_launcher.rs
    - kiosk/src/lib/types.ts
    - kiosk/src/components/LiveSessionPanel.tsx
    - kiosk/src/components/KioskPodCard.tsx

key-decisions:
  - "Reused PausedGamePause status (from Phase 2 crash recovery) for launch failure billing pause -- no new enum variant needed"
  - "StateLabel component receives gameInfo prop for context-aware crashed/launch-failed label"

patterns-established:
  - "Diagnostics-aware error banners: check gameInfo.diagnostics first, fall back to error_message string"

requirements-completed: [LAUNCH-02, LAUNCH-03]

# Metrics
duration: 5min
completed: 2026-03-15
---

# Phase 3 Plan 02: Billing Auto-Pause on Launch Failure + Kiosk Diagnostics Display Summary

**Billing auto-pauses (PausedGamePause) when Race Engineer exhausts 2 relaunch attempts, kiosk shows structured CM/fallback diagnostics instead of raw error strings**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-15T09:12:16Z
- **Completed:** 2026-03-15T09:17:28Z
- **Tasks:** 1
- **Files modified:** 4

## Accomplishments
- Billing automatically pauses when all 2 auto-relaunch attempts are exhausted, preventing customers from paying for downtime
- Kiosk LiveSessionPanel and KioskPodCard show structured launch diagnostics (CM exit code, log errors, fallback status) when available
- Pod card state label and error banner distinguish "Launch Failed" from "Game Crashed" based on whether CM was attempted
- Relaunch button continues to work unchanged -- existing endpoint resumes billing when game reaches Running state

## Task Commits

Each task was committed atomically:

1. **Task 1: Auto-pause billing on relaunch exhaustion + kiosk diagnostics** - `48f59ad` (feat)

## Files Created/Modified
- `crates/racecontrol/src/game_launcher.rs` - Added billing pause (PausedGamePause) in Race Engineer relaunch-limit-reached branch
- `kiosk/src/lib/types.ts` - Added LaunchDiagnostics TypeScript interface + diagnostics field on GameLaunchInfo
- `kiosk/src/components/LiveSessionPanel.tsx` - Structured diagnostics display in error banner (CM status, log errors, fallback info)
- `kiosk/src/components/KioskPodCard.tsx` - Context-aware "Launch Failed" vs "Crashed" in StateLabel + structured diagnostics in card error banner

## Decisions Made
- Reused existing `PausedGamePause` billing status from Phase 2 crash recovery -- avoids new enum variant, keeps billing state machine simple
- Only pause billing when status is `Active` (guard prevents double-pause if already paused for another reason)
- StateLabel component receives optional `gameInfo` prop to derive context-aware label -- minimal interface change

## Deviations from Plan

None - plan executed exactly as written. The `to_info()` diagnostics field was already present from Plan 03-01.

## Issues Encountered

None

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Phase 3 (Launch Resilience) is now complete -- both plans done
- Phase 4 (Multiplayer Server Lifecycle) can proceed
- All billing-game lifecycle paths covered: normal billing, crash pause, relaunch resume, launch failure pause

---
*Phase: 03-launch-resilience*
*Completed: 2026-03-15*
