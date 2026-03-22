---
phase: 154-ordering-core
plan: "01"
subsystem: cafe-ordering
tags: [cafe, ordering, wallet, atomicity, sqlite, inventory]
dependency_graph:
  requires:
    - cafe.rs (CafeItem types, existing handlers)
    - wallet.rs (debit function)
    - cafe_alerts.rs (check_low_stock_alerts)
    - db/mod.rs (migrate function)
  provides:
    - place_cafe_order handler (POST /api/v1/cafe/orders)
    - place_cafe_order_customer handler (POST /api/v1/customer/cafe/orders)
    - cafe_orders table with indexes
    - stock info in public menu endpoint
  affects:
    - routes.rs (two new route registrations)
    - public_menu response shape (added is_countable, stock_quantity, out_of_stock)
tech_stack:
  added: []
  patterns:
    - BEGIN IMMEDIATE exclusive write lock for concurrent stock protection
    - Compensating update (stock rollback) when wallet debit fails post-commit
    - RP-YYYYMMDD-NNNN receipt number generation via COUNT(*) + format
key_files:
  created: []
  modified:
    - crates/racecontrol/src/cafe.rs
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/api/routes.rs
decisions:
  - "wallet::debit() placed OUTSIDE raw SQLite transaction because it uses pool internally — stock lock released first, then wallet debited atomically via UPDATE WHERE balance >= ?"
  - "Compensating stock rollback via tokio::spawn after wallet failure — best-effort, logged as warn"
  - "Customer route uses extract_driver_id pattern from existing customer handlers, overrides driver_id in request body to prevent spoofing"
  - "Receipt number uses COUNT(*) LIKE 'RP-{date}-%' inside transaction — sequential within day, reset daily"
metrics:
  duration_minutes: 11
  completed_date: "2026-03-22"
  tasks_completed: 2
  tasks_total: 2
  files_modified: 3
---

# Phase 154 Plan 01: Cafe Ordering Backend Summary

Atomic cafe order placement endpoint: BEGIN IMMEDIATE stock check + decrement + receipt generation, then wallet debit with compensating rollback. Public menu now includes stock availability for frontend blocking.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | cafe_orders table + place_cafe_order atomic handler | fa858364 | cafe.rs, db/mod.rs |
| 2 | Register routes + public menu stock info | 494f7ebf | routes.rs |

## What Was Built

### cafe_orders Table (db/mod.rs)

New table auto-created on startup:
- `id TEXT PRIMARY KEY`, `receipt_number TEXT NOT NULL UNIQUE`, `driver_id TEXT NOT NULL`
- `items TEXT NOT NULL` — JSON array of OrderItemDetail
- `total_paise INTEGER NOT NULL`, `wallet_txn_id TEXT NOT NULL`, `status TEXT DEFAULT 'confirmed'`
- Three indexes: `idx_cafe_orders_driver`, `idx_cafe_orders_receipt`, `idx_cafe_orders_created`

### place_cafe_order_inner (cafe.rs)

Shared order logic with full atomic flow:

1. **Validation** — empty items, empty driver_id, quantity < 1 all return 400 before touching DB
2. **BEGIN IMMEDIATE** — acquires exclusive SQLite write lock on raw connection
3. **Item validation** — each item fetched within lock: checks existence, is_available, stock >= quantity
4. **Stock decrement** — `UPDATE ... WHERE is_countable = 1 AND stock_quantity >= ?` with `rows_affected == 1` check; returns 409 on concurrent conflict
5. **Receipt generation** — `RP-YYYYMMDD-NNNN` via `COUNT(*) LIKE 'RP-{date}-%'` within transaction
6. **COMMIT** — stock and receipt committed before wallet debit
7. **Wallet debit** — `wallet::debit()` outside transaction; on failure: tokio::spawn compensating `stock_quantity + qty` updates
8. **Order insert** — `cafe_orders` row inserted after successful debit
9. **Low-stock alerts** — `check_low_stock_alerts()` called for each countable item post-insert

### Public Menu (cafe.rs)

`public_menu` handler updated:
- `MenuItem` struct extended with `is_countable: bool`, `stock_quantity: i64`
- SQL query includes `ci.is_countable, ci.stock_quantity`
- Response serialized with computed `out_of_stock` boolean: `is_countable && stock_quantity <= 0`

### Routes (routes.rs)

- `POST /api/v1/cafe/orders` — staff auth layer, driver_id from request body
- `POST /api/v1/customer/cafe/orders` — customer routes, driver_id extracted from Authorization JWT header

## Verification

- `cargo check -p racecontrol-crate` — no errors
- `cargo test -p racecontrol-crate` — 449 passed, 1 pre-existing failure (`config::tests::config_fallback_preserved_when_no_env_vars`) confirmed pre-existing before this plan
- `cargo build --release --bin racecontrol` — succeeded

## Deviations from Plan

None — plan executed exactly as written.

The one design nuance: `place_cafe_order` and `place_cafe_order_customer` return `Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)>` which implements axum's `IntoResponse` directly, matching the pattern in the plan and consistent with axum's blanket impl for `(StatusCode, Json<T>)`.

## Self-Check: PASSED

- cafe.rs: FOUND
- db/mod.rs: FOUND
- routes.rs: FOUND
- commit fa858364: FOUND
- commit 494f7ebf: FOUND
