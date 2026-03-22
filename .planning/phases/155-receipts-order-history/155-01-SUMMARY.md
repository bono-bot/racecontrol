---
phase: 155-receipts-order-history
plan: 01
subsystem: cafe
tags: [whatsapp, thermal-print, order-history, rust, axum]
dependency_graph:
  requires: [154-ordering-core]
  provides: [ORD-05, ORD-06, ORD-09]
  affects: [cafe.rs, config.rs, routes.rs]
tech_stack:
  added: [CafeConfig struct, print_script_path config field]
  patterns: [fire-and-forget tokio::spawn, Arc<AppState> pass-through, JWT auth reuse]
key_files:
  created: []
  modified:
    - crates/racecontrol/src/cafe.rs
    - crates/racecontrol/src/config.rs
    - crates/racecontrol/src/api/routes.rs
decisions:
  - "Arc<AppState> passed to side-effect fns instead of cloning Config (Config does not implement Clone)"
  - "Deserialize added to OrderItemDetail to enable JSON round-trip parsing in list_customer_orders"
  - "alerting.enabled guards WhatsApp receipt (mirrors cafe_alerts pattern, fail-silent if disabled)"
metrics:
  duration_minutes: 35
  completed_date: "2026-03-22T17:00:00+05:30"
  tasks_completed: 2
  files_modified: 3
---

# Phase 155 Plan 01: Receipts and Order History Summary

**One-liner:** WhatsApp order receipt dispatch (Evolution API), thermal print (Node.js script), and GET order history endpoint — all wired as fire-and-forget after every cafe order.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | WhatsApp receipt dispatch after order placement | 10078e53 | cafe.rs, config.rs |
| 2 | Thermal receipt print dispatch + order history API endpoint | 6864c454 | cafe.rs, routes.rs |

## What Was Built

### send_order_receipt_whatsapp (cafe.rs)
Private async function. Guards on `config.alerting.enabled`. Fetches driver phone from `drivers` table. Resolves Evolution API credentials from `config.auth`. Formats IST timestamp with order items, total, balance. POSTs to Evolution API `/message/sendText/{instance}`. All errors are `warn!` only — never propagated.

Spawned via Step L in `place_cafe_order_inner` (after Step J low-stock alerts, before Step K return).

### print_thermal_receipt (cafe.rs)
Private async function. Guards on `config.cafe.print_script_path` presence. Formats multi-line receipt text. Calls `tokio::process::Command::new("node")` with the script path and receipt text as args. Wrapped in `tokio::time::timeout(10s)`. All errors are `warn!` only.

Spawned via Step M in `place_cafe_order_inner` (after Step L).

### list_customer_orders (cafe.rs)
Public handler. Extracts driver_id from Authorization JWT (same `crate::auth::verify_jwt` pattern as `place_cafe_order_customer`). Queries `cafe_orders WHERE driver_id = ? ORDER BY created_at DESC`. Parses `items` JSON column into `Vec<OrderItemDetail>`. Returns `{ "orders": [...] }` JSON.

### CafeConfig (config.rs)
New `[cafe]` TOML section. Single field `print_script_path: Option<String>` — defaults to `None`, silently skips thermal printing when unset.

### GET /customer/cafe/orders/history (routes.rs)
Registered in customer authenticated routes block immediately after the existing POST `/customer/cafe/orders` route.

## Verification

1. `cargo test -p racecontrol-crate` — 448 passed, 2 pre-existing env-pollution failures (pass in isolation)
2. `cargo build --release --bin racecontrol` — clean compile, finished successfully
3. All 3 functions present in cafe.rs: `send_order_receipt_whatsapp`, `print_thermal_receipt`, `list_customer_orders`
4. Route `customer/cafe/orders/history` registered in routes.rs line 168
5. Zero new `.unwrap()` calls added

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Config not Clone — used Arc<AppState> instead**
- **Found during:** Task 1
- **Issue:** Plan specified `config: &crate::config::Config` as function parameter but `Config` doesn't implement `Clone`, making tokio::spawn capture impossible
- **Fix:** Changed function signatures to accept `&Arc<AppState>` (matching existing pattern in file), extracted `&state.config` and `&state.db` inside
- **Files modified:** crates/racecontrol/src/cafe.rs
- **Commit:** 10078e53

**2. [Rule 2 - Missing] Deserialize on OrderItemDetail**
- **Found during:** Task 2
- **Issue:** `list_customer_orders` uses `serde_json::from_str::<Vec<OrderItemDetail>>()` but `OrderItemDetail` only derived `Serialize, Clone` — missing `Deserialize`
- **Fix:** Added `Deserialize` to the derive macro on `OrderItemDetail`
- **Files modified:** crates/racecontrol/src/cafe.rs
- **Commit:** 10078e53

## Self-Check: PASSED
