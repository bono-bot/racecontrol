# Requirements: RaceControl v4.0

**Defined:** 2026-03-15
**Core Value:** Every pod survives any failure without physical intervention. Pods self-heal and remain remotely manageable at all times.

## v4.0 Requirements

### Service & Crash Recovery

- [x] **SVC-01**: rc-watchdog.exe runs as a Windows Service (SYSTEM) and auto-starts on boot
- [x] **SVC-02**: Watchdog detects rc-agent crash within 10 seconds and restarts it in Session 1
- [x] **SVC-03**: Watchdog reports crash events to rc-core via HTTP (startup count, crash time, exit code)
- [x] **SVC-04**: Install script registers watchdog service with SCM failure actions (restart on failure)

### Remote Exec via WebSocket

- [x] **WSEX-01**: rc-core can send shell commands to any connected pod via WebSocket (CoreToAgentMessage::Exec)
- [x] **WSEX-02**: rc-agent executes WebSocket commands with independent semaphore (separate from HTTP exec slots)
- [x] **WSEX-03**: Exec responses include stdout, stderr, exit code, and request_id correlation
- [x] **WSEX-04**: deploy.rs uses WebSocket exec as fallback when HTTP :8090 is unreachable

### Firewall Auto-Config

- [x] **FW-01**: rc-agent configures firewall rules (ICMP + TCP 8090) in Rust on every startup
- [x] **FW-02**: Firewall rules use profile=any and are idempotent (no duplicate accumulation)
- [x] **FW-03**: Firewall configuration runs before HTTP server bind (ensures port 8090 is reachable immediately)

### Startup & Self-Healing

- [x] **HEAL-01**: rc-agent verifies config file, start script, and registry key on every startup — repairs if missing
- [x] **HEAL-02**: rc-agent reports startup status to rc-core immediately after WebSocket connect (version, uptime, config hash, crash recovery flag)
- [x] **HEAL-03**: Startup errors are captured to a log file before rc-agent exits (for post-mortem)

### Deploy Resilience

- [x] **DEP-01**: Self-swap preserves previous binary as rc-agent-prev.exe for rollback
- [x] **DEP-02**: deploy.rs verifies pod health (WS + HTTP + process) after deploy, triggers rollback on failure
- [x] **DEP-03**: Defender exclusion covers rc-agent-new.exe staging filename (prevents AV interference)
- [x] **DEP-04**: Fleet deploy reports per-pod success/failure summary with retry for failed pods

### Fleet Health Dashboard

- [x] **FLEET-01**: Kiosk /fleet page shows all 8 pods with real-time status (WS connected, HTTP reachable, version, uptime)
- [x] **FLEET-02**: Pod status distinguishes WS-connected vs HTTP-reachable (a pod can be WS-up but HTTP-blocked)
- [ ] **FLEET-03**: Dashboard accessible from Uday's phone (mobile-first layout)

## Future Requirements

### Advanced Fleet Ops (v5.0+)

- **ADVFLEET-01**: One-click deploy from dashboard (select pods, upload binary, deploy with progress)
- **ADVFLEET-02**: Historical uptime and crash graphs per pod
- **ADVFLEET-03**: Automated canary deploy (Pod 8 first, wait, then fleet)
- **ADVFLEET-04**: Remote pod screenshot from dashboard

## Out of Scope

| Feature | Reason |
|---------|--------|
| Lock screen move to kiosk | Deferred — watchdog service pattern avoids this refactor |
| Native Windows Service API in rc-agent | Session 0 kills GUI; watchdog pattern chosen instead |
| NSSM dependency | External binary; prefer Rust watchdog we control |
| Cloud-based fleet monitoring | v4.0 is LAN-only; cloud dashboard is future milestone |
| Automatic security patching | Out of scope — OS updates are manual |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| FW-01 | Phase 16 | Complete |
| FW-02 | Phase 16 | Complete |
| FW-03 | Phase 16 | Complete |
| WSEX-01 | Phase 17 | Complete |
| WSEX-02 | Phase 17 | Complete |
| WSEX-03 | Phase 17 | Complete |
| WSEX-04 | Phase 17 | Complete |
| HEAL-01 | Phase 18 | Complete |
| HEAL-02 | Phase 18 | Complete |
| HEAL-03 | Phase 18 | Complete |
| SVC-01 | Phase 19 | Complete |
| SVC-02 | Phase 19 | Complete |
| SVC-03 | Phase 19 | Complete |
| SVC-04 | Phase 19 | Complete |
| DEP-01 | Phase 20 | Complete |
| DEP-02 | Phase 20 | Complete |
| DEP-03 | Phase 20 | Complete |
| DEP-04 | Phase 20 | Complete |
| FLEET-01 | Phase 21 | Complete |
| FLEET-02 | Phase 21 | Complete |
| FLEET-03 | Phase 21 | Pending |

**Coverage:**
- v4.0 requirements: 21 total
- Mapped to phases: 21
- Unmapped: 0

---
*Requirements defined: 2026-03-15*
*Last updated: 2026-03-15 — traceability populated after roadmap creation*
