---
gsd_state_version: 1.0
milestone: v4.0
milestone_name: Pod Fleet Self-Healing
status: in_progress
stopped_at: Completed 22-01-PLAN.md
last_updated: "2026-03-16T08:31:28.181Z"
last_activity: 2026-03-16 — Phase 22 Plan 01 RCAGENT_SELF_RESTART sentinel complete.
progress:
  total_phases: 7
  completed_phases: 6
  total_plans: 14
  completed_plans: 13
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-15)

**Core value:** Every pod survives any failure without physical intervention. Pods self-heal and remain remotely manageable at all times.
**Current focus:** v4.0 COMPLETE. All 6 phases shipped. Ready for milestone completion.

## Current Position

Phase: 22 of 22 (Pod 6/7/8 Recovery and Remote Restart Reliability) — In Progress (1/2 plans done)
Plan: 22-01 complete. 22-02 pending.
Status: Phase 22 in progress. RCAGENT_SELF_RESTART sentinel shipped. Deploy binary to pods.
Last activity: 2026-03-16 — Phase 22 Plan 01 RCAGENT_SELF_RESTART sentinel complete.

Progress: [█████████░] 93% (13/14 plans)

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
| Phase 22-pod-6-7-8-recovery-and-remote-restart-reliability P01 | 12 | 3 tasks | 3 files |

## Accumulated Context

### Decisions

- Watchdog pattern: rc-watchdog.exe as SYSTEM service wrapping start-rcagent.bat — NOT native ServiceMain (Session 0 GUI boundary), NOT NSSM (external dependency)
- Firewall: Move entirely to Rust (std::process::Command calling netsh) — eliminate CRLF-sensitive batch files permanently
- WebSocket exec: CoreToAgentMessage::Exec with request_id correlation; independent semaphore from HTTP exec slots
- Deploy rollback: Preserve rc-agent-prev.exe; auto-rollback on 60s health gate failure (automatic, not manual confirm)
- AV exclusion: Directory-wide C:\RacingPoint\ exclusion — simpler, avoids staging filename enumeration
- v3.0 Phases 14 and 15 paused until v4.0 completes
- Pod-agent merged into rc-agent (Phase 13.1, commit eea644e) — single binary per pod
- [Phase 16]: synchronous std::process::Command for netsh, non-fatal on failure, RacingPoint-prefixed rule names
- [Phase 17 P01]: Struct-style enum variants for Exec/ExecResult; serde default 10s timeout; request_id correlation
- [Phase 17 P03]: Pod-prefixed request_id; HTTP-first WS-fallback exec; oneshot channel resolution
- [Phase 18 P01]: Synchronous self-heal before load_config; embedded config template; START_SCRIPT_CONTENT const with CRLF
- [Phase 18 P02]: StartupReport sent once per process lifetime; fire-and-forget; Register -> StartupReport -> ContentManifest ordering
- [Phase 19 P01]: rc-watchdog crate with windows-service 0.8; tasklist polling; 15s restart grace; WTSQueryUserToken for Session 1
- [Phase 19 P02]: Bare StatusCode::OK crash report; log_pod_activity source='watchdog'; sc.exe failure actions
- [Phase 20 P01]: RollingBack is active (prevents concurrent deploy); SWAP via /write endpoint; rollback success = Failed with reason
- [Phase 20 P02]: Defender check non-fatal; failed.drain() retry pattern avoids double-counting
- [Phase 21 P01]: Used futures_util::join_all (existing dep) for parallel probes; dedicated 3s timeout client; uptime_secs computed live from agent_started_at
- [Phase 21 P01]: fleet_health route is public (no auth) for Uday's LAN phone access; clear_on_disconnect preserves http_reachable
- [Phase 21 P02]: No auth on /fleet page; keep last known pod data on poll error; error shown as yellow banner, cards never blank
- [Phase 22-pod-6-7-8-recovery-and-remote-restart-reliability]: RCAGENT_SELF_RESTART sentinel: direct Rust call to relaunch_self() bypasses cmd.exe/batch, eliminating PowerShell interpretation issues on pods 6/7/8
- [Phase 22-pod-6-7-8-recovery-and-remote-restart-reliability]: deploy_pod.py connection-close-as-success: treats eof/reset/timeout as success since rc-agent exits before HTTP response completes

### Roadmap Evolution

- Phase 22 added: Pod 6/7/8 Recovery and Remote Restart Reliability

### Pending Todos

- Pod 3 still not verified running after fix-pod.bat — needs physical reboot + verification
- Version string inconsistency: USB-installed pods report v0.1.0, HTTP-deployed report v0.5.2
- Pod 8 canary verification for Phases 19+20 deferred until fleet deploy

### Blockers/Concerns

- Fleet deploy needed: all v4.0 code complete but needs release build + fleet rollout to verify end-to-end

## Session Continuity

Last session: 2026-03-16T08:31:15.165Z
Stopped at: Completed 22-01-PLAN.md
Resume file: None
