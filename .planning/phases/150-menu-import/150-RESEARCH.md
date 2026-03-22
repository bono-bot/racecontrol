# Phase 150: Menu Import - Research

**Researched:** 2026-03-22
**Domain:** Axum multipart file upload, Rust XLSX/CSV parsing, server-side image resize, Next.js file upload modal
**Confidence:** HIGH

## Summary

Phase 150 adds two independent features to the existing cafe admin: (1) bulk import of menu items from Excel/CSV files with a preview-and-confirm flow, and (2) per-item image upload stored locally at `data/cafe-images/` and served as static files.

The Rust side uses `calamine` (0.34.0) for XLSX and `csv` (1.4.0) for CSV — both pure-Rust, no native deps, clean integration with Axum's `multipart` feature. Image upload uses `axum::extract::Multipart` to receive files, the `image` crate (0.25.10) for resize, and `tower-http ServeDir` to serve the `data/cafe-images/` directory as static files. The DB schema needs one additional column: `image_path TEXT` on `cafe_items`.

The frontend uses browser-native `FileReader` API to read the file and send it via `fetch` with `FormData` — no new npm packages needed for import. The existing page (407 LOC) gains an Import button that opens a modal with a preview table, plus an image column in the items table.

**Primary recommendation:** Parse files server-side in Rust (calamine + csv crates), keep frontend as a thin file-upload + preview-render layer with no parsing logic. For image resize, use the `image` crate server-side — no npm dependency on `sharp` needed.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Support Excel (.xlsx) and CSV formats — no PDF
- Upload → parse → show preview table → admin reviews/edits invalid rows → confirm to import
- Invalid rows flagged inline (highlighted in red) — admin can fix before confirming
- Auto-detect column mapping by header names with fuzzy matching, allow manual column override
- Store images in local filesystem: `data/cafe-images/` directory on server
- Serve as static files via Axum static file handler
- Accept JPEG, PNG, WebP — resize to max 800px width on upload
- Per-item upload button (camera icon) in admin table — click to upload/replace image
- Import button on existing /cafe page — opens a modal with file upload area + preview table
- No separate import page needed
- Categories auto-created from spreadsheet's category column if they don't already exist

### Claude's Discretion
- Exact fuzzy matching algorithm for column header detection
- Preview table pagination/scrolling for large imports
- Image resize library choice (sharp vs jimp vs browser-side)
- Error message wording for parse failures

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| MENU-01 | Admin can upload cafe items from PDF or spreadsheet (name, price, category, cost price, description) with preview-and-confirm flow | calamine + csv crates for server-side parsing; Axum multipart for upload; preview response shape defined in Architecture Patterns |
| MENU-06 | Admin can upload item images that display in PWA and POS | axum Multipart + image crate for resize; ServeDir for static serving; image_path column on cafe_items; frontend per-row camera icon trigger |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| calamine | 0.34.0 | Parse .xlsx files in pure Rust | Pure Rust, no native deps, handles Excel formats (xlsx/xlsm/xls), strong ecosystem use |
| csv | 1.4.0 | Parse .csv files with serde integration | The standard Rust CSV crate, supports flexible delimiters, BOM stripping |
| image | 0.25.10 | Decode/resize/encode JPEG/PNG/WebP | Pure Rust, supports all required formats, resize API is trivial |
| axum (multipart feature) | 0.8 (already in Cargo.toml) | Receive file uploads via multipart/form-data | Built into axum, no extra dependency |
| tower-http ServeDir | 0.6 (already in Cargo.toml) | Serve data/cafe-images/ as static files | Already used for cors/trace, fs feature just needs enabling |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| uuid | already present | Generate image filenames | Collision-free filenames |
| tokio::fs | stdlib (tokio already present) | Write image bytes to disk async | Async file I/O pattern already in project |

### Frontend — No New npm Packages Needed
Browser `FileReader` + `fetch` + `FormData` covers file upload. The import preview is pure React state. No xlsx/csv parsing in the browser — parsing happens server-side.

**Installation (Rust — add to crates/racecontrol/Cargo.toml):**
```toml
calamine = "0.34"
csv = "1.4"
image = { version = "0.25", default-features = false, features = ["jpeg", "png", "webp"] }
```

**Axum multipart feature (update existing axum entry):**
```toml
axum = { version = "0.8", features = ["ws", "macros", "multipart"] }
```

**tower-http fs feature (update existing tower-http entry):**
```toml
tower-http = { version = "0.6", features = ["cors", "fs", "trace"] }
```
Note: `fs` is already listed in Cargo.toml — no change needed there.

**Version verification:** Confirmed against crates.io registry on 2026-03-22.

## Architecture Patterns

### Recommended Project Structure Changes
```
crates/racecontrol/src/
├── cafe.rs              # ADD: bulk_import_preview, confirm_bulk_import, upload_item_image handlers
├── db/mod.rs            # ADD: image_path TEXT column to cafe_items + ALTER TABLE migration
└── api/routes.rs        # ADD: 3 new routes for import + image upload

data/
└── cafe-images/         # CREATE: image storage directory (served as static files)

web/src/app/cafe/
└── page.tsx             # ADD: Import button + modal, image column in table
web/src/lib/
└── api.ts               # ADD: CafeItem.image_path field, importCafePreview, confirmCafeImport, uploadCafeItemImage methods
```

### Pattern 1: Two-Stage Import (Preview then Confirm)
**What:** First POST returns a preview (parsed rows + validation errors, nothing written to DB). Second POST with confirmed rows writes to DB.
**When to use:** Any bulk import where admin must review before committing.

**Preview endpoint response shape:**
```rust
// Source: established project convention + CONTEXT.md decision
{
  "preview_id": "uuid",   // opaque token for confirm step
  "rows": [
    {
      "row_num": 1,
      "name": "Espresso",
      "category": "Beverages",
      "selling_price_rupees": "150",
      "cost_price_rupees": "40",
      "description": "...",
      "valid": true,
      "errors": []
    },
    {
      "row_num": 2,
      "name": "",
      "category": "Meals",
      "selling_price_rupees": "0",
      "cost_price_rupees": "5",
      "description": null,
      "valid": false,
      "errors": ["name is required", "selling_price must be > 0"]
    }
  ],
  "total_rows": 50,
  "valid_rows": 48,
  "invalid_rows": 2
}
```

**Confirm endpoint request shape:**
```rust
// Admin sends back corrected/filtered rows
POST /api/v1/cafe/import/confirm
{
  "rows": [
    {
      "name": "Espresso",
      "category": "Beverages",     // category name — auto-created if not exists
      "selling_price_paise": 15000,
      "cost_price_paise": 4000,
      "description": "..."
    }
    // ... only valid rows, admin has removed/fixed invalid ones
  ]
}
```

### Pattern 2: Preview State — In-Memory (No DB Storage)
**What:** Preview is held in server memory (or re-parsed from original upload on confirm). No preview_id needed if the frontend sends the confirmed rows directly.
**Recommended approach:** Skip the preview_id/token design. Frontend receives the parsed rows from the preview endpoint, admin edits in the modal, then POSTs the corrected rows to `/confirm`. This avoids server-side state management entirely.

This is simpler than the preview_id approach and matches the CONTEXT.md flow: "parse → show preview → admin reviews/edits → confirm".

### Pattern 3: Column Auto-Detection with Fuzzy Matching
**What:** Normalize header names to lowercase ASCII, strip spaces/underscores/hyphens, then map to known column names.

```rust
// Source: project discretion — simple normalization algorithm
fn normalize_header(h: &str) -> String {
    h.to_lowercase()
     .chars()
     .filter(|c| c.is_alphanumeric())
     .collect()
}

fn detect_column(normalized: &str) -> Option<&'static str> {
    match normalized {
        "name" | "itemname" | "item" | "productname" => Some("name"),
        "category" | "cat" | "categoryname" | "group" => Some("category"),
        "sellingprice" | "price" | "sp" | "mrp" | "rate" => Some("selling_price"),
        "costprice" | "cost" | "cp" | "purchaseprice" => Some("cost_price"),
        "description" | "desc" | "details" => Some("description"),
        _ => None,
    }
}
```

**Confidence:** MEDIUM — algorithm is Claude's discretion per CONTEXT.md. The key insight is that normalization (lowercase + alphanumeric only) makes matching robust to typical spreadsheet variations.

### Pattern 4: Image Upload and Serving
**What:** Axum Multipart handler reads bytes, `image` crate decodes and resizes, `tokio::fs::write` saves file, ServeDir serves static.

```rust
// Source: axum 0.8 docs + tower-http ServeDir docs
// In cafe.rs:
use axum::extract::Multipart;
use tower_http::services::ServeDir;

pub async fn upload_item_image(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // 1. Read field bytes from multipart
    // 2. Decode with image::load_from_memory()
    // 3. Resize: img.resize(800, u32::MAX, image::imageops::FilterType::Lanczos3)
    //    — or use thumbnail() which preserves aspect ratio
    // 4. Encode back to JPEG bytes (always normalize to JPEG for storage)
    // 5. Generate filename: format!("{}.jpg", uuid::Uuid::new_v4())
    // 6. tokio::fs::create_dir_all("./data/cafe-images").await?
    // 7. tokio::fs::write(path, bytes).await?
    // 8. UPDATE cafe_items SET image_path = ?, updated_at = datetime('now') WHERE id = ?
    // 9. Return { "image_url": "/static/cafe-images/{filename}" }
}

// In main.rs — mount static file handler:
.nest_service("/static/cafe-images", ServeDir::new("./data/cafe-images"))
```

### Pattern 5: DB Schema Migration for image_path
**What:** SQLite `ALTER TABLE` for adding a nullable column. Must be idempotent (use try/ignore on error).

```rust
// Source: SQLite docs — ALTER TABLE ADD COLUMN is safe and idempotent check pattern
// In db/mod.rs migrate() — append after existing cafe table creation:
let _ = sqlx::query(
    "ALTER TABLE cafe_items ADD COLUMN image_path TEXT"
)
.execute(pool)
.await;
// Intentionally ignore error — column already exists on second run
```

This is the established SQLite migration pattern for adding columns: `ALTER TABLE ADD COLUMN` silently fails if the column already exists (returns error, which we discard).

### Pattern 6: calamine XLSX Parsing
```rust
// Source: calamine 0.34.0 README
use calamine::{Reader, open_workbook_from_rs, Xlsx, DataType};
use std::io::Cursor;

// bytes: Vec<u8> from multipart
let cursor = Cursor::new(bytes);
let mut workbook: Xlsx<_> = open_workbook_from_rs(cursor)
    .map_err(|_| StatusCode::BAD_REQUEST)?;

let sheet = workbook.worksheet_range_at(0)
    .ok_or(StatusCode::BAD_REQUEST)?
    .map_err(|_| StatusCode::BAD_REQUEST)?;

// First row = headers
let mut rows = sheet.rows();
let headers: Vec<String> = rows.next()
    .unwrap_or_default()
    .iter()
    .map(|c| c.to_string())
    .collect();

for row in rows {
    // row is &[DataType]
    let cell_str = |i: usize| row.get(i).map(|d| d.to_string()).unwrap_or_default();
}
```

### Pattern 7: csv Crate Parsing
```rust
// Source: csv 1.4.0 docs
use csv::ReaderBuilder;

// bytes: Vec<u8> from multipart
let mut reader = ReaderBuilder::new()
    .has_headers(true)
    .flexible(true)  // allow rows with different column counts
    .from_reader(bytes.as_slice());

let headers: Vec<String> = reader.headers()
    .map_err(|_| StatusCode::BAD_REQUEST)?
    .iter()
    .map(|h| h.to_string())
    .collect();

for result in reader.records() {
    let record = result.map_err(|_| StatusCode::BAD_REQUEST)?;
    // record.get(0) -> Option<&str>
}
```

### Anti-Patterns to Avoid
- **Storing preview state server-side:** Don't use a preview_id/session approach — have the frontend re-send the confirmed rows in the confirm POST. Simpler, no memory leak risk.
- **Storing images in SQLite as blobs:** We store path only in DB, images on filesystem — follows the CONTEXT.md decision and avoids DB bloat.
- **Parsing spreadsheets in the browser (JS):** Server-side Rust parsing is more reliable (encoding, cell types), consistent with backend validation, and keeps frontend thin.
- **Using unwrap() anywhere:** All calamine/csv errors must be mapped to StatusCode. No `.unwrap()` per project rules.
- **Serving images via a proxy handler:** Use `ServeDir` (static file middleware), not an Axum handler that reads the file. ServeDir handles ETags, range requests, and content-type automatically.
- **Forgetting to add `multipart` to axum features:** The `Multipart` extractor is behind a feature flag. Without it, compile fails with a cryptic error.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| XLSX parsing | Custom zip+XML parser | calamine 0.34 | Handles multiple Excel formats, cell types, merged cells, BOM |
| CSV parsing | Manual split(",") | csv 1.4 | Handles quoted fields, embedded newlines, BOM, encoding variants |
| Image decode/resize | Custom pixel math | image 0.25 | Format detection, color space handling, multiple resize algorithms |
| Static file serving | Custom file-read handler | ServeDir (tower-http) | ETags, Content-Type, Range, If-Modified-Since all handled |

**Key insight:** XLSX files are zip archives containing XML — the format has dozens of edge cases (shared strings table, inline strings, date types, number formats). calamine handles all of this.

## Common Pitfalls

### Pitfall 1: calamine Cell Type Coercion
**What goes wrong:** Prices stored as numbers in Excel come back as `DataType::Float(150.0)`, not `DataType::String("150")`. Calling `.to_string()` on a float gives "150" but on a cell formatted as currency gives "150.00".
**Why it happens:** Excel stores all numbers as IEEE 754 floats internally.
**How to avoid:** When reading price columns, match on `DataType::Float(f)` and use `(*f * 100.0).round() as i64` to convert to paise. Fall back to string parsing only as a secondary path.
**Warning signs:** Import test with "150" price arrives as 15000000 paise (float * 100 without rounding).

### Pitfall 2: axum Multipart Feature Not Enabled
**What goes wrong:** `use axum::extract::Multipart` compiles only with `features = ["multipart"]`. Without it: compile error `no module named multipart`.
**Why it happens:** axum 0.8 gates Multipart behind a feature flag.
**How to avoid:** Update `axum = { version = "0.8", features = ["ws", "macros", "multipart"] }` in Cargo.toml before writing the handler.

### Pitfall 3: CSV BOM (Byte Order Mark)
**What goes wrong:** Excel-saved CSV files often start with a UTF-8 BOM (`\xEF\xBB\xBF`). The first column header becomes `"\u{feff}name"` instead of `"name"`, breaking column detection.
**Why it happens:** Excel adds BOM when saving "CSV UTF-8" format.
**How to avoid:** Strip BOM from first header: `header.trim_start_matches('\u{feff}')`. The `csv` crate does NOT strip BOM automatically.

### Pitfall 4: image Crate Feature Flags
**What goes wrong:** `image` crate by default compiles many format decoders (gif, tiff, bmp, ico, etc.) adding ~2MB to binary and slow compile times.
**Why it happens:** Default features include all formats.
**How to avoid:** Use `default-features = false, features = ["jpeg", "png", "webp"]` in Cargo.toml.

### Pitfall 5: ServeDir Path Resolution
**What goes wrong:** `ServeDir::new("./data/cafe-images")` resolves relative to the process CWD, which is the `crates/racecontrol/` directory during dev but `C:\RacingPoint\` in production.
**Why it happens:** Relative paths in Rust resolve from CWD at runtime, not at compile time.
**How to avoid:** Use the same `./data/` convention the project already uses (confirmed: `./data/email_verified.flag` is written at `FLAG_PATH: &str = "./data/email_verified.flag"` in main.rs). The production server runs from `C:\RacingPoint\` where `./data/` already exists. This is consistent.

### Pitfall 6: Large Import Atomicity
**What goes wrong:** If the confirm endpoint inserts 100 items and fails on row 47, the DB has a partial import.
**Why it happens:** Each INSERT is a separate DB call.
**How to avoid:** Wrap the entire confirm insert loop in a SQLite transaction: `let mut tx = state.db.begin().await?;` ... `tx.commit().await?;`. On any error, the transaction rolls back automatically.

### Pitfall 7: image_path Column Not In CafeItem Struct
**What goes wrong:** After adding `image_path TEXT` to DB, the existing `sqlx::query_as::<_, CafeItem>` queries fail because the struct doesn't have the field.
**Why it happens:** sqlx FromRow requires struct fields to match SELECT columns.
**How to avoid:** Add `pub image_path: Option<String>` to the `CafeItem` struct in cafe.rs AND add `image_path` to all SELECT * queries that use `CafeItem`. Also add to the TypeScript `CafeItem` interface in api.ts.

### Pitfall 8: Frontend FormData File Upload
**What goes wrong:** Sending a file with `fetch` + `JSON.stringify()` doesn't work. Must use `FormData` without `Content-Type` header (browser sets multipart boundary automatically).
**Why it happens:** Setting `Content-Type: application/json` on a multipart request corrupts the boundary.
**How to avoid:**
```typescript
const fd = new FormData();
fd.append("file", file);
// DO NOT set Content-Type header — let browser set it
await fetch(`/api/v1/cafe/items/${id}/image`, { method: "POST", body: fd });
```

## Code Examples

### Axum Multipart Handler Skeleton
```rust
// Source: axum 0.8 multipart extractor docs
use axum::extract::Multipart;

pub async fn upload_item_image(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut image_bytes: Option<Vec<u8>> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        tracing::warn!("multipart error: {}", e);
        StatusCode::BAD_REQUEST
    })? {
        if field.name() == Some("file") {
            image_bytes = Some(field.bytes().await.map_err(|_| StatusCode::BAD_REQUEST)?.to_vec());
        }
    }

    let bytes = image_bytes.ok_or(StatusCode::BAD_REQUEST)?;
    // ... decode/resize/save
    Ok(Json(serde_json::json!({ "image_url": "/static/cafe-images/xxx.jpg" })))
}
```

### image Crate Resize
```rust
// Source: image 0.25 docs
use image::{ImageFormat, imageops::FilterType, DynamicImage};
use std::io::Cursor;

let img = image::load_from_memory(&bytes).map_err(|_| StatusCode::BAD_REQUEST)?;
let resized = if img.width() > 800 {
    img.resize(800, u32::MAX, FilterType::Lanczos3)
} else {
    img
};
let mut out = Cursor::new(Vec::new());
resized.write_to(&mut out, ImageFormat::Jpeg).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
let jpeg_bytes = out.into_inner();
```

### ServeDir Mount in main.rs
```rust
// Source: tower-http 0.6 ServeDir docs
use tower_http::services::ServeDir;

// In the Router::new() chain, before .layer() calls:
.nest_service("/static/cafe-images", ServeDir::new("./data/cafe-images"))
```

### SQLite Transaction for Bulk Insert
```rust
// Source: sqlx 0.8 transaction docs
let mut tx = state.db.begin().await.map_err(|e| {
    tracing::warn!("begin tx error: {}", e);
    StatusCode::INTERNAL_SERVER_ERROR
})?;

for row in &confirmed_rows {
    sqlx::query("INSERT INTO cafe_items (...) VALUES (...)")
        .bind(...)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            tracing::warn!("insert error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
}

tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
```

### Frontend File Upload (TypeScript)
```typescript
// No any — typed FormData upload
async function uploadImage(itemId: string, file: File): Promise<string> {
  const fd = new FormData();
  fd.append("file", file);
  const res = await fetch(`${API_BASE}/cafe/items/${itemId}/image`, {
    method: "POST",
    body: fd,
    // DO NOT set Content-Type
  });
  if (!res.ok) throw new Error("Image upload failed");
  const data = await res.json() as { image_url: string };
  return data.image_url;
}
```

### Frontend Import POST (TypeScript)
```typescript
// Parse preview (upload file as multipart)
async function importPreview(file: File): Promise<ImportPreview> {
  const fd = new FormData();
  fd.append("file", file);
  const res = await fetch(`${API_BASE}/cafe/import/preview`, {
    method: "POST",
    body: fd,
  });
  if (!res.ok) throw new Error("Import parse failed");
  return res.json() as Promise<ImportPreview>;
}

// Confirm import (send corrected rows as JSON)
async function importConfirm(rows: ConfirmedRow[]): Promise<{ imported: number }> {
  const res = await fetch(`${API_BASE}/cafe/import/confirm`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ rows }),
  });
  if (!res.ok) throw new Error("Import confirm failed");
  return res.json() as Promise<{ imported: number }>;
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| server-side xlsx: rust_xlsxwriter | calamine for reading | calamine has been standard for reads since ~2020 | calamine is read-only but lighter; rust_xlsxwriter is write-only |
| axum multipart: separate crate | axum built-in feature | axum 0.4+ | No extra dependency |
| image resize: imagemagick binding | pure Rust image crate | image crate stable since ~2019 | No system deps, works on Windows |

**Deprecated/outdated:**
- `xlsx` npm package (JS): Don't use browser-side parsing — server-side Rust is more reliable for this project.
- `multer` (Node.js): Not applicable — backend is Rust/Axum.

## Open Questions

1. **Column mapping override UI**
   - What we know: CONTEXT.md says "allow manual column override" after auto-detect
   - What's unclear: Is this a dropdown per column in the preview modal, or a separate mapping step before preview?
   - Recommendation: Keep it simple — show detected mapping as editable dropdowns above the preview table. Only needed when auto-detect fails (which is rare with fuzzy matching).

2. **Preview table scrolling for large imports**
   - What we know: CONTEXT.md marks this as Claude's discretion
   - What's unclear: How many rows a typical import will have (10? 200?)
   - Recommendation: Limit preview to first 100 rows displayed; show total row count. All rows are still sent to confirm, just not all displayed. Use `max-h-96 overflow-y-auto` on the table container.

3. **Image URL in API responses**
   - What we know: image_path stored in DB, served at `/static/cafe-images/`
   - What's unclear: Should `image_path` be stored as just the filename or the full URL path?
   - Recommendation: Store just the filename (e.g., `abc123.jpg`) in DB. API responses include `image_url: format!("/static/cafe-images/{}", filename)` computed at query time, or let the frontend construct the URL.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (tokio::test for async) |
| Config file | none — standard cargo test |
| Quick run command | `cargo test -p racecontrol -- cafe::tests` |
| Full suite command | `cargo test -p racecontrol` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| MENU-01 | XLSX parsing extracts rows with correct field mapping | unit | `cargo test -p racecontrol -- cafe::tests::test_parse_xlsx` | ❌ Wave 0 |
| MENU-01 | CSV parsing extracts rows with BOM stripping | unit | `cargo test -p racecontrol -- cafe::tests::test_parse_csv` | ❌ Wave 0 |
| MENU-01 | Validation flags empty name and zero price | unit | `cargo test -p racecontrol -- cafe::tests::test_import_validation` | ❌ Wave 0 |
| MENU-01 | Confirm inserts valid rows in a transaction | unit | `cargo test -p racecontrol -- cafe::tests::test_import_confirm` | ❌ Wave 0 |
| MENU-01 | Categories auto-created from import | unit | `cargo test -p racecontrol -- cafe::tests::test_import_creates_categories` | ❌ Wave 0 |
| MENU-06 | image_path column added to cafe_items schema | unit | `cargo test -p racecontrol -- cafe::tests::test_image_path_column` | ❌ Wave 0 |
| MENU-06 | Image upload saves file and updates image_path in DB | integration (needs filesystem) | `cargo test -p racecontrol -- cafe::tests::test_upload_image` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol -- cafe::tests`
- **Per wave merge:** `cargo test -p racecontrol`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] Unit tests for `parse_xlsx_bytes()` and `parse_csv_bytes()` — pure functions testable without Axum
- [ ] Unit test for `validate_import_row()` — pure function
- [ ] Unit test for `test_import_confirm` — requires in-memory SQLite (same pattern as existing `test_db()`)
- [ ] Unit test for `test_import_creates_categories` — requires in-memory SQLite
- [ ] Unit test for `test_image_path_column` — verify schema has column
- [ ] Integration test for image upload — requires tmp dir; can use `std::env::temp_dir()`

*(Design recommendation: Extract parsing and validation to pure functions `parse_xlsx_bytes(bytes: &[u8]) -> Result<Vec<RawImportRow>, String>` and `validate_import_row(row: &RawImportRow) -> Vec<String>`. Pure functions are trivially unit-testable without an HTTP server.)*

## Sources

### Primary (HIGH confidence)
- calamine 0.34.0 crates.io + README — XLSX parsing API, `open_workbook_from_rs`, DataType variants
- csv 1.4.0 crates.io docs — ReaderBuilder, headers(), records()
- axum 0.8 docs — Multipart extractor, feature flag requirement
- image 0.25.10 crates.io — load_from_memory, resize, write_to, ImageFormat
- tower-http 0.6 ServeDir — nest_service pattern
- sqlx 0.8 — transaction begin/commit pattern
- Project source: `crates/racecontrol/src/cafe.rs` — existing CafeItem struct, handler patterns, test_db() harness
- Project source: `crates/racecontrol/src/main.rs` — `./data/` path convention, CORS config, Router structure
- Project source: `web/src/app/cafe/page.tsx` — existing 407-line page to extend
- Project source: `web/src/lib/api.ts` — CafeItem/CafeCategory interfaces, fetchApi pattern
- Project source: `crates/racecontrol/Cargo.toml` — confirmed axum = 0.8, tower-http = 0.6 with `fs` already listed

### Secondary (MEDIUM confidence)
- npm registry: xlsx@0.18.5, papaparse@5.5.3 — verified current versions (not used, but documented for reference)
- Cargo search: calamine@0.34.0, csv@1.4.0, image@0.25.10 — confirmed current versions 2026-03-22

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — verified crate versions from cargo search on 2026-03-22; axum multipart feature confirmed via `cargo add --dry-run`
- Architecture: HIGH — derived from existing project patterns (cafe.rs, main.rs data/ convention, routes.rs patterns)
- Pitfalls: HIGH — calamine DataType float, CSV BOM, and image_path struct sync are well-documented gotchas verified from library docs

**Research date:** 2026-03-22
**Valid until:** 2026-06-22 (stable crates, 90-day window reasonable)
