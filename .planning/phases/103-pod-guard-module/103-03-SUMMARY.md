---
phase: 103-pod-guard-module
plan: 03
subsystem: infra
tags: [rust, tokio, walkdir, registry, process-guard, autostart, websocket]

# Dependency graph
requires:
  - phase: 103-02
    provides: process_guard::spawn() entry point, run_scan_cycle, log_guard_event
  - phase: 103-01
    provides: ProcessGuardConfig, guard_whitelist Arc<RwLock<MachineWhitelist>>, guard_violation channel on AppState
  - phase: 102-whitelist-schema-config-fetch-endpoint
    provides: GET /api/v1/guard/whitelist/pod-N endpoint, MachineWhitelist type with autostart_keys
  - phase: 101-protocol-foundation
    provides: AgentMessage::ProcessViolation, CoreToAgentMessage::UpdateProcessWhitelist
provides:
  - run_autostart_audit(): HKCU Run + HKLM Run + Startup folder scan with backup-before-remove
  - parse_run_key_entries() + is_autostart_whitelisted() helper functions (pub crate)
  - main.rs whitelist fetch from GET /api/v1/guard/whitelist/pod-N on startup with fallback
  - process_guard::spawn() called from main.rs after AppState construction
  - event_loop.rs guard_violation_rx select! arm forwarding violations to WebSocket
  - ws_handler.rs UpdateProcessWhitelist handler replaces whitelist under write lock
affects:
  - 104 (server-side process violation handling — violations now flow over WS)
  - 105 (netstat port audit module — same pattern: spawn + drain + WS forward)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Autostart audit: reg query via spawn_blocking, parse stdout with split_whitespace, backup JSON before remove
    - Startup folder scan: walkdir::WalkDir::new().max_depth(1) in spawn_blocking for .lnk/.url/.bat files
    - Whitelist fetch-on-startup: derive HTTP base from WS URL (replace ws:// + split /ws), fallback to default on any error
    - Audit interval dual-select!: scan_interval and audit_interval in same select! loop (5 min vs configurable)
    - #[cfg(windows)] use std::os::windows::process::CommandExt inside spawn_blocking closure for creation_flags

key-files:
  created:
    - crates/rc-agent/src/process_guard.rs (extended — run_autostart_audit, parse_run_key_entries, etc.)
  modified:
    - crates/rc-agent/src/process_guard.rs
    - crates/rc-agent/src/main.rs
    - crates/rc-agent/src/event_loop.rs
    - crates/rc-agent/src/ws_handler.rs
    - crates/rc-agent/src/config.rs

key-decisions:
  - "ProcessGuardConfig missing Clone derive — added #[derive(Clone)] to config.rs (required for spawn() call which takes ownership)"
  - "Whitelist fetch placed before AppState construction (config not yet moved) — initializes guard_whitelist Arc with fetched value directly"
  - "audit_startup_folder in kill_and_report mode: flag only (no delete) — file removal requires Phase 104 staff approval per design"
  - "#[cfg(windows)] use inside spawn_blocking closure — avoids non-windows compilation issues, mirrors ws_handler.rs/debug_server.rs pattern"

patterns-established:
  - "Dual interval select!: add new interval arm to existing scan loop — no separate tokio::spawn needed for periodic audit"
  - "Whitelist fetch fallback pattern: match on resp status + json parse, default() on any error branch — guard never blocks startup"

requirements-completed: [AUTO-01, AUTO-02, AUTO-04, ALERT-01, DEPLOY-01]

# Metrics
duration: 11min
completed: 2026-03-21
---

# Phase 103 Plan 03: Process Guard Wiring Summary

**Autostart audit (HKCU/HKLM Run + Startup folder) added to process_guard.rs; whitelist fetch, process_guard::spawn(), guard_violation_rx drain, and UpdateProcessWhitelist handler wired into rc-agent — 17 tests green, zero compile errors**

## Performance

- **Duration:** 11 min
- **Started:** 2026-03-21T09:16:09Z
- **Completed:** 2026-03-21T09:27:09Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments

- Task 1 (TDD): Added `run_autostart_audit()`, `audit_run_key()`, `audit_startup_folder()`, `parse_run_key_entries()`, `is_autostart_whitelisted()`, `backup_autostart_entry()` to process_guard.rs; 5 new tests green (17 total)
- Task 2: Wired whitelist fetch in main.rs (fallback to default on any error), process_guard::spawn() after AppState, guard_violation_rx select! arm in event_loop.rs, UpdateProcessWhitelist arm in ws_handler.rs
- Zero compile errors; all 17 tests passing; all 5 acceptance criteria met for both tasks

## Task Commits

Each task was committed atomically:

1. **Task 1: Add run_autostart_audit() + autostart tests to process_guard.rs** - `b5035a1` (feat, TDD)
2. **Task 2: Wire process guard into rc-agent** - `3416f9e` (feat)

**Plan metadata:** (docs commit follows)

_Note: Task 1 used TDD. One auto-fix deviation applied (Rule 2): ProcessGuardConfig missing Clone derive — fixed inline in Task 2._

## Files Created/Modified

- `crates/rc-agent/src/process_guard.rs` - Added run_autostart_audit, audit_run_key, audit_startup_folder, parse_run_key_entries, is_autostart_whitelisted, backup_autostart_entry; updated spawn() loop with audit_interval + immediate startup audit call
- `crates/rc-agent/src/main.rs` - Whitelist fetch block (GET /api/v1/guard/whitelist/pod-N, 10s timeout, fallback to default); process_guard::spawn() call after AppState
- `crates/rc-agent/src/event_loop.rs` - guard_violation_rx arm in select! loop — forwards AgentMessage::ProcessViolation to ws_tx
- `crates/rc-agent/src/ws_handler.rs` - UpdateProcessWhitelist match arm — write-locks guard_whitelist and replaces with server-pushed value
- `crates/rc-agent/src/config.rs` - Added Clone derive to ProcessGuardConfig

## Decisions Made

- `ProcessGuardConfig` was missing `Clone` — added `#[derive(Clone)]`. Required because `process_guard::spawn()` takes ownership of the config. Plan noted to check; derive was absent.
- Whitelist fetch placed before `AppState` construction (line ~665) so `config.core.url` and `config.pod.number` are accessible before `config` is moved into AppState. `guard_whitelist` Arc is then initialized with the fetched value directly instead of `MachineWhitelist::default()`.
- Startup folder audit: flag-only even in `kill_and_report` mode (no file deletion). Per plan comment: file removal requires Phase 104 staff approval. Backup still written.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] ProcessGuardConfig missing Clone derive**
- **Found during:** Task 2 (wiring process_guard::spawn() call)
- **Issue:** `process_guard::spawn()` takes `config: ProcessGuardConfig` by value. Called as `state.config.process_guard.clone()` — requires Clone on ProcessGuardConfig. The struct had `#[derive(Debug, Deserialize)]` but not `Clone`.
- **Fix:** Added `Clone` to derive macro: `#[derive(Debug, Clone, Deserialize)]`
- **Files modified:** `crates/rc-agent/src/config.rs`
- **Verification:** `cargo build -p rc-agent-crate 2>&1 | grep "^error"` returns empty
- **Committed in:** `3416f9e` (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (Rule 2 — missing critical derive)
**Impact on plan:** Single-line fix, no scope change. ProcessGuardConfig now follows the same derive pattern as KioskConfig and PreflightConfig (both have Clone).

## Issues Encountered

None beyond the auto-fixed Clone derive above.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Phase 103 complete. rc-agent now scans processes (Plan 02) AND autostart entries (Plan 03), fetches whitelist on startup, and forwards all violations to racecontrol over WebSocket.
- Phase 104 (racecontrol server-side violation handling) can now receive `AgentMessage::ProcessViolation` and respond with `CoreToAgentMessage::UpdateProcessWhitelist` pushes.
- All 5 requirements (AUTO-01, AUTO-02, AUTO-04, ALERT-01, DEPLOY-01) satisfied.

---
*Phase: 103-pod-guard-module*
*Completed: 2026-03-21*
