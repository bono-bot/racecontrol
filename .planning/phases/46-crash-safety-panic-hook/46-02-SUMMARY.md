---
phase: 46-crash-safety-panic-hook
plan: 02
subsystem: infra
tags: [rust, panic-hook, ffb-safety, port-bind, axum, tokio, e2e-test, bash]

requires:
  - phase: 46-01
    provides: zero_force_with_retry on FfbController + StartupReport boot verification fields in rc-common protocol

provides:
  - Panic hook installed as first code in main() — zeros FFB, logs crash, shows System Error lock screen, exits
  - start_server_checked() on LockScreenManager — observable port bind result via oneshot
  - start_checked() on remote_ops — observable port bind result via oneshot (retains 10-retry CLOSE_WAIT recovery)
  - Real lock_screen_bound + remote_ops_bound + hid_detected values wired into StartupReport BootVerification
  - startup-verify.sh E2E test for all 8 pods (remote ops, lock screen port, WS connected)

affects:
  - Any plan deploying rc-agent to pods (crash now safe + observable)
  - Fleet health monitoring (BootVerification now carries real port-bind status)
  - E2E test suite (startup-verify.sh as post-deploy gate)

tech-stack:
  added: []
  patterns:
    - "Panic hook pattern: AtomicBool guard + OnceLock state handle — avoids deadlock via try_lock"
    - "Oneshot bind signaling: fire-and-forget task signals bind result back to main() for observability"
    - "serve_with_listener() extraction: accept loop shared between fire-and-forget and checked variants"

key-files:
  created:
    - tests/e2e/fleet/startup-verify.sh
  modified:
    - crates/rc-agent/src/main.rs
    - crates/rc-agent/src/lock_screen.rs
    - crates/rc-agent/src/remote_ops.rs

key-decisions:
  - "PANIC_HOOK_ACTIVE AtomicBool + OnceLock<Arc<Mutex<LockScreenState>>> as statics — allows panic hook to safely update lock screen without holding locks"
  - "Panic hook uses try_lock not lock to avoid deadlock if panic occurs while state mutex is held"
  - "remote_ops start_checked() started early (before FFB/HID init) so 30s retry window runs concurrently — await moved to just before WS loop"
  - "start() kept as dead code in remote_ops.rs — it is a public function, removing would be a breaking change"
  - "E2E Gate 2 (lock screen :18923) uses remote_ops /exec with PowerShell Test-NetConnection — port is localhost-only, not LAN-reachable"
  - "hid_detected captured from zero_force_with_retry return value — true only if device found AND estop succeeded"

patterns-established:
  - "Panic hook: always installed BEFORE tracing/config init — catches even config-load panics"
  - "Bind signaling: oneshot channel pattern for observable async server startup"

requirements-completed: [SAFETY-01, SAFETY-02, SAFETY-04, SAFETY-05]

duration: 6min
completed: 2026-03-19
---

# Phase 46 Plan 02: Panic Hook + Port-Bind Signaling + BootVerification Summary

**Conspit Ares wheelbase panic safety: FFB zeroed + lock screen error shown + crash logged + port-bind failures exit cleanly with observable BootVerification in StartupReport**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-19T01:16:24Z
- **Completed:** 2026-03-19T01:22:14Z
- **Tasks:** 2
- **Files modified:** 4 (3 Rust, 1 bash)

## Accomplishments
- Panic hook installed as the very first code in main() — any unhandled panic now: zeros FFB with 3 retries (physical safety), writes to rc-bot-events.log, transitions lock screen to "System Error — Please Contact Staff", then exits with code 1
- Port bind failures for :18923 (lock screen) and :8090 (remote ops) are now observable and fatal — silent return replaced with oneshot signaling + clean exit on failure
- Extended StartupReport BootVerification now carries real values: lock_screen_bound, remote_ops_bound, hid_detected, udp_ports_bound — no longer false/empty placeholders from Plan 01
- E2E startup-verify.sh created: verifies all 8 pods have remote ops reachable, lock screen port bound, and WebSocket connected after agent restart

## Task Commits

Each task was committed atomically:

1. **Task 1: Panic hook + port-bind signaling + BootVerification wiring** - `7f16401` (feat)
2. **Task 2: Create startup-verify.sh E2E test** - `1410295` (test)

**Plan metadata:** (final commit follows)

## Files Created/Modified
- `crates/rc-agent/src/main.rs` - PANIC_HOOK_ACTIVE + PANIC_LOCK_STATE statics; panic hook as first code; remote_ops start_checked(); lock_screen start_server_checked(); hid_detected from zero_force_with_retry; remote_ops_bound await before WS loop; real BootVerification fields in StartupReport
- `crates/rc-agent/src/lock_screen.rs` - Added start_server_checked() method; extracted serve_with_listener() from serve_lock_screen() to share accept loop
- `crates/rc-agent/src/remote_ops.rs` - Added start_checked() alongside existing start(); same retry loop, signals bind result via oneshot
- `tests/e2e/fleet/startup-verify.sh` - Fleet E2E test: 3 gates per pod (remote ops ping, lock screen port 18923 bound via exec, WS connected via fleet health)

## Decisions Made
- Panic hook uses `try_lock` not `lock` to update lock screen state — avoids deadlock if panic occurs while the state mutex is held by another task
- `remote_ops::start_checked()` is started early (before FFB/HID init) so the 30s retry window (10 attempts * 3s) runs concurrently with other startup work; the `await` on the result is deferred to just before the WS reconnect loop
- `hid_detected` captures the return value of `zero_force_with_retry(3, 100)` — `true` only when the OpenFFBoard device is found AND the estop command succeeds; `false` if device absent (permanent) or all retries fail
- E2E Gate 2 (lock screen :18923) uses remote_ops `/exec` with PowerShell `Test-NetConnection` since the lock screen only listens on 127.0.0.1 (not LAN-reachable)
- `start()` retained in remote_ops.rs as dead code — removing a public function is a breaking change; it may be used by tests or future callers

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None - build succeeded first attempt, all 116 rc-common tests pass.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 46 is complete: all 5 requirements done (SAFETY-01 through SAFETY-05)
- rc-agent binary is ready to deploy to pods with panic safety wired
- startup-verify.sh can be run after any rc-agent deploy as a post-deploy gate
- Server-side BootVerification data now flows from pods — racecontrol can surface it in fleet health

---
*Phase: 46-crash-safety-panic-hook*
*Completed: 2026-03-19*

## Self-Check: PASSED

- FOUND: crates/rc-agent/src/main.rs
- FOUND: crates/rc-agent/src/lock_screen.rs
- FOUND: crates/rc-agent/src/remote_ops.rs
- FOUND: tests/e2e/fleet/startup-verify.sh
- FOUND: .planning/phases/46-crash-safety-panic-hook/46-02-SUMMARY.md
- FOUND commit 7f16401 (feat: panic hook + port-bind signaling)
- FOUND commit 1410295 (test: startup-verify.sh)
