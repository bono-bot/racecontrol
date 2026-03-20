# Roadmap: James-Bono Comms Link

## Milestones

- v1.0 Comms Link MVP - Phases 1-8 (shipped 2026-03-12, partially in progress)
- v2.0 Reliable AI-to-AI Communication - Phases 9-14 (planned)

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

<details>
<summary>v1.0 Comms Link MVP (Phases 1-8)</summary>

- [x] **Phase 1: WebSocket Connection** - Persistent outbound WebSocket from James to Bono with PSK auth and connection state tracking
- [x] **Phase 2: Reconnection & Reliability** - Auto-reconnect with backoff and offline message queuing with replay
- [x] **Phase 3: Heartbeat** - Application-level heartbeat with process status and system metrics payloads
- [x] **Phase 4: Watchdog Core** - Claude Code process monitoring, zombie cleanup, auto-restart via Task Scheduler
- [x] **Phase 5: Watchdog Hardening** - Escalating cooldown, post-restart self-test, WebSocket re-establishment, and email notification
- [x] **Phase 6: Alerting** - WhatsApp and email notifications to Uday with flapping suppression
- [ ] **Phase 7: LOGBOOK Sync** - File-based LOGBOOK.md synchronization over WebSocket with conflict detection
- [ ] **Phase 8: Coordination & Daily Ops** - Real-time AI-to-AI messaging, Bono coordination, daily health summary, failsafe retirement

</details>

### v2.0 Reliable AI-to-AI Communication

- [ ] **Phase 9: Protocol Foundation** - ACK tracker, durable message queue, and protocol extensions as standalone modules with full test coverage
- [x] **Phase 10: Process Supervisor** - Standalone daemon supervisor with health checks, PID lockfile, and Task Scheduler watchdog-of-watchdog (parallel with Phase 9) (completed 2026-03-20)
- [x] **Phase 11: Reliable Delivery Wiring** - Wire ACK/queue into both daemons, bidirectional task routing with correlation IDs, demote INBOX.md (completed 2026-03-20)
- [x] **Phase 12: Remote Execution** - Bidirectional command execution with enum allowlist, array-args invocation, three-tier approval, and sanitized environment (completed 2026-03-20)
- [x] **Phase 13: Observability** - Health snapshots in heartbeats, metrics counters, JSON metrics endpoint, email fallback E2E validation
- [x] **Phase 14: Graceful Degradation** - Automatic email fallback, disk-buffered offline queue, and explicit connection mode visibility

## Phase Details

### Phase 1: WebSocket Connection
**Goal**: James can establish and maintain a persistent, authenticated WebSocket connection to Bono's VPS
**Depends on**: Nothing (first phase)
**Requirements**: WS-01, WS-03, WS-04
**Success Criteria** (what must be TRUE):
  1. James process connects to Bono's VPS over WebSocket (outbound, NAT-safe) and the connection stays open
  2. Connection is rejected without valid pre-shared key -- unauthorized clients cannot connect
  3. Connection state is observable as one of CONNECTED, RECONNECTING, or DISCONNECTED at any time
  4. A simple JSON message sent from James arrives at Bono (and vice versa) over the open connection
**Plans**: 2 plans

Plans:
- [x] 01-01-PLAN.md -- Project scaffold, shared protocol, state machine, unit tests
- [x] 01-02-PLAN.md -- WebSocket server (Bono) + client (James) + integration tests

### Phase 2: Reconnection & Reliability
**Goal**: The WebSocket connection self-heals after network disruptions without losing messages
**Depends on**: Phase 1
**Requirements**: WS-02, WS-05
**Success Criteria** (what must be TRUE):
  1. After pulling the network cable (or killing the connection), James automatically reconnects with exponential backoff (1s start, 30s cap)
  2. Messages sent while disconnected are queued and replayed in order upon reconnection -- no messages are lost
  3. Bono receives all queued messages after reconnection without duplicates
**Plans**: 1 plan

Plans:
- [x] 02-01-PLAN.md -- Auto-reconnect with exponential backoff + message queue with replay (TDD)

### Phase 3: Heartbeat
**Goal**: Bono can detect within 45 seconds when James is down, and both sides know the health of the connection
**Depends on**: Phase 2
**Requirements**: HB-01, HB-02, HB-03, HB-04
**Success Criteria** (what must be TRUE):
  1. James sends an application-level heartbeat ping every 15 seconds over the WebSocket
  2. Bono marks James as DOWN within 45 seconds of the last received heartbeat
  3. Each heartbeat payload includes whether Claude Code is currently running or stopped
  4. Each heartbeat payload includes system metrics: CPU usage, memory usage, and uptime
**Plans**: 1 plan

Plans:
- [x] 03-01-PLAN.md -- HeartbeatSender + SystemMetrics + HeartbeatMonitor with TDD (all 4 requirements)

### Phase 4: Watchdog Core
**Goal**: Claude Code is automatically restarted within seconds of crashing, with clean process state
**Depends on**: Phase 1
**Requirements**: WD-01, WD-02, WD-03
**Success Criteria** (what must be TRUE):
  1. Watchdog detects Claude Code crash or exit within 5 seconds
  2. Watchdog kills all zombie/orphan Claude Code processes (taskkill /F /T tree kill) before restarting
  3. Claude Code is successfully relaunched after crash and is responsive
  4. Watchdog runs via Task Scheduler in user session (Session 1, not Session 0) and survives reboots
**Plans**: 2 plans

Plans:
- [x] 04-01-PLAN.md -- ClaudeWatchdog class: crash detection, zombie kill, auto-restart with TDD (WD-01, WD-02)
- [x] 04-02-PLAN.md -- Watchdog runner entry point + Task Scheduler registration + human verification (WD-03)

### Phase 5: Watchdog Hardening
**Goal**: The watchdog handles repeated failures gracefully and re-establishes full connectivity after every restart
**Depends on**: Phase 4, Phase 2
**Requirements**: WD-04, WD-05, WD-06, WD-07
**Success Criteria** (what must be TRUE):
  1. Repeated rapid crashes trigger escalating cooldown (5s, 15s, 30s, 60s, 5min) instead of thrashing
  2. After restart, watchdog runs a self-test confirming Claude Code is actually responding (not just PID alive)
  3. After restart, the WebSocket connection to Bono is re-established automatically
  4. Bono receives an email from James after every successful restart saying "James is back online"
**Plans**: 2 plans

Plans:
- [x] 05-01-PLAN.md -- EscalatingCooldown class + self-test events in ClaudeWatchdog (TDD, WD-04, WD-05)
- [x] 05-02-PLAN.md -- Runner integration: CommsClient + HeartbeatSender + email notification (WD-06, WD-07)

### Phase 6: Alerting
**Goal**: Uday is immediately notified via WhatsApp when James goes down or comes back, with email as fallback
**Depends on**: Phase 3, Phase 5
**Requirements**: AL-01, AL-02, AL-03, AL-04
**Success Criteria** (what must be TRUE):
  1. Uday receives a WhatsApp message within 60 seconds of James going down (via Bono's Evolution API)
  2. Uday receives a WhatsApp message when James comes back online
  3. If WebSocket is down, the same alert information is sent via email as a fallback
  4. Rapid crash/restart cycles do not flood Uday with alerts -- flapping is suppressed with a cooldown window
**Plans**: 2 plans

Plans:
- [x] 06-01-PLAN.md -- AlertManager + AlertCooldown + sendEvolutionText + protocol recovery type (TDD, AL-01, AL-02, AL-04)
- [x] 06-02-PLAN.md -- James recovery signal + email fallback + Bono AlertManager wiring (TDD, AL-02, AL-03, AL-04)

### Phase 7: LOGBOOK Sync
**Goal**: Both AIs always have the current LOGBOOK.md within 30 seconds of either side making a change
**Depends on**: Phase 2
**Requirements**: LS-01, LS-02, LS-03, LS-04, LS-05
**Success Criteria** (what must be TRUE):
  1. Changes to LOGBOOK.md are detected via file hash comparison (not git-based) within seconds
  2. The full file content is transmitted over WebSocket to the other side on change detection
  3. The receiving side writes atomically (temp file + rename) so the file is never in a partial state
  4. If both sides modified LOGBOOK.md since the last sync, a conflict is detected and flagged (not silently overwritten)
  5. After any change on either side, both AIs have identical LOGBOOK.md content within 30 seconds
**Plans**: 2 plans

Plans:
- [ ] 07-01-PLAN.md -- LogbookWatcher + logbook-merge: file polling, hash comparison, atomic write, conflict detection (TDD, LS-01, LS-03, LS-04)
- [ ] 07-02-PLAN.md -- Wiring + integration: wireLogbook() for James and Bono, ack flow, reconnect sync (LS-02, LS-05)

### Phase 8: Coordination & Daily Ops
**Goal**: James and Bono can exchange real-time coordination messages, and Uday gets a daily health summary
**Depends on**: Phase 6, Phase 7
**Requirements**: CO-01, CO-02, CO-03, AL-05
**Success Criteria** (what must be TRUE):
  1. James and Bono can send and receive structured coordination messages (task requests, status updates) in real time over the WebSocket -- not just heartbeats
  2. Bono's existing [FAILSAFE] heartbeat mechanism is retired/integrated so there are no duplicate monitoring systems
  3. Uday receives a daily health summary (uptime percentage, restart count, connection stability) via WhatsApp
  4. Coordination protocol is documented and agreed between both sides (message types, expected responses)
**Plans**: 2 plans

Plans:
- [x] 08-01-PLAN.md -- Protocol extension + HealthAccumulator + DailySummaryScheduler (TDD, CO-01, AL-05)
- [ ] 08-02-PLAN.md -- Coordination wiring + PROTOCOL.md documentation + [FAILSAFE] retirement (CO-01, CO-02, CO-03, AL-05)

### Phase 9: Protocol Foundation
**Goal**: The ACK tracker and durable message queue exist as tested, standalone modules ready to be wired into the daemon
**Depends on**: Nothing (parallel track, builds on v1.0 protocol patterns)
**Requirements**: REL-01, REL-02, REL-03, REL-04, REL-05, REL-06, TQ-01, TQ-02, TQ-03, TQ-04
**Success Criteria** (what must be TRUE):
  1. A message sent through ack-tracker gets a monotonic sequence number and is retried with exponential backoff if no ACK arrives within timeout (3 retries max)
  2. On reconnect, the tracker replays all unACKed messages starting from the last ACKed sequence number
  3. The receiver deduplicates messages via a seen-message cache (last 1000 IDs, 1hr TTL) so replayed messages are not processed twice
  4. Messages enqueued to the WAL-backed queue survive a process crash -- restarting the daemon loads and resends unACKed messages
  5. Control messages (heartbeat, msg_ack) are explicitly excluded from ACK tracking -- no ACK storms possible
**Plans**: 2 plans

Plans:
- [ ] 09-01-PLAN.md -- Protocol extensions (msg_ack, CONTROL_TYPES, isControlMessage) + AckTracker + DeduplicatorCache (TDD, REL-01, REL-02, REL-03, REL-04, REL-05, REL-06)
- [ ] 09-02-PLAN.md -- MessageQueue with JSON Lines WAL persistence (TDD, TQ-01, TQ-02, TQ-03, TQ-04)

### Phase 10: Process Supervisor
**Goal**: The comms-link daemon is automatically restarted mid-session if it crashes, without waiting for a reboot
**Depends on**: Nothing (parallel track, independent of Phase 9)
**Requirements**: SUP-01, SUP-02, SUP-03, SUP-04, SUP-05
**Success Criteria** (what must be TRUE):
  1. A standalone process supervisor monitors the daemon via HTTP health check every 15 seconds and respawns it on failure
  2. Escalating cooldown prevents thrashing on repeated failures (reuses EscalatingCooldown pattern from v1.0 watchdog)
  3. A PID lockfile prevents duplicate daemon instances -- the supervisor checks the lock before spawning
  4. A Windows Task Scheduler task runs every 5 minutes to verify the supervisor itself is alive (watchdog-of-watchdog)
  5. The deprecated ping-heartbeat.js and wmic usage are fully replaced by the new supervisor
**Plans**: 2 plans

Plans:
- [x] 10-01-PLAN.md -- ProcessSupervisor class with health check, restart, escalating cooldown, PID lockfile (TDD, SUP-01, SUP-02, SUP-03)
- [x] 10-02-PLAN.md -- Supervisor runner + Task Scheduler registration + startup script update + deprecate ping-heartbeat (SUP-04, SUP-05)

### Phase 11: Reliable Delivery Wiring
**Goal**: Messages between James and Bono are reliably delivered with ACK confirmation, and either side can initiate structured task requests
**Depends on**: Phase 9, Phase 10 (benefits from supervisor being deployed -- daemon deploy mistakes are self-healing)
**Requirements**: TQ-05, BDR-01, BDR-02, BDR-03
**Success Criteria** (what must be TRUE):
  1. Task messages sent from James to Bono (and vice versa) receive an ACK within 1 second confirming delivery -- unACKed messages are retried automatically
  2. Either side can initiate a structured task request with a correlation ID, and the response is routed back to the originator via reply_to
  3. Unanswered task requests time out after a configurable period (default 5 minutes) and the sender is notified
  4. INBOX.md is demoted to a human-readable audit log only -- no code reads it programmatically
**Plans**: 2 plans

Plans:
- [ ] 11-01-PLAN.md -- James-side wiring: sendRaw, AckTracker, MessageQueue, dedup, INBOX audit, task timeout (TQ-05, BDR-01, BDR-02, BDR-03)
- [ ] 11-02-PLAN.md -- Bono-side wiring: ACK auto-send, dedup, task timeout in wireBono (TQ-05, BDR-01, BDR-02, BDR-03)

### Phase 12: Remote Execution
**Goal**: Bono can send commands to James (and vice versa) with a three-tier approval flow, and results are returned reliably
**Depends on**: Phase 11 (reliable delivery must exist before remote execution)
**Requirements**: EXEC-01, EXEC-02, EXEC-03, EXEC-04, EXEC-05, EXEC-06, EXEC-07, EXEC-08
**Success Criteria** (what must be TRUE):
  1. Either AI can send an exec_request specifying a command from an enum allowlist -- commands not on the list are rejected
  2. Read-only commands (status, health, version) execute immediately without human approval and return results
  3. Dangerous commands pause and wait for human approval via WhatsApp notification -- unapproved commands default-deny after 10 minutes
  4. Command results (stdout, stderr, exit code) are returned as a structured exec_result message to the requester
  5. Commands use array-args child_process form only with sanitized environment (PATH/SYSTEMROOT/TEMP only) -- shell-string form is impossible by design
**Plans**: 3 plans

Plans:
- [x] 12-01-PLAN.md -- Exec protocol: command registry, approval tiers, sanitized env, protocol extensions (TDD, EXEC-01, EXEC-02, EXEC-08)
- [x] 12-02-PLAN.md -- ExecHandler: 3-tier approval flow, timeout default-deny, dedup, structured results (TDD, EXEC-01, EXEC-03, EXEC-04, EXEC-05, EXEC-06, EXEC-07)
- [x] 12-03-PLAN.md -- Wiring: ExecHandler into james/index.js, HTTP relay routes, bono exec sending + human verify (EXEC-01..08)

### Phase 13: Observability
**Goal**: Bono has full visibility into James's operational state through structured metrics and validated fallback channels
**Depends on**: Phase 11 (metrics push uses reliable delivery)
**Requirements**: OBS-01, OBS-02, OBS-03, OBS-04
**Success Criteria** (what must be TRUE):
  1. Heartbeat payloads include pod status, queue depth, and deployment state -- Bono sees the full picture every 15 seconds
  2. In-process metrics counters track uptime, reconnect count, ACK latency, and queue depth continuously
  3. A JSON metrics endpoint (HTTP) on James exposes current metrics for Bono to query on demand
  4. The email fallback path is validated end-to-end in production -- a test email is sent and receipt confirmed
**Plans**: 2 plans

Plans:
- [x] 13-01-PLAN.md -- MetricsCollector class + extended collectMetrics with queue depth, pod status, deploy state (TDD, OBS-01, OBS-02)
- [x] 13-02-PLAN.md -- GET /relay/metrics endpoint, MetricsCollector wiring, email fallback validation (OBS-03, OBS-04)

### Phase 14: Graceful Degradation
**Goal**: When connectivity degrades, the system automatically falls through ordered modes (realtime, email, offline) without losing messages
**Depends on**: Phase 13 (observability validates email fallback first), Phase 11
**Requirements**: GD-01, GD-02, GD-03
**Success Criteria** (what must be TRUE):
  1. When WebSocket is down, critical messages (alerts, exec requests) automatically fall back to email delivery
  2. When both WebSocket and email are unavailable, messages buffer to a disk queue and are sent when connectivity returns
  3. The current connection mode (REALTIME, EMAIL_FALLBACK, OFFLINE_QUEUE) is visible in health snapshots and metrics
**Plans**: 2 plans

Plans:
- [x] 14-01-PLAN.md -- ConnectionMode state machine + sendCritical routing + offline drain (TDD, GD-01, GD-02, GD-03)
- [x] 14-02-PLAN.md -- Wire ConnectionMode into james/index.js, extend metrics/heartbeat, human verify (GD-01, GD-02, GD-03)

## Progress

**Execution Order:**
v1.0: 1 -> 2 -> 3 -> 4 -> 5 -> 6 -> 7 -> 8
v2.0: 9 + 10 (parallel) -> 11 -> 12 -> 13 -> 14

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. WebSocket Connection | v1.0 | 2/2 | Complete | 2026-03-12 |
| 2. Reconnection & Reliability | v1.0 | 1/1 | Complete | 2026-03-12 |
| 3. Heartbeat | v1.0 | 1/1 | Complete | 2026-03-12 |
| 4. Watchdog Core | v1.0 | 2/2 | Complete | 2026-03-12 |
| 5. Watchdog Hardening | v1.0 | 2/2 | Complete | 2026-03-12 |
| 6. Alerting | v1.0 | 2/2 | Complete | 2026-03-12 |
| 7. LOGBOOK Sync | v1.0 | 1/2 | In progress | - |
| 8. Coordination & Daily Ops | v1.0 | 1/2 | In progress | - |
| 9. Protocol Foundation | v2.0 | 0/2 | Planning | - |
| 10. Process Supervisor | v2.0 | 2/2 | Complete | 2026-03-20 |
| 11. Reliable Delivery Wiring | v2.0 | 2/2 | Complete | 2026-03-20 |
| 12. Remote Execution | v2.0 | Complete    | 2026-03-20 | 2026-03-20 |
| 13. Observability | v2.0 | Complete    | 2026-03-20 | 2026-03-20 |
| 14. Graceful Degradation | v2.0 | Complete    | 2026-03-20 | 2026-03-20 |
