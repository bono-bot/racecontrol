---
phase: 153-inventory-alerts
plan: "01"
subsystem: cafe-inventory
tags: [alerts, whatsapp, email, cooldown, low-stock, rust]
dependency_graph:
  requires: [152-01, 152-02]
  provides: [check_low_stock_alerts, reset_alert_cooldown, list_low_stock_items]
  affects: [cafe.rs, routes.rs, db/mod.rs]
tech_stack:
  added: []
  patterns: [Evolution API WA dispatch, tokio::process email dispatch, 4-hour DB-tracked cooldown]
key_files:
  created:
    - crates/racecontrol/src/cafe_alerts.rs
  modified:
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/lib.rs
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/src/cafe.rs
decisions:
  - "Timestamp recorded BEFORE alert dispatch to prevent phantom cooldown gaps on slow networks"
  - "reset_alert_cooldown called on restock above threshold so next breach always re-alerts"
  - "WA enabled check done first (before Evolution URL check) to short-circuit cleanly"
  - "list_low_stock_items tested via direct SQL in unit tests (axum State not available in unit tests)"
metrics:
  duration_minutes: 9
  completed_date: "2026-03-22"
  tasks_completed: 2
  files_changed: 5
---

# Phase 153 Plan 01: Low-Stock Alert Engine Summary

## One-liner

WhatsApp + email low-stock alerts with 4-hour per-item cooldown tracked via `last_stock_alert_at` column in `cafe_items`.

## What Was Built

- **`cafe_alerts.rs`** — new module with three public exports:
  - `check_low_stock_alerts(db, config, item_id)` — checks breach, fires WA + email, records timestamp
  - `reset_alert_cooldown(db, item_id)` — sets `last_stock_alert_at = NULL` for restock-above-threshold
  - `list_low_stock_items(State)` — Axum handler returning `{"items": [...]}` for dashboard banner
- **DB migration** — `last_stock_alert_at TEXT` column added idempotently to `cafe_items` in `db/mod.rs`
- **Route** — `GET /api/v1/cafe/items/low-stock` registered in `routes.rs` BEFORE the `{id}` wildcard
- **Restock integration** — `restock_cafe_item` in `cafe.rs` calls alert check/reset after every successful stock UPDATE

## Tasks

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | DB migration + cafe_alerts module | f8b6333c | cafe_alerts.rs, db/mod.rs, lib.rs |
| 2 | Wire route + restock integration | 8695adff | routes.rs, cafe.rs |

## Test Results

- 8 cafe_alerts unit tests: all pass
- 23 total cafe tests: all pass
- `cargo build -p racecontrol`: zero errors
- 3 pre-existing test failures (billing_rates, notification) confirmed unrelated to this plan

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Config::default() does not exist**
- **Found during:** Task 1 (TDD RED phase)
- **Issue:** `Config` struct has required fields (`venue`, `server`, `database`) with no Default impl, so `Config::default()` fails to compile
- **Fix:** Replaced with `toml::from_str(minimal_toml)` — same pattern used in `config.rs` tests
- **Files modified:** `cafe_alerts.rs` test helper
- **Commit:** f8b6333c

## Self-Check

- [x] `crates/racecontrol/src/cafe_alerts.rs` exists
- [x] `last_stock_alert_at` migration in `db/mod.rs`
- [x] Module registered in `lib.rs` (alphabetical after `cafe`)
- [x] Route `/cafe/items/low-stock` before `{id}` wildcard in `routes.rs`
- [x] `restock_cafe_item` calls `check_low_stock_alerts` / `reset_alert_cooldown`
- [x] No `.unwrap()` in `cafe_alerts.rs`
- [x] All 7 behavioral tests pass (8 total including list_low_stock query test)
- [x] `cargo build` zero errors
