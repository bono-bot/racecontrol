---
phase: 105-port-audit-scheduled-tasks-james-binary
plan: "03"
subsystem: james-binary
tags: [rust, standalone-binary, http-post, process-guard, james, sysinfo, reqwest, walkdir, static-crt]

# Dependency graph
requires:
  - phase: 105-02
    provides: POST /api/v1/guard/report endpoint and X-Guard-Token auth
  - phase: 103-pod-guard-module
    provides: sysinfo 0.33 patterns, scan cycle logic, autostart audit patterns
  - phase: 101-protocol-foundation
    provides: MachineWhitelist, ProcessViolation, ViolationType from rc-common
dependency_graph:
  requires: [105-02, 103-02, rc-common types]
  provides: [rc-process-guard standalone binary, DEPLOY-03]
  affects: [James workstation process enforcement, /fleet/health james violations]

# Tech tracking
tech-stack:
  added:
    - crates/rc-process-guard (new standalone binary crate)
    - sysinfo 0.33 (crate-local dep — same version as rc-agent)
    - reqwest 0.12 with json feature (HTTP client for violation POST + whitelist GET)
    - walkdir 2 (Startup folder scan)
    - winapi 0.3 (CREATE_NO_WINDOW process flag on Windows)
  patterns:
    - "Whitelist fetch: try Tailscale URL first, then LAN, 5-retry 5s backoff, fallback to report_only default"
    - "Process scan: sysinfo 0.33 System::new() + refresh_processes(All, true) + .iter().filter() — NO direct .filter() (compile error)"
    - "Kill: kill_process_verified_james with PID+start_time identity check before taskkill /F /PID"
    - "HTTP POST violation: reqwest::Client + X-Guard-Token header, 401 logged as warn"
    - "Autostart: reg shell-out for Run keys, walkdir for Startup folders (no winreg dep)"
    - "Port audit: netstat -ano + rfind(':') for IPv6, fallback direct taskkill when sysinfo can't find PID"
    - "Schtask audit: schtasks /query CSV + unconditional \\Microsoft\\ skip + disable action"
    - "Log rotation: 512KB truncate at C:\\Users\\bono\\racingpoint\\process-guard-james.log"
    - "60s startup amnesty — same as rc-agent, allows Windows Update / transient processes to settle"
    - "Whitelist refresh: every 5 minutes via separate interval (same tick as audit)"

key-files:
  created:
    - crates/rc-process-guard/Cargo.toml
    - crates/rc-process-guard/src/main.rs
  modified:
    - Cargo.toml (workspace members)

key-decisions:
  - "winreg not added as dep — winreg 0.52 not in workspace; used reg shell-out (same as rc-agent) for registry audit"
  - "parse_netstat_listening_james uses rfind(':') — handles IPv6 [::]:port format (inherited from 105-01 pattern)"
  - "JAMES_CRITICAL_BINARIES = [rc-agent.exe, kiosk.exe] — standing rule #2; zero grace kills on James"
  - "HTTP POST (never WebSocket) — standing rule #2 compliance; binary is standalone not a ws client"
  - "whitelist_refresh and audit_interval both at 300s but separate tokio::time::interval — clean separation"
  - "fetch_whitelist returns MachineWhitelist::default() on total failure — report_only mode is safe default"

requirements-completed: [DEPLOY-03]

# Metrics
duration: 35 min
completed: 2026-03-21
tasks_completed: 2
files_modified: 3
---

# Phase 105 Plan 03: rc-process-guard Standalone Binary Summary

**One-liner:** Standalone rc-process-guard.exe binary for James workstation — whitelist fetch, process scan, port audit, autostart audit, schtask audit, violations POSTed via HTTP with X-Guard-Token; never WebSocket.

## Performance

- **Duration:** ~35 min
- **Started:** 2026-03-21T11:41:41Z (IST 17:11)
- **Completed:** 2026-03-21 (IST ~17:46)
- **Tasks:** 2
- **Files created:** 2 new (Cargo.toml + main.rs), 1 modified (workspace Cargo.toml)

## Accomplishments

- Created `crates/rc-process-guard/` standalone binary crate with full Cargo.toml (sysinfo 0.33, reqwest 0.12, walkdir 2, winapi 0.3, rc-common workspace dep)
- Added `crates/rc-process-guard` to workspace members in root Cargo.toml
- Implemented `main.rs` (486 lines of impl + 70 lines of tests):
  - `fetch_whitelist_with_retry`: GET /api/v1/guard/whitelist/james, 5-retry 5s backoff, Tailscale-first, fallback to `MachineWhitelist::default()` (report_only safe)
  - `run_scan_cycle`: sysinfo 0.33 API, own-PID exclusion, JAMES_CRITICAL_BINARIES zero grace, two-cycle grace for others, kills via `kill_process_verified_james`
  - `post_violation`: HTTP POST to /api/v1/guard/report with X-Guard-Token header, 401 warned, connection errors logged locally
  - `log_james_event`: 512KB rotation to `C:\Users\bono\racingpoint\process-guard-james.log`
  - `run_autostart_audit_james`: HKCU/HKLM Run keys (reg shell-out), per-user + all-users Startup folders (walkdir)
  - `run_port_audit_james`: netstat -ano + rfind(':') IPv6 handling, kill_process_verified_james + direct taskkill fallback
  - `run_schtasks_audit_james`: schtasks /query CSV parse, unconditional `\\Microsoft\\` skip, disable action in kill_and_report mode
  - `is_james_self_excluded`, `is_james_critical`, `is_process_whitelisted`, `parse_netstat_listening_james`, `parse_schtasks_csv_james`
- 10 unit tests — all green
- `cargo build --release --bin rc-process-guard` — zero errors, 4.0MB binary with static CRT

## Task Commits

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 (scaffold) | Create crate scaffold + workspace member | 50d144e | Cargo.toml, crates/rc-process-guard/Cargo.toml, src/main.rs (stub) |
| 2 (TDD RED) | Failing tests for helper functions | 01f44eb | crates/rc-process-guard/src/main.rs |
| 2 (TDD GREEN) | Full implementation — 10 tests pass | e83b33e | crates/rc-process-guard/src/main.rs |

## Verification Results

- `cargo test -p rc-process-guard` — 10 passed, 0 failed
- `cargo build --release --bin rc-process-guard` — zero errors
- Binary size: 4.0MB (`target/release/rc-process-guard.exe`)
- `grep "post_violation\|fetch_whitelist\|run_scan_cycle" src/main.rs` — all three present (11 matches)
- `grep "rc-agent.exe\|kiosk.exe" src/main.rs` — JAMES_CRITICAL_BINARIES defined (7 matches)
- Workspace Cargo.toml includes `crates/rc-process-guard`
- Pre-existing racecontrol test failures (config_fallback_preserved, crypto encryption) — not caused by this plan

## Decisions Made

- `winreg` dep not added — not in workspace; used `reg` shell-out consistent with rc-agent (plan note honored)
- HTTP POST only — never WebSocket — standing rule #2 compliance
- `JAMES_CRITICAL_BINARIES = ["rc-agent.exe", "kiosk.exe"]` — zero grace kills on James
- `fetch_whitelist_with_retry` falls back to `MachineWhitelist::default()` on total failure — safe report_only default, never panics
- `whitelist_refresh` (5min) and `audit_interval` (5min) are separate `tokio::time::interval` instances — conceptually distinct tasks

## Deviations from Plan

None — plan executed exactly as written.

The only decision point was the `winreg` dep: plan explicitly stated to use reg shell-out if winreg not already in workspace. winreg was not present. Used reg shell-out as specified.

## User Setup Required

Before deploying to James .27:

1. Create `C:\Users\bono\racingpoint\rc-process-guard.toml`:
```toml
server_url = "http://192.168.31.23:8080"
tailscale_url = "http://100.71.226.83:8080"
report_secret = "rp-guard-2026"
scan_interval_secs = 60
machine_id = "james"
log_file = "C:\\Users\\bono\\racingpoint\\process-guard-james.log"
```

2. Copy `target/release/rc-process-guard.exe` to `C:\Users\bono\racingpoint\`

3. Add to HKLM Run key or Task Scheduler for autostart (use `start-rc-process-guard.bat`)

4. Verify James whitelist covers: Ollama, node, python, VS Code, cargo, comms-link (configured in `C:\RacingPoint\racecontrol.toml` under `[process_guard.overrides.james]` on server .23 — not hardcoded in binary)

## Next Phase Readiness

- DEPLOY-03 complete — James .27 can now be protected by the same process whitelist framework as pods
- rc-process-guard.exe reports to POST /api/v1/guard/report (105-02) — violations visible in /fleet/health under `pod_violations["james"]`
- No new crates or schema changes in rc-agent or racecontrol — pods unaffected
- Phase 105 complete (all 3 active plans done: 01 port+schtask audit in rc-agent, 02 HTTP intake endpoint, 03 James standalone binary)

## Self-Check: PASSED

- FOUND: crates/rc-process-guard/Cargo.toml
- FOUND: crates/rc-process-guard/src/main.rs
- FOUND: .planning/phases/105-port-audit-scheduled-tasks-james-binary/105-03-SUMMARY.md
- FOUND commit: 50d144e (Task 1 scaffold)
- FOUND commit: 01f44eb (Task 2 TDD RED)
- FOUND commit: e83b33e (Task 2 TDD GREEN)

---
*Phase: 105-port-audit-scheduled-tasks-james-binary*
*Completed: 2026-03-21*
