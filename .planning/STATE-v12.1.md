# Project State: v12.1 E2E Process Guard

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-21)

**Core value:** No stale or unauthorized processes survive on any Racing Point machine — whitelist-enforced, continuously monitored, auto-killed.
**Current focus:** Phase 101 — Protocol Foundation

## Current Position

Phase: 101 of 105 (Protocol Foundation)
Plan: 0 of TBD in current phase
Status: Ready to plan
Last activity: 2026-03-21 — Roadmap created, 25 requirements mapped to 5 phases

Progress: [░░░░░░░░░░] 0%

## Performance Metrics

**Velocity:**
- Total plans completed: 0
- Average duration: -
- Total execution time: 0 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| - | - | - | - |

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

### Pending Todos

- Phase 103 pre-work: run sysinfo inventory dump on all 8 pods to capture full legitimate process set before enabling enforcement
- Phase 105 pre-work: confirm scheduled task names for venue tasks (Kiosk, WebDashboard) on server .23
- Phase 105 pre-work: verify James whitelist covers Ollama, node, python, comms-link, VS Code, cargo, deploy tooling

### Blockers/Concerns

None yet.

## Session Continuity

Last session: 2026-03-21
Stopped at: Roadmap written — ready to run /gsd:plan-phase 101
Resume file: None
