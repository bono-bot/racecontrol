---
phase: 149-menu-data-model-crud
plan: "01"
subsystem: cafe
tags: [rust, axum, sqlite, crud, menu, cafe]
dependency_graph:
  requires: []
  provides: [cafe_categories table, cafe_items table, cafe.rs CRUD module, cafe API routes]
  affects: [crates/racecontrol/src/db/mod.rs, crates/racecontrol/src/cafe.rs, crates/racecontrol/src/lib.rs, crates/racecontrol/src/api/routes.rs]
tech_stack:
  added: []
  patterns: [sqlx::FromRow on CafeItem/CafeCategory, axum State extractor, dynamic UPDATE SET builder, tokio::test with in-memory SQLite]
key_files:
  created:
    - crates/racecontrol/src/cafe.rs
  modified:
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/lib.rs
    - crates/racecontrol/src/api/routes.rs
decisions:
  - "CafeItem.is_available uses bool (not i64) via sqlx::FromRow BOOLEAN mapping"
  - "Dynamic UPDATE SET builder builds clause string then binds non-None fields in order"
  - "public_menu uses inline MenuItem struct with category_name from JOIN, avoids extra DB round trip"
  - "create_cafe_category uses INSERT OR IGNORE + SELECT by name for idempotency"
metrics:
  duration: "8 minutes"
  completed_date: "2026-03-22T11:20:48Z"
  tasks_completed: 2
  files_created: 1
  files_modified: 3
requirements: [MENU-02, MENU-03, MENU-04, MENU-05]
---

# Phase 149 Plan 01: Cafe Menu Data Model and CRUD Summary

SQLite schema (cafe_categories + cafe_items), Rust CRUD module (cafe.rs with 8 handlers + 5 DB tests), and Axum route registration for all admin and public cafe endpoints.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Add cafe schema to db/mod.rs and create cafe.rs CRUD module with unit tests | 16ec9e6b | crates/racecontrol/src/db/mod.rs, crates/racecontrol/src/cafe.rs, crates/racecontrol/src/lib.rs |
| 2 | Register cafe routes in api/routes.rs | aa78dc67 | crates/racecontrol/src/api/routes.rs |

## What Was Built

**Schema (db/mod.rs):**
- `cafe_categories` table: id, name (UNIQUE), sort_order, created_at
- `cafe_items` table: id, name, description, category_id (FK), selling_price_paise (i64), cost_price_paise (i64), is_available (BOOLEAN), timestamps
- Indexes: idx_cafe_items_category, idx_cafe_items_available
- Seeded default categories: Beverages, Snacks, Meals (idempotent via INSERT OR IGNORE)

**Handlers (cafe.rs):**
1. `list_cafe_items` — SELECT all, returns `{ items, total, page: 1 }`
2. `create_cafe_item` — validates name/price, checks category FK, inserts, returns 201 with `{ id }`
3. `update_cafe_item` — dynamic SET builder for non-None fields, always sets updated_at
4. `delete_cafe_item` — DELETE, returns 404 if no row affected
5. `toggle_cafe_item_availability` — NOT is_available + updated_at, returns new value
6. `list_cafe_categories` — ORDER BY sort_order, name
7. `create_cafe_category` — INSERT OR IGNORE + SELECT by name (idempotent)
8. `public_menu` — JOIN with cafe_categories, WHERE is_available = 1, returns `{ items, total, page: 1 }`

**Routes (routes.rs):**
- Staff routes (JWT-protected + pod-blocked): GET/POST /cafe/items, PUT/DELETE /cafe/items/{id}, POST /cafe/items/{id}/toggle, GET/POST /cafe/categories
- Public route (no auth): GET /cafe/menu

## Unit Tests (all passing)

- `test_create_and_list_items` — DB-layer insert + SELECT, asserts count=1, name, price, is_available
- `test_is_available_filter` — inserts available + unavailable, asserts WHERE is_available=1 returns 1 row
- `test_foreign_key_enforcement` — INSERT with nonexistent category_id returns error (FK violation)
- `test_category_unique_constraint` — second INSERT with same name fails UNIQUE constraint
- `test_toggle_availability` — NOT is_available twice, asserts false then true

## Verification Results

- `cargo check -p racecontrol-crate` — passes with 0 errors (warnings are pre-existing)
- `cargo build --release -p racecontrol-crate` — produces binary (2m 17s)
- `cargo test -p racecontrol-crate -- cafe::tests` — 5/5 pass
- `cargo test -p racecontrol-crate` — 422 pass, 2 pre-existing failures (config + crypto modules, out of scope)

## Deviations from Plan

None — plan executed exactly as written.

**Pre-existing test failures (out of scope, not caused by this plan):**
- `config::tests::config_fallback_preserved_when_no_env_vars` — failing before this plan
- `crypto::encryption::tests::load_keys_wrong_length` — failing before this plan

These are deferred to `deferred-items.md` per scope boundary rules.

## Self-Check

- [x] cafe.rs created at crates/racecontrol/src/cafe.rs
- [x] db/mod.rs modified with cafe tables and indexes
- [x] lib.rs has pub mod cafe
- [x] routes.rs has all 9 cafe route registrations
- [x] Commit 16ec9e6b exists (Task 1)
- [x] Commit aa78dc67 exists (Task 2)
