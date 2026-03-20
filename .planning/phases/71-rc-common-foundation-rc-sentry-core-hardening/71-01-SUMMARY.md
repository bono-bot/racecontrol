---
phase: 71-rc-common-foundation-rc-sentry-core-hardening
plan: 01
subsystem: infra
tags: [rust, rc-common, rc-sentry, wait-timeout, tokio, feature-gate, exec]

# Dependency graph
requires: []
provides:
  - ExecResult struct with stdout, stderr, exit_code, timed_out, truncated fields
  - run_cmd_sync: stdlib-only child process execution with timeout via wait-timeout crate
  - run_cmd_async: tokio-backed async exec, only compiled when tokio feature is enabled
  - truncate_output: safe Vec<u8>-first truncation helper
  - rc-sentry now depends on rc-common without pulling in tokio (SHARED-03 verified)
affects: [71-02, 71-03, 72, 74]

# Tech tracking
tech-stack:
  added: [wait-timeout = "0.2"]
  patterns:
    - Feature-gated tokio dep: `optional = true` + `[features] tokio = ["dep:tokio"]`
    - Truncate Vec<u8> before String::from_utf8_lossy to avoid UTF-8 char boundary panics
    - CREATE_NO_WINDOW on Windows for headless subprocess spawning in rc-common
    - After wait_timeout returns Ok(None), call child.kill() then child.wait() then take stdout/stderr handles

key-files:
  created:
    - crates/rc-common/src/exec.rs
  modified:
    - crates/rc-common/Cargo.toml
    - crates/rc-common/src/lib.rs
    - crates/rc-sentry/Cargo.toml

key-decisions:
  - "wait-timeout = 0.2 is the only correct stdlib-compatible child process timeout on Windows -- no tokio needed"
  - "tokio dep is optional in rc-common with explicit [features] tokio = [dep:tokio] gate to prevent accidental activation"
  - "truncation operates on Vec<u8> before UTF-8 conversion to prevent char boundary panics"
  - "rc-sentry depends on rc-common with NO features = [...] so tokio feature is never activated"

patterns-established:
  - "Feature gate pattern: optional dep + explicit [features] declaration blocks transitive tokio pull"
  - "Exec pipe pattern: after wait_timeout take stdout/stderr via child.stdout.take() and read_to_end"

requirements-completed: [SHARED-01, SHARED-02, SHARED-03]

# Metrics
duration: 20min
completed: 2026-03-20
---

# Phase 71 Plan 01: rc-common exec Module Summary

**Feature-gated exec module in rc-common with run_cmd_sync (wait-timeout, stdlib-only) and run_cmd_async (tokio, behind feature gate), verified that rc-sentry tree has zero tokio references**

## Performance

- **Duration:** ~20 min
- **Started:** 2026-03-20T11:40:00Z
- **Completed:** 2026-03-20T12:00:25Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- Created `crates/rc-common/src/exec.rs` with ExecResult, run_cmd_sync, run_cmd_async, truncate_output, and 5 unit tests
- Added `wait-timeout = "0.2"` as a required dep and tokio as optional dep with `[features] tokio = ["dep:tokio"]` gate
- Wired rc-sentry to depend on rc-common without the tokio feature; `cargo tree -p rc-sentry` shows zero tokio references
- 128/128 rc-common tests pass including all 5 new exec tests

## Task Commits

Each task was committed atomically:

1. **Task 1: Create rc-common exec.rs** - `075414e` (feat)
2. **Task 2: Wire rc-common into rc-sentry, verify tokio isolation** - `dc99840` (feat)

**Plan metadata:** (docs commit below)

## Files Created/Modified

- `crates/rc-common/src/exec.rs` - ExecResult struct, run_cmd_sync, run_cmd_async (feature-gated), truncate_output, 5 unit tests
- `crates/rc-common/Cargo.toml` - wait-timeout dep + optional tokio dep + [features] tokio gate
- `crates/rc-common/src/lib.rs` - added `pub mod exec;`
- `crates/rc-sentry/Cargo.toml` - added `rc-common = { path = "../rc-common" }` (no tokio feature)

## Decisions Made

- Used `wait-timeout = "0.2"` (not tokio) for run_cmd_sync so rc-sentry stays stdlib-only
- Optional tokio dep uses workspace dep reference to stay in sync with the rest of the project
- Truncation operates on `Vec<u8>` before `String::from_utf8_lossy` to prevent UTF-8 char boundary panics
- After `wait_timeout` returns `Ok(None)`, kill + wait + take pipe handles to read accumulated bytes

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- rc-common exec module is ready; rc-sentry and rc-agent can both consume it
- rc-sentry can call `rc_common::exec::run_cmd_sync` directly for timeout-enforced commands
- rc-agent can enable the `tokio` feature on rc-common and use `run_cmd_async`
- Ready for Phase 71 Plan 02 (rc-sentry core hardening: timeout enforcement, truncation, concurrency cap, partial read fix, structured logging)

---
*Phase: 71-rc-common-foundation-rc-sentry-core-hardening*
*Completed: 2026-03-20*
