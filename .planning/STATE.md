---
gsd_state_version: 1.0
milestone: v4.0
milestone_name: Pod Fleet Self-Healing
status: active
stopped_at: "Defining requirements"
last_updated: "2026-03-15"
last_activity: 2026-03-15 — Milestone v4.0 started. Researching domain before requirements.
progress:
  total_phases: 0
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
  percent: 0
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-15)

**Core value:** Every pod survives any failure without physical intervention. Pods self-heal and remain remotely manageable at all times.
**Current focus:** Defining requirements for v4.0 Pod Fleet Self-Healing

## Current Position

Phase: Not started (defining requirements)
Plan: —
Status: Defining requirements
Last activity: 2026-03-15 — Milestone v4.0 started

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

## Accumulated Context

### Decisions

- v4.0 scope: Pod fleet self-healing — Windows Service, WebSocket exec, firewall auto-config, startup reporting, config self-heal, deploy resilience, health dashboard
- v3.0 paused: Phases 14 (Events) and 15 (Telemetry) deferred until fleet is bulletproof
- Pod-agent merged into rc-agent (v3.0 Phase 13.1, commit eea644e) — single binary per pod
- CRLF bug in batch files caused silent firewall rule failures on Pods 1/3/4 — batch file dependency must be eliminated
- HKLM Run key insufficient — only runs at login, no crash restart; needs Windows Service
- WebSocket protocol lacks exec capability — CoreToAgentMessage has no shell/exec variant; pods unreachable when HTTP firewall blocked
- Server (.23) also cannot reach Pod 3 when firewall blocks inbound — not just James (.27)
- 2-minute scheduled task watchdog created as interim fix for Pod 3

### Pending Todos

- Pod 3 still not verified running after fix-pod.bat — needs physical reboot + verification
- fix-firewall.bat needs to be run on Pod 1 (ICMP blocked)
- Version string inconsistency: USB-installed pods report v0.1.0, HTTP-deployed report v0.5.2

### Roadmap Evolution

- v4.0 created to address all issues from Mar 15 debugging session (4 hours, Pods 1/3/4 offline)

### Blockers/Concerns

- [Pre-Phase 14/15] v3.0 Phases 14 and 15 paused until v4.0 completes
- [Pre-Phase 15] Driver rating class boundaries still need Uday sign-off
- NSSM vs native Windows Service API — needs research to determine best approach

## Session Continuity

Last session: 2026-03-15T06:35:00Z
Stopped at: Researching domain before defining requirements
Resume file: None
