# Requirements: v32.0 Autonomous Meshed Intelligence

**Defined:** 2026-04-01
**Core Value:** Close all action loops — diagnose -> fix -> permanent fix -> cascade -> never debug the same issue twice

## Proactive Immediate Resolution (PRO)

- [ ] **PRO-01**: Anomaly detection triggers immediate diagnosis — no waiting for next 5-min scan cycle
- [ ] **PRO-02**: Diagnosis result triggers immediate fix application — no human approval queue for Tier 1-3
- [ ] **PRO-03**: Fix verification runs immediately after application (< 30 seconds)
- [x] **PRO-04**: Every resolved issue (all tiers) recorded in KB with problem signature, fix action, verification result, and timestamp
- [x] **PRO-05**: KB lookup happens BEFORE any AI model call — if problem was solved before, apply instantly ($0)
- [ ] **PRO-06**: Event-driven pipeline (not polling) — diagnostic engine emits events the moment anomaly crosses threshold

## Autonomous Game Launch Fix (GAME)

- [ ] **GAME-01**: Game launch failure triggers immediate diagnosis + fix + retry — customer sees recovery within 60 seconds
- [ ] **GAME-02**: After fix applied, auto-retry launch (max 2 retries with clean state reset between)
- [ ] **GAME-03**: If deterministic fix fails, escalate to Tier 3/4 MMA for AI diagnosis
- [ ] **GAME-04**: Successful fix encoded in KB with problem signature for future instant replay
- [ ] **GAME-05**: Fix cascaded via mesh gossip to all pods + POS (fleet pre-immunization)

## Predictive Alert Pipeline (PRED)

- [ ] **PRED-10**: Predictive alerts converted to FleetEvent and fed into tier engine (not just logged)
- [ ] **PRED-11**: Tier engine acts on predictive alerts immediately — fix now if severity warrants, defer to session gap only for low-severity
- [ ] **PRED-12**: Predictive alerts that lead to successful pre-emptive fixes recorded in KB

## Experience Scoring (CX)

- [ ] **CX-05**: Experience score calculated per pod every 5 minutes from diagnostic scan data
- [ ] **CX-06**: Score fed to fleet health API (/api/v1/fleet/health includes experience_score per pod)
- [ ] **CX-07**: Score < 80% auto-flags pod for maintenance in fleet status
- [ ] **CX-08**: Score < 50% auto-removes pod from rotation + WhatsApp alert to Uday

## Tier 5 WhatsApp Escalation (ESC)

- [ ] **ESC-01**: Tier 5 sends WhatsApp alert to Uday via Bono VPS Evolution API
- [ ] **ESC-02**: Alert includes severity, pod/service, issue summary, AI actions tried, dashboard link
- [ ] **ESC-03**: Deduplication — same incident ID suppressed for 30 minutes
- [ ] **ESC-04**: Fallback to comms-link INBOX.md if WhatsApp send fails

## KB Hardening Pipeline (KB)

- [ ] **KB-01**: Promotion ladder: Observed -> Shadow -> Canary -> Quorum -> Deterministic Rule
- [ ] **KB-02**: Shadow mode — new rule executes alongside but logs only for 1 week or 25 applications
- [ ] **KB-03**: Canary — apply on Pod 8 first, verify before fleet
- [ ] **KB-04**: Quorum — 3+ successes across 2+ pods triggers promotion to Tier 1
- [ ] **KB-05**: Promoted rules stored as typed Rule structs with matchers/actions/verifier/TTL

## Model Reputation (REP)

- [ ] **REP-01**: Models with accuracy < 30% across 5+ runs auto-removed from MMA roster
- [ ] **REP-02**: Models with accuracy > 90% across 10+ runs promoted to higher priority

## Revenue Protection (REV)

- [ ] **REV-01**: Detect game running without active billing session -> alert staff
- [ ] **REV-02**: Detect billing session ended but game still active -> grace period -> auto-end game
- [ ] **REV-03**: Pod down during peak hours (12-22 IST) -> prioritize recovery

## Weekly Fleet Report (RPT)

- [ ] **RPT-01**: Weekly report generated every Sunday midnight with 5-8 KPIs
- [ ] **RPT-02**: Report includes: uptime %, auto-resolution rate, MTTR, top 3 issues, budget spent, KB growth
- [ ] **RPT-03**: Report sent to Uday via WhatsApp (text summary + chart image)

## Runaway Prevention (SAFE)

- [ ] **SAFE-01**: Blast radius limiter — max 2 of 10 nodes under simultaneous autonomous fix
- [ ] **SAFE-02**: Per-action circuit breaker — 40% fail rate -> open -> 2-min cooldown
- [ ] **SAFE-03**: Idempotency keys on every executor action (node + rule_version + incident_fingerprint)

## Traceability

| REQ | Phase | Status |
|-----|-------|--------|
| PRO-01 | Phase 273 | Pending |
| PRO-02 | Phase 273 | Pending |
| PRO-03 | Phase 273 | Pending |
| PRO-04 | Phase 273 | Complete |
| PRO-05 | Phase 273 | Complete |
| PRO-06 | Phase 273 | Pending |
| SAFE-01 | Phase 273 | Pending |
| SAFE-02 | Phase 273 | Pending |
| SAFE-03 | Phase 273 | Pending |
| ESC-01 | Phase 274 | Pending |
| ESC-02 | Phase 274 | Pending |
| ESC-03 | Phase 274 | Pending |
| ESC-04 | Phase 274 | Pending |
| GAME-01 | Phase 275 | Pending |
| GAME-02 | Phase 275 | Pending |
| GAME-03 | Phase 275 | Pending |
| GAME-04 | Phase 275 | Pending |
| GAME-05 | Phase 275 | Pending |
| PRED-10 | Phase 276 | Pending |
| PRED-11 | Phase 276 | Pending |
| PRED-12 | Phase 276 | Pending |
| CX-05 | Phase 276 | Pending |
| CX-06 | Phase 276 | Pending |
| CX-07 | Phase 276 | Pending |
| CX-08 | Phase 276 | Pending |
| REV-01 | Phase 277 | Pending |
| REV-02 | Phase 277 | Pending |
| REV-03 | Phase 277 | Pending |
| REP-01 | Phase 277 | Pending |
| REP-02 | Phase 277 | Pending |
| KB-01 | Phase 278 | Pending |
| KB-02 | Phase 278 | Pending |
| KB-03 | Phase 278 | Pending |
| KB-04 | Phase 278 | Pending |
| KB-05 | Phase 278 | Pending |
| RPT-01 | Phase 279 | Pending |
| RPT-02 | Phase 279 | Pending |
| RPT-03 | Phase 279 | Pending |

## Future Requirements (deferred)

- NIGHT-05: Night cycle MMA diagnostic on lingering issues
- NIGHT-06: Auto-apply fleet-verified fixes requiring restart
- NIGHT-07: Morning readiness report to Uday via WhatsApp

## Out of Scope

- Multi-venue KB sync (Section 7.5 of MESHED-INTELLIGENCE.md) — requires second venue
- Competitive intelligence benchmarking (Section 7.6) — marketing, not operations
- ML-based predictive maintenance — threshold-based is sufficient for 8 pods
