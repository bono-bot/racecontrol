---
gsd_state_version: 1.0
milestone: v3.0
milestone_name: Billing & Point of Sale
status: executing
stopped_at: "Phase 1 complete — all 5 SYNC requirements met"
last_updated: "2026-03-14"
last_activity: 2026-03-14 — Phase 1 Plan 01-01 complete (billing_events sync + 3 extra columns)
progress:
  total_phases: 5
  completed_phases: 1
  total_plans: 10
  completed_plans: 1
  percent: 10
---

# Project State

## Project Reference

See: .planning/billing-pos/PROJECT.md (created 2026-03-14)

**Core value:** Every rupee earned is tracked, every customer gets a receipt, and Uday can see venue revenue in real-time from anywhere.
**Current focus:** Phase 1 complete. Next: Phase 4 (PWA Session Results & Receipt) per execution order.

## Current Position

Phase: 1 of 5 — Billing Cloud Sync (COMPLETE)
Plan: 01-01 complete (only plan in phase)
Status: Phase 1 complete. All 5 SYNC requirements met.
Last activity: 2026-03-14 — Plan 01-01 complete (billing_events sync, 3 extra billing_sessions columns, created_at index)

Progress: [##        ] 10%

## Key Research Finding (Phase 1)

billing_sessions and wallet_transactions were ALREADY synced in cloud_sync.rs. Only billing_events was missing (~50 lines of code). SYNC-01, SYNC-02, SYNC-04, SYNC-05 were already met. Only SYNC-03 needed implementation. Phase 1 completed with 1 plan.

## Performance Metrics

**Velocity:**
- Total plans completed: 1
- Total execution time: ~8 minutes

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 01-billing-cloud-sync | 1/1 | 8min | 8min |

## Accumulated Context

### Decisions

- billing_sessions stays LOCAL-authoritative — cloud gets read-only copy via sync
- Sync strategy: billing_sessions and wallet_transactions already in push payload; add billing_events to collect_push_payload() following wallet_transactions pattern
- IMPORTANT: Do NOT add billing tables to SYNC_TABLES constant — that controls the PULL path (cloud -> venue). Billing data only flows venue -> cloud.
- WhatsApp receipt delivered via Bono (comms-link) — James sends webhook, Bono calls Evolution API
- racingpoint-admin (port 3200) is the target for cloud dashboard pages (already deployed at dashboard.racingpoint.cloud)
- payment_method column needs to be added to billing_sessions table (currently missing)
- POST /billing/{id}/refund already exists in rc-core — needs audit_log write wired in
- Phase execution order: 1 -> 4 -> 5 -> 2 -> 3 (cloud sync first, then PWA/kiosk work in parallel while cloud API builds)
- Plan 01-01: INSERT OR IGNORE for billing_events (immutable lifecycle records)
- Plan 01-01: COALESCE for 3 new billing_sessions columns in ON CONFLICT (backward-compat with old venue code)
- Plan 01-01: Tests use in-memory SQLite with inline schema (no AppState construction)

### Existing Infrastructure (do NOT rebuild)

- BillingManager + BillingTimer: complete lifecycle management in rc-core/billing.rs
- compute_session_cost(): two-tier retroactive pricing, fully tested
- Dynamic pricing rules: peak/off-peak multipliers and group discounts
- wallet module: credit/debit + double-entry journal entries
- POST /billing/{id}/refund: exists, just needs audit_log
- GET /billing/sessions/{id}: exists, needs /detail extension
- cloud_sync.rs: relay + HTTP fallback infrastructure, ALL 3 billing tables now pushed
- billing_events table: all lifecycle events already logged (started, paused, resumed, warnings, expired)

### Pending Todos

None — Phase 1 complete.

### Blockers/Concerns

- Need Bono's cooperation for WhatsApp receipt (Phase 4) — send James->Bono webhook spec before planning Phase 4
- Need to verify cloud API schema before Phase 2 planning — check racingpoint-api-gateway for existing auth patterns
- payment_method DB migration needed (Phase 5) — ensure idempotent SQL migration

## Session Continuity

Last session: 2026-03-14
Stopped at: Phase 1 complete
Resume file: .planning/billing-pos/phases/01-billing-cloud-sync/01-01-SUMMARY.md
