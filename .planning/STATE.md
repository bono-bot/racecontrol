---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: executing
stopped_at: Completed 05-01-PLAN.md
last_updated: "2026-03-13T03:38:59.136Z"
progress:
  total_phases: 5
  completed_phases: 4
  total_plans: 15
  completed_plans: 13
  percent: 87
---

---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: executing
stopped_at: Phase 04 complete — verification passed (human_needed items approved), PERF-02 moved to Phase 5
last_updated: "2026-03-13T14:00:00.000Z"
progress:
  [█████████░] 87%
  completed_phases: 4
  total_plans: 12
  completed_plans: 12
---

---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: executing
stopped_at: Completed 04-01-PLAN.md
last_updated: "2026-03-13T01:59:35.468Z"
progress:
  total_phases: 5
  completed_phases: 3
  total_plans: 12
  completed_plans: 10
  percent: 83
---

---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: executing
stopped_at: Phase 03 complete — all 3 plans executed, verification passed
last_updated: "2026-03-13T12:00:00.000Z"
progress:
  [████████░░] 83%
  completed_phases: 3
  total_plans: 9
  completed_plans: 9
---

---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: planning
stopped_at: Completed 02-03-PLAN.md
last_updated: "2026-03-13T00:10:42.143Z"
progress:
  total_phases: 5
  completed_phases: 1
  total_plans: 6
  completed_plans: 5
---

---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: planning
stopped_at: "Completed 01-03-PLAN.md — Phase 1 complete"
last_updated: "2026-03-13T01:00:00.000Z"
last_activity: 2026-03-13 — Plan 01-03 complete (deploy config template fix verified on Pod 8, DEPLOY-04 closed)
progress:
  total_phases: 5
  completed_phases: 1
  total_plans: 3
  completed_plans: 3
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-13)

**Core value:** Pods self-heal, deployments work reliably, customers never see system internals
**Current focus:** Phase 5 — Blanking Screen Protocol (Phases 1-4 complete)

## Current Position

Phase: 4 of 5 complete (Deployment Pipeline Hardening — DONE)
Next: Phase 5 — Blanking Screen Protocol
Status: Phase 4 Complete — ready to start Phase 5

Progress: [████████░░] 80%

## Performance Metrics

**Velocity:**
- Total plans completed: 3
- Average duration: ~21 min/plan
- Total execution time: ~1.5 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 01-state-wiring-config-hardening | 3 | 3 | ~21min |

**Recent Trend:**
- Last 5 plans: 01-01 (45min), 01-02 (8.5min), 01-03 (10min)
- Trend: On track

*Updated after each plan completion*
| Phase 02-watchdog-hardening P01 | 6 | 2 tasks | 5 files |
| Phase 02-watchdog-hardening P02 | 18 | 2 tasks | 1 files |
| Phase 02-watchdog-hardening P03 | 4 | 2 tasks | 1 files |
| Phase 03-websocket-resilience P03 | 2 | 2 tasks | 2 files |
| Phase 04-deployment-pipeline P01 | 6 | 4 tasks | 6 files |
| Phase 04-deployment-pipeline P02 | 6 | 2 tasks | 4 files |
| Phase 04-deployment-pipeline P03 | 20 | 4 tasks | 9 files |
| Phase 05-blanking-screen-protocol P01 | 28 | 2 tasks | 4 files |

## Accumulated Context

### Decisions

- Roadmap: EscalatingBackoff (rc-common/watchdog.rs) and EmailAlerter (rc-core/email_alerts.rs) already implemented with tests — Phase 1 is integration/wiring, not new design
- Roadmap: pod_monitor gets exclusive restart ownership; pod_healer reads shared state only — resolves concurrent restart race
- Roadmap: PERF requirements embedded into phases where the work lives (PERF-03/04 in Phase 3, PERF-01/02 in Phase 4) — no standalone performance phase
- Roadmap: AUTH-01 grouped with SCREEN requirements in Phase 5 — both address customer-facing UX consistency
- [Phase 01]: Used serial_test to prevent global semaphore contention in pod-agent unit tests
- [Phase 01]: Committed deploy scripts to racecontrol/deploy/ since deploy-staging/ is not a git repo
- [Phase 01]: LAN bind falls back to 0.0.0.0 with warning log rather than panicking if 192.168.x.x not detected
- [01-01]: pod_backoffs keyed "pod_{N}" (underscore, not dash) to match pod_monitor.rs entry() pattern
- [01-01]: ConfigError lock screen shows generic message only — technical details to tracing::error! — customer never sees internals
- [01-01]: EscalatingBackoff fields are private in rc-common — tests must use public API (attempt(), ready()) not direct field access
- [01-01]: wss:// accepted in addition to ws:// for cloud TLS connections
- [01-03]: Template uses [pod] with number/name/sim and [core] with url — matching AgentConfig/PodConfig/CoreConfig serde layout exactly
- [01-03]: sim defaults to assetto_corsa in template (primary game at venue) — not a per-pod variable
- [01-03]: deploy_pod.py script left unchanged — template fix sufficient, script logic correct
- [Phase 02-01]: WatchdogState defined in rc-core not rc-common — it is a core-side FSM, not a shared protocol type
- [Phase 02-01]: health_response_body() extracted as pure fn for testability; /health always returns 200, JSON body distinguishes ok/degraded
- [Phase 02-01]: verify_restart gains last_seen param so email alerts carry accurate heartbeat context at failure time
- [Phase 02-03]: Healer reads pod_watchdog_states but never writes it — FSM transitions are pod_monitor exclusive
- [Phase 02-03]: needs_restart set only for Rule 2 no-WS failure — disk/memory/zombie issues are healer-only, no restart flag
- [Phase 02-03]: Healer reads backoff.ready() for cooldown gating but does NOT call record_attempt() — advancing backoff is monitor-only
- [Phase 02-03]: should_skip_for_watchdog_state() extracted as pure fn — tests verify skip logic without async AppState
- [Phase 02-02]: Partial recovery (process+WS ok, lock screen fail) is FAILED — lock screen is essential for customer flow, no special-case
- [Phase 02-02]: is_closed() replaces contains_key() for WS liveness — stale sender entries can linger in map after receiver drops
- [Phase 02-02]: determine_failure_reason() + failure_type_from_reason() extracted as pure fns — enables testing failure path without network
- [Phase 02-02]: check_lock_screen URL updated to /health (from /) to align with Plan 01 /health endpoint addition
- [Phase 03-03]: disconnectTimerRef is useRef not useState -- timer state change must not trigger re-render
- [Phase 03-03]: React.memo uses default shallow equality -- Map copy in setPods preserves object identity for unchanged pod entries
- [Phase 03-03]: Sub-components (TransmissionToggle, FfbToggle) are NOT memoized -- they have local state that memo could interfere with
- [Phase 04-01]: DeployState uses serde(tag=state, content=detail) — consistent with protocol.rs adjacently-tagged enums; TS union uses { state: 'x' } discriminant matching Rust output
- [Phase 04-01]: DeployPodStatus placed in protocol.rs (not types.rs) — it is a protocol-level DTO, not a domain type
- [Phase 04-01]: is_active() returns false for Idle/Complete/Failed — all terminal/no-op states from the watchdog perspective
- [Phase 04-02]: Binary URL validated via HEAD request before killing old process — prevents leaving pod without agent on URL errors
- [Phase 04-02]: CancelDeploy sets Failed state in AppState; deploy_pod() checks is_cancelled() at each async step boundary
- [Phase 04-02]: DeployRolling sorts Pod 8 canary first, then ascending, with 5s inter-pod delay
- [Phase 04-02]: Config write failure is non-fatal — proceeds with existing config, logs warning
- [Phase 04-02]: POST /api/deploy/:pod_id returns 409 for active billing sessions AND active deploys
- [Phase 04-03]: WaitingSession is_active() returns false — queued state, not actively deploying; watchdog must not block on it
- [Phase 04-03]: deploy_rolling() resolves pod IPs at call time from AppState.pods — only deploys to known/connected pods
- [Phase 04-03]: Session-end hook wired at both billing timer removal points: tick_all_timers (natural/pause-timeout) and end_billing_session (manual/early end)
- [Phase 04-03]: DeployRolling WS handler replaced inline implementation with deploy_rolling() call for DRY behavior
- [Phase 04-03]: Pod IDs use pod_N (underscore) format throughout — plan context showed pod-N (dash), adapted to match existing AppState maps
- [Phase 04-03]: POST /api/deploy/rolling route placed before /deploy/:pod_id to avoid Axum path conflict
- [Phase 05-01]: LaunchSplash is active customer-facing state — health returns ok, is_idle_or_blanked() returns false
- [Phase 05-01]: Lock screen before game kill: all session-ending handlers follow show_lock_screen -> sleep(500ms) -> game.stop() -> enforce_safe_state()
- [Phase 05-01]: DIALOG_PROCESSES pub const in ac_launcher.rs — single source of truth for 5 dialog processes in both enforce_safe_state and cleanup_after_session

### Pending Todos

None yet.

### Blockers/Concerns

- Node.js on Racing-Point-Server (.23) must be verified before Phase 2 deploys email alerting — run `node --version` on .23; install Node.js LTS if absent
- agent_senders channel liveness: RESOLVED in 02-02 — is_closed() now used in pod_monitor; pod_healer resolved in 02-03
- Defender exclusions must be verified individually on all 8 pods before Phase 4 deployment hardening

## Session Continuity

Last session: 2026-03-13T03:38:59.134Z
Stopped at: Completed 05-01-PLAN.md
Resume file: None
