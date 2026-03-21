# Phase 115: Face Enrollment System - Research

**Researched:** 2026-03-21
**Domain:** Face enrollment CRUD API, multi-angle capture, embedding persistence, gallery sync
**Confidence:** HIGH

## Summary

Phase 115 builds the enrollment API on top of a mature existing codebase. The recognition infrastructure (SQLite CRUD in `db.rs`, in-memory Gallery in `gallery.rs`, QualityGates in `quality.rs`, ArcFace embedding extraction in `arcface.rs`, face alignment in `alignment.rs`, CLAHE lighting normalization in `clahe.rs`) was all built in Phase 114. The Axum HTTP server already runs at `:8096` with health and privacy routes. The privacy audit system (`AuditWriter` with JSONL append) is operational.

The work is entirely about wiring these existing components into new Axum HTTP handlers: person CRUD, photo upload with quality validation and embedding extraction, multi-angle enrollment sessions, gallery hot-reload, and duplicate detection via cosine similarity against existing embeddings.

**Primary recommendation:** Build a new `enrollment` module with Axum handlers that compose existing `db`, `gallery`, `quality`, `arcface`, `alignment`, and `clahe` modules. Share state via a new `EnrollmentState` struct containing `Arc<Gallery>`, `Arc<ArcfaceRecognizer>`, `Arc<AuditWriter>`, and a `rusqlite::Connection` pool (or `Arc<Mutex<Connection>>`).

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
None explicitly locked -- all implementation choices at Claude's discretion.

Key constraints from context:
- SQLite database already exists (recognition/db.rs from Phase 114 has persons + embeddings tables)
- Gallery already exists (recognition/gallery.rs with in-memory Vec + load_from_db)
- Quality gates already exist (recognition/quality.rs)
- ArcFace already exists (recognition/arcface.rs)
- Axum HTTP server already at :8096
- Privacy audit log must be used for all enrollment operations (DPDP compliance)
- API endpoints should follow existing pattern (e.g., /api/v1/privacy/*)

### Claude's Discretion
All implementation choices are at Claude's discretion -- infrastructure phase building on established patterns from Phase 114.

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| ENRL-01 | Face profile management (add/remove/update face photos) | Person CRUD in db.rs (insert_person), Gallery sync (reload/add_entry), AuditWriter for compliance logging, existing deletion handler in privacy/deletion.rs to extend |
| ENRL-02 | Multi-angle enrollment capture for better recognition accuracy | QualityGates.check() for frame validation, ArcFace extract_embedding() for 512-D vectors, alignment + CLAHE pipeline, cosine_similarity for duplicate detection |
</phase_requirements>

## Standard Stack

### Core (Already in Cargo.toml)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| axum | 0.7 | HTTP API framework | Already used for health + privacy routes |
| rusqlite | 0.32 (bundled) | SQLite persistence | Already used for persons + face_embeddings tables |
| tokio | workspace | Async runtime | Already the runtime |
| serde / serde_json | workspace | JSON serialization | Already used throughout |
| image | 0.25 | Image decoding (JPEG/PNG upload) | Already a dependency for face alignment |
| chrono | workspace | Timestamps | Already used in RecognitionResult |
| uuid | workspace | Unique IDs for enrollment sessions | Already a dependency |
| anyhow | workspace | Error handling | Already used throughout |
| tracing | workspace | Structured logging | Already used throughout |

### Supporting (Already Available)
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| ort | 2.0.0-rc.12 | ONNX Runtime for ArcFace inference | Embedding extraction during enrollment |
| ndarray | 0.17 | Tensor creation for ArcFace input | Preprocessing aligned face crops |
| imageproc | 0.26 | Geometric transformations | Face alignment via similarity transform |
| clahe | 0.1 | Lighting normalization | CLAHE before ArcFace inference |

### No New Dependencies Needed
All required functionality is covered by existing dependencies. No new crates required.

## Architecture Patterns

### Recommended Module Structure
```
crates/rc-sentry-ai/src/
  enrollment/
    mod.rs          # pub mod declarations
    routes.rs       # Axum handlers (POST/GET/PUT/DELETE)
    service.rs      # Business logic (enroll, update, delete, duplicate check)
    types.rs        # Request/response DTOs
  recognition/
    db.rs           # EXTEND: add phone column, delete_person, update_person, get_person, list_persons
    gallery.rs      # EXTEND: add add_entry() and remove_person() methods
  health.rs         # EXTEND: merge enrollment_router into app
  main.rs           # EXTEND: initialize EnrollmentState, merge router
```

### Pattern 1: Enrollment State Sharing
**What:** Single `EnrollmentState` struct shared across all enrollment handlers via Axum's `State` extractor.
**When to use:** Every enrollment route needs access to DB, Gallery, ArcFace, and AuditWriter.
**Example:**
```rust
pub struct EnrollmentState {
    pub db: Arc<Mutex<rusqlite::Connection>>,
    pub gallery: Arc<Gallery>,
    pub recognizer: Arc<ArcfaceRecognizer>,
    pub audit: Arc<AuditWriter>,
    pub quality_gates: QualityGates,
}

pub fn enrollment_router(state: Arc<EnrollmentState>) -> axum::Router {
    axum::Router::new()
        .route("/api/v1/enrollment/persons", post(create_person).get(list_persons))
        .route("/api/v1/enrollment/persons/:person_id", get(get_person).put(update_person).delete(delete_person))
        .route("/api/v1/enrollment/persons/:person_id/photos", post(upload_photo))
        .with_state(state)
}
```

### Pattern 2: Photo Upload and Processing Pipeline
**What:** Accept JPEG/PNG upload, run through quality gates, extract embedding, store in DB, sync gallery.
**When to use:** POST /api/v1/enrollment/persons/:person_id/photos
**Example:**
```rust
// 1. Decode uploaded image bytes to RgbImage
// 2. Run SCRFD face detection on the image (must find exactly 1 face)
// 3. QualityGates.check() -- reject blurry, small, side-profile
// 4. alignment::align_face() -- 112x112 crop
// 5. clahe::apply_clahe() -- lighting normalization
// 6. arcface::preprocess() -- NCHW tensor
// 7. recognizer.extract_embedding() -- 512-D vector
// 8. Duplicate check: cosine_similarity against all gallery entries
// 9. db::insert_embedding() -- persist with retention_days
// 10. gallery.reload() or add_entry() -- sync in-memory gallery
// 11. audit.log() -- DPDP compliance
```

### Pattern 3: Multi-Angle Enrollment
**What:** Require 3-5 quality frames per person before enrollment is considered complete. Each photo is independently validated and stored as a separate embedding row.
**When to use:** ENRL-02 -- multiple embeddings per person improve recognition accuracy by covering different angles/lighting.
**Example:**
```rust
// persons table: add `enrollment_status` column (TEXT: "partial", "complete")
// face_embeddings table: already supports multiple rows per person_id
// GET /api/v1/enrollment/persons/:id returns embedding_count
// Business rule: status = "complete" when embedding_count >= 3
// Gallery loads ALL embeddings for a person (already does this in db::load_gallery)
// find_match in gallery already picks best match across all embeddings
```

### Pattern 4: Duplicate Detection
**What:** Before finalizing enrollment, compare new embedding against all existing gallery entries using cosine similarity. If similarity > threshold, flag as potential duplicate.
**When to use:** During photo upload to prevent enrolling the same person twice under different names.
**Example:**
```rust
// Use gallery.find_match() with the new embedding
// If match found with similarity > duplicate_threshold (e.g., 0.6):
//   - Return 409 Conflict with matched person_id and person_name
//   - Staff can override if it's genuinely a different person (force=true param)
```

### Anti-Patterns to Avoid
- **Holding DB connection across await points:** rusqlite::Connection is !Send. Always use `spawn_blocking` for DB operations, matching the pattern in main.rs.
- **Processing photos synchronously in the handler:** ArcFace inference is GPU-bound. Use `spawn_blocking` (already used in `extract_embedding`).
- **Gallery full reload on every change:** For single additions, add the entry directly. Full reload is for startup only.
- **Accepting arbitrarily large uploads:** Set a reasonable body size limit (e.g., 10MB) via Axum's `DefaultBodyLimit`.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Face detection in uploads | Custom face finder | SCRFD detector (already exists) | Need landmarks for alignment |
| Face quality validation | Custom blur/pose checks | QualityGates.check() | Already handles size, blur, yaw |
| Face alignment | Custom affine transform | alignment::align_face() | Already has similarity transform + warp |
| Lighting normalization | Custom histogram equalization | clahe::apply_clahe() | Already configured for entrance conditions |
| Embedding extraction | Custom ML inference | ArcfaceRecognizer.extract_embedding() | Already handles ONNX + CUDA + L2 norm |
| Embedding comparison | Custom distance metric | gallery::cosine_similarity() | Already correct for L2-normalized vectors |
| Audit logging | Custom file writer | AuditWriter.log() | Already handles Windows file locking via mpsc |
| Image decoding | Custom decoder | image::open() / image::load_from_memory() | Already a dependency, handles JPEG/PNG/etc |

**Key insight:** This phase is almost entirely composition -- wiring existing modules into HTTP handlers. The ML pipeline, quality gates, persistence, and gallery are all done.

## Common Pitfalls

### Pitfall 1: rusqlite Connection is !Send
**What goes wrong:** Trying to hold a `rusqlite::Connection` across `.await` points causes compiler errors.
**Why it happens:** SQLite connections are not thread-safe by default; rusqlite marks Connection as !Send.
**How to avoid:** Wrap all DB operations in `tokio::task::spawn_blocking`. This pattern is already used in main.rs for `load_gallery`.
**Warning signs:** Compiler error: "future cannot be sent between threads safely."

### Pitfall 2: SCRFD Detector Needed for Enrollment
**What goes wrong:** Enrollment photo upload needs face detection (to find landmarks for alignment), but the detector is currently only used in the detection pipeline.
**Why it happens:** The enrollment handler needs to detect exactly one face in the uploaded photo.
**How to avoid:** Share `Arc<ScrfdDetector>` in EnrollmentState, or create a second detector instance for enrollment. Since SCRFD session requires `&mut self`, it uses `Arc<Mutex<Session>>` internally -- sharing is safe via `clone_shared()` pattern (same as ArcfaceRecognizer).
**Warning signs:** Enrollment works without face detection = no landmarks = no alignment = bad embeddings.

### Pitfall 3: Gallery Stale After Enrollment
**What goes wrong:** New person enrolled in SQLite but gallery not updated -- recognition pipeline can't find them.
**Why it happens:** Gallery is in-memory. DB writes don't auto-propagate.
**How to avoid:** After inserting embedding in DB, call `gallery.reload()` with fresh `db::load_gallery()` result. Or add a targeted `add_entry` method to Gallery.
**Warning signs:** Person enrolled but not recognized in live camera feed.

### Pitfall 4: Missing Phone Column in persons Table
**What goes wrong:** Success criteria says "name, role, phone" but the current schema only has `name` and `role`.
**Why it happens:** Phase 114 created a minimal schema. Phase 115 needs phone for staff profiles.
**How to avoid:** Add an `ALTER TABLE persons ADD COLUMN phone TEXT DEFAULT ''` migration, or better: add a migration system. Since this is SQLite with `CREATE TABLE IF NOT EXISTS`, modify the `create_tables` function to add the column if missing.
**Warning signs:** Phone number silently dropped on insert.

### Pitfall 5: Body Size Limit for Photo Uploads
**What goes wrong:** Default Axum body limit (2MB) may be too small for high-res photos, or no limit allows DoS.
**Why it happens:** Axum has a default body limit, but enrollment photos from phone cameras can be 5-10MB.
**How to avoid:** Explicitly set `DefaultBodyLimit::max(10 * 1024 * 1024)` on upload routes.
**Warning signs:** 413 Payload Too Large on photo upload.

### Pitfall 6: Duplicate Detection Threshold vs Recognition Threshold
**What goes wrong:** Using the same similarity threshold (0.45) for both recognition and duplicate detection leads to false positives on duplicates.
**Why it happens:** Recognition threshold is deliberately low to avoid missing known people. Duplicate detection needs a higher bar.
**How to avoid:** Use a higher threshold for duplicate detection (e.g., 0.6-0.7). Different thresholds serve different purposes.
**Warning signs:** Every enrollment flagged as duplicate of someone else.

## Code Examples

### Database Schema Extension
```rust
// Add to db.rs create_tables() -- idempotent column addition
pub fn create_tables(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS persons (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            role TEXT NOT NULL DEFAULT 'customer',
            phone TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        -- ... existing face_embeddings table ...
        "
    )?;
    // Idempotent migration: add phone column if missing
    let has_phone: bool = conn
        .prepare("SELECT phone FROM persons LIMIT 0")
        .is_ok();
    if !has_phone {
        conn.execute("ALTER TABLE persons ADD COLUMN phone TEXT NOT NULL DEFAULT ''", [])?;
    }
    Ok(())
}
```

### New DB Functions Needed
```rust
// Get a single person by ID
pub fn get_person(conn: &Connection, person_id: i64) -> rusqlite::Result<Option<PersonInfo>> { ... }

// List all persons
pub fn list_persons(conn: &Connection) -> rusqlite::Result<Vec<PersonInfo>> { ... }

// Update person metadata
pub fn update_person(conn: &Connection, person_id: i64, name: &str, role: &str, phone: &str) -> rusqlite::Result<bool> { ... }

// Delete person and cascade embeddings (ON DELETE CASCADE handles embeddings)
pub fn delete_person(conn: &Connection, person_id: i64) -> rusqlite::Result<bool> { ... }

// Count embeddings for a person (enrollment progress)
pub fn embedding_count(conn: &Connection, person_id: i64) -> rusqlite::Result<u64> { ... }
```

### Gallery Extension
```rust
// Add to Gallery impl:
pub async fn add_entry(&self, entry: GalleryEntry) {
    let mut entries = self.entries.write().await;
    entries.push(entry);
}

pub async fn remove_person(&self, person_id: i64) {
    let mut entries = self.entries.write().await;
    entries.retain(|e| e.person_id != person_id);
}
```

### Enrollment Photo Processing
```rust
async fn upload_photo(
    Path(person_id): Path<i64>,
    State(state): State<Arc<EnrollmentState>>,
    body: axum::body::Bytes,
) -> Result<Json<Value>, StatusCode> {
    // 1. Decode image
    let img = image::load_from_memory(&body)
        .map_err(|_| StatusCode::BAD_REQUEST)?
        .to_rgb8();

    // 2. Detect face (need SCRFD -- spawn_blocking because it's GPU-bound)
    let detector = state.detector.clone_shared();
    let faces = tokio::task::spawn_blocking(move || {
        detector.detect(&img_bytes, width, height, conf_threshold)
    }).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // 3. Exactly one face required
    if faces.len() != 1 {
        return Err(StatusCode::UNPROCESSABLE_ENTITY); // 422
    }
    let face = &faces[0];

    // 4. Quality gate
    let gray = /* convert to grayscale */;
    state.quality_gates.check(face, &gray, w, h)
        .map_err(|reason| {
            // Return 422 with rejection reason in body
            StatusCode::UNPROCESSABLE_ENTITY
        })?;

    // 5. Align + CLAHE + embed
    let aligned = alignment::align_face(rgb_bytes, w, h, &face.landmarks);
    let clahe_img = clahe::apply_clahe(&aligned);
    let tensor = arcface::preprocess(&clahe_img);
    let embedding = state.recognizer.extract_embedding(tensor).await?;

    // 6. Duplicate check
    if let Some((dup_id, dup_name, sim)) = state.gallery.find_match(&embedding).await {
        if dup_id != person_id && sim > DUPLICATE_THRESHOLD {
            return /* 409 Conflict */;
        }
    }

    // 7. Persist + gallery sync + audit
    // ... (spawn_blocking for DB, gallery.add_entry, audit.log)
}
```

### API Response Types
```rust
#[derive(serde::Serialize)]
pub struct PersonResponse {
    pub id: i64,
    pub name: String,
    pub role: String,
    pub phone: String,
    pub embedding_count: u64,
    pub enrollment_status: String, // "partial" or "complete"
    pub created_at: String,
    pub updated_at: String,
}

#[derive(serde::Deserialize)]
pub struct CreatePersonRequest {
    pub name: String,
    pub role: String,
    #[serde(default)]
    pub phone: String,
}

#[derive(serde::Serialize)]
pub struct PhotoUploadResponse {
    pub embedding_id: i64,
    pub embedding_count: u64,
    pub enrollment_status: String,
    pub quality: QualityInfo,
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Full gallery reload after every enrollment | Targeted add_entry/remove_person | Phase 115 | Avoids O(n) DB read on every enrollment |
| Single embedding per person | Multi-angle (3-5 embeddings) | Phase 115 | Better recognition across angles/lighting |
| No duplicate detection | Cosine similarity check pre-enrollment | Phase 115 | Prevents accidental duplicate profiles |

## Open Questions

1. **SCRFD sharing with enrollment**
   - What we know: ScrfdDetector exists, used in detection pipeline, wraps ONNX session with `Arc<Mutex<Session>>`
   - What's unclear: Whether ScrfdDetector has a `clone_shared()` method like ArcfaceRecognizer. Need to check scrfd.rs.
   - Recommendation: If no `clone_shared()`, add one (it's just Arc::clone pattern). Or pass `Arc<ScrfdDetector>` directly since it already uses internal Mutex.

2. **Enrollment quality gate thresholds**
   - What we know: Default quality gates: min_face_size=80, min_laplacian_var=100.0, max_yaw_degrees=45.0
   - What's unclear: Whether enrollment photos (controlled conditions, close-up) should use stricter thresholds than live camera detection
   - Recommendation: Use stricter thresholds for enrollment (min_face_size=120, min_laplacian_var=150.0, max_yaw_degrees=30.0) since enrollment photos should be high quality. Make configurable.

3. **Phone field optionality**
   - What we know: Success criteria says "name, role, phone"
   - What's unclear: Whether phone is required or optional
   - Recommendation: Make phone optional (default empty string). Not all persons (customers) will have phone numbers.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test + tokio::test for async |
| Config file | Cargo.toml (test target already configured with lib + bin) |
| Quick run command | `cargo test -p rc-sentry-ai --lib` |
| Full suite command | `cargo test -p rc-sentry-ai` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| ENRL-01a | Create person with name/role/phone | unit | `cargo test -p rc-sentry-ai --lib -- enrollment::tests::test_create_person` | No -- Wave 0 |
| ENRL-01b | Get/list persons | unit | `cargo test -p rc-sentry-ai --lib -- enrollment::tests::test_list_persons` | No -- Wave 0 |
| ENRL-01c | Update person metadata | unit | `cargo test -p rc-sentry-ai --lib -- enrollment::tests::test_update_person` | No -- Wave 0 |
| ENRL-01d | Delete person cascades embeddings + gallery sync | unit | `cargo test -p rc-sentry-ai --lib -- enrollment::tests::test_delete_person` | No -- Wave 0 |
| ENRL-02a | Photo quality rejection (blur/size/yaw) | unit | `cargo test -p rc-sentry-ai --lib -- recognition::quality::tests` | Yes (existing) |
| ENRL-02b | Multi-angle enrollment tracks embedding count | unit | `cargo test -p rc-sentry-ai --lib -- enrollment::tests::test_multi_angle` | No -- Wave 0 |
| ENRL-02c | Duplicate detection returns 409 on match | unit | `cargo test -p rc-sentry-ai --lib -- enrollment::tests::test_duplicate_detection` | No -- Wave 0 |
| ENRL-01/02 | DB schema migration (phone column) | unit | `cargo test -p rc-sentry-ai --lib -- recognition::db::tests::test_phone_column` | No -- Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-sentry-ai --lib`
- **Per wave merge:** `cargo test -p rc-sentry-ai`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `enrollment/mod.rs` -- new module declaration
- [ ] `enrollment/routes.rs` -- Axum handler tests (unit tests with mock state)
- [ ] `enrollment/service.rs` -- business logic tests (DB + gallery integration)
- [ ] `enrollment/types.rs` -- serde roundtrip tests
- [ ] `recognition/db.rs` -- extend existing tests for new CRUD functions + phone column migration
- [ ] `recognition/gallery.rs` -- add tests for add_entry() and remove_person()

## Sources

### Primary (HIGH confidence)
- Codebase inspection: `crates/rc-sentry-ai/src/recognition/db.rs` -- existing schema and CRUD
- Codebase inspection: `crates/rc-sentry-ai/src/recognition/gallery.rs` -- existing gallery with RwLock
- Codebase inspection: `crates/rc-sentry-ai/src/recognition/quality.rs` -- existing quality gates
- Codebase inspection: `crates/rc-sentry-ai/src/recognition/arcface.rs` -- existing ArcFace with CUDA
- Codebase inspection: `crates/rc-sentry-ai/src/health.rs` -- existing Axum router pattern
- Codebase inspection: `crates/rc-sentry-ai/src/main.rs` -- existing state initialization
- Codebase inspection: `crates/rc-sentry-ai/src/privacy/audit.rs` -- AuditWriter pattern
- Codebase inspection: `crates/rc-sentry-ai/src/config.rs` -- existing config structure
- Codebase inspection: `crates/rc-sentry-ai/Cargo.toml` -- all dependencies already present

### Secondary (MEDIUM confidence)
- None needed -- all findings from direct codebase inspection

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all dependencies already in Cargo.toml, no new crates needed
- Architecture: HIGH -- follows established Axum + Arc state patterns from existing codebase
- Pitfalls: HIGH -- identified from direct code inspection (rusqlite !Send, gallery sync, schema gaps)

**Research date:** 2026-03-21
**Valid until:** 2026-04-21 (stable -- internal codebase, no external API changes expected)
