---
phase: 259-coupon-discount-system
plan: 01
subsystem: payments
tags: [rust, sqlite, wallet, billing, atomicity, discounts]

requires:
  - phase: 252-financial-atomicity-core
    provides: debit_in_tx/credit_in_tx with wallet locking, FATM-01/FATM-03 patterns
  - phase: 257-billing-edge-cases
    provides: BILL-04 extension validation, billing session status enum

provides:
  - Atomic extend_billing_session: wallet debit + time addition in single SQLite transaction
  - DISCOUNT_FLOOR_PAISE constant enforced in start_billing and apply_billing_discount
  - Floor enforcement log with FATM-10 tag in both handlers

affects:
  - 259-02 (coupon validation builds on same billing flow)
  - Any future billing or discount work

tech-stack:
  added: []
  patterns:
    - "Snapshot timer data, drop lock, do async DB work, re-acquire lock (no RwLock across .await)"
    - "Result<(), String> return from billing functions enables route-level error propagation"
    - "Atomic debit_in_tx + DB UPDATE in single sqlx transaction for financial safety"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/billing.rs
    - crates/racecontrol/src/api/routes.rs

key-decisions:
  - "extend_billing_session returns Result<(), String> instead of void — callers can propagate errors"
  - "DashboardCommand::ExtendBilling path logs errors as warn (fire-and-forget)"
  - "routes.rs extend_billing handler calls billing fn directly (not via DashboardCommand) to return errors to HTTP caller"
  - "DISCOUNT_FLOOR_PAISE=0 default disables floor; venue can change compile-time constant"
  - "apply_billing_discount floor check reads current discount headroom to prevent stacking past floor across multiple calls"
  - "In-memory timer updated ONLY after DB commit — no partial state on commit failure"

requirements-completed:
  - FATM-07
  - FATM-10

duration: 25min
completed: 2026-03-29
---

# Phase 259 Plan 01: Coupon & Discount System (Atomicity + Floor) Summary

**Atomic wallet-debit-plus-time-addition for session extensions via single SQLite transaction, plus server-side discount stacking floor enforced in start_billing and apply_billing_discount**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-03-29T09:58:00Z
- **Completed:** 2026-03-29T10:23:00Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- FATM-07: `extend_billing_session` rewritten to debit wallet + update `allocated_seconds` in one `sqlx::Transaction`. Commit failure rolls back both. In-memory timer updated only after commit.
- FATM-07: Route handler returns `{"ok": false, "error": "..."}` on insufficient balance instead of silently succeeding.
- FATM-10: `DISCOUNT_FLOOR_PAISE` constant added to billing.rs. Enforcement in `start_billing` caps total discount after stacking coupon + staff + group. Enforcement in `apply_billing_discount` checks remaining headroom before each mid-session discount application.
- Both handlers include `discount_floor_paise` in JSON responses for dashboard awareness.

## Task Commits

1. **Task 1 + Task 2 (FATM-07 + FATM-10)** - `6838fe5c` (feat)

## Files Created/Modified

- `crates/racecontrol/src/billing.rs` - `extend_billing_session` rewritten as atomic fn returning `Result<(), String>`; `DISCOUNT_FLOOR_PAISE` constant added; `DashboardCommand::ExtendBilling` path updated to log errors
- `crates/racecontrol/src/api/routes.rs` - `extend_billing` route calls billing fn directly; `start_billing` floor check after all discounts; `apply_billing_discount` floor headroom check before UPDATE

## Decisions Made

- `extend_billing_session` signature changed to `pub async fn ... -> Result<(), String>` — enables HTTP route to return proper error vs fire-and-forget dashboard path
- DashboardCommand path logs errors as `tracing::warn` (fire-and-forget, no panic)
- DISCOUNT_FLOOR_PAISE defaults to 0 (disabled) — safe default, venue enables by changing constant and rebuilding
- In `apply_billing_discount`, the floor check reads `(original_price_paise, COALESCE(discount_paise,0))` from DB before updating — prevents cumulative discount calls from bypassing floor across multiple staff actions
- Timer snapshot + lock drop before all `.await` calls — standing rule compliance (no RwLock across .await)

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered

Three compile errors on first check:
1. `wallet::debit_in_tx` needed `crate::wallet::debit_in_tx` (module path in billing.rs context)
2. Type inference on `debit_in_tx` return — added explicit `Result<(i64, String), String>` annotation
3. `COALESCE(discount_paise, 0)` return type was `Option<i64>` in sqlx query — changed tuple type to `i64` to match

All fixed before commit. `cargo check` passes with zero errors.

## User Setup Required

None — no external service configuration required. DISCOUNT_FLOOR_PAISE=0 disables the floor by default; enable by changing the constant in billing.rs and rebuilding.

## Next Phase Readiness

- FATM-07 and FATM-10 complete. Phase 259 Plan 02 can proceed (remaining FATM-08, FATM-09, FATM-11 — coupon CRUD and validation logic).

---
*Phase: 259-coupon-discount-system*
*Completed: 2026-03-29*

## Self-Check: PASSED

- FOUND: crates/racecontrol/src/billing.rs
- FOUND: crates/racecontrol/src/api/routes.rs
- FOUND: .planning/phases/259-coupon-discount-system/259-01-SUMMARY.md
- FOUND: commit 6838fe5c
