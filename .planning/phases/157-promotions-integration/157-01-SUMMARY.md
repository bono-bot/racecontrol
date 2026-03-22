---
phase: 157-promotions-integration
plan: "01"
subsystem: cafe-ordering
tags: [promotions, promo-engine, checkout, discounts, cafe]
dependency_graph:
  requires: [156-01]
  provides: [PROMO-05, PROMO-06]
  affects: [cafe.rs, cafe_promos.rs, db/mod.rs, api/routes.rs]
tech_stack:
  added: []
  patterns: [idempotent-alter-table, evaluate-promos-pure-fn, promo-fetch-non-blocking]
key_files:
  created: []
  modified:
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/cafe_promos.rs
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/src/cafe.rs
decisions:
  - "Happy hour discount applies to total_paise (not per-item) — avoids needing unit prices in evaluate_promos"
  - "Gaming bundle: listed in active promos for display only, no auto-apply in v1 (needs billing session lookup)"
  - "Single largest discount wins across stacking groups in v1 (not multi-group summing) — simplest correct behavior"
  - "Promo fetch in place_cafe_order_inner uses unwrap_or_default so promo failures never block orders"
  - "Inline IST time check in Step C2 (forward windows only) — full overnight logic lives in cafe_promos helpers"
metrics:
  duration_minutes: 30
  completed_date: "2026-03-22"
  tasks_completed: 3
  tasks_total: 3
  files_modified: 4
---

# Phase 157 Plan 01: Promotions Integration — Backend Summary

**One-liner:** Promo engine wired into checkout: public `/cafe/promos/active` endpoint with IST time-window filtering, `evaluate_promos` picking best discount per stacking group, and `place_cafe_order_inner` applying discount before wallet debit with full traceability via `applied_promo_id`/`discount_paise` on `cafe_orders`.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | DB migration — applied_promo_id and discount_paise | f2bfaf31 | db/mod.rs |
| 2 | list_active_promos endpoint + evaluate_promos engine | c8c8e71e | cafe_promos.rs, api/routes.rs |
| 3 | Wire evaluate_promos into place_cafe_order_inner | b35676cc | cafe.rs |

## What Was Built

**Task 1 — DB Migration**
- Idempotent `ALTER TABLE cafe_orders ADD COLUMN applied_promo_id TEXT` — nullable, stores which promo was applied
- Idempotent `ALTER TABLE cafe_orders ADD COLUMN discount_paise INTEGER NOT NULL DEFAULT 0` — for traceability
- Both use `let _ = ...` (swallow duplicate-column error) — safe to run on existing databases

**Task 2 — Promo Engine**
- `ActivePromo` struct: serializable promo returned by endpoint and used internally
- `PromoEvalResult` struct: output of evaluation (applied_promo_id, promo_name, discount_paise)
- `list_active_promos` handler: fetches `is_active=1` promos, filters to current IST time window, returns `Vec<ActivePromo>`
- `evaluate_promos` function: pure (no DB), takes cart_items + active_promos + total_paise, returns best single discount
- `calc_promo_discount`: handles `combo` (bundle_price_paise or discount_percent on gross) and `happy_hour` (% of total_paise)
- Stacking: one winner per `stacking_group` key, then largest discount across all groups wins
- IST helpers: `ist_now_hhmm`, `time_in_window` (handles overnight wraps), `fmt_hhmm` ("3:00 PM" format)
- Route: `GET /cafe/promos/active` registered in public tier (no auth) alongside `/cafe/menu`

**Task 3 — Checkout Integration**
- `PlaceOrderResponse` gains `discount_paise: i64`, `applied_promo_id: Option<String>`, `applied_promo_name: Option<String>`
- Step C2 inserted between Step C (total calc) and Step D (stock decrement): fetches active promos outside transaction, calls `evaluate_promos`, computes `final_total_paise = total_paise - discount_paise`
- Wallet debit uses `final_total_paise` (customer pays discounted amount)
- `cafe_orders` INSERT includes `discount_paise` and `applied_promo_id` bindings
- WhatsApp receipt and thermal print both receive `final_total_paise`
- Log line updated: `gross=Xp discount=Yp final=Zp promo=Some("id")`

## Decisions Made

1. **Happy hour applies to total_paise** — avoids needing unit prices in evaluate_promos (which only receives item_id + quantity). Simpler, correct for v1.
2. **Gaming bundle: display only** — auto-applying gaming_bundle requires checking active billing session, which would add DB complexity to the order hot path. Listed in `/active` for frontend display.
3. **Single largest discount wins** — when promos from different stacking groups both apply, pick the one with the largest discount rather than summing. Simpler and avoids unexpected over-discounting in v1.
4. **Promo fetch failure is non-fatal** — `unwrap_or_default()` on the promo fetch in `place_cafe_order_inner` ensures orders never fail due to promo subsystem errors.
5. **Inline time check in Step C2** — simplified forward-window check (handles most cases). Full overnight logic is in `cafe_promos::time_in_window` helper if needed later.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed `ref` keyword in pattern match**
- **Found during:** Task 2 build
- **Issue:** `if let (Some(ref start), Some(ref end)) = (&p.start_time, &p.end_time)` — explicit `ref` not allowed when implicitly borrowing via `&` on tuple
- **Fix:** Removed `ref` keywords: `if let (Some(start), Some(end)) = (&p.start_time, &p.end_time)`
- **Files modified:** cafe_promos.rs
- **Commit:** c8c8e71e (included in same commit)

## Verification Results

```
cargo build --release --bin racecontrol  -> Finished (no errors)
grep "promos/active" routes.rs           -> line 98: .route("/cafe/promos/active", ...)
grep "evaluate_promos" cafe.rs           -> line 1207: crate::cafe_promos::evaluate_promos(...)
grep "applied_promo_id" db/mod.rs        -> line 2466: ALTER TABLE cafe_orders ADD COLUMN applied_promo_id
```

## Self-Check: PASSED

- f2bfaf31 exists in git log: confirmed
- c8c8e71e exists in git log: confirmed
- b35676cc exists in git log: confirmed
- crates/racecontrol/src/cafe_promos.rs: ActivePromo, PromoEvalResult, list_active_promos, evaluate_promos present
- crates/racecontrol/src/cafe.rs: PlaceOrderResponse has discount_paise, applied_promo_id, applied_promo_name
- crates/racecontrol/src/db/mod.rs: both ALTER TABLE statements present
- Route /cafe/promos/active registered in routes.rs public tier
