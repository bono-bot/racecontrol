# Requirements: RC Sentry AI Debugger (v11.2)

**Defined:** 2026-03-21
**Core Value:** When rc-agent crashes, rc-sentry diagnoses WHY, applies fixes, and restarts with context — instead of blind restarts.

## v11.2 Requirements

### Crash Detection

- [ ] **DETECT-01**: rc-sentry polls rc-agent health endpoint (localhost:8090/health) every 5s via std::net::TcpStream — anti-cheat safe, no process APIs
- [ ] **DETECT-02**: 3-poll hysteresis (15s of consecutive failures) before declaring rc-agent crashed — avoids false positives during shader compilation or game launch
- [ ] **DETECT-03**: start-rcagent.bat updated with stderr redirect (`2>> rc-agent-stderr.log`) and `RUST_BACKTRACE=1` — prerequisite for crash log analysis
- [ ] **DETECT-04**: self_heal.rs START_SCRIPT_CONTENT updated to match new bat file — prevents self-heal from reverting stderr capture
- [ ] **DETECT-05**: After crash detected, rc-sentry reads startup_log + stderr log to extract crash context (panic message, exit code, last phase)

### Tier 1 Deterministic Fixes

- [ ] **FIX-01**: Kill zombie rc-agent processes (taskkill by name, not PID — anti-cheat safe)
- [ ] **FIX-02**: Wait for port 8090 to leave TIME_WAIT before restarting (poll up to 10s) — prevents bind failure on restart
- [ ] **FIX-03**: Clean stale CLOSE_WAIT sockets if detected in crash log context
- [ ] **FIX-04**: Repair missing/corrupt rc-agent.toml and start-rcagent.bat (reuse self_heal patterns)
- [ ] **FIX-05**: Clear shader cache directories if crash log mentions DirectX/GPU errors
- [ ] **FIX-06**: All Tier 1 fix functions have `#[cfg(test)]` guards returning mock results — never execute real system commands during cargo test

### Pattern Memory

- [ ] **MEM-01**: Local DebugMemory struct in rc-sentry (serde, 50 lines) reads/writes debug-memory.json with atomic write (tmp + rename)
- [ ] **MEM-02**: Pattern matching keys on crash log content patterns (not SimType/exit_code since sentry sees logs, not game events)
- [ ] **MEM-03**: After successful fix + restart, sentry writes fix result back to debug-memory.json — closes learning loop

### Tier 3 Ollama Query

- [ ] **LLM-01**: Blocking HTTP POST to Ollama on James (.27:11434) via std::net::TcpStream with connect_timeout(5s) and read_timeout(45s)
- [ ] **LLM-02**: Ollama query is fire-and-forget — restart rc-agent immediately, update pattern memory when result arrives
- [ ] **LLM-03**: If Ollama unreachable (James offline, Ollama down), skip gracefully and restart with Tier 1 fixes only

### Escalation

- [ ] **ESC-01**: Escalation FSM: 3+ restarts within 10 minutes triggers staff alert + pod enters maintenance mode
- [ ] **ESC-02**: Reuse EscalatingBackoff from rc-common for restart cooldown (5s → 15s → 30s → 60s → 5min)
- [ ] **ESC-03**: On escalation, report crash diagnostics to server via fleet API and send email alert to Uday + Bono

### Fleet Reporting

- [ ] **FLEET-01**: SentryCrashReport and CrashDiagResult structs in rc-common for cross-crate sharing
- [ ] **FLEET-02**: POST /api/v1/sentry/crash endpoint in racecontrol accepts crash reports from pods
- [ ] **FLEET-03**: FleetHealthStore extended with last_sentry_crash field — crash history visible in dashboard

## Future Requirements

### Enhanced Diagnostics

- **DIAG-01**: Windows Event Log integration for system-level crash data
- **DIAG-02**: Memory dump analysis for rc-agent panics
- **DIAG-03**: Correlation of crash patterns across pods (fleet-wide crash analysis)

## Out of Scope

| Feature | Reason |
|---------|--------|
| Process inspection APIs (CreateToolhelp32Snapshot, OpenProcess) | Anti-cheat risk — EAC/iRacing flag these |
| Game crash debugging in sentry | Stays in rc-agent — rc-agent is alive for game crashes |
| Tokio/async in rc-sentry | Deliberate pure-std design — std::thread + std::net only |
| Windows Job Object for spawn management | Would require rc-sentry to be the launcher — too invasive |
| Cloud Claude (Tier 4) | Manual escalation only, not auto-triggered |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| DETECT-01 | TBD | Pending |
| DETECT-02 | TBD | Pending |
| DETECT-03 | TBD | Pending |
| DETECT-04 | TBD | Pending |
| DETECT-05 | TBD | Pending |
| FIX-01 | TBD | Pending |
| FIX-02 | TBD | Pending |
| FIX-03 | TBD | Pending |
| FIX-04 | TBD | Pending |
| FIX-05 | TBD | Pending |
| FIX-06 | TBD | Pending |
| MEM-01 | TBD | Pending |
| MEM-02 | TBD | Pending |
| MEM-03 | TBD | Pending |
| LLM-01 | TBD | Pending |
| LLM-02 | TBD | Pending |
| LLM-03 | TBD | Pending |
| ESC-01 | TBD | Pending |
| ESC-02 | TBD | Pending |
| ESC-03 | TBD | Pending |
| FLEET-01 | TBD | Pending |
| FLEET-02 | TBD | Pending |
| FLEET-03 | TBD | Pending |

**Coverage:**
- v11.2 requirements: 23 total
- Mapped to phases: 0
- Unmapped: 23

---
*Requirements defined: 2026-03-21*
*Last updated: 2026-03-21 after initial definition*
