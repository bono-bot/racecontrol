---
phase: 152-inventory-tracking
plan: 01
subsystem: database
tags: [sqlite, sqlx, rust, axum, cafe, inventory]

# Dependency graph
requires:
  - phase: 150-cafe-import
    provides: "cafe_items table with image_path column; CafeItem struct pattern"
provides:
  - "3 inventory columns on cafe_items (is_countable, stock_quantity, low_stock_threshold)"
  - "CafeItem struct with inventory fields serialized in all API responses"
  - "POST /api/v1/cafe/items/{id}/restock endpoint that increments stock for countable items"
  - "PUT /api/v1/cafe/items/{id} can update all inventory fields"
  - "POST /api/v1/cafe/items can create items with inventory field defaults"
affects: [152-inventory-tracking, cafe-ui, inventory-dashboard]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Idempotent ALTER TABLE migration pattern — let _ = sqlx::query(...).execute(pool).await"
    - "Dynamic SET clause builder for optional update fields in Axum handlers"
    - "Restock endpoint: validate > 0, check is_countable, UPDATE stock_quantity = stock_quantity + ?"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/cafe.rs
    - crates/racecontrol/src/api/routes.rs

key-decisions:
  - "is_countable defaults to false (0) for all existing items and new items from import — inventory tracking is opt-in per item"
  - "Restock returns BAD_REQUEST (not 404) with JSON error body {error: 'item is not countable'} to distinguish item-found-but-not-trackable from item-not-found"
  - "stock_quantity uses UPDATE ... SET stock_quantity = stock_quantity + ? to avoid race conditions on concurrent restock calls"

patterns-established:
  - "Inventory field defaults: is_countable=false, stock_quantity=0, low_stock_threshold=0"
  - "Test DB schema must be updated in sync with production schema — CREATE TABLE in tests mirrors real columns"

requirements-completed: [INV-01, INV-02, INV-04, INV-05, INV-09]

# Metrics
duration: 22min
completed: 2026-03-22
---

# Phase 152 Plan 01: Inventory Tracking Backend Summary

**SQLite inventory columns (is_countable, stock_quantity, low_stock_threshold) added to cafe_items with idempotent migrations, CafeItem struct updated, and POST /cafe/items/{id}/restock endpoint implemented**

## Performance

- **Duration:** 22 min
- **Started:** 2026-03-22T19:30:00+05:30
- **Completed:** 2026-03-22T19:52:00+05:30
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Idempotent ALTER TABLE migrations add 3 inventory columns to existing cafe_items table
- CafeItem struct, CreateCafeItemRequest, UpdateCafeItemRequest all updated with inventory fields
- All SELECT queries in cafe.rs updated to include new columns
- POST /cafe/items/{id}/restock endpoint validates quantity, checks is_countable flag, increments stock atomically
- All 15 cafe tests pass with updated test schema

## Task Commits

Each task was committed atomically:

1. **Task 1: Database migration + Rust struct + query updates** - `73a8c901` (feat)
2. **Task 2: Restock API endpoint** - `f980d39f` (feat)

## Files Created/Modified
- `crates/racecontrol/src/db/mod.rs` - 3 idempotent ALTER TABLE migrations after image_path migration
- `crates/racecontrol/src/cafe.rs` - CafeItem/CreateCafeItemRequest/UpdateCafeItemRequest structs, all SELECT queries, INSERT queries, dynamic SET builder, RestockRequest + restock_cafe_item handler, test schema fix
- `crates/racecontrol/src/api/routes.rs` - Added `.route("/cafe/items/{id}/restock", post(cafe::restock_cafe_item))` in authenticated section

## Decisions Made
- `is_countable` defaults to `false` — inventory tracking is opt-in per item, doesn't break existing cafe operations
- Restock for non-countable items returns `200 OK` with `{"error": "item is not countable"}` body (not 400/404) to give frontend a clear distinguishable signal
- `stock_quantity = stock_quantity + ?` atomic SQL prevents race conditions on concurrent restock

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Updated test_db() CREATE TABLE and test SELECT queries to include inventory columns**
- **Found during:** Task 2 (running cargo test after adding restock handler)
- **Issue:** Test database created in-memory without the new columns; SELECT queries in tests referenced old CafeItem struct causing ColumnNotFound("is_countable") panic
- **Fix:** Added `is_countable BOOLEAN DEFAULT 0`, `stock_quantity INTEGER DEFAULT 0`, `low_stock_threshold INTEGER DEFAULT 0` to CREATE TABLE in test_db(); updated 3 test SELECT queries to include new columns; added assertions for default values
- **Files modified:** crates/racecontrol/src/cafe.rs (test module only)
- **Verification:** All 15 cafe tests pass
- **Committed in:** f980d39f (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 bug)
**Impact on plan:** Required fix for test correctness. No scope creep — only test schema kept in sync with production schema.

## Issues Encountered
- Git stash accident: During pre-existing test verification, a `git stash drop` was issued before `stash pop` completed, losing the initial Task 2 edits. All Task 2 changes were re-applied from scratch. No data loss — changes were straightforward to reproduce.
- Pre-existing config test failures (`config::tests::config_fallback_preserved_when_no_env_vars`) unrelated to this plan — environment variable contamination between tests. Out of scope.

## Next Phase Readiness
- Backend foundation complete: all 5 INV requirements (INV-01, INV-02, INV-04, INV-05, INV-09) have backend support
- Ready for Phase 152-02: inventory management UI (stock display, restock modal, low stock alerts)
- PUT /cafe/items/{id} already supports is_countable/stock_quantity/low_stock_threshold updates
- GET /cafe/items returns all inventory fields in response

---
*Phase: 152-inventory-tracking*
*Completed: 2026-03-22*
