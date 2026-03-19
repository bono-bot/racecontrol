---
phase: 49-session-lifecycle-autonomy
plan: 02
subsystem: billing
tags: [rust, tokio, billing, crash-recovery, websocket, overlay, e2e-test]

# Dependency graph
requires:
  - phase: 49-01
    provides: SessionAutoEnded/BillingPaused/BillingResumed protocol variants, billing_paused in FailureMonitorState
  - phase: 46-crash-safety-panic-hook
    provides: overlay.show_toast(), game_process::GameProcess struct, ac_launcher::launch_ac
provides:
  - CrashRecoveryState enum replacing crash_recovery_armed + crash_recovery_timer in main.rs
  - 2-attempt crash recovery state machine: pause billing, show overlay, relaunch with stored args, auto-end on 2nd failure
  - WS 30s grace window suppressing Disconnected screen during brief drops with active billing
  - tests/e2e/api/session-lifecycle.sh E2E test script (Gates 0-5)
affects: [SESSION-03, SESSION-04, rc-agent crash resilience, venue WiFi blip handling]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - CrashRecoveryState enum with embedded Pin<Box<Sleep>> timer in PausedWaitingRelaunch variant
    - std::mem::replace pattern for consuming enum variant in async select! arm
    - ws_disconnected_at: Option<Instant> grace window with get_or_insert_with pattern

key-files:
  created:
    - tests/e2e/api/session-lifecycle.sh
  modified:
    - crates/rc-agent/src/main.rs

key-decisions:
  - "CrashRecoveryState timer embedded in enum variant (not a separate armed bool) — eliminates the split state"
  - "Attempt 2 relaunch uses stored last_launch_args_stored from LaunchGame handler — same JSON roundtripped back through ac_launcher::launch_ac"
  - "overlay.show_toast() is the correct method name (not show_message) — plan had wrong name, fixed by Rule 1 auto-fix"
  - "game_process = Some(GameProcess { ... }) constructed from LaunchResult.pid — mirrors LaunchGame handler pattern"
  - "WS grace window uses get_or_insert_with pattern on both inner-loop break and outer reconnect failure paths"
  - "session-lifecycle.sh Gate 4 polls 7 times at 5s intervals (35s total) to match 30s blank_timer + 5s buffer"

patterns-established:
  - "Async select! timer via inner async block: async { match &mut state { EnumVariant { timer } => timer.as_mut().await, _ => pending::<()>().await } }"
  - "ws_disconnected_at grace window pattern: set on disconnect, clear on reconnect, get_or_insert_with on first use"

requirements-completed: [SESSION-03, SESSION-04]

# Metrics
duration: 12min
completed: 2026-03-19
---

# Phase 49 Plan 02: Crash Recovery State Machine + WS Grace Window Summary

**CrashRecoveryState enum (2-attempt billing-aware crash recovery) + 30s WS disconnection grace window so venue WiFi blips don't disturb active customer sessions**

## Performance

- **Duration:** 12 min
- **Started:** 2026-03-19T03:28:15Z (IST 08:58:15)
- **Completed:** 2026-03-19T03:40:00Z (IST 09:10:00)
- **Tasks:** 2
- **Files modified:** 2 (main.rs already committed in prior session, session-lifecycle.sh new)

## Accomplishments

- SESSION-03: `CrashRecoveryState` enum fully replaces `crash_recovery_armed` + `crash_recovery_timer`. On game crash during billing: pause billing via `failure_monitor_tx.send_modify(s.billing_paused=true)`, show overlay toast, start 60s attempt-1 timer. On timer fire: check for game PID (success) or escalate. Attempt 2 calls `ac_launcher::launch_ac(&params)` in `spawn_blocking` with stored `last_launch_args_stored`. After attempt 2 timeout: sends `SessionAutoEnded{reason:"crash_limit"}`, resets to idle PinEntry.
- SESSION-04: `ws_disconnected_at: Option<Instant>` tracks first disconnect moment. On reconnect failure: checks `elapsed > 30s` before calling `lock_screen.show_disconnected()`. Billing, game, and overlay continue running during 30s grace window. On reconnect: `ws_disconnected_at = None`.
- E2E test: `session-lifecycle.sh` with 6 gates: server health, end_reason schema, pod status API, billing create, session end + pod reset timing (35s poll), end_reason field verification. Cleanup trap prevents stale billing on Pod 8.

## Task Commits

Each task was committed atomically:

1. **Task 1: CrashRecoveryState + WS grace window** - `c9996ea` (already in prior session commit — feat(50-03) included 49-02 work)
2. **Task 2: session-lifecycle.sh E2E test** - `f729206` (feat)

Note: Task 1 was discovered to already be committed in `c9996ea` (Phase 50-03 session), which included rc-agent/src/main.rs changes implementing both the crash recovery state machine and WS grace window. The implementation was confirmed correct and all tests pass.

## Files Created/Modified

- `crates/rc-agent/src/main.rs` - `CrashRecoveryState` enum (Idle, PausedWaitingRelaunch, AutoEndPending), `last_launch_args_stored`, `ws_disconnected_at`, 6 unit tests for crash recovery transitions and WS grace window boundary
- `tests/e2e/api/session-lifecycle.sh` - 6-gate E2E test script, cleanup trap, SESSION-01/02 validation

## Decisions Made

- `CrashRecoveryState` enum embeds the `Pin<Box<Sleep>>` timer inside the `PausedWaitingRelaunch` variant — cleaner than a separate armed flag because the timer is logically part of the state
- The plan specified `overlay.show_message()` but the actual method is `overlay.show_toast()` — auto-fixed
- `game_process = Some(result)` was wrong type — need `GameProcess { sim_type, state, child: None, pid: Some(result.pid), last_exit_code: None }` — auto-fixed by mirroring LaunchGame handler pattern

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] overlay.show_message() does not exist — correct method is show_toast()**
- **Found during:** First cargo check run
- **Issue:** Plan specified `overlay.show_message("...")` but `OverlayManager` has `show_toast()` not `show_message()`
- **Fix:** Replaced all 3 occurrences with `overlay.show_toast("...".to_string())`
- **Files modified:** crates/rc-agent/src/main.rs
- **Commit:** c9996ea (pre-existing)

**2. [Rule 1 - Bug] game_process = Some(result) — result is LaunchResult not GameProcess**
- **Found during:** First cargo check run
- **Issue:** `ac_launcher::launch_ac()` returns `LaunchResult` (with `.pid: u32`), not `GameProcess`. Needed to construct `GameProcess` from result fields.
- **Fix:** Replaced with `game_process = Some(game_process::GameProcess { sim_type: last_sim_type, state: GameState::Running, child: None, pid: Some(result.pid), last_exit_code: None })` mirroring the LaunchGame handler (line ~2010)
- **Files modified:** crates/rc-agent/src/main.rs
- **Commit:** c9996ea (pre-existing)

**3. [Rule 1 - Bug] ref mut timer in match pattern — implicit borrow error**
- **Found during:** First cargo check run
- **Issue:** Rust 2021 edition implicit reborrow means `ref mut` is redundant/conflicting inside async block match
- **Fix:** Removed `ref mut` from `CrashRecoveryState::PausedWaitingRelaunch { ref mut timer, .. }` → `{ timer, .. }`
- **Files modified:** crates/rc-agent/src/main.rs
- **Commit:** c9996ea (pre-existing)

---

**Total deviations:** 3 auto-fixed (Rule 1 — bugs in plan's code specification)
**Impact on plan:** All fixes were for incorrect method/type usage in the plan's code snippets. The logical behavior is correct as specified.

## Issues Encountered

- Task 1 implementation was already committed in `c9996ea` (Phase 50-03 session) — a prior session had implemented the crash recovery state machine and WS grace window while also implementing Phase 50 work. The code was confirmed correct by cargo check + tests passing.
- The auto-fixes (show_toast, LaunchResult→GameProcess, ref mut) were already applied in that prior commit.

## User Setup Required

None — no external service configuration required. CrashRecoveryState changes are internal to rc-agent. The WS grace window requires no config changes (hardcoded 30s per the locked decision).

## Next Phase Readiness

- SESSION-03 + SESSION-04 complete. rc-agent now handles game crashes gracefully with billing pause, 2 relaunch attempts, and auto-end fallback.
- Phase 49 fully complete (both plans).
- `session-lifecycle.sh` can be run against live server to verify billing session create/end/reset flow.

---
*Phase: 49-session-lifecycle-autonomy*
*Completed: 2026-03-19*
