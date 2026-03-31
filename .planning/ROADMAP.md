# Roadmap: v32.0 Autonomous Meshed Intelligence

## Milestone Goal

Close all action loops in Meshed Intelligence so the venue self-heals end-to-end: anomaly detected -> diagnosed -> fixed -> verified -> permanent fix encoded in KB -> cascaded to fleet -> never debug the same issue twice. Event-driven pipeline (not polling) with runaway prevention guardrails.

## Phases

**Phase Numbering:** Continues from v31.0 (ended at Phase 272). Start at 273.

**Parallelism Map:**
- Phase 273 (Foundation) and Phase 274 (Escalation) run SEQUENTIALLY first
- Phases 275, 276, 277 run IN PARALLEL after 273+274 complete
- Phase 278 (KB Hardening) runs after 273 + at least one of {275, 276, 277}
- Phase 279 (Weekly Report & Audit) runs last after all others complete

```
273 ──> 274 ──┬──> 275 (Game) ────────┐
              ├──> 276 (Pred+CX) ─────┤──> 278 (KB) ──> 279 (Report+Audit)
              └──> 277 (Rev+Rep) ─────┘
```

- [ ] **Phase 273: Event Pipeline & Safety Foundation** - Event bus, proactive pipeline, blast radius limiter, circuit breakers, idempotency
- [ ] **Phase 274: WhatsApp Escalation** - Tier 5 WhatsApp alerts via Bono VPS Evolution API with dedup and fallback
- [ ] **Phase 275: Autonomous Game Launch Fix** - Game launch failure -> diagnosis -> fix -> retry -> KB encode -> fleet cascade
- [ ] **Phase 276: Predictive Alerts & Experience Scoring** - Predictive alerts fed into tier engine + per-pod experience scoring with auto-flag/remove
- [ ] **Phase 277: Revenue Protection & Model Reputation** - Billing/game mismatch detection + model accuracy auto-demotion/promotion
- [ ] **Phase 278: KB Hardening Pipeline** - Promotion ladder: Observed -> Shadow -> Canary -> Quorum -> Deterministic Rule
- [ ] **Phase 279: Weekly Report & Integration Audit** - Weekly KPI report to Uday via WhatsApp + full MMA audit of v32.0

## Phase Details

### Phase 273: Event Pipeline & Safety Foundation
**Goal**: All anomaly detection and fix application flows through an event-driven pipeline with safety guardrails that prevent runaway autonomous actions
**Depends on**: Nothing (foundation phase for v32.0)
**Requirements**: PRO-01, PRO-02, PRO-03, PRO-04, PRO-05, PRO-06, SAFE-01, SAFE-02, SAFE-03
**Success Criteria** (what must be TRUE):
  1. When a diagnostic scan detects an anomaly crossing threshold, a FleetEvent is emitted within 1 second (not waiting for next 5-min cycle) and the tier engine receives it via mpsc channel
  2. Tier 1-3 fixes are applied automatically without human approval, and verification runs within 30 seconds of application confirming fix or escalating
  3. Every resolved issue (all tiers) is recorded in KB with problem signature, fix action, verification result, and timestamp -- KB lookup runs before any AI model call
  4. No more than 2 of 10 nodes are under simultaneous autonomous fix at any time (blast radius limiter enforced via DashMap + RAII FixGuard)
  5. Per-action circuit breaker trips at 40% fail rate with 2-minute cooldown, and every executor action carries an idempotency key (node + rule_version + incident_fingerprint)
**Plans**: 4 plans
Plans:
- [x] 273-01-PLAN.md — Event bus & FleetEvent types + broadcast wiring
- [x] 273-02-PLAN.md — Safety guardrails (blast radius, circuit breaker, idempotency)
- [x] 273-03-PLAN.md — KB-first gate & solution recording
- [ ] 273-04-PLAN.md — Immediate fix & 30-second verification loop
**UI hint**: no

### Phase 274: WhatsApp Escalation
**Goal**: Tier 5 escalations and critical alerts reach Uday's WhatsApp within 30 seconds via Bono VPS Evolution API, with deduplication preventing alert fatigue
**Depends on**: Phase 273
**Requirements**: ESC-01, ESC-02, ESC-03, ESC-04
**Success Criteria** (what must be TRUE):
  1. A Tier 5 escalation sends a WhatsApp message to Uday containing severity, pod/service ID, issue summary, AI actions tried, and a clickable dashboard link
  2. The same incident ID is suppressed for 30 minutes after first alert -- no duplicate messages for the same ongoing issue
  3. If WhatsApp send fails (Evolution API down, VPS unreachable), the alert falls back to comms-link INBOX.md entry within 60 seconds
  4. WhatsApp messages are routed through Bono VPS Evolution API (POST /message/sendText/:instance) with apikey auth -- never direct from venue
**Plans**: TBD
**UI hint**: no

### Phase 275: Autonomous Game Launch Fix
**Goal**: A customer whose game fails to launch sees recovery within 60 seconds -- diagnosis, fix, retry, and if deterministic fix works, the solution is encoded in KB and cascaded to all pods
**Depends on**: Phase 273, Phase 274
**Requirements**: GAME-01, GAME-02, GAME-03, GAME-04, GAME-05
**Success Criteria** (what must be TRUE):
  1. A game launch failure triggers immediate diagnosis and the customer sees the game recover (or a clear escalation message) within 60 seconds
  2. After a fix is applied, the system auto-retries launch up to 2 times with clean state reset between attempts
  3. If deterministic fix fails after retries, the system escalates to Tier 3/4 MMA for AI diagnosis without further customer wait
  4. Every successful game launch fix is encoded in KB with problem signature, and cascaded via mesh gossip to all pods + POS for fleet pre-immunization
**Plans**: TBD
**UI hint**: no

### Phase 276: Predictive Alerts & Experience Scoring
**Goal**: Predictive alerts drive proactive action (not just logging), and every pod has a live experience score that auto-flags degraded pods and removes critically broken ones from rotation
**Depends on**: Phase 273, Phase 274
**Requirements**: PRED-10, PRED-11, PRED-12, CX-05, CX-06, CX-07, CX-08
**Success Criteria** (what must be TRUE):
  1. Predictive alerts are converted to FleetEvent and processed by the tier engine -- high-severity predictions trigger immediate pre-emptive fix, low-severity defer to session gap
  2. Predictive alerts that lead to successful pre-emptive fixes are recorded in KB with the prediction-to-fix mapping
  3. Each pod has an experience score (0-100) calculated every 5 minutes from diagnostic scan data, visible at /api/v1/fleet/health as experience_score per pod
  4. A pod scoring below 80% is auto-flagged for maintenance in fleet status; a pod scoring below 50% is auto-removed from customer rotation and triggers a WhatsApp alert to Uday
**Plans**: TBD
**UI hint**: no

### Phase 277: Revenue Protection & Model Reputation
**Goal**: Billing-game mismatches are caught and resolved automatically, and AI models that consistently fail are removed from the MMA roster while high-performers are promoted
**Depends on**: Phase 273, Phase 274
**Requirements**: REV-01, REV-02, REV-03, REP-01, REP-02
**Success Criteria** (what must be TRUE):
  1. A game running without an active billing session triggers an immediate staff alert (revenue leak detected)
  2. A billing session that ends while a game is still active triggers a grace period followed by auto-end of the game process
  3. Pod recovery during peak hours (12-22 IST) is prioritized over off-peak recovery in the tier engine queue
  4. Models with accuracy below 30% across 5+ runs are automatically removed from the MMA roster; models with accuracy above 90% across 10+ runs are promoted to higher priority
**Plans**: TBD
**UI hint**: no

### Phase 278: KB Hardening Pipeline
**Goal**: Fixes that prove themselves across multiple pods and contexts automatically graduate from observed anomalies to deterministic Tier 1 rules that cost $0 and apply instantly forever
**Depends on**: Phase 273, and at least one of {Phase 275, Phase 276, Phase 277} (needs real fix data flowing)
**Requirements**: KB-01, KB-02, KB-03, KB-04, KB-05
**Success Criteria** (what must be TRUE):
  1. A newly recorded fix enters the promotion ladder at Observed status and progresses through Shadow -> Canary -> Quorum -> Deterministic Rule based on success criteria
  2. Shadow mode runs the candidate fix alongside the existing pipeline for 1 week or 25 applications (whichever comes first), logging only -- no customer impact
  3. Canary deploys the candidate fix on Pod 8 first and verifies success before any other pod receives it
  4. After 3+ successes across 2+ different pods, the fix is promoted to Tier 1 as a typed Rule struct with matchers, actions, verifier, and TTL -- applied instantly at $0 cost
**Plans**: TBD
**UI hint**: no

### Phase 279: Weekly Report & Integration Audit
**Goal**: Uday receives a weekly intelligence report summarizing fleet health and AI effectiveness, and the entire v32.0 codebase passes MMA audit with zero P1 findings
**Depends on**: Phase 274, Phase 275, Phase 276, Phase 277, Phase 278
**Requirements**: RPT-01, RPT-02, RPT-03
**Success Criteria** (what must be TRUE):
  1. Every Sunday at midnight IST, a weekly report is auto-generated containing: uptime %, auto-resolution rate, MTTR, top 3 issues, AI budget spent, and KB growth metrics
  2. The report is sent to Uday via WhatsApp as a text summary with an attached chart image (via Evolution API /message/sendMedia)
  3. Unified MMA Protocol audit of all v32.0 code produces zero P1 findings on the final iteration (convergence)
  4. All Unified Protocol v3.1 gates pass: Quality Gate, E2E round-trip, Standing Rules compliance, and MMA consensus
**Plans**: TBD
**UI hint**: no

## Coverage Map

| Requirement | Phase |
|-------------|-------|
| PRO-01 | 273 |
| PRO-02 | 273 |
| PRO-03 | 273 |
| PRO-04 | 273 |
| PRO-05 | 273 |
| PRO-06 | 273 |
| SAFE-01 | 273 |
| SAFE-02 | 273 |
| SAFE-03 | 273 |
| ESC-01 | 274 |
| ESC-02 | 274 |
| ESC-03 | 274 |
| ESC-04 | 274 |
| GAME-01 | 275 |
| GAME-02 | 275 |
| GAME-03 | 275 |
| GAME-04 | 275 |
| GAME-05 | 275 |
| PRED-10 | 276 |
| PRED-11 | 276 |
| PRED-12 | 276 |
| CX-05 | 276 |
| CX-06 | 276 |
| CX-07 | 276 |
| CX-08 | 276 |
| REV-01 | 277 |
| REV-02 | 277 |
| REV-03 | 277 |
| REP-01 | 277 |
| REP-02 | 277 |
| KB-01 | 278 |
| KB-02 | 278 |
| KB-03 | 278 |
| KB-04 | 278 |
| KB-05 | 278 |
| RPT-01 | 279 |
| RPT-02 | 279 |
| RPT-03 | 279 |

**Coverage:** 38/38 v1 requirements mapped. No orphans.

## Progress

**Execution Order:**
- Sequential: 273 -> 274
- Parallel group: {275, 276, 277} (all three simultaneously after 274)
- Sequential: 278 (after 273 + at least one parallel phase)
- Sequential: 279 (after all others)

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 273. Event Pipeline & Safety Foundation | 3/4 | In Progress|  |
| 274. WhatsApp Escalation | 0/TBD | Not started | - |
| 275. Autonomous Game Launch Fix | 0/TBD | Not started | - |
| 276. Predictive Alerts & Experience Scoring | 0/TBD | Not started | - |
| 277. Revenue Protection & Model Reputation | 0/TBD | Not started | - |
| 278. KB Hardening Pipeline | 0/TBD | Not started | - |
| 279. Weekly Report & Integration Audit | 0/TBD | Not started | - |

---
*Roadmap created: 2026-04-01*
*Milestone: v32.0 Autonomous Meshed Intelligence*
*Phase range: 273-279*
*v31.0 ended at Phase 272*
