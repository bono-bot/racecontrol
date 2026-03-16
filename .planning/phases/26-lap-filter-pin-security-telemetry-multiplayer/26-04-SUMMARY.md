---
phase: 26-lap-filter-pin-security-telemetry-multiplayer
plan: 04
subsystem: bot-coordinator
tags: [rust, axum, telemetry, multiplayer, failure-monitor, ws, bot-coordinator]

# Dependency graph
requires:
  - phase: 26-02
    provides: "LapData session_type, catalog min_lap_time_ms, review_required post-INSERT logic"
  - phase: 26-03
    provides: "Wave 0 stubs (todo!() tests) for TELEM-01 and MULTI-01 in bot_coordinator.rs"
  - phase: 25
    provides: "handle_billing_anomaly, end_billing_session_public, bot_coordinator.rs structure"
provides:
  - "handle_telemetry_gap(): game-state guard (Running only) + billing guard + staff email via EmailAlerter"
  - "handle_multiplayer_failure(): ordered teardown (BlankScreen → end_billing → log_pod_activity)"
  - "failure_monitor.rs: TELEM_GAP_SECS=60, telem_gap_fired task-local flag, TelemetryGap send site"
  - "ws/mod.rs: MultiplayerFailure arm calls handle_multiplayer_failure() (replaces log stub)"
affects:
  - "Phase 27 bot-coordinator expansions"
  - "Any feature that handles AC server disconnects"
  - "Any feature that reads telemetry gap state"

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "game-state guard before email alert — only email when GameState::Running AND billing_active"
    - "MULTI-01 teardown order: lock-screen → end-billing → log (non-negotiable sequential awaits)"
    - "telem_gap_fired task-local flag pattern (same as launch_timeout_fired) prevents duplicate sends"
    - "BlankScreen for MULTI-01 lock: blanks display before billing ends, FFB zero via StopGame chain"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/bot_coordinator.rs
    - crates/rc-agent/src/failure_monitor.rs
    - crates/racecontrol/src/ws/mod.rs

key-decisions:
  - "BlankScreen used for MULTI-01 lock step (not ClearLockScreen) — blanks display immediately, FFB zero guaranteed downstream by end_billing_session_public → StopGame arm"
  - "handle_multiplayer_failure is a no-op when billing is inactive — consistent with bot billing guard pattern"
  - "telem_gap_fired is task-local (not in FailureMonitorState) — transition detection requires task-private prev state, same rationale as launch_timeout_fired"
  - "TELEM_GAP_SECS=60 hardcoded per TELEM-01 requirement — not configurable"

patterns-established:
  - "TELEM-01 guard pattern: game_state check → billing_active check → email (two-guard before action)"
  - "MULTI-01 teardown pattern: lock-screen send → billing end → activity log (strict order via sequential await)"

requirements-completed: [TELEM-01, MULTI-01]

# Metrics
duration: 6min
completed: 2026-03-16
---

# Phase 26 Plan 04: Telemetry Gap Email Alert + Multiplayer Failure Teardown Summary

**TELEM-01 and MULTI-01 promoted from stubs to full implementations: staff email on 60s UDP silence (game Running + billing active), and ordered pod teardown (BlankScreen + end billing + log) on AC server disconnect**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-16T13:40:50Z
- **Completed:** 2026-03-16T13:47:18Z
- **Tasks:** 2 (+ BlankScreen correction fix)
- **Files modified:** 3

## Accomplishments

- `handle_telemetry_gap()` promoted from a single `tracing::warn!` stub to a full implementation with two guards (GameState::Running + billing_active) before calling `email_alerter.send_alert()`
- `handle_multiplayer_failure()` added as new `pub async fn` with guaranteed teardown order: BlankScreen → end_billing_session_public(EndedEarly) → log_pod_activity
- `failure_monitor.rs` gains `TELEM_GAP_SECS=60` constant and `telem_gap_fired` task-local flag; sends `AgentMessage::TelemetryGap` once per silence window, resets when UDP data resumes or game exits
- `ws/mod.rs` MultiplayerFailure arm wired to call `handle_multiplayer_failure()` (was just a log statement)
- All 4 Wave 0 `todo!()` stubs in bot_coordinator tests turned GREEN (9 tests total pass)

## Task Commits

1. **Task 1: bot_coordinator.rs stubs promoted** - `61c72ed` (feat)
2. **Task 2: failure_monitor.rs + ws/mod.rs** - `e01d7dc` (feat)
3. **Fix: BlankScreen correction per updated plan** - `431b3ce` (fix)

## Files Created/Modified

- `crates/racecontrol/src/bot_coordinator.rs` — handle_telemetry_gap() full impl + new handle_multiplayer_failure(); 4 todo!() tests replaced
- `crates/rc-agent/src/failure_monitor.rs` — TELEM_GAP_SECS, telem_gap_fired, TelemetryGap send arm; 6 new TELEM-01 unit tests (added by linter)
- `crates/racecontrol/src/ws/mod.rs` — MultiplayerFailure arm calls handle_multiplayer_failure() instead of logging

## Decisions Made

- `BlankScreen` used for MULTI-01 lock step (not `ClearLockScreen`). The plan was updated (commit 248b31c) to specify BlankScreen as the correct variant — blanks the display immediately, and FFB zero is guaranteed by the StopGame path inside `end_billing_session_public`.
- `handle_multiplayer_failure()` is a no-op when `billing.active_timers` has no entry for the pod — consistent with the existing bot billing guard pattern.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Used BlankScreen instead of ClearLockScreen for MULTI-01 lock step**
- **Found during:** Task 1 (after checking commit 248b31c updated the plan)
- **Issue:** The plan's `<action>` section referenced `ClearLockScreen` in the code snippet. The plan update commit (248b31c) revised this to `BlankScreen` per RESEARCH. Initial implementation used `ClearLockScreen`.
- **Fix:** Updated `handle_multiplayer_failure()` to send `CoreToAgentMessage::BlankScreen`; added comment confirming FFB zero via StopGame chain
- **Files modified:** `crates/racecontrol/src/bot_coordinator.rs`
- **Verification:** `cargo test -p racecontrol-crate -- bot_coordinator` — 9/9 pass
- **Committed in:** `431b3ce`

---

**Total deviations:** 1 auto-fixed (Rule 1 — used correct protocol variant per updated plan)
**Impact on plan:** Correctness fix — BlankScreen is the right lock mechanism for MULTI-01.

## Issues Encountered

None beyond the ClearLockScreen → BlankScreen correction above. All tests compiled and passed on the first attempt for both tasks.

## Next Phase Readiness

- All 7 Phase 26 requirements now have passing tests (LAP-01/02/03, PIN-01/02, TELEM-01, MULTI-01)
- Phase 26 is complete pending human verification checkpoint
- Checkpoint: human should run `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` to confirm full 3-crate suite green

---
*Phase: 26-lap-filter-pin-security-telemetry-multiplayer*
*Completed: 2026-03-16*
