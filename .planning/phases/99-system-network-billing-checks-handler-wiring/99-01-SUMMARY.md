---
phase: 99-system-network-billing-checks-handler-wiring
plan: 01
subsystem: agent
tags: [rust, pre-flight, billing, disk-space, memory, ws-stability, sysinfo, tokio-join]

requires:
  - phase: 98-02
    provides: 5-check run_concurrent_checks (hid, conspit, orphan, http, rect) + run() entry point

provides:
  - check_billing_stuck(billing_active): Fail if previous session still active (SYS-02)
  - check_disk_space(): Fail if C: drive < 1GB free via sysinfo::Disks (SYS-03)
  - check_memory(): Fail if < 2GB available RAM via sysinfo::System (SYS-04)
  - check_ws_stability(ws_connect_elapsed_secs): Warn if < 10s uptime (NET-01)
  - run_concurrent_checks now runs 9 checks via 9-way tokio::join!
  - run() accepts ws_connect_elapsed_secs: u64 from caller

affects:
  - crates/rc-agent/src/pre_flight.rs — 4 new check fns + 7 new unit tests + 9-way runner + run() signature
  - crates/rc-agent/src/ws_handler.rs — pass ws_elapsed to pre_flight::run at BillingStarted gate
  - crates/rc-agent/src/event_loop.rs — pass ws_elapsed to pre_flight::run in maintenance retry arm

tech-stack:
  added: []
  patterns:
    - "check_billing_stuck is pure logic (no I/O) — billing_active bool passed directly from state"
    - "check_disk_space + check_memory use spawn_blocking + sysinfo — same pattern as self_test.rs probe_disk/probe_memory"
    - "check_ws_stability is pure logic — ws_connect_elapsed_secs: u64 passed from conn.ws_connect_time.elapsed().as_secs()"
    - "WS stability is Warn (not Fail) per NET-01 spec — advisory only, does not block sessions"
    - "Disk threshold 1GB (not 2GB like self_test) — lower bar to avoid false positives on gaming pods"
    - "Memory threshold 2GB (not 1GB like self_test) — higher bar because sim racing requires RAM headroom"

key-files:
  created: []
  modified:
    - crates/rc-agent/src/pre_flight.rs
    - crates/rc-agent/src/ws_handler.rs
    - crates/rc-agent/src/event_loop.rs

key-decisions:
  - "ws_connect_elapsed_secs passed as u64 parameter to run() — avoids passing conn reference into pre_flight module"
  - "check_billing_stuck uses same billing_active bool already captured before tokio::join! — no extra state read"
  - "Disk check returns Warn (not Fail) if C: drive not found in sysinfo — graceful degradation on unexpected disk layout"
  - "test_concurrent_checks_returns_five renamed to expect 9 — historical name kept, assertion updated"

metrics:
  duration: 5min
  completed: 2026-03-21
  tasks: 1
  files_modified: 3
---

# Phase 99 Plan 01: System/Network Billing Checks Handler Wiring Summary

**4 new pre-flight checks (billing_stuck, disk_space, memory, ws_stability) wired into 9-way tokio::join! runner; run() signature extended with ws_connect_elapsed_secs; both call sites updated**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-21T05:14:09Z (IST: 10:44)
- **Completed:** 2026-03-21T05:18:51Z (IST: 10:49)
- **Tasks:** 1
- **Files modified:** 3 (0 created, 3 modified)

## Accomplishments

### Task 1: Add 4 check functions + extend runner to 9-way join (TDD)

**RED phase:** Added 7 new failing unit tests (test_billing_stuck_pass, test_billing_stuck_fail, test_disk_space_pass, test_memory_pass, test_ws_stability_stable, test_ws_stability_flapping, test_concurrent_checks_returns_nine). Updated test_concurrent_checks_returns_five to use new 5-arg signature.

**GREEN phase:**

- `check_billing_stuck(billing_active: bool)`: Pure logic. Fail if true ("stuck session"), Pass if false ("No stuck billing session").
- `check_disk_space()`: spawn_blocking + sysinfo::Disks. 1GB threshold. Pass/Fail based on C: drive available space. Warn if C: not found.
- `check_memory()`: spawn_blocking + sysinfo::System. 2GB threshold (2_147_483_648 bytes). Pass/Fail based on available RAM.
- `check_ws_stability(ws_connect_elapsed_secs: u64)`: Pure logic. Pass if >= 10s, Warn if < 10s (NOT Fail per NET-01 spec).
- `run_concurrent_checks`: signature extended with `ws_connect_elapsed_secs: u64`; 5-way `tokio::join!` extended to 9-way: `(hid, conspit, orphan, http, rect, billing, disk, memory, ws_stab)`.
- `run()`: signature extended with `ws_connect_elapsed_secs: u64` parameter.
- `ws_handler.rs` line 143: `let ws_elapsed = conn.ws_connect_time.elapsed().as_secs();` + pass to `pre_flight::run`.
- `event_loop.rs` line 698: same pattern in maintenance retry select! arm.

All 17 pre_flight unit tests pass (10 pre-existing + 7 new).

## Task Commits

1. **Task 1 TDD: 4 new checks + 9-way runner** - `9a4234b` (feat)

## Files Created/Modified

- `crates/rc-agent/src/pre_flight.rs` — +250 lines: 4 new check fns, 9-way runner, updated run() signature, 7 new unit tests
- `crates/rc-agent/src/ws_handler.rs` — +2 lines: ws_elapsed capture + pass to pre_flight::run
- `crates/rc-agent/src/event_loop.rs` — +2 lines: ws_elapsed capture + pass to pre_flight::run in retry arm

## Decisions Made

- `ws_connect_elapsed_secs: u64` passed as parameter to `run()` — keeps pre_flight module decoupled from ConnectionState
- Both call sites (ws_handler.rs BillingStarted + event_loop.rs maintenance retry) updated — consistent ws_elapsed injection
- Disk check returns Warn (not Fail) if C: not found — graceful on non-standard disk layouts
- WS stability is Warn (not Fail) per NET-01 spec — advisory, does not block sessions

## Deviations from Plan

None - plan executed exactly as written.

## Verification Results

- `cargo test -p rc-agent-crate pre_flight::tests` — 17 tests pass (10 pre-existing + 7 new)
- `cargo build --bin rc-agent` — compiles with 0 errors, 42 warnings (all pre-existing)
- `grep -c "check_billing_stuck" pre_flight.rs` — 4 (>= 3 required)
- `grep -c "check_disk_space" pre_flight.rs` — 3 (>= 3 required)
- `grep -c "check_memory" pre_flight.rs` — 3 (>= 3 required)
- `grep -c "check_ws_stability" pre_flight.rs` — 4 (>= 3 required)
- `grep -c "ws_connect_elapsed_secs" pre_flight.rs` — 11 (>= 4 required)
- `grep "1_000_000_000" pre_flight.rs` — match (1GB disk threshold)
- `grep "2_147_483_648" pre_flight.rs` — match (2GB memory threshold)
- 9-element vec![hid, conspit, orphan, http, rect, billing, disk, memory, ws_stab] verified in runner

## Self-Check: PASSED

- `crates/rc-agent/src/pre_flight.rs` — check_billing_stuck present (4 occurrences)
- `crates/rc-agent/src/pre_flight.rs` — check_disk_space present (3 occurrences)
- `crates/rc-agent/src/pre_flight.rs` — check_memory present (3 occurrences)
- `crates/rc-agent/src/pre_flight.rs` — check_ws_stability present (4 occurrences)
- `crates/rc-agent/src/pre_flight.rs` — 9-element vec in run_concurrent_checks confirmed
- `crates/rc-agent/src/ws_handler.rs` — ws_elapsed capture + pass confirmed
- `crates/rc-agent/src/event_loop.rs` — ws_elapsed capture + pass confirmed
- Commit `9a4234b` — exists in git log

---
*Phase: 99-system-network-billing-checks-handler-wiring*
*Completed: 2026-03-21*
