---
phase: 186-maintenance-mode-auto-clear
plan: "01"
subsystem: rc-sentry
tags: [maintenance-mode, auto-clear, whatsapp-alert, crash-handler, json]
dependency_graph:
  requires: [185-02]
  provides: [MAINT-01, MAINT-02, MAINT-03]
  affects: [crates/rc-sentry/src/tier1_fixes.rs, crates/rc-sentry/src/main.rs]
tech_stack:
  added: []
  patterns: [serde_json-serialize-deserialize, recv_timeout-loop, cfg-windows-guard, cfg-test-guard]
key_files:
  created: []
  modified:
    - crates/rc-sentry/src/tier1_fixes.rs
    - crates/rc-sentry/src/main.rs
decisions:
  - "186-01: attempt_restart_after_clear uses #[cfg(windows)] guard with no-op on non-Windows — schtasks creation_flags is Windows-only"
  - "186-01: mtime fallback uses .ok() chain (Result->Option) instead of .and_then() chains — metadata/modified return Result not Option"
  - "186-01: check_and_clear_maintenance returns NotInMaintenance in #[cfg(test)] — same pattern as is_maintenance_mode, prevents file system access during tests"
metrics:
  duration: "~35 minutes"
  completed: "2026-03-25"
  tasks: 2
  files_modified: 2
---

# Phase 186 Plan 01: Maintenance Mode Auto-Clear Summary

**One-liner:** MAINTENANCE_MODE upgraded from silent permanent lock to JSON-based self-clearing mechanism with 30-min timeout, WOL_SENT immediate clear, and WhatsApp alert on activation.

## What Was Built

### Task 1: JSON maintenance payload + WhatsApp alert + auto-clear function (tier1_fixes.rs)

Added to `crates/rc-sentry/src/tier1_fixes.rs`:

- `MAINTENANCE_AUTOCLEAR_TIMEOUT: Duration::from_secs(1800)` (30 minutes)
- `WOL_SENT_SENTINEL: r"C:\RacingPoint\WOL_SENT"` constant
- `MaintenanceModePayload` struct with `serde::Serialize + Deserialize` — fields: `reason`, `timestamp_epoch`, `restart_count`, `diagnostic_context`
- `ClearResult` enum: `NotInMaintenance`, `StillLocked { remaining_secs }`, `Cleared { reason }`
- Rewrote `enter_maintenance_mode(reason, restart_count, diagnostic_context)` — writes JSON via `serde_json::to_string_pretty`, fires POST `/api/v1/fleet/alert` with pod_id/message/severity=critical immediately after writing file (MAINT-02, MAINT-03)
- Added `check_and_clear_maintenance() -> ClearResult` — WOL_SENT immediate clear (removes both files + calls `attempt_restart_after_clear()`), 30-min timeout from JSON `timestamp_epoch`, legacy plain-text fallback via mtime
- Added `read_maintenance_payload() -> Option<MaintenanceModePayload>` companion
- Added `attempt_restart_after_clear()` — `#[cfg(windows)]` guard + no-op on non-Windows, runs `schtasks /Run /TN StartRCAgent` with `CREATE_NO_WINDOW`
- Updated `handle_crash` call site to pass `restart_count` and `diagnostic_context` to new signature

### Task 2: Wire auto-clear into crash handler thread (main.rs)

Changed `while let Ok(ctx) = crash_rx.recv()` to `loop` with `recv_timeout(60s)`:

- Every loop iteration calls `check_and_clear_maintenance()` before waiting
- On `Cleared`: resets `tracker = RestartTracker::new()` and `consecutive_failures = 0`
- On `StillLocked`: logs remaining seconds at debug level
- On `Disconnected`: breaks cleanly with error log
- On `Timeout`: `continue` (retry auto-clear check on next 60s cycle)

## Success Criteria Verification

- MAINT-01: Auto-clears after 30 min (MAINTENANCE_AUTOCLEAR_TIMEOUT = 1800s) via check_and_clear_maintenance called every 60s from crash handler thread — PASS
- MAINT-02: MAINTENANCE_MODE file is JSON with reason, timestamp_epoch, restart_count, diagnostic_context (MaintenanceModePayload) — PASS
- MAINT-03: WhatsApp alert fires within enter_maintenance_mode via POST /api/v1/fleet/alert — PASS
- WOL_SENT sentinel triggers immediate clear — PASS
- Legacy plain-text files handled via mtime fallback — PASS
- All 64 rc-sentry tests pass — PASS
- Release build compiles — PASS

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] mtime fallback used wrong chaining method**
- **Found during:** Task 1 release build
- **Issue:** `metadata().and_then(|m| m.modified()).and_then(|t| t.elapsed().ok())` — `elapsed()` returns `Result<Duration>` not `Option<Duration>`, causing type mismatch. Also `.and_then()` on `Result` expects `FnOnce(T) -> Result` not `Option`.
- **Fix:** Changed to `.ok()` chains: `metadata().ok().and_then(|m| m.modified().ok()).and_then(|t| t.elapsed().ok())`
- **Files modified:** `crates/rc-sentry/src/tier1_fixes.rs`
- **Commit:** c7501edf

## Commits

| Hash | Description |
|------|-------------|
| 2fef1d3a | feat(186-01): JSON maintenance mode + auto-clear + WhatsApp alert on activation |
| c7501edf | feat(186-01): wire auto-clear into crash handler thread + fix mtime fallback |

## Self-Check: PASSED

- [x] crates/rc-sentry/src/tier1_fixes.rs exists and contains MaintenanceModePayload
- [x] crates/rc-sentry/src/main.rs exists and contains recv_timeout
- [x] Commit 2fef1d3a exists
- [x] Commit c7501edf exists
