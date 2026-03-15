---
gsd_state_version: 1.0
milestone: v4.0
milestone_name: Pod Fleet Self-Healing
status: active
stopped_at: Completed 17-websocket-exec 17-01-PLAN.md
last_updated: "2026-03-15T08:20:13Z"
last_activity: 2026-03-15 — Completed Plan 17-01 (Protocol Extension). Exec/ExecResult variants + 5 serde tests.
progress:
  total_phases: 6
  completed_phases: 1
  total_plans: 4
  completed_plans: 2
  percent: 87
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-15)

**Core value:** Every pod survives any failure without physical intervention. Pods self-heal and remain remotely manageable at all times.
**Current focus:** Phase 17 — WebSocket Exec (Plan 01 complete, Plans 02-03 remaining)

## Current Position

Phase: 17 of 21 (WebSocket Exec)
Plan: 2 of 3
Status: Plan 01 complete, ready for Plan 02
Last activity: 2026-03-15 — Completed Plan 17-01 (Protocol Extension). Exec/ExecResult variants + 5 serde tests.

Progress: [████████░░] 87%

## Performance Metrics

**Velocity:**
- Total plans completed: 2
- Average duration: ~3 min
- Total execution time: ~7 min

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 16-firewall-auto-config P01 | 4 tasks | 2 files | ~4 min |
| 17-websocket-exec P01 | 2 tasks | 1 file | 3 min |

*Updated after each plan completion*

## Accumulated Context

### Decisions

- Watchdog pattern: rc-watchdog.exe as SYSTEM service wrapping start-rcagent.bat — NOT native ServiceMain (Session 0 GUI boundary), NOT NSSM (external dependency)
- Firewall: Move entirely to Rust (std::process::Command calling netsh) — eliminate CRLF-sensitive batch files permanently
- WebSocket exec: CoreToAgentMessage::Exec with request_id correlation; independent semaphore from HTTP exec slots
- Deploy rollback: Preserve rc-agent-prev.exe; auto-rollback on 60s health gate failure (automatic, not manual confirm)
- AV exclusion: Directory-wide C:\RacingPoint\ exclusion — simpler, avoids staging filename enumeration
- v3.0 Phases 14 and 15 paused until v4.0 completes
- Pod-agent merged into rc-agent (Phase 13.1, commit eea644e) — single binary per pod
- [Phase 16-firewall-auto-config]: Firewall Phase 16: synchronous std::process::Command for netsh, non-fatal on failure, RacingPoint-prefixed rule names, old batch rules left intact as additive safety net
- [Phase 17-websocket-exec P01]: Struct-style enum variants for Exec/ExecResult; serde default 10s timeout; request_id correlation pattern

### Pending Todos

- Pod 3 still not verified running after fix-pod.bat — needs physical reboot + verification
- fix-firewall.bat needs to be run on Pod 1 (ICMP blocked) — Phase 16 will fix permanently
- Version string inconsistency: USB-installed pods report v0.1.0, HTTP-deployed report v0.5.2

### Blockers/Concerns

- [Phase 19] Session 1 lock screen stays in rc-agent — confirmed. Watchdog wraps bat file, preserves Session 1.
- [Phase 21] Fleet dashboard depends on health data flowing from Phases 16–20. Do not start until Phase 19 is deployed.

## Session Continuity

Last session: 2026-03-15T08:20:13Z
Stopped at: Completed 17-websocket-exec 17-01-PLAN.md
Resume file: None
