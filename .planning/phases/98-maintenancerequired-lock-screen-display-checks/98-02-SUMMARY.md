---
phase: 98-maintenancerequired-lock-screen-display-checks
plan: 02
subsystem: agent
tags: [rust, pre-flight, lock-screen, display-checks, maintenance-retry, tcp-probe, winapi]

requires:
  - phase: 98-01
    provides: LockScreenState::MaintenanceRequired + show_maintenance_required() + in_maintenance AtomicBool + ClearMaintenance ws_handler

provides:
  - check_lock_screen_http() probing 127.0.0.1:18923 with 2s timeout + HTTP 200 verification
  - check_window_rect() using FindWindowA(Chrome_WidgetWin_1) + GetWindowRect, Warn if not found
  - run_concurrent_checks now runs 5 checks (was 3): hid, conspit, orphan, http, rect
  - maintenance_retry_interval (30s) in ConnectionState + select! arm auto-clears in_maintenance on Pass

affects:
  - crates/rc-agent/src/pre_flight.rs — 2 new check fns + testable _on helper + 4 unit tests + 5-result runner
  - crates/rc-agent/src/event_loop.rs — maintenance_retry_interval field + init + select! arm

tech-stack:
  added: []
  patterns:
    - "check_lock_screen_http_on(addr) helper for port-param testability — public fn delegates to it"
    - "tokio::io::{AsyncReadExt, AsyncWriteExt} used inside async fn (not at module level)"
    - "spawn_blocking for GetWindowRect — raw unsafe extern system fn, Windows-only cfg gate"
    - "select! arm guard: if !in_maintenance.load() { continue } — zero overhead on healthy pods"
    - "PreFlightPassed has only pod_id field (no timestamp) — verified from rc-common/src/protocol.rs"

key-files:
  created: []
  modified:
    - crates/rc-agent/src/pre_flight.rs
    - crates/rc-agent/src/event_loop.rs

key-decisions:
  - "check_lock_screen_http_on(addr) helper added for testability — public fn delegates, tests use ephemeral port"
  - "PreFlightPassed has no timestamp field — plan snippet was wrong; corrected to match rc-common protocol definition"
  - "Window not found returns Warn (not Fail) per DISP-02 spec — advisory, does not block sessions"
  - "GetWindowRect receives *mut [i32; 4] instead of *mut RECT (no winapi crate dep needed)"

metrics:
  duration: 4min
  completed: 2026-03-21
  tasks: 2
  files_modified: 2
---

# Phase 98 Plan 02: Display Checks + Maintenance Retry Loop Summary

**DISP-01 HTTP probe (127.0.0.1:18923) + DISP-02 GetWindowRect (Chrome_WidgetWin_1) wired into 5-check concurrent runner, plus 30-second maintenance retry select! arm that auto-clears in_maintenance on Pass**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-21T04:50:57Z (IST: 10:20)
- **Completed:** 2026-03-21T04:55:15Z (IST: 10:25)
- **Tasks:** 2
- **Files modified:** 2 (0 created, 2 modified)

## Accomplishments

### Task 1: DISP-01 HTTP probe + DISP-02 GetWindowRect in pre_flight.rs

- `check_lock_screen_http_on(addr: &str)` — core implementation with 2s connect timeout, HTTP GET write, 256-byte response read, "HTTP/1." + "200" check
- `check_lock_screen_http()` — public entry point delegates to `_on("127.0.0.1:18923")`
- `check_window_rect()` — Windows cfg gate with FindWindowA("Chrome_WidgetWin_1") + GetWindowRect; not-found = Warn; < 90% coverage = Fail; non-Windows = Pass
- `run_concurrent_checks` updated from 3-way join to 5-way join: `(hid, conspit, orphan, http, rect)`
- 4 new unit tests: `test_lock_screen_http_fail`, `test_lock_screen_http_pass`, `test_window_rect_non_windows`, `test_concurrent_checks_returns_five`
- All 10 pre_flight tests pass

### Task 2: 30-second maintenance retry loop in event_loop.rs

- `maintenance_retry_interval: tokio::time::Interval` added to ConnectionState struct
- Initialized to `tokio::time::interval(Duration::from_secs(30))` in `ConnectionState::new()`
- New select! arm placed after `overlay_topmost_interval` tick arm:
  - Guard: `if !in_maintenance.load(Relaxed) { continue }` — zero overhead on healthy pods
  - On Pass: clears `in_maintenance`, calls `show_idle_pin_entry()`, sends `PreFlightPassed { pod_id }`
  - On MaintenanceRequired: refreshes lock screen via `show_maintenance_required(failure_strings)`, logs warn
- `cargo build --bin rc-agent` succeeds with 0 errors (42 warnings, all pre-existing)

## Task Commits

1. **Task 1 TDD: DISP-01 + DISP-02 checks** - `41c952a` (feat)
2. **Task 2: Maintenance retry loop** - `5ac39ee` (feat)

## Files Created/Modified

- `crates/rc-agent/src/pre_flight.rs` — +238 lines: check_lock_screen_http, check_lock_screen_http_on, check_window_rect (Windows + non-Windows), 4 unit tests, 5-check runner
- `crates/rc-agent/src/event_loop.rs` — +31 lines: maintenance_retry_interval field + init + select! arm

## Decisions Made

- `check_lock_screen_http_on(addr)` helper added for testability — plan option (b), cleaner than port parameter on the public fn
- `PreFlightPassed { pod_id }` only (no timestamp) — plan snippet included timestamp but rc-common protocol.rs defines only pod_id; auto-fixed during compilation (Rule 1)
- Window not found returns Warn per spec — advisory check, not a session blocker
- Raw `unsafe extern "system"` for GetWindowRect avoids winapi crate dependency (same pattern as lock_screen.rs GetSystemMetrics)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] PreFlightPassed has no timestamp field**
- **Found during:** Task 2 compilation
- **Issue:** Plan code snippet showed `AgentMessage::PreFlightPassed { pod_id, timestamp: ... }` but rc-common/src/protocol.rs defines it as `PreFlightPassed { pod_id: String }` only
- **Fix:** Removed `timestamp` field from the struct literal
- **Files modified:** `crates/rc-agent/src/event_loop.rs`
- **Commit:** Included in `5ac39ee`
- **Impact:** Zero — protocol matches existing definition, no schema change

## Verification Results

- `cargo test -p rc-agent-crate pre_flight::tests` — 10 tests pass (6 pre-existing + 4 new)
- `cargo build --bin rc-agent` — compiles with 0 errors, 42 warnings (all pre-existing)
- `grep -c "check_lock_screen_http" pre_flight.rs` — 7 (>= 3 required)
- `grep -c "check_window_rect" pre_flight.rs` — 5 (>= 3 required)
- `grep -c "18923" pre_flight.rs` — 2 (>= 1 required)
- `grep -c "Chrome_WidgetWin_1" pre_flight.rs` — 2 (>= 1 required)
- `grep -c "maintenance_retry_interval" event_loop.rs` — 3 (>= 3 required)
- `grep -c "in_maintenance" event_loop.rs` — 2 (>= 2 required)
- `grep -c "Maintenance retry" event_loop.rs` — 3 (>= 2 required)
- `grep -c "PreFlightPassed" event_loop.rs` — 2 (>= 1 required)

## Self-Check: PASSED

- `crates/rc-agent/src/pre_flight.rs` — check_lock_screen_http present (7 occurrences)
- `crates/rc-agent/src/pre_flight.rs` — check_window_rect present (5 occurrences)
- `crates/rc-agent/src/pre_flight.rs` — 5-way join in run_concurrent_checks
- `crates/rc-agent/src/event_loop.rs` — maintenance_retry_interval field + init + arm (3 occurrences)
- `crates/rc-agent/src/event_loop.rs` — in_maintenance load + store (2 occurrences)
- Commits `41c952a`, `5ac39ee` — all exist in git log

---
*Phase: 98-maintenancerequired-lock-screen-display-checks*
*Completed: 2026-03-21*
