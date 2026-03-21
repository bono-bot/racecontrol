---
phase: 103-pod-guard-module
plan: 02
subsystem: infra
tags: [rust, sysinfo, tokio, spawn-blocking, process-guard, taskkill, grace-period]

# Dependency graph
requires:
  - phase: 103-01
    provides: ProcessGuardConfig struct, guard_whitelist Arc<RwLock<MachineWhitelist>>, guard_violation_tx mpsc channel on AppState
  - phase: 101-protocol-foundation
    provides: AgentMessage::ProcessViolation and AgentMessage::ProcessGuardStatus protocol variants
  - phase: 102-whitelist-schema-config-fetch-endpoint
    provides: MachineWhitelist type with violation_action + warn_before_kill fields
provides:
  - process_guard::spawn() entry point callable from main.rs with config, whitelist, and tx
  - run_scan_cycle with spawn_blocking sysinfo snapshot and grace_counts HashMap
  - kill_process_verified: PID identity check (name + start_time) before taskkill
  - is_self_excluded, is_whitelisted, is_critical_violation helper functions (pub crate)
  - log_guard_event with 512KB rotation to C:\RacingPoint\process-guard.log
  - CRITICAL_BINARIES constant with racecontrol.exe (zero-grace enforcement)
affects:
  - 103-03 (event_loop.rs guard violation forwarding — wires spawn() call + drains guard_violation_rx)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - spawn_blocking wraps sysinfo::refresh_processes (100-300ms blocking) — consistent with kiosk.rs pattern
    - Grace period via HashMap<String, (u32, u64)> (name -> count, first_start_time) — reset when process leaves violation
    - PID identity guard: re-snapshot in second spawn_blocking before taskkill to prevent PID reuse kills
    - Log rotation: check metadata size, truncate to 0 bytes if >512KB, then append — mirrors self_monitor.rs

key-files:
  created:
    - crates/rc-agent/src/process_guard.rs
  modified:
    - crates/rc-agent/src/main.rs

key-decisions:
  - "sysinfo .processes() returns &HashMap — must use .iter().filter() not .filter() directly"
  - "parent_pid set to 0 sentinel (sysinfo 0.33 has no parent PID API) — name-based exclusion is primary guard"
  - "own_pid exclusion applied inline (pid == own_pid continue) in scan loop, not in is_self_excluded helper"
  - "grace_counts cleaned after each cycle: retain only names still in violation (prevents HashMap memory growth)"

patterns-established:
  - "Process scan pattern: spawn_blocking -> collect Vec<(pid, name, exe, start_time)> -> drop sys -> async processing"
  - "PID identity pattern: collect start_time in scan, re-verify in second spawn_blocking before kill"

requirements-completed: [PROC-01, PROC-02, PROC-03, PROC-04, PROC-05, ALERT-01, ALERT-04]

# Metrics
duration: 18min
completed: 2026-03-21
---

# Phase 103 Plan 02: Process Guard Scanner Summary

**process_guard.rs with spawn_blocking sysinfo scan, two-cycle grace HashMap, PID-verified taskkill, CRITICAL racecontrol.exe detection, and 512KB-rotating audit log — 9 unit tests green**

## Performance

- **Duration:** 18 min
- **Started:** 2026-03-21T09:07:50Z
- **Completed:** 2026-03-21T09:25:40Z
- **Tasks:** 1
- **Files modified:** 2 (process_guard.rs created, main.rs mod declaration added)

## Accomplishments

- Created `process_guard::spawn()` — 60s startup amnesty, configurable scan interval, infinite loop with grace tracking
- Implemented full scan cycle: sysinfo snapshot in spawn_blocking, whitelist check, grace period (two-cycle default), PID-verified kill, violation reporting over mpsc channel
- CRITICAL tier enforced: racecontrol.exe on a pod skips grace period, uses WrongMachineBinary violation type
- 9 unit tests passing: self-exclusion (4), whitelist (2), critical violation (2), log rotation (1)
- Zero compile errors across rc-agent-crate; mod declaration added to main.rs

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement process_guard.rs — scan, grace, kill, logging** - `482079b` (feat, TDD)

**Plan metadata:** (docs commit follows)

_Note: Task 1 used TDD. One auto-fix deviation applied (Rule 3): `.processes()` returns &HashMap in sysinfo 0.33, requiring `.iter().filter()` — fixed inline before GREEN._

## Files Created/Modified

- `crates/rc-agent/src/process_guard.rs` - Full process scanning daemon: spawn(), run_scan_cycle(), kill_process_verified(), is_self_excluded(), is_whitelisted(), is_critical_violation(), log_guard_event(), 9 tests
- `crates/rc-agent/src/main.rs` - Added `mod process_guard;` after `mod overlay;`

## Decisions Made

- `sysinfo::System::processes()` returns `&HashMap<Pid, Process>` in 0.33 API — must call `.iter()` before `.filter()`. Plan's inline code assumed direct iterator (matches newer API). Fixed as Rule 3 deviation.
- `parent_pid` set to `0` sentinel because sysinfo 0.33 does not expose parent PID. Name-based exclusion (`rc-agent.exe`) is the primary self-exclusion guard.
- `own_pid` exclusion applied inline (`pid == own_pid { continue }`) in scan loop rather than in `is_self_excluded` helper — keeps helper pure and testable without process-context dependency.
- `grace_counts.retain()` after each cycle removes entries for processes no longer in violation — prevents unbounded HashMap growth over long uptime.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] sysinfo 0.33 processes() API returns HashMap — no direct filter**
- **Found during:** Task 1 (first cargo test run)
- **Issue:** Plan's inline scan code called `.filter()` directly on `sys.processes()` return value. In sysinfo 0.33, `processes()` returns `&HashMap<Pid, Process>` which does not implement Iterator directly. Compile error: `no method named 'filter' found for reference &HashMap<Pid, sysinfo::Process>`.
- **Fix:** Changed `.filter(...)` to `.iter().filter(...)` in the spawn_blocking closure
- **Files modified:** `crates/rc-agent/src/process_guard.rs`
- **Verification:** `cargo test -p rc-agent-crate process_guard` — 12 tests pass, 0 errors
- **Committed in:** `482079b` (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (Rule 3 — blocking API mismatch)
**Impact on plan:** Single-line fix, no scope change. sysinfo 0.33 pattern now consistent with kiosk.rs enforce_process_whitelist_blocking() which uses a blocking fn rather than chained iterators.

## Issues Encountered

None beyond the auto-fixed sysinfo API issue above.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Plan 03 can now call `process_guard::spawn(state.config.process_guard.clone(), state.guard_whitelist.clone(), state.guard_violation_tx.clone(), machine_id)` from main.rs event setup
- `guard_violation_rx` is ready to be drained in event_loop.rs and forwarded to WebSocket
- All 7 requirements (PROC-01 through PROC-05, ALERT-01, ALERT-04) are satisfied by this module

---
*Phase: 103-pod-guard-module*
*Completed: 2026-03-21*
