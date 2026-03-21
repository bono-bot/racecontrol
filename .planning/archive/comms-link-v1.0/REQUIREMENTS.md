# Requirements: James-Bono Comms Link

**Defined:** 2026-03-12
**Core Value:** James and Bono are always connected and always in sync -- if the link drops, both sides know immediately and recovery is automatic.

## v1 Requirements

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

## v2 Requirements

### Enhanced Monitoring

- **EM-01**: Web-based status dashboard for Uday
- **EM-02**: Historical uptime tracking and graphs
- **EM-03**: Connection latency monitoring

### Advanced Sync

- **AS-01**: Sync additional shared files beyond LOGBOOK.md
- **AS-02**: Bidirectional git sync (pull/push coordination)

## Out of Scope

| Feature | Reason |
|---------|--------|
| CRDTs / operational transforms | Overkill for two-node system where concurrent edits are near-impossible |
| Message persistence / database | Two nodes, one human consumer -- no need to persist message history |
| Web dashboard | v2 -- focus on CLI and WhatsApp alerts first |
| Multi-node topology | Only 2 nodes (James + Bono), no need for mesh/cluster |
| E2E encryption | Internal infrastructure, PSK auth is sufficient |
| Git-based LOGBOOK sync | Claude Code's git polling causes index.lock collisions -- use direct file sync |
| NSSM Windows service | Causes Session 0 isolation -- use Task Scheduler instead |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| WS-01 | Phase 1 | Complete |
| WS-02 | Phase 2 | Complete |
| WS-03 | Phase 1 | Complete |
| WS-04 | Phase 1 | Complete |
| WS-05 | Phase 2 | Complete |
| HB-01 | Phase 3 | Complete |
| HB-02 | Phase 3 | Complete |
| HB-03 | Phase 3 | Complete |
| HB-04 | Phase 3 | Complete |
| WD-01 | Phase 4 | Complete |
| WD-02 | Phase 4 | Complete |
| WD-03 | Phase 4 | Complete |
| WD-04 | Phase 5 | Complete |
| WD-05 | Phase 5 | Complete |
| WD-06 | Phase 5 | Complete |
| WD-07 | Phase 5 | Complete |
| AL-01 | Phase 6 | Complete |
| AL-02 | Phase 6 | Complete |
| AL-03 | Phase 6 | Complete |
| AL-04 | Phase 6 | Complete |
| AL-05 | Phase 8 | Complete |
| LS-01 | Phase 7 | Complete |
| LS-02 | Phase 7 | Complete |
| LS-03 | Phase 7 | Complete |
| LS-04 | Phase 7 | Complete |
| LS-05 | Phase 7 | Complete |
| CO-01 | Phase 8 | Complete |
| CO-02 | Phase 8 | Complete |
| CO-03 | Phase 8 | Complete |

**Coverage:**
- v1 requirements: 29 total
- Mapped to phases: 29
- Unmapped: 0

---
*Requirements defined: 2026-03-12*
*Last updated: 2026-03-12 after roadmap creation (phase mappings added)*
