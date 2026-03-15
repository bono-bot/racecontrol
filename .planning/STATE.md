---
gsd_state_version: 1.0
milestone: v4.0
milestone_name: Pod Fleet Self-Healing
status: active
stopped_at: Completed 16-firewall-auto-config 16-01-PLAN.md
last_updated: "2026-03-15T07:51:42.255Z"
last_activity: 2026-03-15 — Roadmap created for v4.0. 6 phases, 21 requirements, 100% coverage.
progress:
  total_phases: 6
  completed_phases: 1
  total_plans: 1
  completed_plans: 1
  percent: 91
---

---
gsd_state_version: 1.0
milestone: v4.0
milestone_name: Pod Fleet Self-Healing
status: active
stopped_at: "Roadmap created — ready to plan Phase 16"
last_updated: "2026-03-15"
last_activity: 2026-03-15 — Roadmap created. 6 phases (16–21), 21 requirements mapped. Ready to plan Phase 16.
progress:
  [█████████░] 91%
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
  percent: 0
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-15)

**Core value:** Every pod survives any failure without physical intervention. Pods self-heal and remain remotely manageable at all times.
**Current focus:** Phase 16 — Firewall Auto-Config (ready to plan)

## Current Position

Phase: 16 of 21 (Firewall Auto-Config)
Plan: — (not yet planned)
Status: Ready to plan
Last activity: 2026-03-15 — Roadmap created for v4.0. 6 phases, 21 requirements, 100% coverage.

Progress: [░░░░░░░░░░] 0%

## Performance Metrics

**Velocity:**
- Total plans completed: 0
- Average duration: -
- Total execution time: -

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| TBD | - | - | - |

*Updated after each plan completion*
| Phase 16-firewall-auto-config P01 | 4 | 2 tasks | 2 files |

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

### Pending Todos

- Pod 3 still not verified running after fix-pod.bat — needs physical reboot + verification
- fix-firewall.bat needs to be run on Pod 1 (ICMP blocked) — Phase 16 will fix permanently
- Version string inconsistency: USB-installed pods report v0.1.0, HTTP-deployed report v0.5.2

### Blockers/Concerns

- [Phase 19] Session 1 lock screen stays in rc-agent — confirmed. Watchdog wraps bat file, preserves Session 1.
- [Phase 21] Fleet dashboard depends on health data flowing from Phases 16–20. Do not start until Phase 19 is deployed.

## Session Continuity

Last session: 2026-03-15T07:47:07.229Z
Stopped at: Completed 16-firewall-auto-config 16-01-PLAN.md
Resume file: None
