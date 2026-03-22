---
phase: 138-idle-health-monitor
plan: "02"
subsystem: agent
tags: [rust, tokio, rc-agent, event-loop, idle-health, self-heal, websocket]

# Dependency graph
requires:
  - phase: 138-01
    provides: "AgentMessage::IdleHealthFailed variant in rc-common protocol.rs"
provides:
  - "idle_health_interval (60s) field in ConnectionState"
  - "idle_health_fail_count (u32) field in ConnectionState"
  - "select! arm in event_loop.rs implementing IDLE-01/02/03/04"
  - "check_lock_screen_http and check_window_rect exposed as pub(crate) in pre_flight.rs"
affects: [138-03]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Idle health arm follows browser_watchdog_interval arm pattern: early-exit guards, then probe, then heal"
    - "Treat CheckStatus::Warn same as Fail for window_rect (Edge not found = needs healing)"
    - "Hysteresis counter saturating_add(1) to avoid overflow; resets to 0 on clean pass"

key-files:
  created: []
  modified:
    - crates/rc-agent/src/pre_flight.rs
    - crates/rc-agent/src/event_loop.rs

key-decisions:
  - "check_window_rect Warn treated as Fail for healing purposes — Edge not found means healing is needed"
  - "IdleHealthFailed sent every tick at/above threshold, not just once — server handles deduplication"
  - "No cfg gates needed in event_loop.rs for check_window_rect — non-windows stub returns Pass so arm is clean"

patterns-established:
  - "Idle health probe: billing_active check first, safe_mode check second, then IO probes"
  - "Two-phase failure handling: heal immediately on any failure, notify server only after hysteresis threshold"

requirements-completed: [IDLE-01, IDLE-02, IDLE-04]

# Metrics
duration: 15min
completed: 2026-03-22
---

# Phase 138 Plan 02: Idle Health Monitor — Event Loop Summary

**60s idle health loop in rc-agent event_loop.rs: probes lock_screen_http + window_rect, self-heals via close+relaunch, sends IdleHealthFailed after 3 consecutive failures, skips during billing and safe mode**

## Performance

- **Duration:** 15 min
- **Started:** 2026-03-22T09:30:00Z
- **Completed:** 2026-03-22T09:45:00Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Exposed `check_lock_screen_http()` and `check_window_rect()` as `pub(crate)` in `pre_flight.rs` (both `#[cfg(windows)]` and `#[cfg(not(windows))]` stubs) — visibility change only
- Added `idle_health_interval: tokio::time::Interval` (60s) and `idle_health_fail_count: u32` to `ConnectionState` struct and `new()`
- New `select!` arm after `browser_watchdog_interval` arm implements the full IDLE-01/02/03/04 spec:
  - IDLE-04: early-exit guard on `billing_active`
  - Standing rule #10: early-exit guard on `safe_mode_active`
  - IDLE-01: probes `check_lock_screen_http()` + `check_window_rect()` every 60s
  - IDLE-02: calls `close_browser()` + `launch_browser()` on any check failure; `CheckStatus::Warn` on `window_rect` treated as failure
  - IDLE-03: sends `AgentMessage::IdleHealthFailed` via `ws_tx` when `idle_health_fail_count >= 3`
- All 459 existing rc-agent tests pass with no regressions
- `cargo build --release --bin rc-agent` builds clean (48 pre-existing warnings, no new warnings)

## Task Commits

Each task was committed atomically:

1. **Task 1: Expose pre_flight check functions as pub(crate)** - `302953c` (feat)
2. **Task 2: Add idle health interval and select! arm to event_loop.rs** - `ecc832d` (feat)

**Plan metadata:** (pending docs commit)

## Files Created/Modified

- `crates/rc-agent/src/pre_flight.rs` — Changed `check_lock_screen_http`, `check_window_rect` (windows + non-windows) from private to `pub(crate)`
- `crates/rc-agent/src/event_loop.rs` — Added `idle_health_interval` + `idle_health_fail_count` fields, new 60s idle health `select!` arm

## Decisions Made

- `CheckStatus::Warn` on `window_rect` is treated the same as `Fail` for healing — if Edge is not found, we need to relaunch regardless of whether the window rect check returned Warn vs Fail
- `IdleHealthFailed` is sent every tick once the threshold is reached (not just once at exactly 3). Server-side deduplication is the responsibility of plan 03 (receiver)
- No `#[cfg(windows)]` gates in `event_loop.rs` for `check_window_rect()` — the non-windows stub returns `CheckStatus::Pass` so the arm remains correct and clean on all targets

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- Package name is `rc-agent-crate` (not `rc-agent`) in Cargo workspace — plan used `cargo check -p rc-agent` which fails; used `rc-agent-crate` as package spec. No impact on build outcome.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Plan 03 (server receiver for `IdleHealthFailed`) can proceed — the sender side is fully implemented and tested
- No blockers for Wave 3 plans

---
*Phase: 138-idle-health-monitor*
*Completed: 2026-03-22*

## Self-Check: PASSED

- FOUND: .planning/phases/138-idle-health-monitor/138-02-SUMMARY.md
- FOUND: crates/rc-agent/src/pre_flight.rs
- FOUND: crates/rc-agent/src/event_loop.rs
- FOUND commit: 302953c (Task 1)
- FOUND commit: ecc832d (Task 2)
