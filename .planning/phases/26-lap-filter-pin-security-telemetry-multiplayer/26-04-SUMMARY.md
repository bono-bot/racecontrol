---
phase: 26-lap-filter-pin-security-telemetry-multiplayer
plan: 04
subsystem: bot-coordinator
tags: [rust, axum, telemetry, multiplayer, failure-monitor, ws, bot-coordinator, cascade]

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
  - "handle_multiplayer_failure(): ordered teardown (BlankScreen → end_billing → cascade group pods via group_session_members → log_pod_activity)"
  - "failure_monitor.rs: TELEM_GAP_SECS=60, telem_gap_fired task-local flag, TelemetryGap send site"
  - "ws/mod.rs: MultiplayerFailure arm calls handle_multiplayer_failure() (replaces log stub)"
  - "MULTI-01 cascade: all sibling pods in group session are also blanked and billed-ended"
affects:
  - "Phase 27 bot-coordinator expansions"
  - "Any feature that handles AC server disconnects"
  - "Any feature that reads telemetry gap state"
  - "group_session_members table usage"

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "game-state guard before email alert — only email when GameState::Running AND billing_active"
    - "MULTI-01 teardown order: lock-screen → end-billing → cascade-group-pods → log (non-negotiable sequential awaits)"
    - "telem_gap_fired task-local flag pattern (same as launch_timeout_fired) prevents duplicate sends"
    - "BlankScreen for MULTI-01 lock: blanks display before billing ends, FFB zero via StopGame chain"
    - "DB-resolved group membership: group_session_members subquery when BillingTimer lacks group_session_id"

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
  - "MULTI-01 cascade uses group_session_members DB query because BillingTimer lacks group_session_id — subquery: pod_id -> group_session_id -> sibling pod_ids"
  - "cascade loop added AFTER triggering pod steps 1+2 — triggering pod is locked+billed-ended before group pods start cascade"
  - "unwrap_or_default() on DB result — solo session has no group rows, cascade is a no-op"

patterns-established:
  - "TELEM-01 guard pattern: game_state check → billing_active check → email (two-guard before action)"
  - "MULTI-01 cascade pattern: lock+end triggering pod → DB-resolve siblings → lock+end each sibling → log with cascaded_to list"

requirements-completed: [TELEM-01, MULTI-01]

# Metrics
duration: 9min
completed: 2026-03-16
---

# Phase 26 Plan 04: Telemetry Gap Email Alert + Multiplayer Failure Teardown (with cascade) Summary

**TELEM-01 and MULTI-01 fully operational: staff email on 60s UDP silence (game Running + billing active), ordered pod teardown (BlankScreen + end billing + group cascade via group_session_members + log) on AC server disconnect**

## Performance

- **Duration:** 9 min (including prior partial execution + this session's delta)
- **Started:** 2026-03-16T13:40:50Z
- **Completed:** 2026-03-16T13:59:26Z
- **Tasks:** 2 (+ checkpoint: human-verify)
- **Files modified:** 3

## Accomplishments

- `handle_telemetry_gap()` promoted from stub to full implementation with two guards (GameState::Running + billing_active) before calling `email_alerter.send_alert()`
- `handle_multiplayer_failure()` fully implemented with 4-step ordered teardown: BlankScreen → end_billing_session_public(EndedEarly) → cascade all group pods via `group_session_members` DB query → log_pod_activity with `cascaded_to` field
- `failure_monitor.rs` has `TELEM_GAP_SECS=60` constant and `telem_gap_fired` task-local flag; sends `AgentMessage::TelemetryGap` once per silence window
- `ws/mod.rs` MultiplayerFailure arm wired to call `handle_multiplayer_failure()`
- All 4 Wave 0 `todo!()` stubs in bot_coordinator tests GREEN; step order test updated to include `cascade_group_pods`
- Full 3-crate suite: 112 rc-common + rc-agent-crate + 310 racecontrol-crate = all pass

## Task Commits

1. **Task 1: bot_coordinator.rs stubs promoted (initial execution)** - `61c72ed` (feat)
2. **Task 1 fix: BlankScreen correction** - `431b3ce` (fix)
3. **Task 2: failure_monitor.rs + ws/mod.rs** - `e01d7dc` (feat)
4. **Task 1 delta: MULTI-01 cascade loop + test step order** - `39b0743` (feat)

## Files Created/Modified

- `crates/racecontrol/src/bot_coordinator.rs` — handle_telemetry_gap() full impl + handle_multiplayer_failure() with cascade loop; 4 Wave 0 tests GREEN with correct 4-step order
- `crates/rc-agent/src/failure_monitor.rs` — TELEM_GAP_SECS, telem_gap_fired, TelemetryGap send arm; TELEM-01 unit tests
- `crates/racecontrol/src/ws/mod.rs` — MultiplayerFailure arm calls handle_multiplayer_failure()

## Decisions Made

- `BlankScreen` used for MULTI-01 lock step (not `ClearLockScreen`) — blanks display immediately, FFB zero guaranteed by StopGame path inside `end_billing_session_public`
- Cascade uses `sqlx::query_as::<_, (String,)>` subquery on `group_session_members` — BillingTimer lacks `group_session_id`, DB is the only resolution path
- `unwrap_or_default()` on DB result — solo sessions return empty `group_pods`, cascade is a no-op
- cascade loop placed after triggering pod's BlankScreen + end_billing (Steps 1+2) to ensure triggering pod is torn down first

## Deviations from Plan

None — plan and context description matched the actual delta work precisely. Cascade loop was added exactly as specified in the context block.

## Issues Encountered

- Prior execution had implemented TELEM-01 guards, failure_monitor.rs, and ws/mod.rs correctly but was missing the cascade loop in handle_multiplayer_failure(). The stale SUMMARY.md was deleted to force a fresh execution. This session correctly identified the delta and applied only the missing cascade loop.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- All 7 Phase 26 requirements have passing tests: LAP-01, LAP-02, LAP-03, PIN-01, PIN-02, TELEM-01, MULTI-01
- Ready for human verify checkpoint (Task 3): run full 3-crate suite + 7 spot-checks
- Phase 26 complete pending approval

---
*Phase: 26-lap-filter-pin-security-telemetry-multiplayer*
*Completed: 2026-03-16*
