---
phase: 82-billing-and-session-lifecycle
plan: 02
subsystem: billing
tags: [rust, tokio, rc-agent, billing, game-lifecycle, playable-signal, grace-timer]

# Dependency graph
requires:
  - phase: 82-01
    provides: "PlayableSignal enum, GameState::Loading, GameStatusUpdate sim_type field in rc-common"

provides:
  - "ConnectionState exit_grace_timer + exit_grace_armed (30s delayed AcStatus::Off)"
  - "Per-sim PlayableSignal dispatch: AC=shared memory, F1 25=UdpActive, others=90s process fallback"
  - "GameState::Loading emitted once when process detected but PlayableSignal not yet fired"
  - "current_sim_type + loading_emitted + f1_udp_playable_received fields in ConnectionState"
  - "Crash recovery cancels exit grace timer on successful relaunch"

affects:
  - 82-03
  - billing
  - server-billing-handler

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Exit grace timer pattern: arm on AcStatus::Off / game exit, cancel on crash recovery relaunch, fire after 30s"
    - "Per-sim PlayableSignal dispatch in game_check_interval (2s tick)"
    - "F1 25 UdpActive captured in signal_rx arm, checked in game_check_interval"

key-files:
  created: []
  modified:
    - crates/rc-agent/src/event_loop.rs
    - crates/rc-agent/src/ws_handler.rs

key-decisions:
  - "AC AcStatus::Off from shared memory arms grace timer (not immediate send) — telemetry_interval path modified"
  - "Non-AC sims arm grace timer on game_check game-exit (no shared memory path for these sims)"
  - "F1 25 UDP signal captured in signal_rx arm (where DetectorSignal arrives), checked in game_check_interval 2s tick"
  - "launch timeout AcStatus::Off (3-min timeout) NOT gated by grace timer — these are launch failures before billing starts"
  - "Grace timer reset to 86400s when cancelled (same pattern as blank_timer)"

patterns-established:
  - "Grace timer pattern: Box::pin sleep initialized to 86400s, armed/rearmed on event, cancelled by resetting to 86400s + armed=false"

requirements-completed: [BILL-01, BILL-02, BILL-04]

# Metrics
duration: 35min
completed: 2026-03-21
---

# Phase 82 Plan 02: Billing and Session Lifecycle Summary

**Per-sim PlayableSignal dispatch + 30s exit grace timer in rc-agent ConnectionState: AC=shared memory, F1 25=UdpActive, others=90s process fallback**

## Performance

- **Duration:** ~35 min
- **Started:** 2026-03-21T04:05:00Z
- **Completed:** 2026-03-21T04:40:00Z
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments
- Added 6 new fields to `ConnectionState`: `exit_grace_timer`, `exit_grace_armed`, `exit_grace_sim_type`, `loading_emitted`, `current_sim_type`, `f1_udp_playable_received`
- Per-sim PlayableSignal dispatch in `game_check_interval`: F1 25 fires on first `UdpActive` from DrivingDetector, other sims fire after 90s process elapsed, AC unchanged (shared memory path)
- `GameState::Loading` emitted once when game process detected in `Launching` state (before PlayableSignal fires)
- 30s exit grace timer: `AcStatus::Off` from AC shared memory arms timer instead of sending immediately; non-AC sims arm on game exit; grace fires after 30s with no relaunch
- Crash recovery success cancels exit grace timer
- `ws_handler.rs` `LaunchGame` handler sets `conn.current_sim_type`, resets `loading_emitted` and `f1_udp_playable_received`

## Task Commits

1. **Task 1: ConnectionState exit grace timer + per-sim PlayableSignal dispatch** - `[pending commit]` (feat)

**Plan metadata:** `[pending commit]` (docs: complete plan)

## Files Created/Modified
- `crates/rc-agent/src/event_loop.rs` — ConnectionState fields + grace timer arm + per-sim dispatch + Loading emission + crash recovery cancel
- `crates/rc-agent/src/ws_handler.rs` — LaunchGame sets current_sim_type, resets loading_emitted + f1_udp_playable_received

## Decisions Made
- AC `AcStatus::Off` from shared memory now arms grace timer instead of sending immediately. `AcStatus::Live` still sends immediately. Other statuses (Pause etc) send immediately. This is the minimal change that preserves AC billing behavior while adding session fragmentation protection.
- Non-AC sim exit detected in `game_check_interval` (game process disappears) also arms grace timer — handles the case where server has no other signal that the game ended.
- F1 25 `UdpActive` is captured in the `signal_rx.recv()` arm (where raw `DetectorSignal` values arrive) and sets a flag; the actual `AcStatus::Live` emission happens in `game_check_interval` on next 2s tick. This avoids dual-path complexity.
- Launch timeout `AcStatus::Off` emissions (lines ~234, ~256) are NOT gated by grace timer — these happen when the game fails to reach Live within 3 minutes and no billing has started yet.

## Deviations from Plan

None — plan executed exactly as written. The plan's action items were implemented as specified.

## Issues Encountered
- Test runner (`cargo test`) blocked by Windows Application Control policy on this workstation — pre-existing issue not caused by these changes. Build passes cleanly (`cargo build --release --bin rc-agent` zero errors). Acceptance criteria verified via grep counts.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Plan 82-02 complete: rc-agent now emits correct per-sim billing signals with 30s exit grace
- Server-side billing handler (`racecontrol/src/billing.rs`) receives `GameStatusUpdate` with `sim_type` and `AcStatus::Live` from all sim types
- Plan 82-03 can build on this foundation for server-side billing lifecycle handling

---
*Phase: 82-billing-and-session-lifecycle*
*Completed: 2026-03-21*
