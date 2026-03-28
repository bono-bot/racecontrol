---
gsd_state_version: 1.0
milestone: v27.0
milestone_name: Workflow Integrity & Compliance Hardening
status: roadmap_ready
stopped_at: null
last_updated: "2026-03-29T00:00:00.000Z"
last_activity: 2026-03-29
progress:
  total_phases: 10
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
  percent: 0
---

## Current Position

Phase: 251 — Database Foundation (not started)
Plan: —
Status: Roadmap ready, awaiting plan-phase
Last activity: 2026-03-29 — Roadmap created for v27.0 (10 phases, 83 requirements)

Progress: [░░░░░░░░░░] 0% (0/10 phases)

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
| 251 | Database Foundation | RESIL-01, RESIL-02, RESIL-03, FSM-09, FSM-10 | Not started |
| 252 | Financial Atomicity Core | FATM-01–06, FATM-12 | Not started |
| 253 | State Machine Hardening | FSM-01–08 | Not started |
| 254 | Security Hardening | SEC-01–10 | Not started |
| 255 | Legal Compliance | LEGAL-01–09 | Not started |
| 256 | Game-Specific Hardening | GAME-01–08 | Not started |
| 257 | Billing Edge Cases | BILL-01–08 | Not started |
| 258 | Staff Controls & Deployment Safety | STAFF-01–05, DEPLOY-01–05 | Not started |
| 259 | Coupon & Discount System | FATM-07–11 | Not started |
| 260 | Notifications, Resilience & UX | UX-01–08, RESIL-04–08 | Not started |

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

## Session Continuity

Next action: `/gsd:plan-phase 251` — Database Foundation
- RESIL-01: SQLite WAL mode + busy_timeout
- RESIL-02: Staggered timer writes by pod index
- FSM-09: Billing timer persisted every 60s
- FSM-10: Orphaned session detection on startup
- RESIL-03: Orphaned session background job (5-min interval)

Ship gate reminder (Unified Protocol v3.1):
1. Quality Gate: `cd comms-link && COMMS_PSK="..." bash test/run-all.sh`
2. E2E: live exec + chain + health round-trip (REALTIME mode)
3. Standing Rules: auto-push, Bono synced, watchdog, rules categorized
4. Multi-Model AI Audit: all consensus P1s fixed, P2s triaged
