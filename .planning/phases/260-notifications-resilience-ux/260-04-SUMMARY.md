---
phase: 260-notifications-resilience-ux
plan: 04
subsystem: api
tags: [receipt, gst, virtual-queue, notification-outbox, walk-in, ux]

# Dependency graph
requires:
  - phase: 260-01
    provides: notification_outbox.enqueue_notification — durable WhatsApp retry used for receipt delivery
  - phase: 255-legal-compliance
    provides: invoices table with GST breakup (taxable_value_paise, cgst_paise, sgst_paise, total_paise)
provides:
  - GET /customer/sessions/{id}/receipt — full financial breakdown with GST, before/after balance
  - POST /queue/join, GET /queue/status/{id}, POST /queue/{id}/leave — walk-in queue (public)
  - GET /queue, POST /queue/{id}/call, POST /queue/{id}/seat — staff queue management
  - virtual_queue table migration with status FSM and expire background task
  - get_driver_phone() helper in billing.rs
  - UX-03 receipt notification enqueue in post_session_hooks
affects: [notifications, customer-facing PWA, kiosk, staff dashboard]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Receipt endpoint reconstructs pre-session balance: current_balance + debit - refund"
    - "GST breakup from invoices table with fallback to 18% inclusive integer arithmetic"
    - "Queue ETA: position * 30min / 8 pods (ceiling integer, no floating point)"
    - "Background queue expire task: 5min interval, expires 'called' entries > 10min old"
    - "Notification outbox enqueue in post_session_hooks alongside direct Evolution API send (dual-path)"

key-files:
  created:
    - .planning/phases/260-notifications-resilience-ux/260-04-SUMMARY.md
  modified:
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/src/billing.rs
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/main.rs

key-decisions:
  - "Receipt endpoint in customer_routes (JWT-gated) — only session owner can access their own receipt"
  - "GST fallback: if invoices table has no row, compute 18% inclusive split from wallet_debit_paise using integer arithmetic (net = amount * 100 / 118) — consistent with Phase 255 LEGAL-01 decision"
  - "Dual-path receipt notification: existing send_whatsapp_receipt (direct Evolution API) preserved; outbox enqueue added alongside for durable retry (non-blocking, WARN on failure)"
  - "Queue endpoints: join/status/leave in public_routes (no auth for walk-ins); call/seat/list in staff_routes (JWT required)"
  - "ETA formula: ceiling integer (position * 30 + 7) / 8 — avoids f64, 8 pods, 30min avg session"
  - "queue_expire_task is pub so main.rs can spawn it; takes SqlitePool (not AppState) to avoid circular dep"

patterns-established:
  - "Pattern: post_session_hooks dual-path delivery — direct fast path + outbox durable retry"
  - "Pattern: queue position recomputed live on GET status (not cached) to reflect real-time dequeuing"

requirements-completed: [UX-03, UX-08]

# Metrics
duration: 30min
completed: 2026-03-29
---

# Phase 260 Plan 04: Notifications Resilience UX — Receipt + Virtual Queue Summary

**Customer session receipt with GST breakup and before/after balance, plus virtual walk-in queue with live position ETA and staff call/seat workflow**

## Performance

- **Duration:** 30 min
- **Started:** 2026-03-29T11:00:00Z (approx)
- **Completed:** 2026-03-29T11:29:41Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- GET /customer/sessions/{id}/receipt returns full financial breakdown: charges, CGST, SGST, net, refund, before/after balance — verified by receipt acceptance criteria
- UX-03 outbox enqueue added in post_session_hooks alongside direct Evolution API send (durable retry path for receipt delivery)
- virtual_queue table migration with 5-status FSM (waiting/called/seated/left/expired) and idx_queue_status index
- 7 queue API endpoints covering full walk-in lifecycle: join, status check, leave, staff list, call, seat
- Background queue_expire_task expires 'called' entries after 10 minutes (every 5 min), spawned in main.rs
- get_driver_phone() helper with proper Err handling for anonymized/missing drivers

## Task Commits

1. **Task 1: Customer receipt generation and auto-send on session end** - `5fcfe239` (feat)
2. **Task 2: Virtual queue management** - `f736cedc` (feat)

## Files Created/Modified
- `crates/racecontrol/src/api/routes.rs` — Added customer_session_receipt handler, 7 virtual queue handlers, queue_expire_task, routes in customer_routes/public_routes/staff_routes
- `crates/racecontrol/src/billing.rs` — Added UX-03 outbox enqueue in post_session_hooks, get_driver_phone() helper
- `crates/racecontrol/src/db/mod.rs` — virtual_queue table migration + idx_queue_status index
- `crates/racecontrol/src/main.rs` — Spawn queue_expire_task background task at startup

## Decisions Made
- Receipt endpoint is JWT-gated in customer_routes — only session owner can view their receipt (driver_id from JWT checked against billing_sessions.driver_id)
- GST fallback arithmetic: if invoices row missing (session predates Phase 255), compute 18% inclusive split from wallet_debit_paise using integer arithmetic consistent with LEGAL-01 decision
- Queue endpoints split: public_routes for walk-in actions (join/status/leave), staff_routes for management (list/call/seat)
- Existing send_whatsapp_receipt preserved (direct Evolution API); outbox enqueue added as additional path for durability — same data flow, different delivery mechanism
- queue_expire_task takes SqlitePool directly (not Arc<AppState>) to keep it a standalone pub fn without coupling to AppState internals

## Deviations from Plan

None — plan executed exactly as written.

The existing `send_whatsapp_receipt` function was discovered in `post_session_hooks` — this is not a conflict. The plan specified enqueue_notification via outbox; both paths are now active (direct Evolution API for speed + outbox for retry durability). This matches the plan's stated pattern "non-critical post-commit: failure logs WARN, session end is not blocked."

## Issues Encountered
- `cargo test` integration tests show 8 pre-existing failures (test_lap_not_suspect_*, test_notification_*) — confirmed pre-existing by git stash verification. Not caused by this plan's changes.

## User Setup Required
None — all endpoints are server-side. No environment variables or external service configuration required beyond what Phase 260-01 already set up (notification_outbox worker already running).

## Next Phase Readiness
- This is the FINAL plan of Phase 260 (notifications-resilience-ux) and the FINAL plan of the v27.0 milestone
- All 4 plans of Phase 260 complete: 260-01 (outbox + OTP), 260-02 (leaderboard integrity), 260-03 (hardware disconnect + crash tracking), 260-04 (receipt + virtual queue)
- Ready for milestone ship gate: Unified Protocol v3.1 (Quality Gate + E2E + Standing Rules + MMA audit)

---
*Phase: 260-notifications-resilience-ux*
*Completed: 2026-03-29*
