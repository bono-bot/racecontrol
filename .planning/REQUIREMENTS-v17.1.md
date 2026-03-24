# Requirements: v17.1 Watchdog-to-AI Migration

**Defined:** 2026-03-25
**Core Value:** Replace dumb restart-loop watchdogs with intelligent AI-driven recovery that verifies spawned processes actually started, coordinates between recovery authorities, and escalates intelligently instead of blind-restarting.

## v17.1 Requirements

Requirements for watchdog-to-AI migration. Each maps to roadmap phases.

### Spawn Verification

- [x] **SPAWN-01**: rc-sentry verifies spawned processes are alive by polling /health with 10s timeout and 500ms interval before declaring restart success
- [x] **SPAWN-02**: Recovery events include spawn_verified field (true/false) so server knows if restarts actually worked
- [x] **SPAWN-03**: rc-sentry uses Session 1 spawn path (WTSQueryUserToken + CreateProcessAsUser) for GUI process launches, not std::process::Command

### Recovery Coordination

- [x] **COORD-01**: ProcessOwnership registry is enforced at all restart call sites in rc-sentry, self_monitor, pod_monitor, and rc-watchdog
- [x] **COORD-02**: Recovery intent registry (recovery-intent.json with 2-min TTL) prevents multiple authorities from restarting the same process simultaneously
- [x] **COORD-03**: Sentinel-based deconfliction — GRACEFUL_RELAUNCH distinguishes intentional restarts from crashes across all recovery systems
- [x] **COORD-04**: Server recovery events API (POST /api/v1/recovery/events + GET with pod_id/since_secs filter) provides cross-machine recovery visibility

### Graduated Recovery

- [x] **GRAD-01**: rc-sentry handle_crash() runs Tier 1 deterministic fixes (clean sockets, kill zombies, repair config) before any restart attempt
- [x] **GRAD-02**: rc-sentry checks Tier 2 pattern memory (debug-memory.json) for instant replay of known fixes between Tier 1 and restart
- [x] **GRAD-03**: rc-sentry queries Tier 3 Ollama (qwen2.5:3b on James .27:11434) for unknown crash patterns when Tier 1+2 fail
- [x] **GRAD-04**: Tier 4 escalation alerts staff via WhatsApp/fleet API after 3+ failed recovery attempts
- [x] **GRAD-05**: Pattern memory records include server_reachable context to distinguish server-down disconnects from real rc-agent crashes

### MAINTENANCE_MODE Fix

- [ ] **MAINT-01**: MAINTENANCE_MODE auto-clears after 30 minutes instead of blocking permanently
- [ ] **MAINT-02**: MAINTENANCE_MODE file carries JSON with diagnostic reason, timestamp, and restart count
- [ ] **MAINT-03**: Staff receives WhatsApp alert when MAINTENANCE_MODE activates on any pod
- [x] **MAINT-04**: pod_healer reads MAINTENANCE_MODE via rc-sentry /files before sending Wake-on-LAN, preventing WoL→restart→block infinite loop

### James Watchdog

- [ ] **JAMES-01**: james_watchdog.ps1 replaced with Rust-based AI debugger using shared ollama.rs from rc-common
- [ ] **JAMES-02**: James watchdog uses graduated response: count 1 wait → count 2 restart → count 3 AI diagnosis → count 4+ alert
- [ ] **JAMES-03**: James watchdog monitors all local services (comms-link, go2rtc, rc-sentry-ai, Ollama) with health-poll verification

### Self-Monitor Coordination

- [ ] **SELF-01**: rc-agent self_monitor checks rc-sentry availability (TCP :8091) before relaunch — if sentry alive, writes sentinel and exits instead of PowerShell relaunch
- [ ] **SELF-02**: PowerShell relaunch path becomes rare fallback only when rc-sentry is dead

## Future Requirements

### v17.2 — Extended Recovery

- **RECOV-01**: Replace tasklist-based process detection in rc-watchdog with TCP health poll as primary signal
- **RECOV-02**: james_monitor cascade_guard detects cross-machine recovery conflicts
- **RECOV-03**: Recovery telemetry dashboard in admin showing restart history, success rate, mean time to recovery

## Out of Scope

| Feature | Reason |
|---------|--------|
| Custom Windows Service for AI recovery | Existing rc-sentry + rc-watchdog architecture sufficient; adding another service increases surface area |
| Machine learning crash prediction | Over-engineering — pattern memory with Ollama diagnosis is sufficient for 8 pods |
| Network-based recovery coordination (etcd/consul) | 8 pods on LAN — file-based sentinels + HTTP recovery events API is simpler and proven |
| Replacing Ollama with cloud LLM for Tier 3 | Latency and cost — local qwen2.5:3b is fast enough and free |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| SPAWN-01 | Phase 184 | Complete |
| SPAWN-02 | Phase 184 | Complete |
| SPAWN-03 | Phase 184 | Complete |
| COORD-01 | Phase 185 | Complete |
| COORD-02 | Phase 185 | Complete |
| COORD-03 | Phase 185 | Complete |
| COORD-04 | Phase 183 | Complete |
| GRAD-01 | Phase 184 | Complete |
| GRAD-02 | Phase 184 | Complete |
| GRAD-03 | Phase 184 | Complete |
| GRAD-04 | Phase 184 | Complete |
| GRAD-05 | Phase 184 | Complete |
| MAINT-01 | Phase 186 | Pending |
| MAINT-02 | Phase 186 | Pending |
| MAINT-03 | Phase 186 | Pending |
| MAINT-04 | Phase 185 | Complete |
| JAMES-01 | Phase 188 | Pending |
| JAMES-02 | Phase 188 | Pending |
| JAMES-03 | Phase 188 | Pending |
| SELF-01 | Phase 187 | Pending |
| SELF-02 | Phase 187 | Pending |

**Coverage:**
- v17.1 requirements: 21 total
- Mapped to phases: 21/21
- Unmapped: 0

---
*Requirements defined: 2026-03-25*
*Last updated: 2026-03-25 after roadmap creation (phases 183-188)*