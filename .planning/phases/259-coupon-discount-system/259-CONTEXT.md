# Phase 259: Coupon & Discount System - Context

**Gathered:** 2026-03-29
**Status:** Ready for planning
**Mode:** Auto-generated (discuss skipped)

<domain>
## Phase Boundary

Coupons have a stateful lifecycle with rollback on failure; discount stacking has a hard floor; payment gateway credits are idempotent. Extension purchases are atomic.

Requirements: FATM-07 (extension atomicity), FATM-08 (coupon lifecycle), FATM-09 (coupon restoration), FATM-10 (discount stacking floor), FATM-11 (payment gateway idempotency)

Depends on: Phase 252 (atomicity layer), Phase 253 (session state machine)

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
Key guidance:
- FATM-07: Extension purchase uses debit_in_tx() from Phase 252. Single transaction: wallet debit + UPDATE billing_sessions SET allocated_seconds += extension_seconds. If either fails, rollback.
- FATM-08: Add coupon_status column to coupons table (or create if not exists): available → reserved → redeemed → released → expired. Reserved has TTL (10 min). Background job expires stale reservations.
- FATM-09: On session cancellation (CAS from Phase 252), if coupon_id is set on billing_session, restore coupon to 'available' within the same transaction.
- FATM-10: Add discount_floor_paise to config (default 0 = no floor). In start_billing, after all discounts applied, enforce: final_price >= discount_floor_paise. If below, cap discount.
- FATM-11: Payment gateway webhook handler: check idempotency_key (transaction_id from gateway) before crediting. Use existing idempotency pattern from Phase 252.

</decisions>

<code_context>
## Existing Code Insights

### Key Files
- `crates/racecontrol/src/api/routes.rs` — billing start (has coupon_id handling), extend_billing, topup
- `crates/racecontrol/src/billing.rs` — start_billing_session, discount calculation
- `crates/racecontrol/src/wallet.rs` — debit_in_tx, credit_in_tx from Phase 252
- `crates/racecontrol/src/db/mod.rs` — coupons table (if exists), migrations

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
