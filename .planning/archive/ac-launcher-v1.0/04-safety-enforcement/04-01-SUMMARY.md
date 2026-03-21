---
phase: 04-safety-enforcement
plan: 01
subsystem: safety
tags: [ini-writer, damage, grip, ffb, websocket, tdd]

# Dependency graph
requires:
  - phase: 01-session-types-race-mode
    provides: INI builder (write_assists_section, build_race_ini_string)
  - phase: 03-billing-synchronization
    provides: AgentMessage enum, GameStatusUpdate variant
provides:
  - Hardcoded DAMAGE=0 in all INI writers (race.ini, assists.ini, server_cfg.ini)
  - Hardcoded SESSION_START=100 (grip) in server_cfg.ini
  - Post-write verify_safety_settings() called before AC launch
  - FfbZeroed and GameCrashed AgentMessage variants for Plan 02
affects: [04-02, ffb-safety, session-end, game-crash-handling]

# Tech tracking
tech-stack:
  added: []
  patterns: [safety-hardcode-over-config, post-write-verification, defense-in-depth]

key-files:
  created: []
  modified:
    - crates/rc-agent/src/ac_launcher.rs
    - crates/rc-common/src/protocol.rs
    - crates/rc-core/src/ac_server.rs
    - crates/rc-core/src/ws/mod.rs

key-decisions:
  - "DAMAGE=0 hardcoded in all three INI paths (race.ini, assists.ini, server_cfg.ini) -- params.conditions.damage ignored"
  - "verify_safety_content() is testable string-based function; verify_safety_settings() wraps it with file I/O"
  - "FfbZeroed/GameCrashed are log-only on core side for now; Plan 02 will add FFB zeroing logic on agent side"

patterns-established:
  - "Safety-critical INI values use hardcoded literals, never config/params -- defense-in-depth"
  - "Post-write verification re-reads file from disk before launch -- catches any write bug"

requirements-completed: [BILL-03, BILL-04, BILL-05]

# Metrics
duration: 8min
completed: 2026-03-14
---

# Phase 04 Plan 01: Safety Enforcement Summary

**Hardcoded DAMAGE=0 and SESSION_START=100 in all INI writers with post-write verification and FFB/crash protocol messages**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-13T19:59:42Z
- **Completed:** 2026-03-13T20:07:59Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- DAMAGE=0 hardcoded in write_assists_section(), write_assists_ini(), and generate_server_cfg_ini() -- no user/staff config can ever enable car damage
- Post-write verify_safety_settings() re-reads race.ini from disk and refuses to launch AC if DAMAGE!=0 or SESSION_START!=100
- SESSION_START=100 (full grip) enforced in server_cfg.ini DYNAMIC_TRACK section
- FfbZeroed and GameCrashed AgentMessage variants added for Plan 02's FFB safety reporting
- 11 new tests (9 ac_launcher/protocol + 2 server config), all 354 workspace tests passing

## Task Commits

Each task was committed atomically (TDD: RED then GREEN):

1. **Task 1 RED: Failing safety tests** - `815c987` (test)
2. **Task 1 GREEN: Hardcode DAMAGE=0, verify_safety_settings, FfbZeroed/GameCrashed** - `9b4c1eb` (feat)
3. **Task 2 RED: Failing server config tests** - `d1f451e` (test)
4. **Task 2 GREEN: Server config safety overrides** - `d6e40f0` (feat)

## Files Created/Modified
- `crates/rc-agent/src/ac_launcher.rs` - Hardcoded DAMAGE=0 in write_assists_section() and write_assists_ini(), added verify_safety_content() and verify_safety_settings(), called verification in launch_ac()
- `crates/rc-common/src/protocol.rs` - Added FfbZeroed and GameCrashed variants to AgentMessage enum with serde roundtrip tests
- `crates/rc-core/src/ac_server.rs` - Overrode DAMAGE_MULTIPLIER to 0 and SESSION_START to 100 in generate_server_cfg_ini()
- `crates/rc-core/src/ws/mod.rs` - Added match arms for FfbZeroed and GameCrashed in WebSocket message handler

## Decisions Made
- DAMAGE=0 hardcoded as literal in all three INI paths -- params.conditions.damage field is now dead code (kept for deserialization backward compat)
- verify_safety_content() takes a string for testability; verify_safety_settings() wraps it with file I/O for the real launch path
- FfbZeroed/GameCrashed handlers in ws/mod.rs are log-only for now -- Plan 02 will implement the agent-side FFB zeroing logic that sends these messages

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Handle exhaustive match for new AgentMessage variants in ws/mod.rs**
- **Found during:** Task 2 (RED phase compile check)
- **Issue:** Adding FfbZeroed and GameCrashed to AgentMessage caused non-exhaustive match error in rc-core's WebSocket handler
- **Fix:** Added match arms with tracing::info/warn logging and pod activity tracking
- **Files modified:** crates/rc-core/src/ws/mod.rs
- **Verification:** Full workspace compiles and all 354 tests pass
- **Committed in:** d1f451e (Task 2 RED commit)

---

**Total deviations:** 1 auto-fixed (Rule 3 blocking)
**Impact on plan:** Required for compilation. No scope creep -- handlers are minimal log-only stubs.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Safety-critical INI values are now defense-in-depth hardcoded in all writers
- FfbZeroed and GameCrashed protocol messages are ready for Plan 02 (FFB session-end safety)
- All existing tests still pass -- no regressions

---
*Phase: 04-safety-enforcement*
*Completed: 2026-03-14*
