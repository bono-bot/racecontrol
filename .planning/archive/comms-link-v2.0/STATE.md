---
gsd_state_version: 1.0
milestone: v2.0
milestone_name: Reliable AI-to-AI Communication
status: completed
stopped_at: Completed 14-02-PLAN.md (v2.0 milestone complete)
last_updated: "2026-03-20T09:24:32.534Z"
last_activity: 2026-03-20 -- Phase 14-02 ConnectionMode wiring into James daemon
progress:
  total_phases: 14
  completed_phases: 14
  total_plans: 27
  completed_plans: 27
  percent: 100
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-20)

**Core value:** James and Bono are always connected and always in sync -- if the link drops, both sides know immediately and recovery is automatic.
**Current focus:** v2.0 milestone COMPLETE -- all 14 phases delivered

## Current Position

Phase: 14 of 14 (Graceful Degradation) -- COMPLETE
Plan: 2/2
Status: v2.0 Reliable AI-to-AI Communication milestone complete
Last activity: 2026-03-20 -- Phase 14-02 ConnectionMode wiring into James daemon

Progress: [██████████] 100% (28/28 plans with summaries)

## Performance Metrics

**Velocity:**
- Total plans completed: 28 (v1.0: 12, v2.0: 16)
- Phase 14-02: 4 min (wiring + checkpoint -- ConnectionMode into James daemon)
- Phase 14-01: 3 min (TDD -- ConnectionMode state machine + sendCritical + drain)
- Phase 13-02: 4 min (wiring + checkpoint -- metrics endpoint + email fallback)
- Phase 13-01: 2 min (TDD -- MetricsCollector + extended collectMetrics)
- Phase 12-03: 4 min (wiring + checkpoint)
- Phase 12-02: 3 min (TDD -- exec-handler)
- Phase 12-01: 2 min (TDD -- exec-protocol)
- Phase 11-02: 4 min (TDD + wiring)
- Phase 11-01: 9 min (TDD + wiring)
- Phase 10-01: 2 min (TDD)
- Phase 10-02: 3 min (wiring + checkpoint)
- Phase 09-02: 4 min (TDD -- MessageQueue WAL)

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- [v1.0]: All v1.0 decisions carried forward (see PROJECT.md Key Decisions)
- [v2.0]: HKCU Run key insufficient -- need mid-session watchdog
- [v2.0]: INBOX.md append pattern replaced with transactional queue
- [v2.0]: `wmic` deprecated -- must use modern Windows APIs
- [v2.0]: Remote command execution requires approval flow
- [v2.0]: Phases 9+10 are parallel (protocol foundation + process supervisor are independent)
- [v2.0]: Deploy order for ACK wiring: Bono first (backward compatible), then James
- [v2.0]: NEVER use shell-string child_process for remote execution -- array-args only
- [Phase 10]: Test files in test/ (project convention), all I/O injectable via constructor, poll() public for testing
- [Phase 10]: Two Task Scheduler tasks: CommsLink-Supervisor (onlogon) + CommsLink-SupervisorCheck (5-min interval)
- [Phase 09]: CONTROL_TYPES excludes file_ack/sync_action_ack -- data-layer ACKs need reliable delivery tracking
- [Phase 09]: JSON Lines WAL format for MessageQueue -- append-only with ACK markers, compaction rewrites
- [Phase 11]: INBOX.md is write-only audit log via appendAuditLog -- never read programmatically
- [Phase 11]: sendTracked() wraps createMessage + AckTracker.track for ergonomic tracked sends
- [Phase 11]: Top-level await for ESM data dir init + WAL load
- [Phase 11]: wireBono() returns { sendTaskRequest } for Bono-initiated tracked requests
- [Phase 11]: Dedup guard at top of message handler -- duplicates still ACKed but not re-processed
- [Phase 11]: INBOX.md audit log format: ## timestamp -- from sender
- [Phase 12]: 13 commands in registry: 8 auto, 2 notify, 3 approve -- static args, no parameterization
- [Phase 12]: buildSafeEnv returns only PATH/SYSTEMROOT/TEMP/TMP/HOME -- no secrets leak
- [Phase 12]: No shell:true anywhere in exec-protocol -- injection impossible by construction
- [Phase 12]: ExecHandler uses injected commandRegistry instead of imported validateExecRequest for testability
- [Phase 12]: Approval timeout sends tier='timed_out' (distinct from 'rejected') for telemetry
- [Phase 12]: shutdown() silently clears pending approvals without sending results
- [Phase 12]: sendExecRequest uses ackTracker.track for reliable delivery of exec_request messages
- [Phase 12]: Bono-side exec_request handling gracefully rejects with 'not implemented' (deferred)
- [Phase 12]: HTTP relay routes follow existing pattern: GET listing + POST action/:id
- [Phase 13]: Hardcoded version '2.0.0' in deployState -- simpler than reading package.json
- [Phase 13]: MODULE_STARTED_AT set once at import time for consistent startedAt
- [Phase 13]: DI params in collectMetrics for extensibility without breaking backward compat
- [Phase 13]: Metrics endpoint enriches snapshot with queueDepth/ackPending/wsState at request time
- [Phase 13]: ACK latency tracked via local Map bridging ackTracker.track and ack events
- [Phase 13]: Email fallback OBS-04 = infrastructure ready; needs SEND_EMAIL_PATH + Gmail OAuth renewal
- [Phase 14]: All delivery functions DI-injected in ConnectionMode -- no imports of send_email.js or CommsClient
- [Phase 14]: CRITICAL_TYPES = frozen Set of exec_result, task_request, recovery
- [Phase 14]: Optimistic email default (true) -- mode starts REALTIME until WS disconnects
- [Phase 14]: Drain calls compact() even on empty queue for consistent behavior
- [Phase 14]: sendViaEmail uses execFile with array args following daily-summary.js pattern
- [Phase 14]: probeEmail checks SEND_EMAIL_PATH accessibility via fs.access
- [Phase 14]: connectionMode.startProbe() on WS open, stopProbe() on shutdown

### Pending Todos

- Resolve SQLite vs JSON WAL decision before Phase 9 planning begins

### Blockers/Concerns

- SQLite vs WAL file: STACK.md recommends better-sqlite3, ARCHITECTURE.md recommends JSON WAL. Must decide before Phase 9.
- Bono must update his WebSocket server to support new protocol features (ACK, execution) -- coordinate during Phase 11.
- Email fallback needs real E2E test -- deferred to Phase 13 (OBS-04).

## Session Continuity

Last session: 2026-03-20T09:20:00Z
Stopped at: Completed 14-02-PLAN.md (v2.0 milestone complete)
Resume file: None
