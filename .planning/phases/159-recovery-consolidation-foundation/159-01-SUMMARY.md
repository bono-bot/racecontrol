---
phase: 159-recovery-consolidation-foundation
plan: 01
subsystem: infra
tags: [rust, rc-common, recovery, serde, jsonl, watchdog, process-management]

# Dependency graph
requires: []
provides:
  - RecoveryAuthority enum (RcSentry, PodHealer, JamesMonitor) in rc-common
  - ProcessOwnership registry with single-owner enforcement and conflict detection
  - RecoveryAction enum (Restart, Kill, WakeOnLan, SkipCascadeGuardActive, SkipMaintenanceMode, EscalateToAi, AlertStaff)
  - RecoveryDecision struct with JSONL serialization (to_json_line())
  - RecoveryLogger append-only JSONL writer — warns on I/O error, never panics
  - RECOVERY_LOG_SERVER/POD/JAMES path constants
affects:
  - 159-02 (cascade-guard uses RecoveryDecision)
  - phase-160 (rc-sentry checks RecoveryAuthority::RcSentry before restarting)
  - phase-162 (james watchdog uses RecoveryAuthority::JamesMonitor)

# Tech tracking
tech-stack:
  added: [tracing (added to rc-common)]
  patterns: [JSONL append-only decision log, single-owner process registry, never-panic logger]

key-files:
  created:
    - crates/rc-common/src/recovery.rs
  modified:
    - crates/rc-common/src/lib.rs
    - crates/rc-common/Cargo.toml
    - Cargo.lock
    - LOGBOOK.md

key-decisions:
  - "Used plain enum with impl fmt::Display for OwnershipConflict instead of thiserror — thiserror is in workspace but not in rc-common dependencies; added tracing instead which was needed"
  - "RecoveryLogger.log() always returns Ok(()) — callers must not be burdened with log write failures; I/O errors emit tracing::warn"
  - "ProcessOwnership::register() is idempotent for same authority (same process + same owner = Ok), only conflicts on different owner"

patterns-established:
  - "Recovery logger pattern: create dirs lazily on first write, warn+swallow I/O errors, never panic"
  - "Ownership registry pattern: register-once-per-process, idempotent same-owner, error on conflict"

requirements-completed: [CONS-01, CONS-02]

# Metrics
duration: 20min
completed: 2026-03-22
---

# Phase 159 Plan 01: Recovery Consolidation Foundation Summary

**Shared recovery contracts in rc-common: RecoveryAuthority ownership registry, RecoveryDecision JSONL logger, and single-owner process enforcement for rc-sentry/pod_healer/james_monitor**

## Performance

- **Duration:** ~20 min
- **Started:** 2026-03-22T13:40:00Z
- **Completed:** 2026-03-22T14:00:00Z
- **Tasks:** 1 (TDD: red+green in single pass — all tests written with implementation)
- **Files modified:** 5

## Accomplishments

- RecoveryAuthority enum and ProcessOwnership registry — each process has exactly one declared owner; duplicate registration with different authority returns OwnershipConflict
- RecoveryDecision struct with full JSONL round-trip (to_json_line() + serde_json::from_str) covering all 7 fields
- RecoveryLogger: append-only JSONL writer, creates parent dirs lazily, emits tracing::warn on I/O error and returns Ok(()) — never panics
- 3 log path constants defined: SERVER, POD, JAMES
- 158 total rc-common tests pass; rc-sentry and racecontrol-crate check clean

## Task Commits

Each task was committed atomically:

1. **Task 1: Define recovery authority contracts in rc-common** - `287591b7` (feat)

**Plan metadata:** _(to be created as final doc commit)_

_Note: TDD — tests written alongside implementation in a single commit (all tests green from first run)_

## Files Created/Modified

- `crates/rc-common/src/recovery.rs` — RecoveryAuthority, ProcessOwnership, OwnershipConflict, RecoveryAction, RecoveryDecision, RecoveryLogger, 3 log path constants, 8 tests
- `crates/rc-common/src/lib.rs` — added `pub mod recovery;`
- `crates/rc-common/Cargo.toml` — added `tracing` workspace dependency
- `Cargo.lock` — updated for new dependency

## Decisions Made

- Used plain `enum OwnershipConflict` with `impl fmt::Display + std::error::Error` instead of `#[derive(thiserror::Error)]`. The `thiserror` crate is in the workspace but was not a dependency of rc-common; adding `tracing` was needed anyway for the logger, so kept the error type manual to avoid adding thiserror just for one enum.
- RecoveryLogger.log() signature is `-> std::io::Result<()>` per spec but always returns Ok(()); I/O errors are swallowed after emitting a tracing::warn. This matches the plan's "never returns Err" guarantee.
- ProcessOwnership::register() is idempotent for the same authority (re-registering same process + same owner returns Ok without error). Only different-owner conflicts return Err.

## Deviations from Plan

None - plan executed exactly as written. thiserror note is a clarification, not a deviation — the plan stated "if not present, use a plain enum" which is exactly what was done (thiserror is in workspace.dependencies but not in rc-common's [dependencies], so plan's fallback applied).

## Issues Encountered

None — all tests passed on first run. Both dependent crates (rc-sentry, racecontrol-crate) compile clean with no regressions.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- rc-common recovery module is complete and exported. Ready for:
  - Phase 159 Plan 02: CascadeGuard uses RecoveryDecision to count actions in 60s window
  - Phase 160: rc-sentry checks RecoveryAuthority::RcSentry before restarting rc-agent.exe
  - Phase 162: james_monitor uses RecoveryAuthority::JamesMonitor for Ollama/Claude/comms-link

## Self-Check: PASSED

- `crates/rc-common/src/recovery.rs` — FOUND
- `crates/rc-common/src/lib.rs` — FOUND
- Commit `287591b7` — FOUND

---
*Phase: 159-recovery-consolidation-foundation*
*Completed: 2026-03-22*
