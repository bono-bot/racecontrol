---
phase: 08-pod-lock-screen-hardening
plan: 01
subsystem: ui
tags: [rust, tokio, axum, lock-screen, startup, branding]

# Dependency graph
requires:
  - phase: 07-server-pinning
    provides: rc-agent deployed on pods, lock screen HTTP server infrastructure
provides:
  - StartupConnecting state variant in LockScreenState enum
  - wait_for_self_ready() async method (TCP readiness probe on port 18923)
  - show_startup_connecting() method (sets state + launches Edge kiosk)
  - render_startup_connecting_page() (branded HTML with RACING POINT, spinner, 3s reload)
  - Startup wiring in main.rs: start_server -> wait_for_self_ready -> show_startup_connecting
affects: [08-02, 08-03, pod-deploy, lock-screen-tests]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Port readiness probe: TcpStream::connect with 100ms timeout, 50ms retry, 5s deadline"
    - "Startup page pattern: page_shell + JS 3s reload (same as render_disconnected_page)"
    - "State isolation: StartupConnecting treated as idle/blanked for billing purposes"

key-files:
  created: []
  modified:
    - crates/rc-agent/src/lock_screen.rs
    - crates/rc-agent/src/debug_server.rs
    - crates/rc-agent/src/main.rs

key-decisions:
  - "StartupConnecting classified as is_idle_or_blanked()=true — pod not ready for customers during startup"
  - "health_response_body() returns 'degraded' for StartupConnecting — monitoring should not count startup as healthy"
  - "wait_for_self_ready() never panics — logs warning and returns if port not ready after 5s"
  - "Browser opened once by show_startup_connecting() at boot; subsequent state changes picked up by 3s JS reload, no re-launch needed"

patterns-established:
  - "Startup pattern: start_server() -> wait_for_self_ready() -> show_startup_connecting() before any reconnect attempts"
  - "TDD for lock_screen: tests in existing mod tests block using direct struct construction"

requirements-completed: [LOCK-01, LOCK-02, LOCK-03]

# Metrics
duration: 25min
completed: 2026-03-14
---

# Phase 8 Plan 01: StartupConnecting State Summary

**Port readiness probe + branded RACING POINT startup page eliminates ERR_CONNECTION_REFUSED race and replaces blank boot screen with branded waiting UI**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-03-14T~07:35Z
- **Completed:** 2026-03-14T~08:00Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- Added `StartupConnecting` state to `LockScreenState` enum with full match arm coverage across lock_screen.rs and debug_server.rs
- Implemented `wait_for_self_ready()` async method — polls TCP port 18923 with 100ms connect timeout, 50ms retry interval, 5s deadline with graceful fallback (no panic)
- Added branded `render_startup_connecting_page()` — "RACING POINT" in Enthocentric/#E10600, "Starting up..." subtitle, spin animation, 3s JS auto-reload
- Wired startup sequence in main.rs: `start_server()` → `wait_for_self_ready()` → `show_startup_connecting()` before reconnect loop
- All 16 lock_screen tests pass (6 new TDD tests + 10 pre-existing); 151 total rc-agent tests listed; 85 rc-common tests pass; release build succeeds

## Task Commits

Each task was committed atomically:

1. **Task 1: Add StartupConnecting state, readiness probe, and branded HTML with tests** - `9482e1b` (feat + TDD)
2. **Task 2: Wire startup sequence in main.rs** - `aec6536` (feat)

**Plan metadata:** _Included in final docs commit_

_Note: Task 1 followed full TDD Red-Green cycle: tests written first (RED: compile fails on missing variant/method), then implementation added (GREEN: all 16 tests pass)_

## Files Created/Modified

- `crates/rc-agent/src/lock_screen.rs` — StartupConnecting variant, wait_for_self_ready(), show_startup_connecting(), render_startup_connecting_page(), is_idle_or_blanked() update, health_response_body() update, 6 new tests
- `crates/rc-agent/src/debug_server.rs` — Added `StartupConnecting => "startup_connecting"` match arm (Rule 3 deviation auto-fix)
- `crates/rc-agent/src/main.rs` — Inserted wait_for_self_ready().await and show_startup_connecting() in startup sequence

## Decisions Made

- `StartupConnecting` classified as `is_idle_or_blanked() = true` — pod is not ready for customers during startup, consistent with `Disconnected` and `Hidden`
- `health_response_body()` returns `"degraded"` for `StartupConnecting` — monitoring should not count a starting pod as healthy
- `wait_for_self_ready()` never panics — 5s deadline with graceful log warning and return, ensuring agent always starts even if HTTP server is slow
- Browser opened exactly once by `show_startup_connecting()` at boot; state changes picked up by 3s JS reload, no browser re-launch needed on state transitions

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Non-exhaustive match in debug_server.rs**
- **Found during:** Task 1 (GREEN phase, cargo check --tests)
- **Issue:** `debug_server.rs:80` had exhaustive match on `LockScreenState` without a `StartupConnecting` arm — adding the new variant broke compilation
- **Fix:** Added `LockScreenState::StartupConnecting => "startup_connecting"` match arm
- **Files modified:** `crates/rc-agent/src/debug_server.rs`
- **Verification:** `cargo check -p rc-agent --tests` passes cleanly
- **Committed in:** `9482e1b` (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (Rule 3 — blocking)
**Impact on plan:** Necessary correctness fix caused by adding new enum variant to exhaustive match. No scope creep.

## Issues Encountered

- `cargo test -p rc-core` fails with pre-existing non-exhaustive match error in `crates/rc-core/src/ws/mod.rs` for `AgentMessage` variants (`AssistChanged`, `FfbGainChanged`, `AssistState`) added in a prior phase. This is **out-of-scope** for Plan 08-01 — documented in `deferred-items.md`.
- Bash tool background task output files had inconsistent naming — worked around by running tests with filter flags and using `--list` to confirm total test count.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- Plan 08-02 (show disconnected page on first reconnect attempt) can proceed: `StartupConnecting` state is in place, browser is open from boot
- Binary needs to be built and deployed to pods to validate ERR_CONNECTION_REFUSED elimination in production — Plan 08-03 (deploy) handles this
- Pre-existing rc-core compilation issue in `ws/mod.rs` does not affect rc-agent binary; tracked in deferred-items.md

## Self-Check

- [x] `crates/rc-agent/src/lock_screen.rs` — exists, contains `StartupConnecting`
- [x] `crates/rc-agent/src/main.rs` — exists, contains `wait_for_self_ready`
- [x] Commit `9482e1b` — Task 1 (lock_screen.rs + debug_server.rs)
- [x] Commit `aec6536` — Task 2 (main.rs startup wiring)
- [x] 16 lock_screen tests pass (verified in test run output)
- [x] Release build: `Finished release profile` confirmed

---
*Phase: 08-pod-lock-screen-hardening*
*Completed: 2026-03-14*
