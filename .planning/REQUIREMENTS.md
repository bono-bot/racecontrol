# Requirements: v17.1 Watchdog-to-AI Migration

**Defined:** 2026-03-22
**Core Value:** Recovery systems use intelligent AI-driven decisions instead of blind restart loops — detect, remember, escalate, never cause more problems than they solve.

## v17.1 Requirements

### RC-Sentry Migration

- [x] **SENT-01**: rc-sentry checks pattern memory before restarting rc-agent — if same crash pattern seen 3+ times in 10 min, escalate to AI instead of restarting
- [x] **SENT-02**: rc-sentry queries Ollama for unknown crash patterns before blind restart
- [x] **SENT-03**: rc-sentry logs every restart decision to activity log with reason, pattern match, and outcome
- [x] **SENT-04**: rc-sentry distinguishes graceful restart (sentinel file) from real crash — no escalation on graceful

### Pod Monitor Migration

- [x] **PMON-01**: pod_monitor checks billing_active before triggering WoL/restart — never wake a deliberately offline pod during maintenance
- [x] **PMON-02**: pod_monitor merges with pod_healer into single recovery authority — no separate restart logic
- [x] **PMON-03**: pod recovery uses graduated response: 1st failure → wait 30s, 2nd → Tier 1 fix, 3rd → AI escalation, 4th+ → alert staff

### James Watchdog Migration

- [x] **JWAT-01**: Replace james_watchdog.ps1 with a Rust-based monitor using AI debugger pattern memory
- [x] **JWAT-02**: James monitor checks Ollama, Claude Code, comms-link, webterm with graduated response (not blind restart)
- [x] **JWAT-03**: James monitor alerts Bono via comms-link WS on repeated failures instead of silent restart

### Recovery Consolidation

- [x] **CONS-01**: Single recovery authority per machine — no two systems can restart the same process
- [x] **CONS-02**: Recovery decision log — every restart/kill/wake decision logged with who triggered it and why
- [x] **CONS-03**: Anti-cascade guard — if 3+ recovery actions fire within 60s across different systems, pause all and alert staff

## Future Requirements

- **SENT-05**: rc-sentry learns from successful fixes and applies them faster next time
- **PMON-04**: pod_monitor predicts failures from metric trends (memory pressure, disk fill rate)

## Out of Scope

| Feature | Reason |
|---------|--------|
| Browser watchdog replacement | Already done in v17.0 (server healer + ForceRelaunchBrowser) |
| AI debugger Tier 3/4 execution | Already done in v17.0 (Phase 140 — AIACT whitelist) |
| Full AI autonomy (no human alerts) | Too risky — staff must be in the loop for repeated failures |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| CONS-01 | Phase 159 | Complete (159-01) |
| CONS-02 | Phase 159 | Complete (159-01) |
| CONS-03 | Phase 159 | Complete |
| SENT-01 | Phase 160 | Complete |
| SENT-02 | Phase 160 | Complete |
| SENT-03 | Phase 160 | Complete |
| SENT-04 | Phase 160 | Complete |
| PMON-01 | Phase 161 | Complete |
| PMON-02 | Phase 161 | Complete |
| PMON-03 | Phase 161 | Complete |
| JWAT-01 | Phase 162 | Complete |
| JWAT-02 | Phase 162 | Complete |
| JWAT-03 | Phase 162 | Complete |

**Coverage:**
- v17.1 requirements: 13 total
- Mapped to phases: 13
- Unmapped: 0

---
*Requirements defined: 2026-03-22*
*Last updated: 2026-03-22 after roadmap creation (phases 159-162 assigned)*
