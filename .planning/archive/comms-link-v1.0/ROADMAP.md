# Roadmap: James-Bono Comms Link

## Overview

This roadmap delivers a persistent, real-time communication system between James (on-site, NAT-ed Windows client) and Bono (cloud VPS). Work flows from bare WebSocket connectivity through reconnection hardening, heartbeat monitoring, watchdog process supervision, alerting infrastructure, LOGBOOK file sync, and finally coordination tooling. Each phase delivers a coherent, independently verifiable capability. The WebSocket transport is the spine -- everything else plugs into it.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 1: WebSocket Connection** - Persistent outbound WebSocket from James to Bono with PSK auth and connection state tracking
- [x] **Phase 2: Reconnection & Reliability** - Auto-reconnect with backoff and offline message queuing with replay
- [x] **Phase 3: Heartbeat** - Application-level heartbeat with process status and system metrics payloads
- [x] **Phase 4: Watchdog Core** - Claude Code process monitoring, zombie cleanup, auto-restart via Task Scheduler (completed 2026-03-12)
- [x] **Phase 5: Watchdog Hardening** - Escalating cooldown, post-restart self-test, WebSocket re-establishment, and email notification (completed 2026-03-12)
- [x] **Phase 6: Alerting** - WhatsApp and email notifications to Uday with flapping suppression (completed 2026-03-12)
- [ ] **Phase 7: LOGBOOK Sync** - File-based LOGBOOK.md synchronization over WebSocket with conflict detection
- [ ] **Phase 8: Coordination & Daily Ops** - Real-time AI-to-AI messaging, Bono coordination, daily health summary, failsafe retirement

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

## Progress

**Execution Order:**
Phases execute in numeric order: 1 -> 2 -> 3 -> 4 -> 5 -> 6 -> 7 -> 8

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. WebSocket Connection | 2/2 | Complete | 2026-03-12 |
| 2. Reconnection & Reliability | 1/1 | Complete | 2026-03-12 |
| 3. Heartbeat | 1/1 | Complete | 2026-03-12 |
| 4. Watchdog Core | 2/2 | Complete   | 2026-03-12 |
| 5. Watchdog Hardening | 2/2 | Complete   | 2026-03-12 |
| 6. Alerting | 2/2 | Complete | 2026-03-12 |
| 7. LOGBOOK Sync | 1/2 | In progress | - |
| 8. Coordination & Daily Ops | 1/2 | In progress | - |
