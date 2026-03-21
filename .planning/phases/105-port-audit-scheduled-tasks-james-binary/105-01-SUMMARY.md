---
phase: 105-port-audit-scheduled-tasks-james-binary
plan: 01
subsystem: infra
tags: [rust, tokio, spawn-blocking, netstat, schtasks, process-guard, port-audit, windows]

# Dependency graph
requires:
  - phase: 103-pod-guard-module
    provides: process_guard::spawn(), kill_process_verified(), log_guard_event(), is_autostart_whitelisted() patterns
  - phase: 101-protocol-foundation
    provides: AgentMessage::ProcessViolation, ViolationType::Port/AutoStart in rc-common
  - phase: 102-whitelist-schema-config-fetch-endpoint
    provides: MachineWhitelist.ports and MachineWhitelist.autostart_keys fields
provides:
  - parse_netstat_listening: TCP LISTENING stdout parser (IPv4 + IPv6), returns Vec<(u16, u32)>
  - run_port_audit: netstat -ano shell-out, whitelist.ports enforcement, kill_process_verified + taskkill fallback
  - parse_schtasks_csv: quoted CSV parser for schtasks /query output, returns Vec<(path, name)>
  - run_schtasks_audit: schtasks /query /fo CSV /nh shell-out, Microsoft task skip, autostart_keys whitelist, disable or flag violations
  - All three audits wired into audit_interval.tick() arm: run_autostart_audit + run_port_audit + run_schtasks_audit
affects:
  - Phase 106+ (any future hardening that adds port/task violation handling)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - netstat -ano stdout parsing: split_whitespace, column positional check (TCP=col0, LISTENING=col3, port=col1 rfind(':'), pid=col4)
    - schtasks CSV parsing: split on '","' boundary, strip leading/trailing quotes from first/last fields
    - Port kill: kill_process_verified with sysinfo start_time; direct taskkill /F /PID fallback when sysinfo can't find PID
    - Schtask disable: schtasks /change /tn {path} /disable with CREATE_NO_WINDOW in spawn_blocking
    - System task skip: unconditional path.starts_with("\\Microsoft\\") guard before whitelist check

key-files:
  created: []
  modified:
    - crates/rc-agent/src/process_guard.rs

key-decisions:
  - "parse_netstat_listening uses rfind(':') not split(':') — handles IPv6 addresses like [::]:8090 correctly"
  - "Port kill uses kill_process_verified (sysinfo start_time) as primary path; taskkill /F /PID as fallback when PID not in sysinfo snapshot"
  - "schtasks system task skip is unconditional (starts_with \\Microsoft\\) — never flag Windows built-in tasks regardless of whitelist"
  - "run_schtasks_audit reuses ViolationType::AutoStart (not a new Port-like variant) — plan specified this and autostart_keys are shared"
  - "parse_schtasks_csv splits on '\",' boundary (not ',' alone) — handles quoted CSV without pulling in a CSV library"

patterns-established:
  - "Netstat parse pattern: split_whitespace -> positional checks (col 0/3/1/4) -> rfind(':') for port -> continue on parse fail"
  - "Schtasks parse pattern: split on '\",' boundary -> strip edge quotes -> skip header/empty/short lines"

requirements-completed: [PORT-01, PORT-02, AUTO-03]

# Metrics
duration: 38min
completed: 2026-03-21
---

# Phase 105 Plan 01: Port Audit and Scheduled Task Audit Summary

**netstat -ano port audit (kill_process_verified + taskkill fallback) and schtasks CSV scheduled-task audit (\\Microsoft\\ skip, disable action) wired into rc-agent 5-minute audit cycle — 11 new TDD unit tests, 28 tests green**

## Performance

- **Duration:** 38 min
- **Started:** 2026-03-21T12:05:00Z
- **Completed:** 2026-03-21T12:43:00Z
- **Tasks:** 2
- **Files modified:** 1 (process_guard.rs extended)

## Accomplishments

- Added `parse_netstat_listening` — handles IPv4 (0.0.0.0:port) and IPv6 ([::]:port) address formats, skips UDP, non-LISTENING, and malformed lines
- Added `run_port_audit` — netstat -ano shell-out, whitelist.ports enforcement, PID-identity-verified kill with direct taskkill fallback, ViolationType::Port violations over guard_violation_tx
- Added `parse_schtasks_csv` — split on `","` boundary approach for quoted CSV fields, header skip, empty/short-line skip
- Added `run_schtasks_audit` — schtasks /query /fo CSV /nh shell-out, unconditional `\\Microsoft\\` path skip, autostart_keys whitelist check, disables or flags violations (ViolationType::AutoStart)
- All three audits (`run_autostart_audit`, `run_port_audit`, `run_schtasks_audit`) wired into `audit_interval.tick()` arm
- 11 new unit tests (6 netstat, 5 schtasks): all green; total 28 tests green; `cargo build --release --bin rc-agent` zero errors

## Task Commits

Each task was committed atomically:

1. **Task 1: Port audit — run_port_audit() with netstat parsing and kill** - `8663a45` (feat, TDD)
2. **Task 2: Scheduled task audit — run_schtasks_audit() with schtasks CSV parsing** - `53f4551` (feat, TDD)

**Plan metadata:** (docs commit follows)

_Note: Both tasks used TDD. RED phase confirmed compile errors for missing functions. GREEN implemented and verified with 28 passing tests._

## Files Created/Modified

- `crates/rc-agent/src/process_guard.rs` - Added `parse_netstat_listening`, `run_port_audit`, `parse_schtasks_csv`, `run_schtasks_audit`; wired all three into `audit_interval.tick()` arm; 11 new unit tests

## Decisions Made

- `parse_netstat_listening` uses `rfind(':')` instead of `split(':')` to correctly handle IPv6 addresses like `[::]:8090` where the port follows the last colon.
- Port kill path: try `kill_process_verified` (with sysinfo start_time for PID identity) as primary; if sysinfo can't locate the PID, fall back to direct `taskkill /F /PID` in spawn_blocking. This handles short-lived processes or timing gaps.
- `\\Microsoft\\` system task skip is unconditional — guards are placed before the whitelist check to ensure Windows built-in tasks are never flagged regardless of whitelist state.
- `parse_schtasks_csv` splits on `","` (literal comma-quote) boundary rather than using a CSV library, consistent with the plan's "simple approach" directive and avoiding new dependencies.
- `run_schtasks_audit` reuses `ViolationType::AutoStart` (not a new Port-like variant) because the plan specified this explicitly and `autostart_keys` is the shared whitelist field for both Run keys and scheduled tasks.

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None — no external service configuration required. Pods will pick up port and schtask enforcement on next rc-agent deployment and whitelist fetch from racecontrol.

## Next Phase Readiness

- Phase 105 Plan 01 closes PORT-01, PORT-02, AUTO-03
- Port violations are emitted as `ViolationType::Port` over `guard_violation_tx` — server-side badge (Phase 104) already handles all `ProcessViolation` messages generically
- Schtask violations emit as `ViolationType::AutoStart` — same handling path as Run-key autostart violations
- No new crates, no schema changes — drop-in deployment: build rc-agent, deploy to pods
- Whitelist pre-work (Phase 105 pre-work from STATE.md): confirm schtask names for venue tasks and verify James whitelist coverage before enabling `kill_and_report` mode

---
*Phase: 105-port-audit-scheduled-tasks-james-binary*
*Completed: 2026-03-21*
