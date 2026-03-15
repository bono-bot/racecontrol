---
gsd_state_version: 1.0
milestone: v4.0
milestone_name: Pod Fleet Self-Healing
status: active
stopped_at: Completed 20-deploy-resilience 20-01-PLAN.md
last_updated: "2026-03-15T11:07:58.966Z"
last_activity: 2026-03-15 — Completed Plan 19-02 Task 1 (crash report endpoint + install script). Checkpoint pending for Pod 8 canary verification.
progress:
  total_phases: 6
  completed_phases: 4
  total_plans: 10
  completed_plans: 9
  percent: 91
---

---
gsd_state_version: 1.0
milestone: v4.0
milestone_name: Pod Fleet Self-Healing
status: active
stopped_at: Checkpoint 19-watchdog-service 19-02-PLAN.md Task 2 (human-verify Pod 8 canary)
last_updated: "2026-03-15T10:20:00Z"
last_activity: 2026-03-15 — Completed Plan 19-02 Task 1 (crash report endpoint + install script). Checkpoint pending for Pod 8 canary verification.
progress:
  [█████████░] 91%
  completed_phases: 3
  total_plans: 8
  completed_plans: 8
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-15)

**Core value:** Every pod survives any failure without physical intervention. Pods self-heal and remain remotely manageable at all times.
**Current focus:** Phase 19 — Watchdog Service. Plan 02 Task 1 COMPLETE (endpoint + script). Checkpoint pending: Pod 8 canary verification.

## Current Position

Phase: 19 of 21 (Watchdog Service)
Plan: 2 of 2 (Task 1 done, Task 2 checkpoint pending)
Status: Plan 19-02 Task 1 complete — crash report endpoint in rc-core + install-watchdog.bat. Awaiting Pod 8 canary verification.
Last activity: 2026-03-15 — Completed Plan 19-02 Task 1 (crash report endpoint + install script). Checkpoint pending for Pod 8 canary verification.

Progress: [█████████░] 93%

## Performance Metrics

**Velocity:**
- Total plans completed: 8
- Average duration: ~6 min
- Total execution time: ~48 min

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 16-firewall-auto-config P01 | 4 tasks | 2 files | ~4 min |
| 17-websocket-exec P01 | 2 tasks | 1 file | 3 min |
| 17-websocket-exec P03 | 3 tasks | 3 files | 9 min |
| 18-startup-self-healing P01 | 2 tasks | 3 files | 7 min |
| 18-startup-self-healing P02 | 2 tasks | 3 files | 6 min |
| 19-watchdog-service P01 | 1 task | 8 files | 10 min |
| 19-watchdog-service P02 | 2 tasks | 2 files | 9 min |

*Updated after each plan completion*
| Phase 20-deploy-resilience P01 | 12 | 2 tasks | 2 files |

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
- [Phase 17-websocket-exec P03]: Pod-prefixed request_id (pod_X:uuid) for disconnect cleanup; HTTP-first WS-fallback exec pattern; oneshot channel resolution for ExecResult; deploy.rs public API unchanged
- [Phase 18-startup-self-healing P01]: Synchronous self-heal before load_config; embedded config template via include_str!; START_SCRIPT_CONTENT as const with CRLF; AtomicBool for startup log first-write truncation; cfg(windows) gating for registry ops
- [Phase 18-startup-self-healing P02]: StartupReport sent once per process lifetime using startup_report_sent bool flag; fire-and-forget from agent side; message ordering Register -> StartupReport -> ContentManifest; core logs + records pod activity
- [Phase 19-watchdog-service P01]: rc-watchdog crate with windows-service 0.8 for SCM; tasklist polling (not sysinfo); reqwest blocking (no tokio); 15s restart grace window; WTSQueryUserToken + CreateProcessAsUser for Session 1 spawn; read rc-agent.toml with COMPUTERNAME fallback
- [Phase 19-watchdog-service P02]: Bare StatusCode::OK return (no JSON body) for fire-and-forget crash report; log_pod_activity source='watchdog'; install-watchdog.bat with sc.exe failure actions (5s/10s/30s restart)
- [Phase 20-deploy-resilience]: RollingBack is an active deploy phase (is_active() returns true) — prevents second deploy during rollback
- [Phase 20-deploy-resilience]: SWAP_SCRIPT_CONTENT uses /write endpoint not echo pipeline — avoids shell escaping issues
- [Phase 20-deploy-resilience]: Rollback success sets Failed with rolled-back reason — no separate RolledBack variant needed

### Pending Todos

- Pod 3 still not verified running after fix-pod.bat — needs physical reboot + verification
- fix-firewall.bat needs to be run on Pod 1 (ICMP blocked) — Phase 16 will fix permanently
- Version string inconsistency: USB-installed pods report v0.1.0, HTTP-deployed report v0.5.2

### Blockers/Concerns

- [Phase 19] Session 1 lock screen stays in rc-agent — confirmed. Watchdog wraps bat file, preserves Session 1.
- [Phase 21] Fleet dashboard depends on health data flowing from Phases 16–20. Do not start until Phase 19 is deployed.

## Session Continuity

Last session: 2026-03-15T11:07:58.964Z
Stopped at: Completed 20-deploy-resilience 20-01-PLAN.md
Resume file: None
