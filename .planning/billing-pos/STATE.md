---
gsd_state_version: 1.0
milestone: v3.0
milestone_name: Billing & Point of Sale
status: executing
stopped_at: "Phase 4 complete — PWA session timeline, public share page, WhatsApp receipt"
last_updated: "2026-03-14"
last_activity: 2026-03-14 — Phase 4 complete (session timeline, public share page, WhatsApp receipt)
progress:
  total_phases: 5
  completed_phases: 2
  total_plans: 10
  completed_plans: 3
  percent: 30
  note: "Phase 4.1 (PDF Receipt) inserted — 1 additional plan pending"
---

# Project State

## Project Reference

See: .planning/billing-pos/PROJECT.md (created 2026-03-14)

**Core value:** Every rupee earned is tracked, every customer gets a receipt, and Uday can see venue revenue in real-time from anywhere.
**Current focus:** Phase 4 complete. Next: Phase 5 (Kiosk POS — Staff Operations).

## Current Position

Phase: 4 of 5 — PWA Session Results & Receipt (COMPLETE)
Plan: 04-02 complete (2 of 2 plans in phase)
Status: Phase 4 done. Next: Phase 5 (Kiosk POS).
Last activity: 2026-03-14 — Phase 4 complete (session timeline, public share page, WhatsApp receipt)

Progress: [######    ] 30%

## Key Research Finding (Phase 1)

billing_sessions and wallet_transactions were ALREADY synced in cloud_sync.rs. Only billing_events was missing (~50 lines of code). SYNC-01, SYNC-02, SYNC-04, SYNC-05 were already met. Only SYNC-03 needed implementation. Phase 1 completed with 1 plan.

## Performance Metrics

**Velocity:**
- Total plans completed: 3
- Total execution time: ~22 minutes

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 01-billing-cloud-sync | 1/1 | 8min | 8min |
| 04-pwa-session-results | 2/2 | 14min | 7min |

## Accumulated Context

### Decisions

- billing_sessions stays LOCAL-authoritative — cloud gets read-only copy via sync
- Sync strategy: billing_sessions and wallet_transactions already in push payload; add billing_events to collect_push_payload() following wallet_transactions pattern
- IMPORTANT: Do NOT add billing tables to SYNC_TABLES constant — that controls the PULL path (cloud -> venue). Billing data only flows venue -> cloud.
- WhatsApp receipt delivered directly via Evolution API from racecontrol (NOT via Bono webhook) — per user decision in Phase 4 planning
- racingpoint-admin (port 3200) is the target for cloud dashboard pages (already deployed at dashboard.racingpoint.cloud)
- payment_method column needs to be added to billing_sessions table (currently missing)
- POST /billing/{id}/refund already exists in racecontrol — needs audit_log write wired in
- Phase execution order: 1 -> 4 -> 5 -> 2 -> 3 (cloud sync first, then PWA/kiosk work in parallel while cloud API builds)
- Plan 01-01: INSERT OR IGNORE for billing_events (immutable lifecycle records)
- Plan 01-01: COALESCE for 3 new billing_sessions columns in ON CONFLICT (backward-compat with old venue code)
- Plan 01-01: Tests use in-memory SQLite with inline schema (no AppState construction)
- Plan 04-01: Public session summary shows first name only (split_whitespace().next()) for privacy
- Plan 04-01: Receipt formatting extracted to pure helper functions (format_wa_phone, format_receipt_message) for testability
- Plan 04-01: 5-second HTTP timeout on WhatsApp receipt -- best-effort, never blocks session end
- **BILLING CREDITS MIGRATION (cc3da21, 2026-03-14):** Switched from INR/rupees to credits (1 credit = Rs.1 = 100 paise). compute_session_cost() rewritten from 2-tier retroactive to 3-tier non-retroactive. Rates now DB-driven via billing_rates table + admin CRUD. Default: 25cr/min (0-30), 20cr/min (31-60), 15cr/min (60+). All UI shows "cr" not "₹". billing_rates added to SYNC_TABLES for cloud pull. Protocol field renamed: minutes_to_value_tier → minutes_to_next_tier (serde alias for compat).

### Existing Infrastructure (do NOT rebuild)

- BillingManager + BillingTimer: complete lifecycle management in racecontrol/billing.rs
- compute_session_cost(): **3-tier non-retroactive** pricing with DB-driven rates (BillingRateTier cache, refreshed every 60s)
- billing_rates table: DB-driven per-minute rates with CRUD API at /billing/rates — admin-configurable, no code deploy needed
- BillingManager.rate_tiers: RwLock<Vec<BillingRateTier>> in-memory cache — called every second per active pod
- Dynamic pricing rules: peak/off-peak multipliers and group discounts
- wallet module: credit/debit + double-entry journal entries
- POST /billing/{id}/refund: exists, just needs audit_log
- GET /billing/sessions/{id}: exists, needs /detail extension
- cloud_sync.rs: relay + HTTP fallback infrastructure, ALL 3 billing tables + billing_rates now synced
- billing_events table: all lifecycle events already logged (started, paused, resumed, warnings, expired)
- All UI (web admin, overlay, PWA) displays "credits" (X cr) — internal storage remains paise

### Pending Todos

- Phase 5: Kiosk POS — Staff Operations (next in execution order)

### Blockers/Concerns

- Need to verify cloud API schema before Phase 2 planning — check racingpoint-api-gateway for existing auth patterns
- payment_method DB migration needed (Phase 5) — ensure idempotent SQL migration

## Session Continuity

Last session: 2026-03-14
Stopped at: Phase 4 complete
Resume file: .planning/billing-pos/phases/04-pwa-session-results/04-02-SUMMARY.md
