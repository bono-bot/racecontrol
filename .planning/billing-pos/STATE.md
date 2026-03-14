---
gsd_state_version: 1.0
milestone: v3.0
milestone_name: Billing & Point of Sale
status: in_progress
stopped_at: "Phase 1 planned — ready to execute"
last_updated: "2026-03-14"
last_activity: 2026-03-14 — Phase 1 planned (01-01-PLAN.md created)
progress:
  total_phases: 5
  completed_phases: 0
  total_plans: 10
  completed_plans: 0
  percent: 0
---

# Project State

## Project Reference

See: .planning/billing-pos/PROJECT.md (created 2026-03-14)

**Core value:** Every rupee earned is tracked, every customer gets a receipt, and Uday can see venue revenue in real-time from anywhere.
**Current focus:** Phase 1 — Billing Cloud Sync (1 plan, wave 1)

## Current Position

Phase: 1 — Billing Cloud Sync
Plan: 01-01 (not started)
Status: Phase 1 planned. Ready to execute.
Last activity: 2026-03-14 — Phase 1 planned with 1 plan (research showed 90% already implemented, only billing_events sync + 3 extra columns needed)

Progress: [          ] 0%

## Key Research Finding (Phase 1)

billing_sessions and wallet_transactions are ALREADY synced in cloud_sync.rs. Only billing_events is missing (~50 lines of code). SYNC-01, SYNC-02, SYNC-04, SYNC-05 are already met. Only SYNC-03 needs implementation. Phase 1 reduced from 2 plans to 1 plan.

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

### Existing Infrastructure (do NOT rebuild)

- BillingManager + BillingTimer: complete lifecycle management in rc-core/billing.rs
- compute_session_cost(): two-tier retroactive pricing, fully tested
- Dynamic pricing rules: peak/off-peak multipliers and group discounts
- wallet module: credit/debit + double-entry journal entries
- POST /billing/{id}/refund: exists, just needs audit_log
- GET /billing/sessions/{id}: exists, needs /detail extension
- cloud_sync.rs: relay + HTTP fallback infrastructure, billing_sessions + wallet_transactions already pushed
- billing_events table: all lifecycle events already logged (started, paused, resumed, warnings, expired)

### Pending Todos

- [ ] Execute Phase 1 plan 01-01

### Blockers/Concerns

- Need Bono's cooperation for WhatsApp receipt (Phase 4) — send James->Bono webhook spec before planning Phase 4
- Need to verify cloud API schema before Phase 2 planning — check racingpoint-api-gateway for existing auth patterns
- payment_method DB migration needed (Phase 5) — ensure idempotent SQL migration

## Session Continuity

Last session: 2026-03-14
Stopped at: Phase 1 planned
Resume file: None
