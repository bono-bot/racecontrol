---
gsd_state_version: 1.0
milestone: v26.1
milestone_name: Meshed Intelligence
status: executing
stopped_at: Completed 253-01-PLAN.md
last_updated: "2026-03-28T21:12:40.954Z"
last_activity: 2026-03-28
progress:
  total_phases: 205
  completed_phases: 149
  total_plans: 361
  completed_plans: 354
  percent: 98
---

## Current Position

Phase: 253 (state-machine-hardening) — EXECUTING
Plan: 2 of 3
Status: Ready to execute
Last activity: 2026-03-28

Progress: [██████████] 98% (349/355 plans)

## Project Reference

**Milestone:** v27.0 Workflow Integrity & Compliance Hardening
**Core value:** Every customer interaction — from registration to refund — is atomic, auditable, safe, and legally compliant
**Phase range:** 251–260
**Roadmap:** .planning/ROADMAP-v27.md
**Requirements:** .planning/REQUIREMENTS.md (83 requirements, 10 categories)

See: .planning/PROJECT.md (project context)
See: UNIFIED-PROTOCOL.md (operations protocol v3.1)
See: .planning/ROADMAP-v27.md (this milestone's roadmap)

## Performance Metrics

- Requirements defined: 83
- Phases planned: 10
- Plans written: 0
- Plans complete: 0
- Ship gate status: Not started

## Phase Index

| # | Phase | Requirements | Status |
|---|-------|-------------|--------|
| 251 | Database Foundation | RESIL-01, RESIL-02, RESIL-03, FSM-09, FSM-10 | Plan 01 DONE, Plan 02 pending |
| 252 | Financial Atomicity Core | FATM-01–06, FATM-12 | COMPLETE (3/3 plans) |
| 253 | State Machine Hardening | FSM-01–08 | Not started |
| 254 | Security Hardening | SEC-01–10 | Not started |
| 255 | Legal Compliance | LEGAL-01–09 | Not started |
| 256 | Game-Specific Hardening | GAME-01–08 | Not started |
| 257 | Billing Edge Cases | BILL-01–08 | Not started |
| 258 | Staff Controls & Deployment Safety | STAFF-01–05, DEPLOY-01–05 | Not started |
| 259 | Coupon & Discount System | FATM-07–11 | Not started |
| 260 | Notifications, Resilience & UX | UX-01–08, RESIL-04–08 | Not started |
| Phase 251-database-foundation P01 | 15min | 2 tasks | 3 files |
| Phase 251 P02 | 20 | 2 tasks | 2 files |
| Phase 252 P02 | 20 | 1 tasks | 1 files |
| Phase 252 P01 | 45 | 2 tasks | 4 files |
| Phase 252-financial-atomicity-core P03 | 15 | 1 tasks | 3 files |
| Phase 253-state-machine-hardening P01 | 35 | 2 tasks | 3 files |

## Accumulated Context

### Key Architectural Decisions (from MMA audit that produced requirements)

- **WAL mode + staggered writes first** (Phase 251): All financial transactions require a stable DB layer. Phase 251 is the unblocking dependency for all other phases.
- **FATM split across two phases** (252 + 259): Core billing atomicity (FATM-01–06, FATM-12) is foundational; coupon/extension system (FATM-07–11) builds on top of it and can ship independently.
- **FSM depends on FATM** (Phase 253 after 252): Cross-FSM invariant guards (billing=active requires game≠Idle) need atomic billing start to be reliable first.
- **Security before Legal** (254 before 255): RBAC gates the legal workflow endpoints (waiver signing, minor consent). Phase 254 must ship first.
- **RwLock across .await is banned** (from standing rules): All lock acquisitions must snapshot then drop before any async call. This affects the WS broadcast path in fleet health.
- **Requirement count note**: REQUIREMENTS.md header says 72 but actual count is 83 (10 categories, counts verified line-by-line). Traceability updated to reflect 83.

### Open Issues Inherited from v26.0

- Server .23 Tailscale stuck in NoState — non-blocking
- Pod 3/6 spontaneous reboots (2026-03-22) — under investigation
- BUG: Server restart with fresh DB leaves pods table empty (auto-seed needed)
- Server schtasks (StartRCTemp, StartRCDirect) silently fail to start racecontrol

### Deferred (Out of Scope for v27.0)

- SQLite → PostgreSQL migration
- Multi-venue wallet sharing
- Real-time voice chat
- Mobile native app
- Full i18n/l10n

## Decisions (Phase 251)

- WAL verification uses fail-fast bail! at init_pool — server refuses to start if WAL mode fails (RESIL-01)
- Two coexisting billing sync loops: 5s for dashboard driving_seconds, 60s staggered for crash-recovery elapsed_seconds (RESIL-02, FSM-09)
- Stagger formula (N*7)%60 spreads 8 pods across 56 distinct seconds with no collisions
- COALESCE(elapsed_seconds, driving_seconds) recovery ensures old sessions recover correctly
- WhatsApp alerts use whatsapp_alerter::send_whatsapp gated on config.alerting.enabled (FSM-10, RESIL-03)
- Background orphan task has 300s initial delay to avoid double-alerting sessions caught by startup scan

## Decisions (Phase 252)

- std::sync::OnceLock used for reconciliation status instead of once_cell — avoids new dependency (FATM-12)
- Reconciliation status stored in module-level atomics (not AppState) — diagnostic-only, no state management needed (FATM-12)
- HAVING ABS(balance - computed) > 0 LIMIT 100 caps query cost while catching all meaningful drift (FATM-12)
- 60s initial delay for reconciliation job (orphan detection uses 300s; reconciliation is less urgent) (FATM-12)

## Session Continuity

Stopped at: Completed 253-01-PLAN.md
Next action: Phase 252 complete (FATM-01–06, FATM-12) — proceed to Phase 253 (State Machine Hardening, FSM-01–08)

- RESIL-01: DONE (WAL mode verification — 08acee0c)
- RESIL-02: DONE (Staggered timer writes by pod index — 6babdd40)
- FSM-09: DONE (Billing timer persisted every 60s — 6babdd40)
- FSM-10: DONE (Orphaned session detection on startup — a86f4710)
- RESIL-03: DONE (Background orphan detection job — 9ef6116e)
- FATM-01: DONE (Atomic billing start with single DB transaction — 252-01)
- FATM-02: DONE (Idempotency keys on money-moving endpoints — 252-01)
- FATM-03: DONE (debit_in_tx/credit_in_tx with wallet locking — 252-01)
- FATM-04: DONE (CAS session finalization — 252-02)
- FATM-05: DONE (Tier alignment in compute_session_cost — 252-02)
- FATM-06: DONE (Unified compute_refund() — 252-02)
- FATM-12: DONE (Background reconciliation job — 61c73467)

Ship gate reminder (Unified Protocol v3.1):

1. Quality Gate: `cd comms-link && COMMS_PSK="..." bash test/run-all.sh`
2. E2E: live exec + chain + health round-trip (REALTIME mode)
3. Standing Rules: auto-push, Bono synced, watchdog, rules categorized
4. Multi-Model AI Audit: all consensus P1s fixed, P2s triaged
