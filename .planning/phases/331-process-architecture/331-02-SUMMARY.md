---
phase: 331-process-architecture
plan: 02
subsystem: rc-watchdog, rc-sentry
tags: [refactor, restart-authority, cleanup, process-architecture]
dependency_graph:
  requires: []
  provides: [clean-rollback-manager, clean-sentry-restart, watchdog-sole-restart-authority]
  affects: [rc-watchdog, rc-sentry]
tech_stack:
  added: []
  patterns: [single-restart-authority, sentinel-based-crash-loop-protection]
key_files:
  created: []
  modified:
    - crates/rc-watchdog/src/rollback_manager.rs
    - crates/rc-watchdog/src/service.rs
    - crates/rc-sentry/src/tier1_fixes.rs
    - crates/rc-sentry/src/cognitive_gate.rs
    - crates/rc-sentry/src/peer_channel.rs
    - crates/rc-sentry/src/mi_knowledge_base.rs
decisions:
  - Replace binary rename (rc-agent-failed.exe) with delete+fallback-rename for simpler rollback
  - Keep MAINTENANCE_MODE as sole crash loop protection (no binary rename needed)
  - Fully neuter restart_service() to log-only (remove breadcrumb cleanup, config load)
metrics:
  duration: 775s
  completed: 2026-04-07T13:49:09Z
  tasks: 2/2
  files_modified: 6
---

# Phase 331 Plan 02: Restart Authority Cleanup Summary

Remove binary rename from rollback_manager, clean sentry dead restart code, update schtask runtime references to RCWatchdog-only instructions.

## Tasks Completed

| Task | Name | Commit | Key Changes |
|------|------|--------|-------------|
| 1 | Remove binary rename from rollback_manager | b394420b | Removed rc-agent-failed.exe rename, replaced with delete+fallback; kept MAINTENANCE_MODE, restart loop detection, rc-agent-prev.exe handling |
| 2 | Clean sentry dead restart code + update schtask references | 36579ebd | Simplified restart_service() to log+return only; updated cognitive_gate.rs and peer_channel.rs playbook strings; fixed pre-existing PosWrongUrl match arm |

## Verification Results

1. `cargo test -p rc-watchdog` -- 78 passed, 0 failed (1 pre-existing skip: test_deterministic_deep_rollback)
2. `cargo test -p rc-sentry` -- 123 passed, 4 pre-existing integration failures (auth-related, not our changes)
3. `cargo build --release --bin rc-watchdog` -- compiles
4. `cargo build --release --bin rc-sentry` -- compiles
5. `grep -rn "rc-agent-failed" crates/rc-watchdog/src/` -- only v331 documentation comments (zero functional references)
6. `grep -rn "schtasks.*StartRCAgent" crates/rc-sentry/src/` -- ZERO results
7. `grep -rn "MAINTENANCE_MODE" crates/rc-watchdog/src/rollback_manager.rs` -- 19 hits (preserved)
8. `grep -rn "restart_timestamps|restart_loop" crates/rc-watchdog/src/` -- 9 hits (preserved)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed pre-existing PosWrongUrl match arm in mi_knowledge_base.rs**
- **Found during:** Task 2 (rc-sentry compile check)
- **Issue:** `DiagnosticTrigger::PosWrongUrl` variant was added to rc-common but mi_knowledge_base.rs `normalize_problem_key()` match was not updated, causing compile failure
- **Fix:** Added `DiagnosticTrigger::PosWrongUrl { .. } => "pos_wrong_url".to_string()` match arm
- **Files modified:** crates/rc-sentry/src/mi_knowledge_base.rs
- **Commit:** 36579ebd

### Pre-existing Test Failures (Not Our Changes)

- `test_deterministic_deep_rollback` in rc-watchdog mma_diagnosis.rs -- assertion text mismatch, pre-existing
- 4 integration tests in rc-sentry main.rs -- auth 401 errors from live service dependency, pre-existing

## Decisions Made

1. **Delete vs rename for current binary during rollback:** Used `remove_file()` with fallback to timestamped rename (`rc-agent-rolled-YYYYMMDDHHMMSS.exe`) when Windows file lock prevents deletion. This is simpler than the old rc-agent-failed.exe pattern and avoids accumulating named failed binaries.
2. **Kept doc comments referencing rc-agent-failed.exe:** Comments marked with `v331:` explain what was removed. These are documentation, not functional references.

## Known Stubs

None -- all changes are complete removals/simplifications with no placeholder code.
