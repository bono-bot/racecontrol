---
phase: 187-self-monitor-coordination
plan: 01
subsystem: infra
tags: [rust, rc-agent, self_monitor, tcp, sentry, watchdog, process-restart]

# Dependency graph
requires:
  - phase: 184-rc-sentry-session1-spawn
    provides: "rc-sentry Session 1 spawn path — self_monitor now yields to sentry when alive"
provides:
  - "Sentry-aware relaunch in self_monitor.rs: TCP :8091 check, GRACEFUL_RELAUNCH sentinel + clean exit when sentry alive"
  - "PowerShell fallback preserved for sentry-dead case"
  - "4 new tests for check_sentry_alive: port constant, true/false/timeout"
affects: [rc-agent-deploy, pod-restart-behavior, orphan-powershell-mitigation]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "TCP liveness check before process exit: TcpStream::connect_timeout to avoid PowerShell spawn"
    - "Testable inner helper: check_sentry_alive_on_port(port) injected by tests with ephemeral port"

key-files:
  created: []
  modified:
    - "crates/rc-agent/src/self_monitor.rs"

key-decisions:
  - "187-01: check_sentry_alive uses blocking std::net (not tokio) — runs right before process::exit, async overhead not justified"
  - "187-01: Extracted check_sentry_alive_on_port(port) helper so tests can inject ephemeral port without touching SENTRY_PORT constant"
  - "187-01: Both paths write GRACEFUL_RELAUNCH sentinel — sentry still skips escalation if it comes back before the restart completes"
  - "187-01: PowerShell path kept exactly as-is — proven working, only invoked when sentry dead"

patterns-established:
  - "TCP liveness check before relaunch: prefer yielding to supervisor over self-spawning"

requirements-completed: [SELF-01, SELF-02]

# Metrics
duration: 5min
completed: 2026-03-25
---

# Phase 187 Plan 01: Self-Monitor Sentry Coordination Summary

**rc-agent self_monitor checks TCP :8091 before relaunch — yields to rc-sentry (zero PowerShell) when sentry alive, falls back to PowerShell only when sentry is dead**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-25T02:13:12Z (03:43 IST)
- **Completed:** 2026-03-25T02:48:00Z (03:48 IST)
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- Added `check_sentry_alive()` with 2s TCP timeout to localhost:8091 — eliminates ~90MB PowerShell leak per restart on all 8 pods when rc-sentry is running
- Refactored `relaunch_self()` to branch: sentry alive = write sentinel + `process::exit(0)` (no PowerShell spawned); sentry dead = existing PowerShell+DETACHED_PROCESS fallback
- Both paths write GRACEFUL_RELAUNCH sentinel so sentry won't count either as an escalation crash
- 4 new TDD tests pass: SENTRY_PORT=8091 constant, TCP alive/dead/timeout scenarios
- Release binary compiled: 11.7MB, all 11 self_monitor tests pass

## Task Commits

1. **Task 1: Add sentry-aware relaunch logic** - `5dcbfb2b` (feat) — TDD RED+GREEN in single commit (tests + implementation)
2. **Task 2: Build rc-agent and verify compilation** - No separate commit needed (no file changes; build.rs touch is metadata-only)

## Files Created/Modified
- `crates/rc-agent/src/self_monitor.rs` - Added SENTRY_PORT, SENTRY_CHECK_TIMEOUT, check_sentry_alive(), check_sentry_alive_on_port(), refactored relaunch_self() with sentry branch, 4 new tests

## Decisions Made
- Used blocking `std::net::TcpStream::connect_timeout` (not tokio async) — runs immediately before `process::exit`, no async runtime needed and simpler code
- Extracted `check_sentry_alive_on_port(port)` inner helper for testability — lets tests inject an ephemeral TcpListener port without changing SENTRY_PORT constant
- Kept `check_sentry_alive()` as the public-facing wrapper that hard-codes SENTRY_PORT — clean separation of concerns

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- `cargo test -p rc-agent` uses the actual package name `rc-agent-crate` (not `rc-agent`) in the workspace — discovered at TDD RED phase, resolved immediately.
- Pre-existing racecontrol test failure: `crypto::encryption::tests::load_keys_valid_hex` (469/470 pass) — environment-dependent crypto test, unrelated to this plan, not fixed per out-of-scope rule.

## Next Phase Readiness
- rc-agent binary at commit `5dcbfb2b` is ready for Pod 8 canary deploy
- Deploy via existing RCAGENT_SELF_RESTART sentinel flow — user decision when to deploy
- Phase 188+ can build on this: sentry-aware restart eliminates orphan PowerShell accumulation over time

---
*Phase: 187-self-monitor-coordination*
*Completed: 2026-03-25*
