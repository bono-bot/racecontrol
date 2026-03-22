---
phase: 150-menu-import
verified: 2026-03-22T13:30:00+05:30
status: human_needed
score: 15/15 must-haves verified
re_verification: false
human_verification:
  - test: "Open /cafe admin page and click the Import button"
    expected: "A modal opens with a file upload area (drag-and-drop or click-to-browse) accepting .xlsx and .csv files"
    why_human: "Visual presence and click-target sizing cannot be verified from source code alone"
  - test: "Upload a CSV with 3 valid rows and 1 invalid row (empty name). Observe the preview table."
    expected: "Preview table shows 4 rows. The invalid row has a red background (bg-red-500/10) with error text. Summary line shows '3 valid, 1 invalid of 4 total rows'. Column mapping pills show detected column names."
    why_human: "Row rendering, conditional colour classes, and column-pill layout need visual confirmation"
  - test: "Click 'Import 3 Items'. Observe the items table after the modal closes."
    expected: "3 new items appear in the table. Each shows an 'No img' placeholder in the Image column and a camera icon."
    why_human: "End-to-end confirm flow requires a running backend; table re-render is not statically verifiable"
  - test: "Click the camera icon on any item row. Select a JPEG image larger than 800px wide."
    expected: "Thumbnail appears in the Image column (40x40 rounded). Refreshing the page shows the thumbnail persisted from /static/cafe-images/{uuid}.jpg"
    why_human: "Image resize (max 800px Lanczos3), JPEG encoding, file write, and static serving require a live backend and browser"
  - test: "Upload an XLSX file with the same column structure."
    expected: "Preview table parses correctly (same as CSV path). No format error."
    why_human: "XLSX parsing integration via calamine requires actual file bytes and cannot be fully covered by static analysis"
---

# Phase 150: Menu Import Verification Report

**Phase Goal:** Admin can populate the full cafe menu from existing PDF or spreadsheet files without manual item-by-item entry
**Verified:** 2026-03-22T13:30:00+05:30
**Status:** human_needed — all automated checks pass; 5 items require live visual/functional verification
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | POST /api/v1/cafe/import/preview with an XLSX file returns parsed rows with validation errors | VERIFIED | `pub async fn import_preview` at cafe.rs:657; parses `.xlsx` via `parse_xlsx_bytes`, runs `validate_import_row`, returns columns+rows+counts JSON |
| 2 | POST /api/v1/cafe/import/preview with a CSV file returns parsed rows with BOM stripped | VERIFIED | `parse_csv_bytes` at cafe.rs:222; BOM strip confirmed at line 237; `test_parse_csv_bytes` passes (BOM header asserted == "Item Name") |
| 3 | POST /api/v1/cafe/import/confirm inserts items atomically in a transaction | VERIFIED | `confirm_import_rows` at cafe.rs:280 uses `pool.begin()` + `tx.commit()`; `test_import_confirm_transaction` passes |
| 4 | Categories referenced in import are auto-created if they do not exist | VERIFIED | confirm_import_rows: SELECT cafe_categories WHERE name=?, if not found INSERT; `test_import_creates_categories` passes (COUNT == 1 asserted) |
| 5 | POST /api/v1/cafe/items/{id}/image accepts multipart image, resizes to max 800px, saves JPEG to data/cafe-images/ | VERIFIED | `upload_item_image` at cafe.rs:751; resize at line 797-798 (`img.resize(800, u32::MAX, Lanczos3)`); JPEG write confirmed at line 820 |
| 6 | GET /static/cafe-images/{filename} serves the saved image file | VERIFIED | `nest_service("/static/cafe-images", tower_http::services::ServeDir::new("./data/cafe-images"))` at main.rs:617 |
| 7 | image_path column exists on cafe_items and is returned in list/get responses | VERIFIED | `ALTER TABLE cafe_items ADD COLUMN image_path TEXT` at db/mod.rs:2418; `pub image_path: Option<String>` in CafeItem at cafe.rs:34; included in SELECT at lines 349, 492 |
| 8 | Admin sees an Import button on the /cafe page header | VERIFIED (code) | Button at page.tsx:242 with `onClick={() => setShowImportModal(true)}`; visual confirm needed |
| 9 | Clicking Import opens a modal with a file upload area | VERIFIED (code) | `showImportModal &&` conditional at page.tsx:544 renders modal with file input accepting `.xlsx,.csv`; visual confirm needed |
| 10 | Uploading XLSX/CSV shows preview table with validation errors highlighted red | VERIFIED (code) | `importPreview.rows.slice(0,100)` renders table at page.tsx:646 with `className={row.valid ? "" : "bg-red-500/10"}`; visual confirm needed |
| 11 | Admin can review detected column mapping | VERIFIED (code) | `importPreview.columns.map((col) => ...)` renders mapping pills at page.tsx:613 showing `mapped_to` or `?` for unmapped columns |
| 12 | Admin can confirm import which inserts all valid rows | VERIFIED (code) | `handleImportConfirm` at page.tsx:189 filters valid rows, calls `api.confirmCafeImport(validRows)`; functional confirm needed |
| 13 | Each item row has a camera icon button for image upload | VERIFIED (code) | Camera SVG + hidden file input at page.tsx:303-344; `handleImageUpload` called on file selection |
| 14 | image_path field appears in CafeItem TypeScript interface | VERIFIED | `image_path: string | null` at api.ts:48; `npx tsc --noEmit` exits 0 |
| 15 | 15/15 cafe::tests pass | VERIFIED | `cargo test -p racecontrol-crate -- cafe::tests` result: 15 passed, 0 failed |

**Score:** 15/15 truths verified (5 require live visual/functional confirmation)

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/racecontrol/src/cafe.rs` | import_preview, confirm_import, upload_item_image handlers + parse_xlsx_bytes, parse_csv_bytes, validate_import_row pure functions | VERIFIED | All 7 functions exist at confirmed line numbers; substantive implementations with real logic |
| `crates/racecontrol/src/api/routes.rs` | 3 new cafe routes: import/preview, import/confirm, items/{id}/image | VERIFIED | All 3 routes at lines 377-380 |
| `crates/racecontrol/src/db/mod.rs` | ALTER TABLE cafe_items ADD COLUMN image_path TEXT migration | VERIFIED | Idempotent migration at lines 2417-2418 |
| `crates/racecontrol/src/main.rs` | ServeDir mount for /static/cafe-images | VERIFIED | `nest_service` at line 617 |
| `crates/racecontrol/Cargo.toml` | calamine, csv, image deps; axum multipart feature | VERIFIED | calamine="0.34" at line 32, csv="1.4" at 33, image@0.25 at 34; axum multipart feature at line 27 |
| `web/src/lib/api.ts` | CafeItem.image_path, 4 new types, 3 new API methods | VERIFIED | image_path at line 48; ImportColumnMapping/ImportRowResult/ImportPreview/ConfirmedImportRow at lines 51-82; 3 methods at lines 318/330/337 |
| `web/src/app/cafe/page.tsx` | Import button + modal with preview table, image column with upload button | VERIFIED (code) | Import button at line 242; modal at line 544; image column with camera icon at lines 303-344 |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `crates/racecontrol/src/api/routes.rs` | `crates/racecontrol/src/cafe.rs` | route handler registration | WIRED | `cafe::import_preview`, `cafe::confirm_import`, `cafe::upload_item_image` all referenced in routes.rs at lines 377-380 |
| `crates/racecontrol/src/main.rs` | `data/cafe-images/` | ServeDir static file mount | WIRED | `nest_service("/static/cafe-images", tower_http::services::ServeDir::new("./data/cafe-images"))` at main.rs:617 |
| `crates/racecontrol/src/cafe.rs` | `crates/racecontrol/src/db/mod.rs` | image_path column in CafeItem struct and queries | WIRED | `pub image_path: Option<String>` at cafe.rs:34; included in SELECT at lines 349, 492; fetched in upload handler at line 758 |
| `web/src/app/cafe/page.tsx` | `/api/v1/cafe/import/preview` | FormData fetch in importCafePreview | WIRED | api.ts:321 `fetch(\`${API_BASE}/api/v1/cafe/import/preview\`)`; called from page.tsx:180 |
| `web/src/app/cafe/page.tsx` | `/api/v1/cafe/import/confirm` | JSON POST in confirmCafeImport | WIRED | api.ts:331 `fetchApi("/cafe/import/confirm")`; called from page.tsx:202 |
| `web/src/app/cafe/page.tsx` | `/api/v1/cafe/items/{id}/image` | FormData fetch in uploadCafeItemImage | WIRED | api.ts:340 `fetch(\`${API_BASE}/api/v1/cafe/items/${itemId}/image\`)`; called from page.tsx:165 |
| `web/src/app/cafe/page.tsx` | `/static/cafe-images/` | img src for item thumbnails | WIRED | page.tsx:305 `src={\`/static/cafe-images/${item.image_path}\`}` |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| MENU-01 | 150-01, 150-02 | Admin can upload cafe items from spreadsheet with preview-and-confirm flow | SATISFIED | Backend: import_preview + confirm_import endpoints with XLSX/CSV parsing, validation, atomic transaction. Frontend: Import modal with 2-step flow (upload → preview table → confirm). |
| MENU-06 | 150-01, 150-02 | Admin can upload item images that display in PWA and POS | SATISFIED | Backend: upload_item_image resizes to max 800px JPEG, stores in data/cafe-images/, returns image_url. Frontend: camera icon per row, thumbnail display from /static/cafe-images/. image_path in CafeItem returned by all list/get endpoints. |

No orphaned requirements — only MENU-01 and MENU-06 map to Phase 150 in REQUIREMENTS-v19.md. Both satisfied.

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| — | — | No unwrap() in production Rust | — | 0 occurrences found |
| — | — | No `any` in TypeScript | — | 0 occurrences found |
| — | — | No TODO/FIXME/placeholder stubs | — | 0 occurrences found |

No anti-patterns detected.

---

### Human Verification Required

#### 1. Import Modal Opens

**Test:** Navigate to http://192.168.31.23:3200/cafe. Verify "Import" button is visible next to "Add Item" in the page header.
**Expected:** A grey/dark button labelled "Import" is present. Clicking it opens a modal with a file upload area.
**Why human:** Visual placement and click-target cannot be verified from source code alone.

#### 2. CSV Preview with Validation

**Test:** Click Import. Upload a CSV file with columns: Name, Category, Selling Price, Cost Price, Description. Include one row with an empty Name field.
**Expected:** Preview table renders with the data. The row with empty Name has a red-tinted background. Summary shows valid/invalid counts. Column mapping pills appear above the table showing detected field names.
**Why human:** Row colour classes, pill rendering, and conditional display require a running browser.

#### 3. Confirm Import End-to-End

**Test:** After seeing the preview, click "Import N Items".
**Expected:** Modal closes, items table reloads, new items appear in the cafe admin table. Each new item shows a "No img" placeholder and a camera icon in the Image column.
**Why human:** Requires a running backend with SQLite and the confirm endpoint live.

#### 4. Image Upload and Persistence

**Test:** Click the camera icon on any item. Select a JPEG image wider than 800px.
**Expected:** A 40x40 thumbnail appears in the Image column immediately. Refreshing the page still shows the thumbnail (served from /static/cafe-images/{uuid}.jpg).
**Why human:** Requires backend image processing pipeline (decode → resize → encode → write) and static file serving to be live.

#### 5. XLSX File Upload

**Test:** Repeat the import flow with a .xlsx file instead of .csv.
**Expected:** Preview table parses correctly with the same results as CSV.
**Why human:** XLSX parsing via calamine with real file bytes cannot be fully covered by static grep analysis.

---

### Gaps Summary

No gaps found. All 15 must-have truths are verified against actual code. All 7 required artifacts exist, are substantive, and are wired. Both requirement IDs (MENU-01, MENU-06) are fully satisfied. All 15 cafe tests pass (verified by `cargo test`). TypeScript compiles cleanly (`npx tsc --noEmit` exits 0). No code quality violations found (0 unwrap(), 0 any, 0 stubs).

The 5 human verification items are required because they test visual rendering, end-to-end browser-backend interaction, and file I/O — none of which can be confirmed through static analysis. All automated checks pass.

---

_Verified: 2026-03-22T13:30:00+05:30_
_Verifier: Claude (gsd-verifier)_
