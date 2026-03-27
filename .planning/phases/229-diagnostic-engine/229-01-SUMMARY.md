---
phase: 229-diagnostic-engine
plan: "01"
subsystem: rc-agent
tags: [diagnostic-engine, anomaly-detection, rust, meshed-intelligence]
dependency_graph:
  requires: []
  provides: [diagnostic_engine::DiagnosticEvent, diagnostic_engine::DiagnosticTrigger, diagnostic_engine::spawn]
  affects: [crates/rc-agent/src/main.rs]
tech_stack:
  added: []
  patterns: [tokio::sync::watch receiver clone via subscribe(), try_send for non-blocking event emission, IST timestamp via chrono::FixedOffset]
key_files:
  created:
    - crates/rc-agent/src/diagnostic_engine.rs
  modified:
    - crates/rc-agent/src/main.rs
decisions:
  - "Used failure_monitor_tx.subscribe() for diagnostic_engine watch receiver — avoids consuming failure_monitor_rx before failure_monitor::spawn(), matches billing_guard pattern"
  - "HealthCheckFail and DisplayMismatch variants defined in enum but not emitted in periodic scan — these are for event-triggered callers (Plan 229-02 tier engine or ws_handler)"
  - "_diagnostic_event_rx named with underscore prefix to suppress unused warning — Plan 229-02 will consume it"
metrics:
  duration_minutes: 8
  completed_date: "2026-03-27T14:07:36Z"
  tasks_completed: 2
  tasks_total: 2
  files_created: 1
  files_modified: 1
---

# Phase 229 Plan 01: Diagnostic Engine — DiagnosticTrigger, DiagnosticEvent, spawn() Summary

**One-liner:** DiagnosticEngine with 9-variant DiagnosticTrigger enum, 5-min periodic scan loop, WS/game/sentinel/crash/violation/error-spike detection, wired into main.rs via failure_monitor_tx.subscribe()

## What Was Built

Created `crates/rc-agent/src/diagnostic_engine.rs` as the entry point of the Meshed Intelligence diagnostic pipeline. The module is detection-only — it watches for 9 defined anomaly classes and emits typed `DiagnosticEvent` values that Plan 02's tier decision tree will act on.

### DiagnosticTrigger Enum (9 variants, DIAG-01)

| Variant | Trigger Condition |
|---------|------------------|
| `Periodic` | Every 5-minute scheduled scan (DIAG-07) |
| `HealthCheckFail` | rc-agent HTTP health not responding (event-triggered) |
| `ProcessCrash { process_name }` | WerFault/WerReport process detected via sysinfo |
| `GameLaunchFail` | launch_started_at > 90s + no game_pid |
| `DisplayMismatch { expected, actual }` | Edge count = 0 when blanking active (event-triggered) |
| `ErrorSpike { errors_per_min }` | >5 error lines in last 120 rc-bot-events.log lines |
| `WsDisconnect { disconnected_secs }` | ws_connected == false for >30s |
| `SentinelUnexpected { file_name }` | MAINTENANCE_MODE, FORCE_CLEAN, SAFE_MODE, or unknown all-caps file |
| `ViolationSpike { delta }` | Violation log line count delta >50 per scan cycle |

### spawn() Function

- Lifecycle logs: `lifecycle: started`, `lifecycle: first_scan_complete`, `lifecycle: exited (channel closed)`
- 60s startup grace before first scan
- `tokio::time::interval(300s)` periodic loop
- `failure_monitor_rx.borrow().clone()` for pod state snapshots
- `event_tx.try_send()` — non-blocking, warns if channel full (buffer=32)

### main.rs Wiring

- `mod diagnostic_engine;` declared alphabetically between `debug_server` and `driving_detector`
- `mpsc::channel::<diagnostic_engine::DiagnosticEvent>(32)` channel created
- `diagnostic_engine::spawn(heartbeat_status.clone(), failure_monitor_tx.subscribe(), diagnostic_event_tx.clone())` called after `failure_monitor::spawn`

## Verification Results

- `cargo build -p rc-agent-crate`: PASS — `Finished dev profile in 29.92s`, 24MB binary
- No `grep "^error"` output from cargo check
- `grep -c "DiagnosticTrigger" diagnostic_engine.rs` = 10 (>= 9)
- No bare `.unwrap()` calls in diagnostic_engine.rs
- `lifecycle: started` present: YES
- `lifecycle: first_scan_complete` present: YES
- `mod diagnostic_engine` in main.rs: YES
- `diagnostic_engine::spawn` in main.rs: YES
- `DiagnosticEvent` channel type in main.rs: YES

## Deviations from Plan

None — plan executed exactly as written. The `failure_monitor_tx.subscribe()` approach was the documented alternative in the plan spec and matches the `billing_guard` pattern in main.rs.

## Self-Check: PASSED

Files exist:
- `crates/rc-agent/src/diagnostic_engine.rs` — FOUND (293 lines)
- `crates/rc-agent/src/main.rs` — FOUND (modified +15 lines)

Commits exist:
- `c34f5b8c` — FOUND: feat(229-01): create diagnostic_engine.rs
- `d42a00d9` — FOUND: feat(229-01): wire diagnostic_engine::spawn() into main.rs
