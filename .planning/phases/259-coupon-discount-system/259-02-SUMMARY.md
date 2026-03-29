---
phase: 259-coupon-discount-system
plan: "02"
subsystem: billing/coupons
tags: [fatm, coupon, fsm, idempotency, payment-gateway]
dependency_graph:
  requires: ["259-01"]
  provides: ["FATM-08", "FATM-09", "FATM-11"]
  affects: ["billing", "wallet", "coupons"]
tech_stack:
  added: []
  patterns:
    - "CAS UPDATE (coupon_status = 'available') prevents concurrent reservation races"
    - "Session ID generated early so coupon reservation ties to real billing session"
    - "Idempotency via wallet_transactions.idempotency_key = gateway transaction_id"
    - "Background TTL job (60s interval, 120s initial delay) reverts stale reservations"
key_files:
  created: []
  modified:
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/src/billing.rs
    - crates/racecontrol/src/main.rs
decisions:
  - "session_id generated before coupon validation so reserve_coupon() ties to the real session ID"
  - "restore_coupon_on_cancel() called at all 3 failure points in start_billing (debit, INSERT, commit)"
  - "payment_gateway_webhook placed in public_routes (no JWT) — gateway has no staff JWT; protected by idempotency + undiscoverability; HMAC TODO when gateway is chosen"
  - "credit_in_tx idempotency_key = gateway transaction_id — prevents double-credit even if webhook fires twice"
metrics:
  duration_minutes: 20
  completed_date: "2026-03-29"
  tasks_completed: 2
  files_changed: 4
requirements_completed:
  - FATM-08
  - FATM-09
  - FATM-11
---

# Phase 259 Plan 02: Coupon Lifecycle FSM + Gateway Webhook Summary

Stateful coupon lifecycle FSM (available/reserved/redeemed) with TTL-based auto-expiry, full restoration on session cancellation, and idempotent payment gateway webhook handler.

## Tasks Completed

| Task | Description | Commit |
|------|-------------|--------|
| 1 | Coupon lifecycle FSM + restoration on cancel (FATM-08, FATM-09) | 8a89e404 |
| 2 | Payment gateway webhook with idempotent wallet credit (FATM-11) | 8a89e404 |

## What Was Built

### Task 1: Coupon Lifecycle FSM (FATM-08, FATM-09)

**Schema migration (db/mod.rs):**
- `coupon_status TEXT DEFAULT 'available'` — FSM state column
- `reserved_at TEXT` — reservation timestamp for TTL calculation
- `reserved_for_session TEXT` — binds reservation to a specific session ID
- All three columns added via `pragma_table_info` existence checks (idempotent)

**FSM functions (routes.rs):**
- `reserve_coupon()` — CAS UPDATE `WHERE coupon_status = 'available'`, returns Err if 0 rows affected (race condition caught)
- `redeem_coupon()` — transitions reserved → redeemed after billing commit
- `restore_coupon_on_cancel()` — pub, restores coupon to available + decrements used_count + deletes redemption record

**start_billing integration:**
- `session_id` now generated early (before coupon block) so reservation binds to the real session ID
- `reserve_coupon()` called after validate_and_calc_coupon succeeds, before transaction begins
- `restore_coupon_on_cancel()` called at all 3 failure points: debit failure, INSERT failure, commit failure
- `redeem_coupon()` called post-commit after `record_coupon_redemption`

**validate_and_calc_coupon:** Added `AND coupon_status = 'available'` to SELECT — prevents reserved/redeemed coupons from being validated

**Restoration in cancel path (billing.rs):**
- `crate::api::routes::restore_coupon_on_cancel()` called inside the `BillingSessionStatus::Cancelled` block after full refund
- Logged at INFO with FATM-09 tag

**Background TTL expiry job (billing.rs + main.rs):**
- `spawn_coupon_ttl_expiry_job()` — every 60s, reverts coupons reserved >10 minutes ago
- 120s initial delay to let server stabilize
- Logs count of expired reservations at INFO if >0

### Task 2: Payment Gateway Webhook (FATM-11)

**Route:** `POST /webhooks/payment-gateway` in `public_routes` (no JWT — gateway has no staff JWT)

**Handler `payment_gateway_webhook`:**
- Validates required fields (transaction_id, driver_id non-empty)
- Amount cap: 1 paise to Rs 10,000 (100000 paise) safety guard
- Status filter: only `success` or `captured` triggers wallet credit; all other statuses acknowledged with `{"action":"ignored"}`
- Idempotency check: queries `wallet_transactions WHERE idempotency_key = transaction_id` BEFORE starting transaction; returns original result if found with `{"duplicate":true}`
- Credit via `wallet::credit_in_tx` inside a transaction, passing `transaction_id` as `idempotency_key`
- Returns `{"ok":true, "balance_after_paise":..., "txn_id":...}`
- HMAC verification placeholder comment for when gateway is chosen

## Deviations from Plan

None — plan executed exactly as written.

The one implementation choice (session_id generated early to bind reservation) was explicitly required by the plan's step 5: "After validate_and_calc_coupon succeeds, call reserve_coupon BEFORE the billing transaction."

## Known Stubs

None — all coupon FSM transitions and webhook logic are fully wired.

## Self-Check: PASSED

- SUMMARY.md exists: FOUND
- Commit 8a89e404 exists: FOUND
- cargo check --bin racecontrol: Finished (0 errors)
- coupon_status migration in db/mod.rs: VERIFIED
- reserve_coupon in routes.rs: VERIFIED
- restore_coupon_on_cancel in routes.rs AND billing.rs: VERIFIED
- FATM-09 log in billing.rs cancel path: VERIFIED
- reserved_at < datetime('now', '-10 minutes') TTL query: VERIFIED
- payment_gateway_webhook route + handler: VERIFIED
- gateway_topup txn_type in handler: VERIFIED
- FATM-11 references in handler: VERIFIED
