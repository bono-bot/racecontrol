---
phase: 150-menu-import
plan: "01"
subsystem: cafe-backend
tags: [rust, axum, menu-import, xlsx, csv, image-upload, static-files]
dependency_graph:
  requires: []
  provides: [cafe-import-api, cafe-image-upload-api, cafe-images-static-serve]
  affects: [cafe-admin-frontend]
tech_stack:
  added: [calamine@0.34, csv@1.4, image@0.25, axum-multipart-feature]
  patterns: [two-stage-import, transactional-bulk-insert, category-auto-create, ServeDir-static-serve, image-resize-jpeg]
key_files:
  created: []
  modified:
    - crates/racecontrol/Cargo.toml
    - crates/racecontrol/src/cafe.rs
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/src/main.rs
key_decisions:
  - "calamine::Data enum used (not DataType which is a trait in 0.34)"
  - "old_image stored as Option<String> directly (no nested Option needed)"
  - "XLSX unit test deferred to integration — constructing minimal XLSX bytes inline requires rust_xlsxwriter; CSV path tested thoroughly instead"
  - "ServeDir mounted using tower_http::services::ServeDir qualified path (no new use statement needed)"
metrics:
  duration_seconds: 591
  completed_date: "2026-03-22"
  tasks_completed: 2
  files_modified: 5
requirements: [MENU-01, MENU-06]
---

# Phase 150 Plan 01: Menu Import Backend Summary

Backend bulk import (XLSX/CSV) and image upload for cafe menu with pure parsing functions, transactional confirm, and static file serving.

## What Was Built

**3 new Axum handlers:**
- `POST /api/v1/cafe/import/preview` — accepts multipart file (.xlsx or .csv), returns column mapping + per-row validation results (valid/invalid flags + error messages)
- `POST /api/v1/cafe/import/confirm` — accepts JSON array of confirmed rows, inserts all in a single SQLite transaction with automatic category creation, rolls back on any error
- `POST /api/v1/cafe/items/{id}/image` — accepts multipart image, decodes with `image` crate, resizes to max 800px (Lanczos3), saves as JPEG to `./data/cafe-images/`, returns `{ "image_url": "/static/cafe-images/{filename}" }`

**Static file serving:**
- `/static/cafe-images` mounted via `tower_http::services::ServeDir` — serves uploaded images with ETags and Content-Type automatically

**Pure parsing functions (unit-tested):**
- `normalize_header` — lowercase + alphanumeric filter for fuzzy column detection
- `detect_column` — maps normalized header to known field name (name/category/selling_price/cost_price/description)
- `validate_import_row` — checks name non-empty, selling_price > 0, cost_price >= 0
- `parse_xlsx_bytes` — calamine-based XLSX parser with float-to-integer price coercion
- `parse_csv_bytes` — csv-crate parser with BOM stripping from first header
- `confirm_import_rows` — transactional bulk insert with category auto-creation

**DB schema:**
- `ALTER TABLE cafe_items ADD COLUMN image_path TEXT` (idempotent migration, ignores error on second run)
- `CafeItem` struct updated with `pub image_path: Option<String>`
- All existing SELECT queries updated to include `image_path`

## Tests

15 cafe::tests pass (10 new + 5 existing):

| Test | Coverage |
|------|----------|
| test_normalize_header | normalize_header function |
| test_detect_column | detect_column mapping |
| test_validate_import_row_valid | valid row returns no errors |
| test_validate_import_row_empty_name | empty name flagged |
| test_validate_import_row_zero_price | zero selling_price flagged |
| test_parse_csv_bytes | CSV with BOM: header stripped, rows parsed |
| test_parse_csv_bytes_no_bom | CSV without BOM: normal parse |
| test_image_path_column | image_path INSERT+SELECT roundtrip |
| test_import_confirm_transaction | 3 rows inserted atomically |
| test_import_creates_categories | category auto-created from import |
| test_create_and_list_items | (existing, updated for image_path) |
| test_is_available_filter | (existing, updated for image_path) |
| test_foreign_key_enforcement | (existing) |
| test_category_unique_constraint | (existing) |
| test_toggle_availability | (existing) |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] calamine 0.34 uses `Data` enum, not `DataType` pattern match**
- **Found during:** Task 1 GREEN phase (compile error)
- **Issue:** Plan and research examples used `DataType::Float`, `DataType::String` etc. In calamine 0.34, `DataType` is a trait; the concrete enum is `Data`. Pattern matching on traits is not valid Rust.
- **Fix:** Changed all pattern matches from `DataType::*` to `Data::*`
- **Files modified:** crates/racecontrol/src/cafe.rs
- **Commit:** a10d2470

**2. [Rule 1 - Bug] Option::flatten() called on non-nested Option**
- **Found during:** Task 1 GREEN phase (compile error)
- **Issue:** `old_image` was typed `Option<String>` (query_scalar + fetch_optional + ok_or(NOT_FOUND) resolves to `Option<String>`). Calling `.flatten()` on it failed.
- **Fix:** Removed `.flatten()` call — used `if let Some(old_path) = old_image` directly
- **Files modified:** crates/racecontrol/src/cafe.rs
- **Commit:** a10d2470

## Self-Check: PASSED

Files verified:
- crates/racecontrol/src/cafe.rs — exists, contains pub async fn import_preview, confirm_import, upload_item_image
- crates/racecontrol/src/api/routes.rs — contains cafe/import/preview, cafe/import/confirm, cafe/items/{id}/image
- crates/racecontrol/src/main.rs — contains nest_service cafe-images ServeDir
- crates/racecontrol/src/db/mod.rs — contains image_path TEXT migration

Commits verified:
- a10d2470 — feat(150-01): add menu import parsing, validation, DB migration + 15 tests
- 08322fa2 — feat(150-01): add import/image Axum handlers, routes, static serving
