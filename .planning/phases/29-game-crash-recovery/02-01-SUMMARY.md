---
phase: 02-game-crash-recovery
plan: 01
subsystem: core
tags: [rust, axum, billing-pause, crash-recovery, relaunch-api]

requires: []
provides:
  - "Billing auto-pause on GameCrashed (PausedGamePause transition in ws/mod.rs)"
  - "POST /games/relaunch/:pod_id endpoint for manual crash recovery"
  - "relaunch_game() function in game_launcher.rs"
affects: [03-launch-resilience]

key-files:
  modified:
    - crates/racecontrol/src/ws/mod.rs
    - crates/racecontrol/src/game_launcher.rs
    - crates/racecontrol/src/api/routes.rs

requirements-completed: [CRASH-02, CRASH-04]

duration: 5min
completed: 2026-03-15
---

# Phase 2 Plan 01: Billing Auto-Pause + Relaunch Endpoint

## Accomplishments
- GameCrashed handler in ws/mod.rs now pauses billing (PausedGamePause) when billing_active=true
- relaunch_game() in game_launcher.rs reads stored launch_args, resets auto_relaunch_count, sends LaunchGame
- POST /games/relaunch/:pod_id registered in routes.rs
- When game relaunches and reports AcStatus::Live, billing resumes automatically (existing path)
- Also fixed: ExecResult match arm added to ws/mod.rs (pre-existing unhandled variant)

## Commits
- `d9c2cb0` feat(02-01): billing auto-pause on crash + manual relaunch endpoint

## Files Modified
- `crates/racecontrol/src/ws/mod.rs` — BillingSessionStatus import + pause logic in GameCrashed handler
- `crates/racecontrol/src/game_launcher.rs` — relaunch_game() function
- `crates/racecontrol/src/api/routes.rs` — POST /games/relaunch/:pod_id handler + route

---
*Phase: 02-game-crash-recovery*
*Completed: 2026-03-15*
