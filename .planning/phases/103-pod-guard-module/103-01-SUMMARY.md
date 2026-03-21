---
phase: 103-pod-guard-module
plan: 01
subsystem: infra
tags: [rust, serde, toml, tokio, mpsc, arc, rwlock, walkdir, process-guard]

# Dependency graph
requires:
  - phase: 102-whitelist-schema-config-fetch-endpoint
    provides: MachineWhitelist type in rc-common/types.rs used for guard_whitelist field
  - phase: 101-protocol-foundation
    provides: AgentMessage protocol type used for guard_violation channel
provides:
  - ProcessGuardConfig struct in rc-agent/src/config.rs with enabled + scan_interval_secs fields
  - process_guard field on AgentConfig (serde default, no TOML section required)
  - walkdir = "2" dependency in rc-agent/Cargo.toml
  - guard_whitelist: Arc<RwLock<MachineWhitelist>> on AppState
  - guard_violation_tx/rx: mpsc channel on AppState (capacity 32)
affects:
  - 103-02 (process_guard.rs scanner module — reads all three AppState fields)
  - 103-03 (event_loop.rs guard violation forwarding — drains guard_violation_rx)

# Tech tracking
tech-stack:
  added:
    - walkdir = "2" (Startup folder scanning for process guard)
  patterns:
    - ProcessGuardConfig follows KioskConfig/PreflightConfig pattern: default_true() reused, new default_scan_interval() added
    - AppState guard fields follow ws_exec_result_tx/rx pattern (mpsc channels) and Arc<RwLock<T>> for shared mutable state

key-files:
  created: []
  modified:
    - crates/rc-agent/Cargo.toml
    - crates/rc-agent/src/config.rs
    - crates/rc-agent/src/app_state.rs
    - crates/rc-agent/src/main.rs

key-decisions:
  - "ProcessGuardConfig reuses existing default_true() fn — no second copy added"
  - "guard_violation channel capacity set to 32 (consistent with ws_exec channel pattern)"
  - "guard_whitelist initialized to MachineWhitelist::default() (report_only, empty lists) — safe until WS fetch"

patterns-established:
  - "New scan interval default pattern: fn default_scan_interval() -> u64 { 60 } — add per-config defaults rather than reusing generic ones"
  - "Channel + Arc<RwLock> initialized in main.rs before AppState literal, assigned via shorthand (guard_whitelist, guard_violation_tx, guard_violation_rx)"

requirements-completed: [DEPLOY-01, ALERT-04]

# Metrics
duration: 18min
completed: 2026-03-21
---

# Phase 103 Plan 01: Pod Guard Module Foundations Summary

**ProcessGuardConfig TOML struct, walkdir dep, and AppState guard_whitelist + violation channel added to rc-agent — Plan 02 can now reference all three contracts.**

## Performance

- **Duration:** 18 min
- **Started:** 2026-03-21T09:00:14Z
- **Completed:** 2026-03-21T09:18:00Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- Added `ProcessGuardConfig` with `enabled` (default=true) and `scan_interval_secs` (default=60), following existing KioskConfig/PreflightConfig pattern
- Added `walkdir = "2"` to Cargo.toml for Startup folder scanning in Plan 02
- Added `guard_whitelist: Arc<RwLock<MachineWhitelist>>` and `guard_violation_tx/rx` mpsc channel to AppState; wired in main.rs
- 3 TDD tests green (defaults, partial deserialization, missing TOML section)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add ProcessGuardConfig to config.rs and walkdir to Cargo.toml** - `c6ef4c2` (feat, TDD)
2. **Task 2: Add guard_whitelist and guard_violation channel fields to AppState** - `93060ed` (feat)

**Plan metadata:** (docs commit follows)

_Note: Task 1 used TDD — tests written first (RED: compile failure), then implementation (GREEN: 3 tests pass)._

## Files Created/Modified

- `crates/rc-agent/Cargo.toml` - Added `walkdir = "2"` under dependencies
- `crates/rc-agent/src/config.rs` - Added ProcessGuardConfig struct, default_scan_interval(), process_guard field on AgentConfig, 3 TDD tests
- `crates/rc-agent/src/app_state.rs` - Added RwLock + MachineWhitelist imports; guard_whitelist, guard_violation_tx/rx fields
- `crates/rc-agent/src/main.rs` - Added channel + whitelist initialization before AppState literal; wired 3 new fields

## Decisions Made

- Reused existing `default_true()` fn — plan explicitly stated not to add a second copy
- `guard_violation` channel capacity set to 32 (matches ws_exec_result channel pattern)
- `guard_whitelist` initialized to `MachineWhitelist::default()` (report_only, empty lists) — safe no-op state until WS connect fetches real whitelist

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Plan 02 (process_guard.rs scanner) can now reference `ProcessGuardConfig`, `guard_whitelist`, and `guard_violation_tx` directly from AppState
- `walkdir = "2"` available for Startup folder iteration
- All three contracts are compile-time verified (cargo build zero errors)

---
*Phase: 103-pod-guard-module*
*Completed: 2026-03-21*
