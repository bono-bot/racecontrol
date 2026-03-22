---
phase: 162-james-watchdog-migration
plan: "01"
subsystem: rc-watchdog
tags: [watchdog, james-monitor, graduated-response, failure-state, bono-alert]
dependency_graph:
  requires: [rc-common/recovery.rs]
  provides: [james_monitor::run_monitor, failure_state::FailureState, bono_alert::alert_bono]
  affects: [rc-watchdog binary dual-mode execution]
tech_stack:
  added: [reqwest blocking HTTP check, serde_json atomic state persistence]
  patterns: [graduated-response FSM, persistent failure state across invocations, atomic file write tmp+rename]
key_files:
  created:
    - crates/rc-watchdog/src/failure_state.rs
    - crates/rc-watchdog/src/bono_alert.rs
    - crates/rc-watchdog/src/james_monitor.rs
  modified:
    - crates/rc-watchdog/src/main.rs
decisions:
  - "graduated_action extracted as pub(crate) fn for testability — used by run_monitor loop"
  - "check_service_process is conservative on tasklist failure (returns true = assume running)"
  - "bono_alert uses child.wait() not wait_timeout (node script is fast ~1-2s, no timeout needed)"
  - "main.rs branches on --service arg: service mode uses RCWatchdog dispatcher, else runs james_monitor"
  - "define_windows_service! macro kept at crate root (required by windows-service crate)"
metrics:
  duration_minutes: 32
  completed_date: "2026-03-22T21:11:00+05:30"
  tasks_completed: 2
  files_created: 3
  files_modified: 1
  tests_added: 14
  tests_total: 29
---

# Phase 162 Plan 01: James Watchdog Migration — James Monitor Implementation Summary

Graduated-response James monitor in rc-watchdog: 5-service polling with persistent failure counts, restart attempts, and Bono escalation via comms-link WS.

## Tasks Completed

| # | Task | Commit | Files |
|---|------|--------|-------|
| 1 | failure_state.rs + bono_alert.rs — persistent state + alert | ca187c3c | failure_state.rs, bono_alert.rs, main.rs (modules), james_monitor.rs (stub) |
| 2 | james_monitor.rs full impl + main.rs wiring + release build | 3d67f8b5 | james_monitor.rs, main.rs |

## What Was Built

**failure_state.rs** — Persistent JSON state at `C:\Users\bono\.claude\watchdog-state.json`.
- `FailureState::load_from()` returns Default on missing or corrupt file
- `increment/reset/count` API for per-service failure tracking
- Atomic write: write to `.tmp` then `rename` prevents corruption on concurrent Task Scheduler runs

**bono_alert.rs** — Alert Bono via `node send-message.js`.
- `alert_bono_with_exe(exe, message)` extracted for testability
- Spawns node with `COMMS_PSK`/`COMMS_URL` env vars set
- Returns `Ok(())` even if node.exe is absent — degraded alert, no panic

**james_monitor.rs** — Core monitoring logic.
- 5 services: ollama (HTTP :11434), comms-link (HTTP :8766/relay/health), kiosk (HTTP server:3300), webterm (HTTP :9999), claude-code (process check)
- `check_service_http`: reqwest blocking, 3s timeout — any response = alive
- `check_service_process`: tasklist /NH, conservative (returns true on tasklist failure)
- `graduated_action(count)`: count=1 → Restart+log, count=2 → Restart+attempt, count>=3 → AlertStaff
- `run_monitor()`: loads FailureState, checks all services, logs via RecoveryLogger(RECOVERY_LOG_JAMES), saves state
- Bono alert fired on count>=3 with `[WATCHDOG] {service} DOWN on James (failure #{n})`
- Recovery path: count resets to 0, Restart+recovered decision logged

**main.rs** — Dual-mode entry point.
- `--service` flag → Windows service dispatcher (existing pod watchdog behavior preserved)
- No args → james_monitor::run_monitor() (Task Scheduler single-shot mode)
- Log files: pod mode → `C:\RacingPoint\watchdog.log`, james mode → `C:\Users\bono\.claude\rc-watchdog.log`

## Test Results

29 tests passing across all modules:
- failure_state: 6 tests (load default, corrupt, count, increment, reset, roundtrip save/load)
- bono_alert: 2 tests (missing node returns Ok, empty message returns Ok)
- james_monitor: 7 tests (unused port HTTP false, explorer process found, absent process false, graduated_action counts 1/2/3/10)
- service + reporter + session: existing 14 tests still passing

## Verification

```
cargo build --release -p rc-watchdog  → Finished in 9.33s
cargo test -p rc-watchdog              → 29 passed, 0 failed
target/release/rc-watchdog.exe        → 3.7MB
```

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Missing SubscriberExt import in main.rs**
- **Found during:** Task 1 — first cargo test run
- **Issue:** `tracing_subscriber::registry().with(subscriber)` failed — trait `SubscriberExt` not in scope
- **Fix:** Added `use tracing_subscriber::prelude::*;` to main.rs
- **Files modified:** crates/rc-watchdog/src/main.rs
- **Commit:** Inline in Task 1 commit (ca187c3c)

**2. [Rule 1 - Bug] graduated_action called separately from attempt_restart in run_monitor**
- **Found during:** Task 2 implementation review
- **Issue:** Plan's run_monitor inline code called attempt_restart inside match arm then re-derived action. Cleaner to call graduated_action() then conditionally call attempt_restart based on action+count.
- **Fix:** run_monitor calls `graduated_action(count)` to get (action, reason), then `if action == Restart && count == 2` calls `attempt_restart`. Keeps graduated_action pure and testable.
- **Files modified:** crates/rc-watchdog/src/james_monitor.rs

## Self-Check: PASSED

| Item | Status |
|------|--------|
| failure_state.rs | FOUND |
| bono_alert.rs | FOUND |
| james_monitor.rs | FOUND |
| rc-watchdog.exe (release) | FOUND |
| Commit ca187c3c | FOUND |
| Commit 3d67f8b5 | FOUND |
