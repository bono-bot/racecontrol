# Project State: v12.1 E2E Process Guard

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-21)

**Core value:** No stale or unauthorized processes survive on any Racing Point machine — whitelist-enforced, continuously monitored, auto-killed.
**Current focus:** Phase 105 — Port Scan Audit

## Current Position

Phase: 104 of 105 (Server Guard Module Alerts) — COMPLETE
Plan: 2 of 2 complete
Status: Phase 104 complete — server guard scan loop, CRITICAL rc-agent.exe detection, pod_violations["server"], log rotation
Last activity: 2026-03-21 — Phase 104 Plan 02 executed: spawn_server_guard() in process_guard.rs, wired into main.rs

Progress: [████████░░] 80%

## Performance Metrics

**Velocity:**
- Total plans completed: 6
- Average duration: 21 min
- Total execution time: 2.2 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 101-protocol-foundation | 1 | 35 min | 35 min |
| 102-whitelist-schema-config-fetch-endpoint | 2 | 55 min | 27 min |
| 103-pod-guard-module | 3 (of 3) | 47 min | 16 min |
| 104-server-guard-module-alerts | 2 (of 2) | 40 min | 20 min |

*Updated after each plan completion*

## Accumulated Context

### Incident Context

- Triggered by: manual audit missed Steam (HKCU Run), Leaderboard (HKLM Run), RaceControlVoice (Startup folder) on James's workstation
- Voice assistant watchdog.cmd was an infinite restart loop consuming resources
- Kiosk (Next.js) was running in both dev AND production mode on James — belongs on server .23 only
- Standing rule #2: NEVER run pod binaries on James's PC

### Decisions

- [Roadmap]: report-only mode default (`violation_action = "report_only"`) — whitelist tuning before kills; switch to `"kill_and_report"` after false-positive round on Pod 8
- [Roadmap]: James uses standalone `rc-process-guard.exe` reporting via HTTP — never WebSocket (standing rule #2)
- [Roadmap]: Two-cycle grace period before kill — prevents killing transient system processes (Windows Update, MpCmdRun)
- [Roadmap]: Self-exclusion unconditional — current process excluded before any whitelist lookup
- [Research]: Do NOT upgrade sysinfo past 0.33 — breaking API in 0.38 affects kiosk.rs, game_process.rs, self_test.rs
- [Research]: Two new crates — `netstat2 0.11` (Phase 105) and `walkdir 2` (Phase 103)
- [Research]: Do NOT add `windows = "0.58"` — conflicts with existing `winapi 0.3`
- [101-01]: Manual Default impl required for MachineWhitelist — serde `default =` functions not called by `#[derive(Default)]`
- [101-01]: Wildcard arm added to racecontrol ws/mod.rs AgentMessage match — process guard handling deferred to Phase 103/104
- [102-01]: racecontrol.toml outside git repo — created at C:/RacingPoint/ directly; not tracked in git
- [102-01]: Steam in pod deny_processes only (not global allowed) — enforces v12.1 trigger incident rule
- [102-01]: ollama.exe in both global allowed (machines=["pod"]) AND james allow_extra_processes — needed on both
- [102-01]: cargo package name is `racecontrol-crate` not `racecontrol` — use `-p racecontrol-crate` for test/build
- [102-02]: No guard_config field on AppState — handler reads state.config.process_guard directly, consistent with watchdog/bono/gmail pattern
- [102-02]: Route in public_routes() (no auth) — pods call this before any auth session exists on WS connect
- [103-01]: ProcessGuardConfig reuses existing default_true() fn — no second copy added (plan enforced)
- [103-01]: guard_violation channel capacity=32 — matches ws_exec_result channel pattern
- [103-01]: guard_whitelist initialized to MachineWhitelist::default() (report_only, empty lists) — safe no-op until WS fetch
- [103-02]: sysinfo 0.33 processes() returns &HashMap — must call .iter().filter() (not direct .filter())
- [103-02]: parent_pid sentinel=0 (sysinfo 0.33 has no parent PID API) — name exclusion is primary self-exclusion guard
- [103-02]: own_pid excluded inline in scan loop (pid == own_pid continue) not in is_self_excluded helper
- [103-02]: grace_counts.retain() after each cycle prevents unbounded HashMap growth over long uptime
- [103-03]: ProcessGuardConfig missing Clone derive — added #[derive(Clone)] (required for spawn() call which takes ownership)
- [103-03]: Whitelist fetch placed before AppState construction so config.core.url/config.pod.number accessible before config is moved
- [103-03]: audit_startup_folder flag-only even in kill_and_report mode — file removal requires Phase 104 staff approval
- [103-03]: #[cfg(windows)] use std::os::windows::process::CommandExt inside spawn_blocking closure — mirrors ws_handler.rs/debug_server.rs pattern
- [104-01]: ViolationStore never cleared on disconnect — violations persist across reconnects, only deliberate reset should erase history
- [104-01]: repeat_offender_check uses >= 2 prior kills in history (not >= 3) because current violation is pushed after the check
- [104-01]: pod_key uses registered_pod_id (underscore format) over machine_id (dash format) for consistent HashMap keying
- [104-01]: ProcessGuardStatus arm is log-only — storage deferred to future phase if needed
- [104-02]: sysinfo 0.33 actual API: System::new() + refresh_processes(ProcessesToUpdate::All, true) — NOT System::new_all() + refresh_processes()
- [104-02]: spawn_server_guard() wired in main.rs (not lib.rs) — start_probe_loop is in main.rs; process_guard added to use racecontrol_crate::{ ... } block
- [104-02]: Server guard self-excludes racecontrol.exe by name (own binary) + own PID inline; rc-agent.exe = SERVER_CRITICAL_BINARIES zero grace

### Pending Todos

- Phase 103 pre-work: run sysinfo inventory dump on all 8 pods to capture full legitimate process set before enabling enforcement
- Phase 105 pre-work: confirm scheduled task names for venue tasks (Kiosk, WebDashboard) on server .23
- Phase 105 pre-work: verify James whitelist covers Ollama, node, python, comms-link, VS Code, cargo, deploy tooling

### Blockers/Concerns

None yet.

## Session Continuity

Last session: 2026-03-21 17:02 IST
Stopped at: 104-02-PLAN.md complete — server guard scan loop, CRITICAL rc-agent.exe detection, violations in pod_violations["server"]
Resume file: None
