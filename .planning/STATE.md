---
gsd_state_version: 1.0
milestone: v26.1
milestone_name: Meshed Intelligence
status: verifying
stopped_at: Completed 251-02-PLAN.md
last_updated: "2026-03-28T19:32:05.291Z"
last_activity: 2026-03-28
progress:
  total_phases: 205
  completed_phases: 148
  total_plans: 355
  completed_plans: 350
  percent: 98
---

## Current Position

Phase: 252
Plan: Not started
Status: Phase complete — ready for verification
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
| 252 | Financial Atomicity Core | FATM-01–06, FATM-12 | Not started |
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

## Session Continuity

Stopped at: Completed 251-02-PLAN.md
Next action: Phase 251 complete — proceed to Phase 252 (Financial Atomicity Core)

- RESIL-01: DONE (WAL mode verification — 08acee0c)
- RESIL-02: DONE (Staggered timer writes by pod index — 6babdd40)
- FSM-09: DONE (Billing timer persisted every 60s — 6babdd40)
- FSM-10: DONE (Orphaned session detection on startup — a86f4710)
- RESIL-03: DONE (Background orphan detection job — 9ef6116e)

Ship gate reminder (Unified Protocol v3.1):

1. Quality Gate: `cd comms-link && COMMS_PSK="..." bash test/run-all.sh`
2. E2E: live exec + chain + health round-trip (REALTIME mode)
3. Standing Rules: auto-push, Bono synced, watchdog, rules categorized
4. Multi-Model AI Audit: all consensus P1s fixed, P2s triaged
