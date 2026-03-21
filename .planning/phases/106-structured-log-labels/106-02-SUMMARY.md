---
phase: 106-structured-log-labels
plan: 02
subsystem: infra
tags: [tracing, structured-logging, rust, rc-agent]

# Dependency graph
requires:
  - phase: 106-structured-log-labels
    provides: "LOG_TARGET pattern established in main.rs (plan 01)"
provides:
  - "ws_handler.rs: 60 tracing calls with target: LOG_TARGET=\"ws\""
  - "event_loop.rs: 53 tracing calls with target: LOG_TARGET=\"event-loop\""
  - "ac_launcher.rs: 51 tracing calls with target: LOG_TARGET=\"ac-launcher\""
  - "164 call sites migrated — codebase past 50% milestone"
affects: [106-03, 106-04, 106-05, 106-06]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "const LOG_TARGET: &str = \"label\" defined after use statements in each file"
    - "submodules need use super::LOG_TARGET to access parent const"

key-files:
  created: []
  modified:
    - crates/rc-agent/src/ws_handler.rs
    - crates/rc-agent/src/event_loop.rs
    - crates/rc-agent/src/ac_launcher.rs

key-decisions:
  - "Semantic bracket labels like [billing], [CM_ERROR], [self-test] are NOT stripped — only file-level prefixes [ws]/[event-loop]/[ac-launcher] would be stripped (none existed)"
  - "mid_session submodule in ac_launcher.rs needs use super::LOG_TARGET because Rust child modules do not inherit parent-scope items"

patterns-established:
  - "Python regex migration: same-line pattern (tracing::level!() followed by non-newline) + next-line pattern (tracing::level!( followed by newline + indent)"
  - "Submodule LOG_TARGET access: use super::LOG_TARGET"

requirements-completed: [LOG-02, LOG-03]

# Metrics
duration: 35min
completed: 2026-03-21
---

# Phase 106 Plan 02: Structured Log Labels — Three Highest Call-Count Files Summary

**164 tracing call sites in ws_handler.rs (60), event_loop.rs (53), ac_launcher.rs (51) migrated to structured target: labels, taking rc-agent past the 50% migration mark**

## Performance

- **Duration:** ~35 min
- **Started:** 2026-03-21T09:35:00 IST
- **Completed:** 2026-03-21T10:10:00 IST
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- ws_handler.rs: 60 tracing calls now carry `target: "ws"` for structured log routing
- event_loop.rs: 53 tracing calls carry `target: "event-loop"`
- ac_launcher.rs: 51 tracing calls carry `target: "ac-launcher"` including submodule mid_session
- No legacy bracket file-level prefixes found to strip in any of the 3 files
- cargo check passes (warnings only, pre-existing)

## Task Commits

Each task was committed atomically:

1. **Task 1: Migrate ws_handler.rs and event_loop.rs** - `00e0bd3` (feat)
2. **Task 2: Migrate ac_launcher.rs** - `578d053` (feat)

## Files Created/Modified

- `crates/rc-agent/src/ws_handler.rs` - Added LOG_TARGET="ws", 60 tracing calls migrated
- `crates/rc-agent/src/event_loop.rs` - Added LOG_TARGET="event-loop", 53 tracing calls migrated
- `crates/rc-agent/src/ac_launcher.rs` - Added LOG_TARGET="ac-launcher", 51 tracing calls migrated + submodule scope fix

## Decisions Made

- Bracket labels like `[billing]`, `[CM_ERROR]`, `[self-test]`, `[1/4]` are semantic step labels, NOT file-level prefixes — they were preserved. Only file-level prefixes matching `[ws]`/`[event-loop]`/`[ac-launcher]` would be stripped, and none existed.
- The `mid_session` submodule inside ac_launcher.rs needed `use super::LOG_TARGET;` because Rust child modules do not automatically inherit items from their parent module scope.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added const LOG_TARGET to main.rs**
- **Found during:** Task 1 (initial cargo check)
- **Issue:** main.rs had 38 `target: LOG_TARGET` calls from a previous session but was missing `const LOG_TARGET: &str = "rc-agent";`, causing 16 compile errors
- **Fix:** Applied the same Python migration script to main.rs to add the const and migrate remaining calls (38 already done, 28 more added)
- **Files modified:** crates/rc-agent/src/main.rs
- **Verification:** cargo check passes after fix
- **Committed in:** 00e0bd3 (part of Task 1 commit)

**2. [Rule 1 - Bug] Fixed LOG_TARGET scope in mid_session submodule**
- **Found during:** Task 2 (cargo check after ac_launcher.rs migration)
- **Issue:** Rust child modules don't inherit parent-scope items — `pub mod mid_session` couldn't access `LOG_TARGET` defined in ac_launcher.rs outer scope (2 compile errors at lines 529, 608)
- **Fix:** Added `use super::LOG_TARGET;` at the top of the `mid_session` block
- **Files modified:** crates/rc-agent/src/ac_launcher.rs
- **Verification:** cargo check passes
- **Committed in:** 578d053 (part of Task 2 commit)

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 bug)
**Impact on plan:** Both fixes required for compilation. No scope creep.

## Issues Encountered

- The stash from a previous abandoned session contained partial main.rs changes — the `target: LOG_TARGET` calls had been migrated but the `const LOG_TARGET` definition was missing. Fixed via Rule 3.

## Next Phase Readiness

- 3 of 6 highest call-count files migrated (ws_handler 60, event_loop 53, ac_launcher 51 = 164 calls)
- Phase 106-03 should target the next batch of files
- Pattern established: Python migration script handles both same-line and next-line tracing call formats

---
*Phase: 106-structured-log-labels*
*Completed: 2026-03-21*
