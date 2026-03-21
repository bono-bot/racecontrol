# Project State: v12.1 E2E Process Guard

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-21)

**Core value:** No stale or unauthorized processes survive on any Racing Point machine — whitelist-enforced, continuously monitored, auto-killed.
**Current focus:** Phase 102 — Whitelist Schema + Config + Fetch Endpoint

## Current Position

Phase: 102 of 105 (Whitelist Schema + Config + Fetch Endpoint)
Plan: 1 of TBD in current phase
Status: Plan 01 complete — ready for Plan 02 (HTTP endpoint)
Last activity: 2026-03-21 — Phase 102 Plan 01 executed: ProcessGuardConfig structs + racecontrol.toml whitelist

Progress: [██░░░░░░░░] 10%

## Performance Metrics

**Velocity:**
- Total plans completed: 2
- Average duration: 35 min
- Total execution time: 1.2 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 101-protocol-foundation | 1 | 35 min | 35 min |
| 102-whitelist-schema-config-fetch-endpoint | 1 | 35 min | 35 min |

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

### Pending Todos

- Phase 103 pre-work: run sysinfo inventory dump on all 8 pods to capture full legitimate process set before enabling enforcement
- Phase 105 pre-work: confirm scheduled task names for venue tasks (Kiosk, WebDashboard) on server .23
- Phase 105 pre-work: verify James whitelist covers Ollama, node, python, comms-link, VS Code, cargo, deploy tooling

### Blockers/Concerns

None yet.

## Session Continuity

Last session: 2026-03-21
Stopped at: 102-01-PLAN.md complete — ProcessGuardConfig structs + racecontrol.toml whitelist shipped
Resume file: None
