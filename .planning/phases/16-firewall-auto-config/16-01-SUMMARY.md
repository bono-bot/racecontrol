---
phase: 16-firewall-auto-config
plan: 01
subsystem: infra
tags: [rust, netsh, windows-firewall, std-process-command, rc-agent]

requires: []
provides:
  - "firewall.rs module with configure() that applies ICMP + TCP 8090 rules via netsh on every startup"
  - "Delete-then-add idempotency pattern for named Windows Firewall rules"
  - "FirewallResult enum (Configured / Failed) for testable non-fatal firewall logic"
  - "7 unit tests for firewall arg-building helpers (no admin required)"
affects: [17-watchdog-service, 18-rollback-deploy, 19-watchdog-service-phase2, 21-fleet-dashboard]

tech-stack:
  added: []
  patterns:
    - "DELETE-THEN-ADD for idempotent Windows Firewall rule management (delete-first is always safe — exit 0 even if absent)"
    - "Separate arg-building helpers (build_icmp_args, build_tcp_args, build_delete_args) from subprocess execution so unit tests can verify without admin"
    - "#[cfg(windows)] guard on both CommandExt import and .creation_flags() call — matches remote_ops.rs pattern"

key-files:
  created:
    - crates/rc-agent/src/firewall.rs
  modified:
    - crates/rc-agent/src/main.rs

key-decisions:
  - "Synchronous configure() called before tokio event loop — a 100-200ms startup cost is acceptable vs. complexity of async netsh"
  - "Returns Failed(msg) on netsh error instead of panicking — agent must survive firewall config failure"
  - "Rule names namespaced RacingPoint-ICMP and RacingPoint-RemoteOps — avoids collision with old batch rules (AllowICMP, RCAgent) which coexist harmlessly"
  - "Old batch file rules not cleaned up — they are additive safety net; Rust code only manages rules it owns"

patterns-established:
  - "Firewall pattern: firewall::configure() -> remote_ops::start(port) — ensures rules exist before HTTP server binds"
  - "Non-fatal infra config: match result, log, continue — never crash on optional startup configuration"

requirements-completed: [FW-01, FW-02, FW-03]

duration: 4min
completed: 2026-03-15
---

# Phase 16 Plan 01: Firewall Auto-Config Summary

**Rust firewall module using netsh advfirewall with delete-then-add idempotency — ICMP echo and TCP 8090 rules applied on every rc-agent startup, eliminating CRLF-damaged batch file failures permanently**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-15T07:41:33Z
- **Completed:** 2026-03-15T07:45:52Z
- **Tasks:** 2 of 2
- **Files modified:** 2

## Accomplishments

- Created `firewall.rs` with `configure()`, `FirewallResult` enum, `run_netsh()`, and testable arg-building helpers
- 7 unit tests verify rule name namespacing, distinctness, enum inequality, and all required netsh args (icmpv4:8,any, TCP/8090, profile=any, dir=in, action=allow)
- Wired `firewall::configure()` into `main.rs` before `remote_ops::start(8090)` — FW-03 call order guaranteed
- Full test suite green: 93 rc-common + 184 rc-agent + 254 rc-core = 531 tests, 0 failures
- Release build compiles without errors, all warnings are pre-existing (none introduced by new code)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create firewall.rs module with configure() and unit tests** - `531bf99` (feat)
2. **Task 2: Wire firewall::configure() into main.rs startup sequence before remote_ops::start** - `76a28b7` (feat)

**Plan metadata:** (this SUMMARY commit)

_Note: Task 1 used TDD — module + tests written together, then mod declaration added to trigger GREEN run_

## Files Created/Modified

- `crates/rc-agent/src/firewall.rs` - New module: configure(), FirewallResult, run_netsh, build_icmp_args, build_tcp_args, build_delete_args, 7 unit tests
- `crates/rc-agent/src/main.rs` - Added `mod firewall;` declaration (alphabetically placed) and `match firewall::configure()` call before `remote_ops::start(8090)`

## Decisions Made

- Synchronous std::process::Command used instead of async tokio::process::Command — called before tokio event loop, 100-200ms startup cost is fine
- Non-fatal on failure — agent logs warning and continues even if netsh lacks admin privileges
- Rule names: `RacingPoint-ICMP` and `RacingPoint-RemoteOps` (not old `AllowICMP`/`RCAgent`) — namespaced to avoid future collision
- Old batch file rules left intact — they coexist as additive safety net until pods are physically reinstalled

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None. The research document was thorough and the implementation matched exactly what was specified. All test output file I/O issues were bash-level tool limitations, not code issues.

## User Setup Required

None - no external service configuration required. Pod 8 canary verification (steps 4-6 in the PLAN.md verification section) is a post-deploy manual check that happens after the binary is deployed to Pod 8.

## Next Phase Readiness

- Phase 16 Plan 01 complete — firewall module exists and is wired into startup
- Binary is ready to build and deploy to all 8 pods via the standard pod-deploy workflow
- Pod 8 canary should be verified first: `netsh advfirewall firewall show rule name=RacingPoint-ICMP` and `name=RacingPoint-RemoteOps` should show profile=Domain,Private,Public
- Phase 17 (watchdog service) can proceed — firewall rules will persist through reboots managed by the watchdog

## Self-Check: PASSED

- FOUND: crates/rc-agent/src/firewall.rs
- FOUND: .planning/phases/16-firewall-auto-config/16-01-SUMMARY.md
- FOUND commit 531bf99 (firewall.rs + mod declaration)
- FOUND commit 76a28b7 (main.rs wiring)
- All 531 tests green (rc-common: 93, rc-agent: 184, rc-core: 254)
- Release build compiles with 0 errors

---
*Phase: 16-firewall-auto-config*
*Completed: 2026-03-15*
