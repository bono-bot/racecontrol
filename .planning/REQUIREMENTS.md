# Requirements: Racing Point Operations — v10.0 Connectivity & Redundancy

**Defined:** 2026-03-20
**Core Value:** Customers see their lap times, compete on leaderboards, and compare telemetry

## v10.0 Requirements

Requirements for Connectivity & Redundancy milestone. Each maps to roadmap phases.
Comms Link v2.0 (shipped 2026-03-20) is the coordination backbone for James-Bono communication.

### Infrastructure

- [ ] **INFRA-01**: Server .23 has DHCP reservation pinned to MAC `10-FF-E0-80-B1-A7`
- [ ] **INFRA-02**: James can execute commands on Server .23 via rc-agent :8090 over Tailscale IP
- [ ] **INFRA-03**: James can execute commands on Bono VPS via comms-link exec_request protocol

### Config Sync

- [ ] **SYNC-01**: racecontrol.toml changes detected via sha2 hash and pushed to Bono via comms-link sync_push
- [ ] **SYNC-02**: Config payload is sanitized (credentials/local paths stripped) before push
- [ ] **SYNC-03**: Bono applies received config to cloud racecontrol (pod definitions, billing rates, game catalog)

### Failover Mechanics

- [ ] **FAIL-01**: rc-agent has `failover_url` in CoreConfig pointing to Bono's racecontrol via Tailscale
- [ ] **FAIL-02**: rc-agent WS reconnect loop uses `Arc<RwLock<String>>` for runtime URL switching
- [ ] **FAIL-03**: New `SwitchController` AgentMessage triggers rc-agent URL switch without process restart
- [ ] **FAIL-04**: `self_monitor.rs` suppresses relaunch during intentional failover (last_switch_time guard)

### Health Monitoring

- [ ] **HLTH-01**: James runs health probe loop against server .23 (HTTP + WS check, 5s interval)
- [ ] **HLTH-02**: Hysteresis state machine (3-down/2-up thresholds, reuse cloud_sync pattern) gates failover trigger
- [ ] **HLTH-03**: Minimum 60s continuous outage window before auto-failover fires
- [ ] **HLTH-04**: Bono detects James heartbeat loss via comms-link as secondary watchdog (24/7)

### Failover Orchestration

- [ ] **ORCH-01**: James sends `task_request` to Bono via comms-link to activate cloud racecontrol as primary
- [ ] **ORCH-02**: Racecontrol broadcasts `SwitchController` to all connected pods (failover URL)
- [ ] **ORCH-03**: Pod-side LAN probe confirms .23 is unreachable before honoring switch (split-brain prevention)
- [ ] **ORCH-04**: Uday notified via email + WhatsApp on failover event

### Failback

- [ ] **BACK-01**: James detects server .23 recovery (2-up threshold)
- [ ] **BACK-02**: Sync-before-accept: cloud sessions merged to local DB before .23 resumes primary role
- [ ] **BACK-03**: Racecontrol broadcasts `SwitchController` with original .23 URL to all pods
- [ ] **BACK-04**: Uday notified on failback event

## Future Requirements

### Enhanced Monitoring

- **MON-01**: Grafana dashboard for failover events and server uptime
- **MON-02**: Historical failover log with duration and session impact metrics

### Advanced Redundancy

- **RED-01**: Automatic config sync on every racecontrol.toml change (file watcher)
- **RED-02**: Kiosk and dashboard auto-redirect to cloud during failover

## Out of Scope

| Feature | Reason |
|---------|--------|
| Active-active (dual primary) | SQLite-first architecture incompatible with dual-write; cloud_sync already documents UUID mismatch |
| DNS-based failover | Windows LAN DNS caching causes 90-120s latency; direct IP switching is <15s |
| Tailscale SSH on Windows | Not supported by Tailscale (GitHub #14942); use rc-agent :8090 over Tailscale IP |
| Salt-based fleet management | v6.0 blocked at BIOS AMD-V; this milestone uses existing rc-agent + comms-link |
| Kiosk/dashboard failover UI | Pods switch WS endpoint; kiosk/dashboard on .23 unavailable during failover — staff uses cloud URL directly |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| INFRA-01 | Pending | Pending |
| INFRA-02 | Pending | Pending |
| INFRA-03 | Pending | Pending |
| SYNC-01 | Pending | Pending |
| SYNC-02 | Pending | Pending |
| SYNC-03 | Pending | Pending |
| FAIL-01 | Pending | Pending |
| FAIL-02 | Pending | Pending |
| FAIL-03 | Pending | Pending |
| FAIL-04 | Pending | Pending |
| HLTH-01 | Pending | Pending |
| HLTH-02 | Pending | Pending |
| HLTH-03 | Pending | Pending |
| HLTH-04 | Pending | Pending |
| ORCH-01 | Pending | Pending |
| ORCH-02 | Pending | Pending |
| ORCH-03 | Pending | Pending |
| ORCH-04 | Pending | Pending |
| BACK-01 | Pending | Pending |
| BACK-02 | Pending | Pending |
| BACK-03 | Pending | Pending |
| BACK-04 | Pending | Pending |

**Coverage:**
- v10.0 requirements: 22 total
- Mapped to phases: 0
- Unmapped: 22

---
*Requirements defined: 2026-03-20*
*Last updated: 2026-03-20 after initial definition*
