---
phase: 57-session-end-safety
plan: 02
subsystem: ffb
tags: [openffboard, hid, ffb, safety, conspit-link, wm-close, session-end]

# Dependency graph
requires:
  - phase: 57-01-ffb-hid-building-blocks
    provides: FfbController.fxm_reset(), set_idle_spring(), Clone derive, POWER_CAP_80_PERCENT
provides:
  - close_conspit_link() — WM_CLOSE with 5s timeout and process exit polling
  - restart_conspit_link() — unconditional restart with Global.json integrity check
  - safe_session_end() — async orchestrator (close CL -> fxm.reset -> idlespring ramp -> restart CL)
  - enforce_safe_state(skip_conspit_restart) — new parameter to avoid double-restart
  - All 10 session-end call sites wired to safe_session_end()
affects: [57-03-hardware-validation, 58-conspit-link-process-hardening, 61-ffb-preset-tuning]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "WM_CLOSE via FindWindowW + PostMessageW with multiple title variants and process exit polling"
    - "safe_session_end async orchestrator: spawn_blocking for sync HID ops, fire-and-forget for CL restart"
    - "enforce_safe_state(skip_conspit_restart: bool) pattern to avoid CL lifecycle contention"

key-files:
  created: []
  modified:
    - crates/rc-agent/src/ffb_controller.rs
    - crates/rc-agent/src/ac_launcher.rs
    - crates/rc-agent/src/main.rs

key-decisions:
  - "safe_session_end() is async (not sync) — uses spawn_blocking internally for HID ops, fits naturally into tokio select loop"
  - "ConspitLink restart is fire-and-forget (not awaited) — restart happens in background, no need to block main loop"
  - "Global.json integrity check runs in separate thread 5s after CL restart — non-blocking validation"
  - "Idlespring ramp target = 2000 (empirical starting value, needs hardware validation)"
  - "enforce_safe_state(true) at all 8 post-session-end sites — safe_session_end already handles CL lifecycle"

patterns-established:
  - "Session-end sites use ffb_controller::safe_session_end(&ffb).await instead of spawn_blocking zero_force"
  - "Post-session-end enforce_safe_state always gets skip_conspit_restart=true"

requirements-completed: [SAFE-01, SAFE-06, SAFE-07]

# Metrics
duration: 12min
completed: 2026-03-20
---

# Phase 57 Plan 02: Session-End Safety Orchestrator Summary

**safe_session_end() async orchestrator wired to all 10 session-end sites — close ConspitLink (WM_CLOSE 5s) -> fxm.reset -> idlespring ramp 500ms -> restart CL with JSON verification**

## Performance

- **Duration:** 12 min
- **Started:** 2026-03-20T08:34:20Z
- **Completed:** 2026-03-20T08:46:00Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Implemented close_conspit_link() with WM_CLOSE and 5s process exit polling across 4 window title variants
- Implemented restart_conspit_link() with Global.json integrity verification after 5s
- Implemented safe_session_end() async orchestrator with full shutdown sequence (close CL -> fxm.reset -> 5-step idlespring ramp -> restart CL)
- Replaced all 10 zero_force() session-end call sites in main.rs with safe_session_end()
- Added skip_conspit_restart parameter to enforce_safe_state() to prevent double-restart
- ESTOP paths (panic hook line 423, startup probe line 659) remain unchanged

## Task Commits

Each task was committed atomically:

1. **Task 1: Create close_conspit_link(), restart_conspit_link(), and safe_session_end()** - `fb5640b` (feat)
2. **Task 2: Wire all session-end call sites in main.rs** - `aa6c155` (feat)

## Files Created/Modified
- `crates/rc-agent/src/ffb_controller.rs` - Added close_conspit_link(), restart_conspit_link(), safe_session_end() free functions
- `crates/rc-agent/src/ac_launcher.rs` - Visibility bumps (hidden_cmd, is_process_running, minimize_conspit_window to pub(crate)); enforce_safe_state(skip_conspit_restart: bool)
- `crates/rc-agent/src/main.rs` - All 10 session-end sites wired to safe_session_end(); enforce_safe_state(true) at all 8 post-session-end calls; removed redundant 500ms sleeps

## Decisions Made
- safe_session_end() is async with spawn_blocking for sync HID ops — fits naturally into the existing tokio select loop pattern
- ConspitLink restart is fire-and-forget (not awaited) — no need to block the main loop waiting for CL to fully start
- Global.json integrity check runs in a separate thread 5s after restart — non-blocking verification
- Idlespring ramp target = 2000 as starting value — needs empirical hardware validation on canary pod
- All enforce_safe_state calls switched to true since they always follow safe_session_end

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] enforce_safe_state() callers updated in Task 1 (not Task 2)**
- **Found during:** Task 1
- **Issue:** Changing enforce_safe_state() signature to require a bool parameter broke all 8 callers in main.rs — build would not compile
- **Fix:** Updated all enforce_safe_state() calls to pass `false` in Task 1 to restore compilation; Task 2 then changed the appropriate ones to `true`
- **Files modified:** crates/rc-agent/src/main.rs
- **Verification:** cargo build -p rc-agent-crate passes
- **Committed in:** fb5640b (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Necessary to maintain compilation between tasks. No scope creep.

## Issues Encountered
- Application Control policy blocks execution of test binaries on this machine (os error 4551) — compilation verified via `cargo build --release` instead of `cargo test`. All pre-existing tests still compile successfully; execution is a system policy issue, not a code issue.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- safe_session_end() is fully wired and ready for hardware testing on canary pod
- Plan 03 (hardware validation) can proceed — test all 4 games, verify wheel centers, tune idlespring value
- ESTOP path completely preserved for panic/USB disconnect/manual trigger
- Build compiles in both debug and release modes with zero errors

---
*Phase: 57-session-end-safety*
*Completed: 2026-03-20*
