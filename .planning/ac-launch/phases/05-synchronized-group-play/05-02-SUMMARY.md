---
phase: 05-synchronized-group-play
plan: 02
subsystem: multiplayer
tags: [assetto-corsa, multiplayer, join-failure, retry, continuous-mode, kiosk-dashboard]

# Dependency graph
requires:
  - phase: 05-01
    provides: "Coordinated AC launch, continuous mode, set_continuous_mode(), POST /ac/session/{id}/continuous"
provides:
  - "retry_pod_join(): re-sends StopGame then LaunchGame to a single failed pod for a running session"
  - "update_session_config(): updates track/car config on continuous-mode session (takes effect next restart)"
  - "POST /ac/session/retry-pod and POST /ac/session/update-config endpoints"
  - "KioskPodCard shows 'Join Failed' (orange) for multiplayer pods with game_state=error"
  - "'Retry Join' button on join-failed pods calls api.retryPodJoin()"
  - "useKioskSocket tracks ac_server_update and group_session_all_validated events"
  - "staff/page.tsx wires acSessionId (from multiplayerGroup.pod_ids) and onRetryJoin to KioskPodCard"
affects: [group-play, kiosk-dashboard, multiplayer-recovery]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Stop-before-retry: StopGame sent 500ms before re-launching to kill stuck process"
    - "join_failed as distinct KioskPodState — separate from crashed, own UI block at top level"
    - "multiplayerGroup from group_session_all_validated is authoritative for pod membership (not acServerInfo)"

key-files:
  created: []
  modified:
    - crates/rc-core/src/ac_server.rs
    - crates/rc-core/src/api/routes.rs
    - kiosk/src/lib/types.ts
    - kiosk/src/hooks/useKioskSocket.ts
    - kiosk/src/components/KioskPodCard.tsx
    - kiosk/src/lib/api.ts
    - kiosk/src/app/staff/page.tsx

key-decisions:
  - "join_failed block is a top-level KioskPodCard section (sibling of on_track), not nested inside on_track — TypeScript narrows state to 'on_track' inside that block, making the join_failed check unreachable"
  - "multiplayerGroup.pod_ids (from group_session_all_validated) is the source of truth for pod membership, not acServerInfo (which only has connected_pods)"
  - "acServerInfo destructured from useKioskSocket for future use (config change UI), not yet used in JSX"

requirements-completed: [GROUP-03, GROUP-04]

# Metrics
duration: 20min
completed: 2026-03-16
---

# Phase 5 Plan 02: Synchronized Group Play — Join Failure Recovery + Config Change Summary

**Per-pod join status tracking on kiosk dashboard with 'Join Failed' + 'Retry Join' button for failed multiplayer pods, and mid-session track/car config change for continuous mode between races**

## Performance

- **Duration:** 20 min
- **Started:** 2026-03-16T03:35:00Z
- **Completed:** 2026-03-16T03:55:09Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments

- Added `retry_pod_join()` to ac_server.rs — sends StopGame then LaunchGame to a specific pod for an active session, resets game tracker to Launching state
- Added `update_session_config()` to ac_server.rs — updates track/car on a continuous-mode session; monitor loop re-reads config on next restart
- Registered `POST /ac/session/retry-pod` and `POST /ac/session/update-config` in routes.rs
- Added `AcServerInfo` and `MultiplayerGroupStatus` types to kiosk types.ts
- Extended `KioskPodState` with `"join_failed"` variant (orange styling)
- useKioskSocket handles `ac_server_update` and `group_session_all_validated` WebSocket events; exposes `acServerInfo` and `multiplayerGroup` in return object
- KioskPodCard shows "Join Failed" banner with "Retry Join" button for multiplayer pods in error state
- api.ts adds `retryPodJoin()`, `updateAcSessionConfig()`, `setAcContinuousMode()` calls
- staff/page.tsx wires `acSessionId` (from `multiplayerGroup.pod_ids`) and `onRetryJoin` handler to each KioskPodCard

## Task Commits

1. **Task 1: Backend retry-pod + update-config endpoints** - `0213ddd` (feat)
2. **Task 2: Kiosk join status tracking and retry button** - `93f8582` (feat)

## Files Created/Modified

- `crates/rc-core/src/ac_server.rs` — Added `retry_pod_join()` (StopGame + LaunchGame + GameTracker reset) and `update_session_config()` (continuous-mode config mutation + broadcast)
- `crates/rc-core/src/api/routes.rs` — Added `POST /ac/session/retry-pod` and `POST /ac/session/update-config` routes + handlers; no duplicate continuous handler
- `kiosk/src/lib/types.ts` — Added `AcServerInfo`, `MultiplayerGroupStatus` interfaces; added `"join_failed"` to `KioskPodState`
- `kiosk/src/hooks/useKioskSocket.ts` — Added `AcServerInfo`/`MultiplayerGroupStatus` imports; added `acServerInfo`/`multiplayerGroup` state; added `ac_server_update` and `group_session_all_validated` switch cases; exposed both in return object
- `kiosk/src/components/KioskPodCard.tsx` — Added `onRetryJoin`/`acSessionId` props; updated `derivePodState()` with `isMultiplayerPod` param; added `"join_failed"` to compact/full border classes and StateLabel; added `join_failed` top-level content block with "Retry Join" button
- `kiosk/src/lib/api.ts` — Added `retryPodJoin()`, `updateAcSessionConfig()`, `setAcContinuousMode()` API calls
- `kiosk/src/app/staff/page.tsx` — Destructures `acServerInfo`/`multiplayerGroup` from `useKioskSocket`; passes `acSessionId` and `onRetryJoin` to each KioskPodCard in the grid

## Decisions Made

- `join_failed` is implemented as a separate top-level KioskPodCard JSX block (not nested inside `state === "on_track"`) because TypeScript narrows `state` to `"on_track"` inside that block, making the `state === "join_failed"` check unreachable with TS error TS2367
- `multiplayerGroup.pod_ids` (from `group_session_all_validated`) is the source of truth for pod membership — `AcServerInfo` in TypeScript only has `connected_pods`, not `assigned_pods`, so cannot reliably determine which pods belong to the group
- `acServerInfo` is exposed from useKioskSocket for future use (config change UI in a later plan) but not yet wired to JSX in staff/page.tsx

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Moved join_failed block from inside on_track to top-level sibling**
- **Found during:** Task 2 TypeScript compilation
- **Issue:** Plan placed the "Join Failed" banner inside the `{state === "on_track" && billing && (...)}` JSX block. TypeScript narrows `state` to `"on_track"` inside that block, making `state !== "join_failed"` always true and `state === "join_failed"` unreachable — TS2367 errors
- **Fix:** Removed the banner from the on_track block; added a new top-level `{state === "join_failed" && billing && (...)}` block as a sibling, which includes the driver name, Join Failed banner with Retry Join button, and a minimal End button
- **Files modified:** kiosk/src/components/KioskPodCard.tsx
- **Verification:** `npx tsc --noEmit` passes with no errors
- **Committed in:** 93f8582 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 - Bug)
**Impact on plan:** Structural fix required for TypeScript correctness. Same visual behavior — join_failed pods show the banner and retry button. No scope creep.

## Issues Encountered

- TypeScript correctly caught that `state` is narrowed to `"on_track"` inside the on_track block — comparing it against `"join_failed"` is a type error. The fix (making it a top-level sibling block) is cleaner and correct.

## Next Phase Readiness

- Phase 5 Plan 02 is the final plan in the ac-launch GSD project
- All 5 phases complete: Billing-Game Lifecycle, Crash Recovery, Launch Resilience, Multiplayer Server Lifecycle, Synchronized Group Play
- 238 tests passing (all rc-core unit tests)
- TypeScript compiles cleanly

## Self-Check: PASSED

- SUMMARY.md: FOUND at .planning/ac-launch/phases/05-synchronized-group-play/05-02-SUMMARY.md
- Commit 0213ddd: FOUND (feat(05-02): add retry-pod and update-config endpoints)
- Commit 93f8582: FOUND (feat(05-02): kiosk join status tracking and retry button)
- Commit e4ea573: FOUND (docs(05-02): complete join failure recovery plan)
- retry_pod_join in ac_server.rs: FOUND (3 occurrences)
- update_session_config in ac_server.rs: FOUND
- retry-pod route in routes.rs: FOUND
- update-config route in routes.rs: FOUND
- join_failed in KioskPodCard.tsx: FOUND (6 occurrences)
- retryPodJoin in api.ts: FOUND
- ac_server_update in useKioskSocket.ts: FOUND
- onRetryJoin in staff/page.tsx: FOUND
- acSessionId in staff/page.tsx: FOUND
- TypeScript: compiles with no errors
- Rust tests: 238/238 passing

---
*Phase: 05-synchronized-group-play*
*Completed: 2026-03-16*
