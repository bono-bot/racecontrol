---
phase: 81-game-launch-core
plan: 01
subsystem: api
tags: [rust, axum, crash-recovery, game-launch, websocket, dashboard-events]

# Dependency graph
requires:
  - phase: 74-rc-agent-decomposition
    provides: AppState and game_process module structure used in crash recovery wiring
provides:
  - Non-AC crash recovery calling GameProcess::launch() in PausedWaitingRelaunch state machine
  - DashboardEvent::GameLaunchRequested variant for PWA-to-staff broadcast
  - POST /api/v1/customer/game-request endpoint with pod/game validation
affects: [82-customer-auth, game-launcher, dashboard-ws, rc-agent-crash-recovery]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Crash recovery non-AC relaunch mirrors LaunchGame handler's generic-sim branch exactly"
    - "pwa_game_request uses extract_driver_id() in-handler (customer JWT, same pattern as all customer routes)"
    - "Fire-and-forget broadcast: game request broadcasts GameLaunchRequested to dashboard; staff confirms via existing launch endpoint"

key-files:
  created: []
  modified:
    - crates/rc-agent/src/main.rs
    - crates/rc-common/src/protocol.rs
    - crates/racecontrol/src/api/routes.rs

key-decisions:
  - "Non-AC crash recovery else branch: match last_sim_type to config.games field (7 variants), clone base_config, override args from last_launch_args, then call GameProcess::launch() -- exact mirror of LaunchGame handler"
  - "GameLaunchRequested added at end of DashboardEvent enum before closing brace -- uses existing SimType (no new imports)"
  - "pwa_game_request uses customer JWT (extract_driver_id) not staff JWT -- customer-initiated request, Phase 82+ may add tower middleware layer"
  - "No pending_game_requests HashMap added to AppState -- fire-and-forget broadcast; staff confirmation uses existing POST /api/v1/games/pod/{id}/launch (per plan instruction)"
  - "GameRequestBody has pod_id + sim_type; driver_name fetched from DB using driver_id from JWT"

patterns-established:
  - "Customer game request: validate (pod exists, game installed) -> generate UUID request_id -> broadcast DashboardEvent -> return { ok, request_id }"

requirements-completed: [LAUNCH-02, LAUNCH-04, LAUNCH-05]

# Metrics
duration: 40min
completed: 2026-03-21
---

# Phase 81 Plan 01: Game Launch Core Summary

**Non-AC game crash auto-recovery via GameProcess::launch() + DashboardEvent::GameLaunchRequested + POST /api/v1/customer/game-request PWA endpoint**

## Performance

- **Duration:** ~40 min
- **Started:** 2026-03-21T01:00:00Z (approx)
- **Completed:** 2026-03-21T01:40:49Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- Non-AC game crashes (F1 25, iRacing, LMU, Forza, ACEvo, ACRally, ForzaH5) now auto-relaunch via GameProcess::launch() instead of logging a warning and leaving the pod dead
- DashboardEvent::GameLaunchRequested variant added to rc-common protocol -- enables type-safe broadcast of customer game requests to staff dashboard
- PWA customers can now POST /api/v1/customer/game-request to request a game; validated against pod registry and installed_games list before broadcast

## Task Commits

1. **Task 1: Non-AC crash recovery + DashboardEvent variant** - `ce15e55` (feat)
2. **Task 2: PWA game request endpoint** - `e04805c` (feat)

**Plan metadata:** committed with SUMMARY.md

## Files Created/Modified

- `crates/rc-agent/src/main.rs` - Replaced warn-only stub in PausedWaitingRelaunch else branch with full GameProcess::launch() relaunch; all 7 non-AC sim types mapped to config.games fields
- `crates/rc-common/src/protocol.rs` - Added GameLaunchRequested { pod_id, sim_type, driver_name, request_id } variant to DashboardEvent enum
- `crates/racecontrol/src/api/routes.rs` - Added GameRequestBody struct, pwa_game_request handler, and route registration at /customer/game-request

## Decisions Made

- Non-AC crash recovery matches the LaunchGame handler's generic-sim branch exactly: same config.games match, same game_cfg clone + args override, same Launching state send, same failure_monitor_tx update
- No AppState changes for pending requests -- fire-and-forget broadcast is sufficient for Phase 81; persistence deferred per plan specification
- Customer JWT (extract_driver_id in-handler) used for pwa_game_request -- consistent with all other customer routes; driver_name fetched from DB for broadcast payload

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- Release build (`cargo build --release --bin racecontrol`) showed a pre-existing E0034 error in `crypto/encryption.rs` on first run but resolved on subsequent build (stale artifact from prior sessions). Dev build (`cargo build --bin racecontrol`) was clean throughout.
- `cargo test -p rc-common` fails to execute test binary due to Windows Application Control (WDAC) policy blocking unsigned test executables. Both crates compile cleanly -- this is an OS-level execution restriction, not a code issue.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Backend infrastructure for multi-game launch fully wired: crash recovery, broadcast event, and PWA request endpoint all in place
- Phase 82 (Customer Auth) can promote pwa_game_request to tower middleware JWT validation
- Staff kiosk can display GameLaunchRequested events from dashboard WebSocket and present confirm/deny UI

---
*Phase: 81-game-launch-core*
*Completed: 2026-03-21*
