---
phase: 364-session-quality-monitor
plan: "03"
subsystem: ws-send-audit
tags: [silent-drop, metrics, prometheus, ws, quality]
dependency_graph:
  requires: [364-01]
  provides: [GLD-D-04, GLD-D-05]
  affects: [ws/mod.rs, ws_handler.rs, metrics_producers.rs, metrics_tsdb.rs, metrics_prometheus.rs]
tech_stack:
  added: []
  patterns: [AtomicU64 counter, Prometheus _total counter type heuristic]
key_files:
  created: []
  modified:
    - crates/racecontrol/src/ws/mod.rs
    - crates/rc-agent/src/ws_handler.rs
    - crates/racecontrol/src/metrics_tsdb.rs
    - crates/racecontrol/src/metrics_producers.rs
    - crates/racecontrol/src/api/metrics_prometheus.rs
decisions:
  - "Used local AtomicU64 in test instead of global static to avoid test interference"
  - "Added _total suffix -> counter TYPE heuristic to prometheus formatter (was gauge-only)"
  - "rc-agent silent drops get tracing::warn but NOT overflow counter (different binary/process)"
metrics:
  duration_seconds: 2819
  completed: "2026-04-11T00:37:00Z"
  tasks_completed: 6
  tasks_total: 6
  tests_added: 2
  tests_total_pass: 2033
---

# Phase 364 Plan 03: Silent-Drop Audit Summary

**Replace all `let _ = ws_send(...)` patterns in session/telemetry hot path with proper error handling, expose ws_try_send_overflows_total counter at /metrics.**

## One-liner

Eliminated 7 silent WS send drops (1 racecontrol + 6 rc-agent) with tracing::warn error handling, added AtomicU64 overflow counter flushed to Prometheus every 30s.

## Changes

### Task 1: Add WS_TRY_SEND_OVERFLOWS counter + constant
- Added `pub static WS_TRY_SEND_OVERFLOWS: AtomicU64` in `ws/mod.rs`
- Added `METRIC_WS_TRY_SEND_OVERFLOWS` constant in `metrics_tsdb.rs`

### Task 2: Fix hot-path silent drop in ws/mod.rs
- Replaced `let _ = ws_sender.send(...)` at line 2719 (AI channel auth failure) with `if let Err(e) = ... { tracing::warn!(...) }`
- Verified: `rg 'let _ = ws_sender' crates/racecontrol/src/ws/` returns zero results

### Task 3: Fix hot-path silent drops in rc-agent ws_handler.rs
- Replaced 6 `let _ = state.ws_exec_result_tx.{try_send,send}(...)` patterns:
  - Line 324: LaunchTimelineReport try_send
  - Line 1519: ExecResult send (process guard confirm)
  - Line 1557: KioskLockdown try_send
  - Line 1961: JwtAck try_send (initial JWT)
  - Line 1973: JwtAck try_send (JWT refresh)
  - Line 2005: LaunchTimelineReport try_send (launch timeout)
- All replaced with `if let Err(e) = ... { tracing::warn!("[ws-handler] ...") }`
- dashboard_tx send patterns left untouched (broadcast channel, intentional)

### Task 4: Flush overflow counter in metrics_producers.rs
- Added section 5 in spawn_metric_producers loop: reads `WS_TRY_SEND_OVERFLOWS.load(Relaxed)` every 30s
- Emits as MetricSample to metrics_tx channel
- Added `overflow_counter_increments` unit test

### Task 5: Prometheus formatter update
- Added `_total` suffix heuristic: metrics ending in `_total` get `# TYPE ... counter` instead of `gauge`
- Added help text for `ws_try_send_overflows_total`
- Added `test_prometheus_formats_total_counter` test

### Task 6: Final audit
- SC4 verified: `rg 'let _ = ws_sender' crates/racecontrol/src/ws/` = 0 results
- SC5 verified: `rg 'ws_try_send_overflows_total' crates/` = 6 hits
- All workspace tests pass: 2033 total, 0 failures

## Commits

| Hash | Message |
|------|---------|
| d0e16e99 | feat(364-03): silent-drop audit -- replace let _ = ws_send + overflow metrics (GLD-D-04, GLD-D-05) |

## Decisions Made

1. **rc-agent overflow counter separate from racecontrol**: rc-agent is a different binary/process, so its silent drops get `tracing::warn` but do NOT increment racecontrol's `WS_TRY_SEND_OVERFLOWS` counter. Each binary would need its own counter if needed in future.
2. **Local AtomicU64 in test**: Used a local counter in `overflow_counter_increments` test instead of the global static to avoid interference with parallel tests.
3. **Prometheus _total heuristic**: Added a 1-line heuristic to `format_prometheus()` so any metric ending in `_total` is typed as `counter` instead of `gauge`. This is the Prometheus naming convention.

## Deviations from Plan

None -- plan executed exactly as written.

## Verification

- [x] `rg 'let _ = ws_sender' crates/racecontrol/src/ws/` returns ZERO results (SC4)
- [x] `rg 'let _ = state.ws_exec_result_tx' crates/rc-agent/src/ws_handler.rs` returns ZERO results
- [x] `rg 'ws_try_send_overflows_total' crates/` returns >= 3 hits (SC5)
- [x] `cargo test -p racecontrol-crate -- test_prometheus_formats_total_counter` exits 0
- [x] `cargo test -p rc-common -p rc-agent-crate -p racecontrol-crate` all pass (2033 tests, 0 failures)
- [x] `cargo build -p racecontrol-crate -p rc-agent-crate` exits 0

## Known Stubs

None -- all code is fully wired.
