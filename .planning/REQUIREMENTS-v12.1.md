# Requirements: E2E Process Guard (v12.1)

**Defined:** 2026-03-21
**Core Value:** No stale or unauthorized processes survive on any Racing Point machine — whitelist-enforced, continuously monitored, auto-killed.
**Parent:** v12.0 Operations Security

## v12.1 Requirements

### Process Guard Config

- [ ] **GUARD-01**: Central whitelist in `racecontrol.toml` with `[process_guard]` section defining approved processes, ports, auto-start entries
- [ ] **GUARD-02**: Per-machine overrides (`[process_guard.overrides.james]`, `[process_guard.overrides.pod]`, `[process_guard.overrides.server]`)
- [ ] **GUARD-03**: Category-tagged whitelist entries (system, racecontrol, game, peripheral, ollama) with wildcard/prefix matching
- [x] **GUARD-04**: `ProcessViolation` and `ProcessGuardStatus` AgentMessage variants in rc-common protocol
- [x] **GUARD-05**: `MachineWhitelist` shared types in rc-common for whitelist fetch/merge
- [ ] **GUARD-06**: `GET /api/v1/guard/whitelist/{machine_id}` endpoint returns merged whitelist for a machine

### Process Enforcement

- [ ] **PROC-01**: Continuous process scan (configurable interval, default 60s) comparing running processes against whitelist
- [ ] **PROC-02**: Auto-kill non-whitelisted processes with self-exclusion safety (never kill guard, rc-agent, racecontrol)
- [ ] **PROC-03**: PID identity verification (name + creation time) before kill to prevent PID reuse race
- [ ] **PROC-04**: Pod binary guard — detect rc-agent/racecontrol running on wrong machine (standing rule #2), CRITICAL severity with zero grace period
- [ ] **PROC-05**: Severity tiers per violation: KILL (immediate), ESCALATE (warn staff, auto-kill after TTL), MONITOR (log only)

### Auto-Start Enforcement

- [ ] **AUTO-01**: HKCU/HKLM Run key audit — enumerate all values, flag non-whitelisted entries
- [ ] **AUTO-02**: Startup folder audit — scan `%AppData%\...\Startup` for non-whitelisted shortcuts
- [ ] **AUTO-03**: Scheduled Task audit — `schtasks /query` parse, flag non-whitelisted tasks
- [ ] **AUTO-04**: Three-stage enforcement progression: LOG → ALERT → REMOVE (configurable per entry)

### Port Enforcement

- [ ] **PORT-01**: Listening port audit against approved port list per machine
- [ ] **PORT-02**: Auto-kill process owning non-whitelisted listening port

### Reporting & Alerting

- [ ] **ALERT-01**: Violation report via WebSocket to racecontrol on every kill/escalation
- [ ] **ALERT-02**: Staff kiosk notification badge for active violations
- [ ] **ALERT-03**: Email escalation on repeat offenders (N kills in time window)
- [ ] **ALERT-04**: Append-only audit log per machine (`process-guard.log`, 512KB rotation)
- [ ] **ALERT-05**: Fleet-wide violation summary in `GET /api/v1/fleet/health` (violation_count_24h, last_violation_at)

### Deployment

- [ ] **DEPLOY-01**: Process guard module in rc-agent (all 8 pods), report-only mode for safe rollout
- [ ] **DEPLOY-02**: Process guard module in racecontrol (server .23)
- [ ] **DEPLOY-03**: Standalone `rc-process-guard` binary for James (.27), reports via Tailscale HTTP

## v12.2+ Requirements

Deferred to future release.

- **GUARD-07**: LLM classification for ESCALATE-tier unknowns (reuse kiosk classify_with_llm)
- **GUARD-08**: Auto-whitelisting workflow — staff approves unknown process via kiosk, added to config
- **GUARD-09**: Config version hash to detect out-of-sync whitelist across fleet

## Out of Scope

| Feature | Reason |
|---------|--------|
| ETW real-time process events | COM + SYSTEM privileges + complex Win32 — marginal gain over 60s polling |
| Per-process cryptographic hash verification | Hash DB management across 8 pods + Windows Update = unsustainable |
| Deep packet inspection on flagged ports | Kernel-level DPI is EDR territory, not ops tooling |
| Behavioral analysis (CPU/memory heuristics) | Whitelist is more effective than behavioral analysis for narrow process tree |
| Quarantine mode (suspend instead of kill) | Adds lifecycle complexity, audit log is sufficient evidence |
| DB schema for violation history | In-memory counters + log file sufficient, avoids schema migration |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| GUARD-01 | Phase 102 | Pending |
| GUARD-02 | Phase 102 | Pending |
| GUARD-03 | Phase 102 | Pending |
| GUARD-04 | Phase 101 | Complete |
| GUARD-05 | Phase 101 | Complete |
| GUARD-06 | Phase 102 | Pending |
| PROC-01 | Phase 103 | Pending |
| PROC-02 | Phase 103 | Pending |
| PROC-03 | Phase 103 | Pending |
| PROC-04 | Phase 103 | Pending |
| PROC-05 | Phase 103 | Pending |
| AUTO-01 | Phase 103 | Pending |
| AUTO-02 | Phase 103 | Pending |
| AUTO-03 | Phase 105 | Pending |
| AUTO-04 | Phase 103 | Pending |
| PORT-01 | Phase 105 | Pending |
| PORT-02 | Phase 105 | Pending |
| ALERT-01 | Phase 103 | Pending |
| ALERT-02 | Phase 104 | Pending |
| ALERT-03 | Phase 104 | Pending |
| ALERT-04 | Phase 103 | Pending |
| ALERT-05 | Phase 104 | Pending |
| DEPLOY-01 | Phase 103 | Pending |
| DEPLOY-02 | Phase 104 | Pending |
| DEPLOY-03 | Phase 105 | Pending |

**Coverage:**
- v12.1 requirements: 25 total
- Mapped to phases: 25
- Unmapped: 0

| Phase | Requirements |
|-------|-------------|
| Phase 101 | GUARD-04, GUARD-05 |
| Phase 102 | GUARD-01, GUARD-02, GUARD-03, GUARD-06 |
| Phase 103 | PROC-01, PROC-02, PROC-03, PROC-04, PROC-05, AUTO-01, AUTO-02, AUTO-04, ALERT-01, ALERT-04, DEPLOY-01 |
| Phase 104 | ALERT-02, ALERT-03, ALERT-05, DEPLOY-02 |
| Phase 105 | PORT-01, PORT-02, AUTO-03, DEPLOY-03 |

---
*Requirements defined: 2026-03-21*
*Last updated: 2026-03-21 after roadmap creation — all 25 requirements mapped*
