---
phase: 24-crash-hang-launch-usb-bot-patterns
plan: 01
subsystem: testing
tags: [rust, tdd, ai-debugger, pod-state, red-green, wave0]

# Dependency graph
requires:
  - phase: 23-protocol-contract-concurrency-safety
    provides: AgentMessage bot variants and PodFailureReason enum used by ai_debugger
provides:
  - "PodStateSnapshot with Default derive + 3 new telemetry fields (last_udp_secs_ago, game_launch_elapsed_secs, hid_last_error)"
  - "10 RED test stubs covering CRASH-01/02/03, UI-01, USB-01 requirements"
  - "3 pub(crate) stub functions: fix_frozen_game, fix_launch_timeout, fix_usb_reconnect"
  - "3 new try_auto_fix dispatch arms for frozen game, launch timeout, USB reconnect"
affects: [25-bot-implementations, wave1-fix-functions]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "..Default::default() struct update syntax for PodStateSnapshot construction"
    - "todo!(Phase 24 Wave 1) stub pattern for TDD RED phase"
    - "billing_snapshot(bool) test helper for billing-gated test variants"

key-files:
  created: []
  modified:
    - crates/rc-agent/src/ai_debugger.rs
    - crates/rc-agent/src/main.rs

key-decisions:
  - "3 new try_auto_fix arms added in Wave 0 (not Wave 1) to enable dispatch tests to compile and fail at the stub boundary rather than None return"
  - "fix_frozen_game arm placed BEFORE generic relaunch/game arm to ensure keyword specificity — game frozen dispatches to fix_frozen_game not kill_stale_game"
  - "test_kill_error_dialogs_extended passes in Wave 0 (correct: tests existing arm behavior, not a new stub)"

patterns-established:
  - "Pattern: All PodStateSnapshot constructions use ..Default::default() — safe against future field additions"
  - "Pattern: Wave 0 stubs panic with todo!(Phase 24 Wave N) — makes RED state visible in test output"

requirements-completed: [CRASH-01, CRASH-02, CRASH-03, UI-01, USB-01]

# Metrics
duration: 6min
completed: 2026-03-16
---

# Phase 24 Plan 01: Crash/Hang/Launch/USB Bot Patterns Wave 0 Summary

**PodStateSnapshot gains Default derive + 3 telemetry fields; 10 RED test stubs written for 5 bot fix requirements (CRASH-01/02/03, UI-01, USB-01) — Wave 0 Nyquist compliance complete**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-16T11:06:58Z
- **Completed:** 2026-03-16T11:12:59Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- `PodStateSnapshot` derives `Default` and has 3 new `#[serde(default)]` fields: `last_udp_secs_ago`, `game_launch_elapsed_secs`, `hid_last_error`
- All 7 test snapshot literals in ai_debugger.rs and 4 in main.rs updated to `..Default::default()` struct update syntax — future-proofed against additional fields
- 3 `pub(crate)` stub functions added with `todo!("Phase 24 Wave 1")`: `fix_frozen_game`, `fix_launch_timeout`, `fix_usb_reconnect`
- 3 new `try_auto_fix` dispatch arms for game-frozen, launch-timeout, and wheelbase-usb-reset patterns
- 10 new test functions covering all 5 requirements — 9 fail RED with `todo!` panics, 1 passes (tests existing `kill_error_dialogs` arm)
- 213 previously-passing tests remain GREEN; `cargo build -p rc-agent-crate` compiles clean

## Task Commits

Each task was committed atomically:

1. **Task 1: Default derive + 3 new fields to PodStateSnapshot** - `6bc8637` (feat)
2. **Task 2: 10 failing RED test stubs + 3 stub functions** - `020aab4` (test)

## Files Created/Modified

- `crates/rc-agent/src/ai_debugger.rs` - Default derive on PodStateSnapshot, 3 new fields, 3 stub functions, 3 new dispatch arms, 10 new test functions, billing_snapshot helper
- `crates/rc-agent/src/main.rs` - 4 PodStateSnapshot constructions updated to use ..Default::default()

## Decisions Made

- Added 3 new try_auto_fix arms in Wave 0 (not deferred to Wave 1) so dispatch tests compile and fail at the stub boundary rather than returning None — cleaner RED signal
- `fix_frozen_game` arm placed before the generic `relaunch` + `game` arm to ensure keyword specificity — the string "game frozen relaunch acs.exe" must match fix_frozen_game, not kill_stale_game
- `test_kill_error_dialogs_extended` intentionally passes in Wave 0: it only tests that the existing werfault arm still dispatches with fix_type "kill_error_dialogs" — no stub function required

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed 4 missing fields in main.rs PodStateSnapshot constructions**
- **Found during:** Task 1 (add Default derive + new fields)
- **Issue:** `cargo test` failed with E0063 — 4 PodStateSnapshot literals in main.rs didn't include the 3 new fields
- **Fix:** Added `..Default::default()` to all 4 constructions at lines 945, 1004, 1457, 1511
- **Files modified:** `crates/rc-agent/src/main.rs`
- **Verification:** `cargo test -p rc-agent-crate` passed with all 212 existing tests GREEN after fix
- **Committed in:** `6bc8637` (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (Rule 3 - blocking build error)
**Impact on plan:** Necessary for correctness — adding new fields without fixing all construction sites breaks compilation. No scope creep.

## Issues Encountered

None beyond the auto-fixed blocking issue above.

## Next Phase Readiness

- Wave 0 complete: All 10 RED test stubs in place, build clean, 3 stub functions declared
- Wave 1 (Phase 24 Plan 02) can now implement `fix_frozen_game`, `fix_launch_timeout`, `fix_usb_reconnect` with test-driven confidence
- All stubs panic with `todo!("Phase 24 Wave 1")` — Wave 1 replaces these with real implementations
- Billing gate pattern established: fix_frozen_game must check billing_active and return success=false when inactive

---
*Phase: 24-crash-hang-launch-usb-bot-patterns*
*Completed: 2026-03-16*

## Self-Check: PASSED

- [x] `crates/rc-agent/src/ai_debugger.rs` exists and contains `#[derive(Default)]` on PodStateSnapshot
- [x] `crates/rc-agent/src/main.rs` exists with `..Default::default()` additions
- [x] Commit `6bc8637` exists (feat: Default derive + 3 new fields)
- [x] Commit `020aab4` exists (test: 10 RED test stubs + 3 stub functions)
- [x] `cargo build -p rc-agent-crate` EXIT_CODE=0
- [x] 9 new tests RED (todo! panics), 213 existing tests GREEN
