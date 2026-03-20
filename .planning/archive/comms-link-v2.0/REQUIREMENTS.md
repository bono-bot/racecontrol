# Requirements: James-Bono Comms Link

**Defined:** 2026-03-12 (v1.0), updated 2026-03-20 (v2.0)
**Core Value:** James and Bono are always connected and always in sync -- if the link drops, both sides know immediately and recovery is automatic.

## v1.0 Requirements (Complete)

### WebSocket Transport

- [x] **WS-01**: James establishes persistent WebSocket connection to Bono's VPS (outbound, NAT-safe)
- [x] **WS-02**: Auto-reconnect with exponential backoff (1s start, 30s cap) on connection loss
- [x] **WS-03**: Pre-shared key (PSK) authentication during WebSocket handshake
- [x] **WS-04**: Connection state machine with three states: CONNECTED, RECONNECTING, DISCONNECTED
- [x] **WS-05**: Message queuing during disconnection with replay on reconnect

### Heartbeat

- [x] **HB-01**: Application-level heartbeat ping every 15 seconds from James
- [x] **HB-02**: Bono detects missing heartbeat within 45 seconds and marks James as DOWN
- [x] **HB-03**: Heartbeat payload includes Claude Code process status (running/stopped)
- [x] **HB-04**: Heartbeat payload includes system metrics (CPU usage, memory, uptime)

### Watchdog

- [x] **WD-01**: Monitor Claude Code process and detect crash/exit within 5 seconds
- [x] **WD-02**: Auto-restart Claude Code after crash with zombie cleanup (taskkill /F /T tree kill)
- [x] **WD-03**: Run watchdog in user session via Task Scheduler (NOT as Windows service -- avoids Session 0)
- [x] **WD-04**: Escalating cooldown on repeated crashes (e.g. 5s -> 15s -> 30s -> 60s -> 5min)
- [x] **WD-05**: Startup self-test verifies Claude Code is responding after restart
- [x] **WD-06**: Re-establish WebSocket connection to Bono after restart
- [x] **WD-07**: Email Bono on restart: "James is back online"

### Alerting

- [x] **AL-01**: WhatsApp notification to Uday when James goes down (via Bono's Evolution API)
- [x] **AL-02**: WhatsApp notification to Uday when James comes back online
- [x] **AL-03**: Email fallback -- same alert info sent via email when WebSocket is down
- [x] **AL-04**: Flapping suppression -- suppress repeated alerts during rapid crash/restart cycles
- [x] **AL-05**: Daily health summary -- uptime percentage, restart count, connection stability

### LOGBOOK Sync

- [x] **LS-01**: Watch LOGBOOK.md for changes using file hash comparison (not git-based)
- [x] **LS-02**: Sync full file content over WebSocket on change detection
- [x] **LS-03**: Atomic writes on receiving side (write to temp file, then rename)
- [x] **LS-04**: Conflict detection when both sides modified since last sync
- [x] **LS-05**: Both AIs always have current LOGBOOK.md within 30 seconds of a change

### Coordination

- [x] **CO-01**: Bidirectional real-time messaging for AI-to-AI coordination (not just heartbeat)
- [x] **CO-02**: Coordinate with Bono to implement WebSocket server on VPS
- [x] **CO-03**: Coordinate with Bono to retire/integrate existing [FAILSAFE] heartbeat mechanism

## v2.0 Requirements

Requirements for reliable delivery, remote execution, and observability. Each maps to roadmap phases.

### Reliable Messaging

- [x] **REL-01**: Sender assigns monotonic sequence number to each data message
- [x] **REL-02**: Receiver sends msg_ack with received sequence number within 1 second
- [x] **REL-03**: Sender retries unACKed messages with exponential backoff (3 retries max)
- [x] **REL-04**: On reconnect, sender replays from last ACKed sequence number
- [x] **REL-05**: Receiver deduplicates messages via seen-message cache (last 1000 IDs, 1hr TTL)
- [x] **REL-06**: Control messages (heartbeat, msg_ack) never require ACKs (prevents ACK storms)

### Transactional Queue

- [x] **TQ-01**: Messages are persisted to file-backed WAL before sending over WebSocket
- [x] **TQ-02**: ACKed messages are removed from the WAL (compaction)
- [x] **TQ-03**: On daemon crash and restart, unACKed messages are loaded from WAL and resent
- [x] **TQ-04**: WAL writes are atomic (no partial/corrupt entries on crash)
- [x] **TQ-05**: INBOX.md is demoted to human-readable audit log only (never read programmatically)

### Process Supervision

- [x] **SUP-01**: Standalone process supervisor monitors daemon via HTTP health check every 15 seconds
- [x] **SUP-02**: Supervisor respawns daemon on health check failure with escalating cooldown
- [x] **SUP-03**: Supervisor uses PID lockfile to prevent duplicate daemon instances
- [x] **SUP-04**: Windows Task Scheduler task runs every 5 minutes to verify supervisor is alive
- [x] **SUP-05**: Supervisor replaces deprecated ping-heartbeat.js and wmic usage

### Bidirectional Routing

- [x] **BDR-01**: Both James and Bono can initiate structured task requests with correlation IDs
- [x] **BDR-02**: Task responses are routed back to the originator via reply_to correlation
- [x] **BDR-03**: Unanswered task requests time out after configurable period (default 5 minutes)

### Remote Execution

- [x] **EXEC-01**: Either AI can send exec_request to the other specifying a command from an allowlist
- [x] **EXEC-02**: Commands use array-args child_process form only (never shell string -- prevents injection)
- [x] **EXEC-03**: Auto-approve tier: read-only commands (status, health, version) execute immediately
- [x] **EXEC-04**: Notify-and-execute tier: moderate commands execute immediately + notify Uday via WhatsApp
- [x] **EXEC-05**: Require-approval tier: dangerous commands pause and wait for human approval before execution
- [x] **EXEC-06**: Unapproved commands default-deny after timeout (configurable, default 10 minutes)
- [x] **EXEC-07**: Command results (stdout, stderr, exit code) are returned as exec_result message
- [x] **EXEC-08**: Environment is sanitized -- only PATH/SYSTEMROOT/TEMP passed, never full process env

### Observability

- [x] **OBS-01**: Heartbeat payload extended with pod status, queue depth, and deployment state
- [x] **OBS-02**: Metrics counters accumulated in-process: uptime, reconnect count, ACK latency, queue depth
- [x] **OBS-03**: Metrics exported as structured JSON via HTTP endpoint for Bono to consume
- [x] **OBS-04**: Email fallback path validated end-to-end (send + receive confirmed)

### Graceful Degradation

- [x] **GD-01**: When WebSocket is down, critical messages (alerts, commands) fall back to email
- [x] **GD-02**: When email is also unavailable, messages buffer to disk queue (offline mode)
- [x] **GD-03**: Explicit connection mode visible: REALTIME > EMAIL_FALLBACK > OFFLINE_QUEUE

## Future Requirements (v2.1+)

### Enhanced Monitoring

- **EM-01**: Web-based status dashboard for Uday
- **EM-02**: Historical uptime tracking and graphs

### Advanced Sync

- **AS-01**: Sync additional shared files beyond LOGBOOK.md
- **AS-02**: Bidirectional git sync (pull/push coordination)

### Protocol Evolution

- **PE-01**: Protocol version negotiation (server advertises versions, client picks highest common)
- **PE-02**: Rolling upgrade support without coordinated deploys

## Out of Scope

| Feature | Reason |
|---------|--------|
| External message broker (Redis/RabbitMQ/NATS) | Overkill for 2-node system -- WAL file handles the scale |
| OpenTelemetry / Prometheus stack | One consumer (Bono) -- JSON metrics over WebSocket is sufficient |
| OAuth / JWT for command auth | Two known trusted agents -- PSK + command allowlist is sufficient |
| Consensus protocol (Raft/Paxos) | Two nodes can't run consensus; designated authority per data type works |
| Persistent message database | Messages are coordination ephemera -- WAL for queue, LOGBOOK for state |
| Complex RBAC / permission system | Two agents, one boss -- 3 rules not a permission system |
| GUI dashboard | Metrics exported for Bono to consume, not a standalone UI |
| NSSM Windows service | Causes Session 0 isolation -- use Task Scheduler instead |
| CRDTs / operational transforms | Overkill for two-node system |
| E2E encryption | Internal infrastructure, PSK auth is sufficient |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| WS-01..05 | v1.0 Phase 1-2 | Complete |
| HB-01..04 | v1.0 Phase 3 | Complete |
| WD-01..07 | v1.0 Phase 4-5 | Complete |
| AL-01..05 | v1.0 Phase 6,8 | Complete |
| LS-01..05 | v1.0 Phase 7 | Complete |
| CO-01..03 | v1.0 Phase 8 | Complete |
| REL-01 | Phase 9 | Complete |
| REL-02 | Phase 9 | Complete |
| REL-03 | Phase 9 | Complete |
| REL-04 | Phase 9 | Complete |
| REL-05 | Phase 9 | Complete |
| REL-06 | Phase 9 | Complete |
| TQ-01 | Phase 9 | Complete |
| TQ-02 | Phase 9 | Complete |
| TQ-03 | Phase 9 | Complete |
| TQ-04 | Phase 9 | Complete |
| TQ-05 | Phase 11 | Complete |
| SUP-01 | Phase 10 | Complete |
| SUP-02 | Phase 10 | Complete |
| SUP-03 | Phase 10 | Complete |
| SUP-04 | Phase 10 | Complete |
| SUP-05 | Phase 10 | Complete |
| BDR-01 | Phase 11 | Complete |
| BDR-02 | Phase 11 | Complete |
| BDR-03 | Phase 11 | Complete |
| EXEC-01 | Phase 12 | Complete |
| EXEC-02 | Phase 12 | Complete |
| EXEC-03 | Phase 12 | Complete |
| EXEC-04 | Phase 12 | Complete |
| EXEC-05 | Phase 12 | Complete |
| EXEC-06 | Phase 12 | Complete |
| EXEC-07 | Phase 12 | Complete |
| EXEC-08 | Phase 12 | Complete |
| OBS-01 | Phase 13 | Complete |
| OBS-02 | Phase 13 | Complete |
| OBS-03 | Phase 13 | Complete |
| OBS-04 | Phase 13 | Complete |
| GD-01 | Phase 14 | Complete |
| GD-02 | Phase 14 | Complete |
| GD-03 | Phase 14 | Complete |

**Coverage:**
- v1.0 requirements: 29 total (all complete)
- v2.0 requirements: 34 total
- Mapped to phases: 34/34
- Unmapped: 0

---
*Requirements defined: 2026-03-12*
*Last updated: 2026-03-20 after v2.0 roadmap creation*
