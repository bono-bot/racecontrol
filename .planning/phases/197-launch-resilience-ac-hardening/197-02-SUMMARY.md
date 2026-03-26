---
phase: 197-launch-resilience-ac-hardening
plan: 02
subsystem: game-launcher
tags: [rust, ac-launcher, game-process, polling, sentinel, pre-flight]

requires:
  - phase: 197-01
    provides: exit_code field on GameLaunchInfo, atomic Race Engineer, dynamic timeout
  - phase: 196-game-launcher-structural-rework
    provides: GameProcess, game_process.rs foundation, ac_launcher.rs structure

provides:
  - Pre-launch health checks (MAINTENANCE_MODE, OTA_DEPLOYING, orphan processes, disk space)
  - check_sentinel_files_in_dir() testable helper for sentinel file checks
  - parse_launch_args() for paths with spaces (JSON array or single-arg)
  - clean_state_reset() kills all 13 game exe names, clears game.pid
  - wait_for_acs_exit(5) polling replaces hardcoded 2s sleep after kill
  - wait_for_ac_ready(30) polling replaces hardcoded 8s sleep after launch
  - CM timeout upgraded 15s -> 30s with 5s progress logging
  - CM fallback fresh PID via find_acs_pid() instead of stale child.id()

affects:
  - Phase 198 (AC hardening follow-up)
  - Any phase testing game launch flows on pods

tech-stack:
  added: []
  patterns:
    - "Pre-launch sentinel check: testable by extracting check_sentinel_files_in_dir(dir) with injectable path"
    - "Polling stability wait: PID-stability polling (same PID alive 3s) as proxy for AC window readiness"
    - "parse_launch_args: JSON array preferred; plain string = single-arg (no split_whitespace)"
    - "Fresh PID after spawn: find_acs_pid() + 500ms wait beats child.id() which may track stale CM child"

key-files:
  created: []
  modified:
    - crates/rc-agent/src/game_process.rs - Added pre_launch_checks(), clean_state_reset(), check_sentinel_files_in_dir(), parse_launch_args(); fixed split_whitespace bug; 7 new tests
    - crates/rc-agent/src/ac_launcher.rs - Added wait_for_acs_exit(), wait_for_ac_ready(); upgraded CM timeout to 30s + progress logging; fresh PID on fallback; 5 new tests
    - crates/rc-agent/src/ws_handler.rs - Wired pre_launch_checks() into LaunchGame handler via spawn_blocking

key-decisions:
  - "check_sentinel_files_in_dir(dir) testable helper: takes &Path not hardcoded C:\\RacingPoint, allows unit tests with temp dirs"
  - "parse_launch_args single-arg fallback: plain string passed as one arg, not split on spaces — preserves backward compat for existing exe_path + args configs"
  - "PID-stability polling (3s same PID alive) as AC readiness proxy instead of window handle enumeration — simpler, no winapi complexity, works cross-platform in tests"
  - "CM fallback uses find_acs_pid() + 500ms wait: fresh tasklist query beats child.id() which points to CM child wrapper not actual acs.exe"
  - "Step 5 sleep(2s) between minimize_background_windows and bring_game_to_foreground preserved: small operational delay, not a polling wait target"

patterns-established:
  - "Testable sentinel checks: extract rp_dir parameter for unit testing, use C:\\RacingPoint only at call site"
  - "Polling with progress: 500ms poll interval + 5s progress log interval + warn on timeout-but-continue"
  - "Pre-launch gate: spawn_blocking wrapper → GameState::Error with specific reason on failure → return Continue"

requirements-completed: [LAUNCH-10, LAUNCH-11, LAUNCH-19, AC-01, AC-02, AC-03, AC-04]

duration: 29min
completed: 2026-03-26
---

# Phase 197 Plan 02: Launch Resilience AC Hardening Summary

**Agent-side launch hardening: pre-launch sentinel+orphan+disk checks, AC polling waits replacing hardcoded sleeps, CM 30s timeout with progress logging, fresh PID via find_acs_pid() on fallback, and split_whitespace fix for paths with spaces.**

## Performance

- **Duration:** ~29 min
- **Started:** 2026-03-26
- **Completed:** 2026-03-26
- **Tasks:** 2/2
- **Files modified:** 3

## Accomplishments

- Pre-launch checks gate every game launch: MAINTENANCE_MODE sentinel, OTA_DEPLOYING sentinel, orphan game process scan (13 exe names), disk space > 1GB. Failed check sends GameState::Error with specific reason to server and returns immediately.
- AC post-kill wait polls for acs.exe absence (max 5s, 500ms interval) instead of sleeping 2s blindly — faster on fast systems, safer on slow ones.
- AC load wait polls for PID stability (same PID alive >= 3s, max 30s) instead of sleeping 8s blindly — adapts to actual process startup time.
- Content Manager timeout upgraded 15s -> 30s with 5-second progress logging intervals.
- CM fallback uses `find_acs_pid()` fresh tasklist query + `persist_pid()` instead of stale `child.id()` which references the CM wrapper not acs.exe.
- Fixed split_whitespace bug: `GameExeConfig::args` now parsed as JSON array (multiple args) or single string (preserves paths with spaces).
- 12 new tests total (7 in game_process.rs, 5 in ac_launcher.rs). All 81 targeted tests pass.

## Task Commits

1. **Task 1: Pre-launch checks + clean state reset + arg parsing fix** - `7a05058b` (feat)
2. **Task 2: AC polling waits + CM 30s timeout with progress + fresh PID on fallback** - `b8cff553` (feat)

## Files Created/Modified

- `crates/rc-agent/src/game_process.rs` - Added `check_sentinel_files_in_dir()`, `parse_launch_args()`, `pre_launch_checks()`, `clean_state_reset()`; fixed split_whitespace to parse_launch_args in GameProcess::launch(); 7 new tests
- `crates/rc-agent/src/ac_launcher.rs` - Added `wait_for_acs_exit()`, `wait_for_ac_ready()` polling helpers; updated `wait_for_ac_process()` to 30s + 5s progress logging; fresh PID on CM fallback; 5 new tests
- `crates/rc-agent/src/ws_handler.rs` - Added pre_launch_checks block in LaunchGame handler (after feature flag gate, before safe mode entry); sends GameState::Error on failure

## Decisions Made

- `check_sentinel_files_in_dir(&Path)` accepts an injectable path so tests can use temp dirs instead of requiring a real `C:\RacingPoint`. Production call site passes the real path.
- PID-stability polling (same PID alive 3s) chosen over Win32 EnumWindows window enumeration for AC readiness — simpler, no winapi complexity, testable on all platforms.
- Step 5 sleep(2s) between `minimize_background_windows()` and `bring_game_to_foreground()` preserved — it's a valid operational delay between window operations, not a polling wait target.
- CM fallback: 500ms `thread::sleep` before `find_acs_pid()` gives the newly-spawned acs.exe time to register with the OS process list. `child.id()` retained as fallback if find still returns None.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Python patch script added duplicate function definitions**
- **Found during:** Task 1 (GREEN phase)
- **Issue:** The first Edit tool call added the new functions correctly, but rust-analyzer appeared to cause issues and the Python backup script was also run, creating duplicate definitions
- **Fix:** Created `remove_duplicates.py` and `remove_dup_tests.py` to identify and remove the second copy of each function and test block
- **Files modified:** crates/rc-agent/src/game_process.rs
- **Committed in:** 7a05058b

**2. [Rule 1 - Bug] include_str! tests had self-referential false matches**
- **Found during:** Task 2 (GREEN phase)
- **Issue:** `include_str!("ac_launcher.rs")` reads the ENTIRE source file including the test code itself, so `!source.contains("wait_for_ac_process(15)")` matched the literal string in the assertion message and failed
- **Fix:** Rewrote Task 2 tests to use runtime behavior (call functions with 0s timeout, verify they return quickly) instead of source-code string matching
- **Files modified:** crates/rc-agent/src/ac_launcher.rs
- **Committed in:** b8cff553

---

**Total deviations:** 2 auto-fixed (Rule 1: 1, Rule 3: 1)
**Impact on plan:** Both auto-fixes necessary for correctness. No scope changes. Plan executed as designed.

## Issues Encountered

- The `src/ws_handler.rs` pre-launch check initially used `conn.ws_tx.send()` which doesn't exist — the correct pattern is `ws_tx.send(Message::Text(json.into()))`. Fixed with a targeted Python script patch.
- `--no-fail-fast` is not a valid `cargo test` flag (it's for `cargo nextest`). Used `cargo test` without it.

## Next Phase Readiness

Phase 197 Plan 02 complete. All agent-side launch resilience changes are in place:
- Pre-launch checks run before every game spawn
- AC timing is now adaptive (polling) not hardcoded (sleep)
- CM timeout is 30s with visibility into progress
- State is clean before retry via clean_state_reset()

---
*Phase: 197-launch-resilience-ac-hardening*
*Completed: 2026-03-26*
