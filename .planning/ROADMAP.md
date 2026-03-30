# Roadmap: v31.0 Autonomous Survival System

## Milestone Goal

No single system failure can kill the healing brain. Three independent survival layers — pod watchdog, server fleet healer, and external guardian — operate autonomously using a shared Unified MMA Protocol to detect, diagnose, and repair failures without human intervention.

## Phases

- [ ] **Phase 267: Survival Foundation** - Shared types, sentinel coordination, recovery protocol for all 5 existing healers
- [ ] **Phase 268: Unified MMA Protocol** - 5-model OpenRouter roster with fact-checker, dual reasoning, cost guard, per-pod keys
- [ ] **Phase 269: Layer 1 Smart Watchdog** - Binary SHA256 + PE validation, rollback depth tracking, MMA diagnosis in watchdog
- [ ] **Phase 270: Layer 2 Server Fleet Healer** - SSH diagnostic runner, fleet-pattern detection, autonomous Tier 1 repair with canary
- [ ] **Phase 271: Layer 3 External Guardian** - External server health polling, graduated restart, WhatsApp escalation from Bono VPS
- [ ] **Phase 272: Integration & MMA Audit** - Cross-layer coordination smoke test + Unified Protocol v3.1 MMA audit of all v31.0 code

## Phase Details

### Phase 267: Survival Foundation
**Goal**: All 5 existing recovery systems coordinate via shared sentinel protocol and structured types so they cannot fight each other over the same patient
**Depends on**: Nothing (foundation phase)
**Requirements**: SF-01, SF-02, SF-03, SF-04, SF-05
**Success Criteria** (what must be TRUE):
  1. HEAL_IN_PROGRESS sentinel is checked by rc-sentry, RCWatchdog, self_monitor, pod_monitor, and WoL before acting — all 5 systems refuse to act when a sentinel with valid TTL exists
  2. Server grants heal leases with TTL via POST /api/v1/pods/{id}/heal-lease and the healer renews while working — expired lease frees the pod within TTL seconds
  3. Every cross-layer operation log entry carries the same action_id, traceable end-to-end from diagnosis to rollback
  4. SurvivalReport, HealLease, BinaryManifest, DiagnosisContext structs exist in rc-common and compile cleanly across all crates
  5. OTA_DEPLOYING and HEAL_IN_PROGRESS sentinel checks are present in all existing recovery code paths
**Plans**: 3 plans
Plans:
- [ ] 267-01-PLAN.md — Survival types, sentinel protocol, and OpenRouter trait in rc-common
- [ ] 267-02-PLAN.md — Server heal-lease endpoints and LeaseManager
- [ ] 267-03-PLAN.md — Retrofit all 5 recovery systems with sentinel checks
**UI hint**: no

### Phase 268: Unified MMA Protocol
**Goal**: A reusable 5-model MMA protocol is available as a library that any layer can invoke for diagnosis, with cost guardrails, structured findings, and fallback to deterministic rules when OpenRouter is unreachable
**Depends on**: Phase 267
**Requirements**: MP-01, MP-02, MP-03, MP-04, MP-05, MP-06, MP-07, MP-08, MP-09
**Success Criteria** (what must be TRUE):
  1. An MMA session invocation returns structured findings with P0/P1/P2 severity, finding type, affected component, and recommended action — one model acts as fact-checker cross-referencing against standing rules before any action recommendation is finalized
  2. Both non-thinking and thinking model variants are used in the same session — non-thinking for architecture bugs, thinking variants for execution-path bugs
  3. MMA session refuses to start when daily budget is exhausted, and daily spend survives process reboot (persisted to budget_state.json)
  4. When OpenRouter returns 3 consecutive failures, the protocol falls back to a deterministic rule engine and logs the fallback clearly
  5. Per-pod child API keys exist with $10/day caps and sessions tagged training=true during the 30-day training window
**Plans**: TBD
**UI hint**: no

### Phase 269: Layer 1 Smart Watchdog
**Goal**: rc-watchdog validates the rc-agent binary before every launch, auto-rolls back to the previous binary on failure, and invokes the Unified MMA Protocol when it detects a restart loop — all without blocking the main service poll loop
**Depends on**: Phase 268
**Requirements**: SW-01, SW-02, SW-03, SW-04, SW-05, SW-06, SW-07, SW-08, SW-09, SW-10, SW-11, SW-12, SW-13, SW-14
**Success Criteria** (what must be TRUE):
  1. rc-watchdog refuses to launch rc-agent.exe if SHA256 does not match release-manifest.toml, or if PE header is not valid x86_64 Windows PE — the refusal is logged and a survival report is sent to the server
  2. A new binary that fails the health poll within 30 seconds is automatically replaced by rc-agent-prev.exe and the rollback is logged with depth counter; after 3 consecutive rollback failures the watchdog escalates to Layer 2 ("both binaries bad")
  3. When >2 launch failures occur within 10 minutes, a Unified MMA session is triggered from a dedicated async runtime thread — the main service poll loop is not blocked during the MMA call
  4. MAINTENANCE_MODE is auto-cleared after a clean binary + clean health poll sequence — it is never written by the watchdog during its own MMA diagnostic cycle
  5. Staff receives a WhatsApp alert after any rollback event, and MMA_DIAGNOSING sentinel prevents a second concurrent diagnosis session
**Plans**: TBD
**UI hint**: no

### Phase 270: Layer 2 Server Fleet Healer
**Goal**: The server can SSH into dark pods, run structured diagnostic scripts, detect fleet-wide failure patterns, and dispatch autonomous Tier 1 repairs with canary rollout and billing safety enforcement
**Depends on**: Phase 267
**Requirements**: FH-01, FH-02, FH-03, FH-04, FH-05, FH-06, FH-07, FH-08, FH-09, FH-10, FH-11, FH-12
**Success Criteria** (what must be TRUE):
  1. Server can SSH into a pod that is not responding to rc-agent HTTP and return structured diagnostic JSON (tasklist, netstat, Event Log patterns) mapped to a symptom schema
  2. When the same failure pattern appears on 3 or more pods within 5 minutes, a single shared MMA session is launched instead of 8 parallel sessions
  3. Autonomous repair is dispatched only when confidence >= 0.8 AND fix_type is deterministic or config — no autonomous code_change repairs; every SSH command and response is logged to incident_log with action_id
  4. Fixes are applied to Pod 8 first, verified (build_id match AND edge_process_count > 0), then to 3 pods, then remaining — a pod with an active billing session is never restarted
  5. Layer 1 watchdog survival reports are ingested and stored by survival_coordinator, and the new POST /api/v1/pods/{id}/survival-report endpoint returns 200 for authenticated watchdog requests
**Plans**: TBD
**UI hint**: no

### Phase 271: Layer 3 External Guardian
**Goal**: rc-guardian running on Bono VPS monitors the venue server every 60 seconds, detects deadness via 3 consecutive misses, attempts graduated restart, escalates to WhatsApp when unsafe or stuck, and coordinates with James to avoid simultaneous guardian actions
**Depends on**: Phase 267
**Requirements**: EG-01, EG-02, EG-03, EG-04, EG-05, EG-06, EG-07, EG-08, EG-09, EG-10
**Success Criteria** (what must be TRUE):
  1. rc-guardian (new Linux crate) polls /api/v1/health every 60 seconds from Bono VPS and correctly distinguishes dead (connection refused), busy (200 but slow), and unreachable (timeout) — 3 consecutive missed polls declare server dead
  2. Graduated restart executes: soft restart (schtasks /Run /TN StartRCTemp) first, hard restart (taskkill + start) if soft fails, then report-only escalation — billing safety check blocks restart when active_billing_sessions > 0
  3. Staff receives a WhatsApp alert when restart is blocked by active sessions, when hard restart is triggered, or when all restart attempts fail
  4. GUARDIAN_ACTING state is shared via comms-link WS — James and Bono guardians do not act simultaneously on the same server restart
  5. Guardian sends a heartbeat to James via comms-link every 6 hours and on every triggered event, and pm2/systemd keeps rc-guardian alive on the VPS
**Plans**: TBD
**UI hint**: no

### Phase 272: Integration & MMA Audit
**Goal**: All three survival layers coordinate correctly under simulated failure scenarios, and the entire v31.0 codebase passes a Unified Protocol v3.1 MMA audit with zero P1 findings
**Depends on**: Phase 269, Phase 270, Phase 271
**Requirements**: (no new requirements — cross-layer integration gate)
**Success Criteria** (what must be TRUE):
  1. A simulated pod crash loop triggers Layer 1 MMA diagnosis, Layer 1 reports to Layer 2 via survival-report endpoint, and Layer 2 dispatches a canary fix — all three layers log the same action_id throughout the incident
  2. A simulated server outage triggers Layer 3 dead-man detection, graduated restart attempt, and WhatsApp escalation — Layer 2 SSH repair and Layer 3 Guardian restart do not conflict (GUARDIAN_ACTING sentinel prevents double-act)
  3. Unified MMA Protocol audit of all v31.0 code produces zero P1 findings on the final iteration (convergence)
  4. All Unified Protocol v3.1 gates pass: Quality Gate (comms-link run-all.sh), E2E round-trip, Standing Rules compliance, and MMA consensus
**Plans**: TBD
**UI hint**: no

## Coverage Map

| Requirement | Phase |
|-------------|-------|
| SF-01 | 267 |
| SF-02 | 267 |
| SF-03 | 267 |
| SF-04 | 267 |
| SF-05 | 267 |
| MP-01 | 268 |
| MP-02 | 268 |
| MP-03 | 268 |
| MP-04 | 268 |
| MP-05 | 268 |
| MP-06 | 268 |
| MP-07 | 268 |
| MP-08 | 268 |
| MP-09 | 268 |
| SW-01 | 269 |
| SW-02 | 269 |
| SW-03 | 269 |
| SW-04 | 269 |
| SW-05 | 269 |
| SW-06 | 269 |
| SW-07 | 269 |
| SW-08 | 269 |
| SW-09 | 269 |
| SW-10 | 269 |
| SW-11 | 269 |
| SW-12 | 269 |
| SW-13 | 269 |
| SW-14 | 269 |
| FH-01 | 270 |
| FH-02 | 270 |
| FH-03 | 270 |
| FH-04 | 270 |
| FH-05 | 270 |
| FH-06 | 270 |
| FH-07 | 270 |
| FH-08 | 270 |
| FH-09 | 270 |
| FH-10 | 270 |
| FH-11 | 270 |
| FH-12 | 270 |
| EG-01 | 271 |
| EG-02 | 271 |
| EG-03 | 271 |
| EG-04 | 271 |
| EG-05 | 271 |
| EG-06 | 271 |
| EG-07 | 271 |
| EG-08 | 271 |
| EG-09 | 271 |
| EG-10 | 271 |

**Coverage:** 45/45 v1 requirements mapped. No orphans.

## Progress

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 267. Survival Foundation | 0/3 | Planning complete | - |
| 268. Unified MMA Protocol | 0/TBD | Not started | - |
| 269. Layer 1 Smart Watchdog | 0/TBD | Not started | - |
| 270. Layer 2 Server Fleet Healer | 0/TBD | Not started | - |
| 271. Layer 3 External Guardian | 0/TBD | Not started | - |
| 272. Integration & MMA Audit | 0/TBD | Not started | - |

---
*Roadmap created: 2026-03-30*
*Milestone: v31.0 Autonomous Survival System*
*Phase range: 267-272*
*v30.0 ended at Phase 266*
