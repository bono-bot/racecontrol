---
gsd_state_version: 1.0
milestone: v5.0
milestone_name: RC Bot Expansion
status: ready_to_plan
stopped_at: Completed 23-01-PLAN.md — PodFailureReason enum and 5 AgentMessage variants committed, full test suite green
last_updated: "2026-03-16T10:37:59.437Z"
last_activity: 2026-03-16 — v5.0 roadmap written (Phases 23-26, 19 requirements, 100% coverage)
progress:
  total_phases: 4
  completed_phases: 1
  total_plans: 2
  completed_plans: 2
  percent: 90
---

---
gsd_state_version: 1.0
milestone: v5.0
milestone_name: RC Bot Expansion
status: ready_to_plan
stopped_at: ~
last_updated: "2026-03-16T00:00:00.000Z"
last_activity: 2026-03-16 — v5.0 roadmap created. Phases 23-26 defined, 19/19 requirements mapped.
progress:
  [█████████░] 90%
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-16)

**Core value:** The auto-fix bot handles every common failure class autonomously — staff only intervene for hardware replacement and physical reboots.
**Current focus:** v5.0 RC Bot Expansion — Phase 23 ready to plan.

## Current Position

Phase: 23 of 26 (Protocol Contract + Concurrency Safety)
Plan: — (not started)
Status: Ready to plan Phase 23
Last activity: 2026-03-16 — v5.0 roadmap written (Phases 23-26, 19 requirements, 100% coverage)

Progress: [░░░░░░░░░░] 0%

## Performance Metrics

**Velocity:**
- Total plans completed: 12
- Average duration: ~7 min
- Total execution time: ~80 min

**By Phase:**

| Phase | Duration | Tasks | Files |
|-------|----------|-------|-------|
| 16-firewall-auto-config P01 | ~4 min | 4 tasks | 2 files |
| 17-websocket-exec P01 | 3 min | 2 tasks | 1 file |
| 17-websocket-exec P03 | 9 min | 3 tasks | 3 files |
| 18-startup-self-healing P01 | 7 min | 2 tasks | 3 files |
| 18-startup-self-healing P02 | 6 min | 2 tasks | 3 files |
| 19-watchdog-service P01 | 10 min | 1 task | 8 files |
| 19-watchdog-service P02 | 9 min | 2 tasks | 2 files |
| 20-deploy-resilience P01 | 12 min | 2 tasks | 2 files |
| 20-deploy-resilience P02 | 4 min | 2 tasks | 3 files |
| 21-fleet-health-dashboard P01 | 6 min | 2 tasks | 6 files |
| 21-fleet-health-dashboard P02 | 5 min | 1 task | 3 files |
| 22-pod-recovery P01 | 12 min | 3 tasks | 3 files |
| Phase 22-pod-6-7-8-recovery-and-remote-restart-reliability P02 | 95 | 1 tasks | 1 files |
| Phase 23-protocol-contract-concurrency-safety P02 | 10 | 1 tasks | 1 files |
| Phase 23-protocol-contract-concurrency-safety P01 | 15 | 2 tasks | 3 files |

## Accumulated Context

### Decisions

- Build order for v5.0 is non-negotiable: rc-common first (Phase 23) — cross-crate compile dependency
- All bot fix functions must gate on billing_active inside the fix itself — pattern memory replay bypasses call-site guards
- billing.rs characterization tests required before any billing bot code (BILL-01 is a prerequisite gate, not a deliverable)
- Wallet sync fence required before recover_stuck_session() ships — CRDT MAX(updated_at) race documented in CONCERNS.md P1
- Multiplayer scope: detection + safe teardown only — auto-rejoin deferred (no AC session token path exists)
- Lap filter: game-reported isValidLap is authoritative; bot analysis sets review_required flag only, never hard-deletes
- PIN counters: strict type separation — customer and staff counters never share state
- [Phase 22]: RCAGENT_SELF_RESTART sentinel: direct Rust call to relaunch_self() bypasses cmd.exe/batch
- [Phase 22-pod-6-7-8-recovery-and-remote-restart-reliability]: deploy_pod.py server-exec fallback: probe :8090 first, use racecontrol server WS proxy when blocked by GPO domain firewall policy
- [Phase 22-pod-6-7-8-recovery-and-remote-restart-reliability]: PowerShell -EncodedCommand (UTF-16LE base64) for all WS-exec-proxied file writes: cmd.exe strips dollar-signs in -Command mode
- [Phase 22-pod-6-7-8-recovery-and-remote-restart-reliability]: Rename-then-copy binary swap: Windows allows ren while running but not overwrite; rename old.exe, copy new.exe into vacated name, kill old PID, start new
- [Phase 23-protocol-contract-concurrency-safety]: is_pod_in_recovery() lives in racecontrol (not rc-common) — WatchdogState is server-local; RecoveryFailed returns false so bots may still attempt fixes after watchdog gives up
- [Phase 23-protocol-contract-concurrency-safety]: PodFailureReason derives Copy not Hash — not needed as HashMap key in Phase 23, Hash deferred to Phase 24 if needed
- [Phase 23-protocol-contract-concurrency-safety]: AgentMessage bot variants committed atomically with ws/mod.rs stub arms — adding variants to enum immediately breaks match exhaustiveness; both files must change together

### Roadmap Evolution

- Phase 22 added: Pod 6/7/8 Recovery and Remote Restart Reliability
- Phases 23-26 added: v5.0 RC Bot Expansion roadmap (2026-03-16)

### Pending Todos

- Pod 3 still not verified running after fix-pod.bat — needs physical reboot + verification
- Version string inconsistency: USB-installed pods report v0.1.0, HTTP-deployed report v0.5.2
- Phase 22 plan 22-02 still pending: build release binary + fleet deploy
- Wallet sync fence mechanism decision needed before Phase 25 coding begins

### Blockers/Concerns

- Phase 22 incomplete: 22-02 (fleet deploy + verification) not yet executed
- Phase 25 pre-condition: wallet sync fence mechanism must be decided before recover_stuck_session() is implemented — options: (a) timestamp skew, (b) venue_authoritative flag, (c) transaction log migration

## Session Continuity

Last session: 2026-03-16T10:17:10.253Z
Stopped at: Completed 23-01-PLAN.md — PodFailureReason enum and 5 AgentMessage variants committed, full test suite green
Resume file: None
