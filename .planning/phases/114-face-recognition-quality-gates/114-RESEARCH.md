# Phase 114: Face Recognition & Quality Gates - Research

**Researched:** 2026-03-21
**Domain:** ArcFace face recognition, quality gates, CLAHE normalization, face tracking
**Confidence:** HIGH

## Summary

Phase 114 adds face recognition to the existing SCRFD detection pipeline in rc-sentry-ai. The pipeline flow is: SCRFD detects faces -> quality gates filter (blur, size, pose) -> CLAHE lighting normalization -> face alignment (affine transform to 112x112) -> ArcFace embedding extraction -> cosine similarity matching against an in-memory gallery backed by SQLite. A face tracker with 60-second cooldown prevents redundant re-identifications.

The technical stack is well-established. ArcFace-R100 (glintr100.onnx from InsightFace's antelopev2 pack) is industry-standard for face recognition. The model takes 112x112 aligned face crops and produces 512-D embeddings. Quality gates use simple image metrics (Laplacian variance for blur, bbox dimensions for size, landmark geometry for yaw estimation). CLAHE is available via a standalone Rust crate (`clahe` 0.1.2) that integrates with the `image` crate. Face alignment requires computing a similarity transform from 5 detected landmarks to InsightFace's standard reference points, then warping with `imageproc`'s projective transform.

**Primary recommendation:** Use the existing `ort` + CUDA pattern from SCRFD for a second ArcFace session (shared via `Arc<Mutex<Session>>`). Add `imageproc` 0.26 for affine warp, `clahe` 0.1.2 for CLAHE, and `rusqlite` 0.32 with `bundled` feature for embedding persistence. Implement quality gates as pure functions on `DetectedFace` before passing to recognition.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- ArcFace-R100 model variant (most accurate, ~5ms on RTX 4070)
- Cosine similarity threshold of 0.45 for matching (balanced precision/recall for ~100 faces)
- Embedding gallery: in-memory Vec + SQLite persistence -- fast lookup at small scale
- Blur rejection: Laplacian variance below 100.0 (standard for surveillance cameras)
- Minimum face size: 80x80px (practical for 4MP cameras at entrance distance)
- Pose rejection: yaw > 45 degrees rejected (more permissive than roadmap's 30 degrees to avoid over-rejection)
- Face tracker cooldown: 60 seconds (same person recognized once per minute)
- CLAHE (Contrast Limited Adaptive Histogram Equalization) applied always before ArcFace -- consistent embeddings regardless of lighting conditions
- No conditional backlight detection needed -- CLAHE is always beneficial
- ArcFace ONNX model stored in C:\RacingPoint\models\ alongside SCRFD model
- Face alignment uses standard InsightFace reference points for 112x112 crops

### Claude's Discretion
- ArcFace ONNX model source (HuggingFace/InsightFace official)
- Face alignment implementation details (affine transform from 5 landmarks)
- Face tracker data structure and tracking algorithm
- SQLite schema for embedding gallery

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| FACE-02 | ArcFace embedding extraction for identity matching | ArcFace-R100 (glintr100.onnx) via ort 2.0 CUDA EP; 112x112 aligned input; 512-D embedding output; cosine similarity at 0.45 threshold; in-memory Vec gallery backed by SQLite |
| FACE-03 | Quality gates to reject blurry, side-profile, and backlit captures | Laplacian variance < 100.0 for blur; bbox < 80x80 for size; landmark-based yaw > 45 degrees for pose; all implemented as pure filter functions on DetectedFace |
| FACE-04 | Lighting normalization for entrance camera conditions | CLAHE via `clahe` crate 0.1.2 applied to grayscale face crop before ArcFace; convert back to RGB for embedding extraction |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `ort` | 2.0.0-rc.12 | ONNX Runtime inference for ArcFace (CUDA EP) | Already in Cargo.toml from Phase 113. Reuse same pattern as SCRFD. |
| `imageproc` | 0.26.1 | Affine/projective warp for face alignment to 112x112 | Standard Rust image processing library. Provides `warp()` with `Projection` for similarity transforms. |
| `clahe` | 0.1.2 | CLAHE lighting normalization on face crops | Pure Rust CLAHE implementation. Works with `image::GrayImage` via `clahe_image()`. Only viable CLAHE crate in the ecosystem. |
| `rusqlite` | 0.32.x | SQLite for embedding persistence | Standard Rust SQLite bindings. `bundled` feature compiles SQLite from source -- no system dependency. Embeddings stored as BLOB (512 x f32 = 2048 bytes). |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `ndarray` | 0.17 | Tensor manipulation for ArcFace input | Already in Cargo.toml. Convert 112x112 RGB to NCHW f32 tensor. |
| `image` | 0.25.x | Image type conversions (RGB to Luma for CLAHE) | Already in Cargo.toml. Also used for face crop extraction from full frame. |
| `chrono` | workspace | Timestamps for face tracker cooldown and SQLite records | Already in workspace. |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `clahe` crate | Hand-roll CLAHE | CLAHE algorithm is ~100 lines but has subtle edge cases (tile boundaries, interpolation). Crate is tested and maintained. |
| `imageproc` warp | Manual affine math | Similarity transform + bilinear interpolation is ~50 lines. imageproc handles subpixel interpolation correctly and is already a standard dependency. |
| `rusqlite` | `sqlx` (async) | rusqlite is synchronous -- use with `spawn_blocking`. sqlx adds async overhead for SQLite which is already fast. rusqlite is simpler for embedded use. |
| In-memory Vec | sqlite-vec extension | With ~100 faces, brute-force cosine similarity over Vec<[f32; 512]> takes microseconds. Vector index adds complexity for zero benefit at this scale. |

**Installation (additions to rc-sentry-ai/Cargo.toml):**
```toml
imageproc = "0.26"
clahe = "0.1"
rusqlite = { version = "0.32", features = ["bundled"] }
```

**Version verification:**
- `imageproc` 0.26.1 -- confirmed latest on crates.io via `cargo search`
- `clahe` 0.1.2 -- confirmed latest on crates.io via `cargo search`
- `rusqlite` 0.32.x -- confirmed standard version, bundled feature well-supported

## Architecture Patterns

### Recommended Module Structure
```
crates/rc-sentry-ai/src/
  detection/
    mod.rs             # Add recognition exports
    scrfd.rs           # Existing -- unchanged
    pipeline.rs        # Existing -- extend to call recognition after detection
    types.rs           # Existing -- extend with QualityResult, RecognitionResult
    decoder.rs         # Existing -- unchanged
  recognition/
    mod.rs             # Recognition module exports
    arcface.rs         # ArcFace ONNX session, embedding extraction
    alignment.rs       # 5-point landmark -> 112x112 affine warp
    quality.rs         # Quality gates: blur, size, pose checks
    clahe.rs           # CLAHE preprocessing wrapper
    gallery.rs         # In-memory embedding gallery + cosine similarity
    tracker.rs         # Face tracker with 60s cooldown per person
    db.rs              # SQLite schema + CRUD for persons/embeddings
  config.rs            # Add [recognition] config section
  main.rs              # Add ArcFace session init, gallery load, tracker spawn
```

### Pattern 1: Quality Gate Pipeline (Filter Chain)
**What:** Quality gates as a chain of pure functions that each return `Result<(), RejectReason>`. If any gate rejects, skip recognition for that face.
**When to use:** Between SCRFD detection output and ArcFace input.
**Example:**
```rust
#[derive(Debug, Clone)]
pub enum RejectReason {
    TooSmall { width: u32, height: u32 },
    TooBlurry { laplacian_var: f64 },
    ExcessiveYaw { estimated_yaw: f64 },
}

pub struct QualityGates {
    pub min_face_size: u32,          // 80
    pub min_laplacian_var: f64,      // 100.0
    pub max_yaw_degrees: f64,        // 45.0
}

impl QualityGates {
    pub fn check(&self, face: &DetectedFace, frame_rgb: &[u8], frame_w: u32) -> Result<(), RejectReason> {
        self.check_size(face)?;
        self.check_blur(face, frame_rgb, frame_w)?;
        self.check_pose(face)?;
        Ok(())
    }
}
```

### Pattern 2: Dual ONNX Session (SCRFD + ArcFace)
**What:** Two separate `Arc<Mutex<Session>>` instances -- one for SCRFD, one for ArcFace. Both use CUDA EP on the same GPU.
**When to use:** Always. ONNX Runtime manages GPU memory across sessions automatically.
**Example:**
```rust
// In main.rs initialization:
let scrfd = ScrfdDetector::new(&config.detection.model_path)?;
let arcface = ArcfaceRecognizer::new(&config.recognition.model_path)?;

// Both sessions are Arc<Mutex<Session>> internally.
// ort manages CUDA memory allocation per session.
// They share the same GPU but don't interfere.
```

### Pattern 3: Face Alignment via Similarity Transform
**What:** Compute similarity transform from 5 detected landmarks to InsightFace reference points, warp face region to 112x112.
**When to use:** Before every ArcFace inference.
**Key details:**
- InsightFace reference points for 112x112: `[[38.2946, 51.6963], [73.5318, 51.5014], [56.0252, 71.7366], [41.5493, 92.3655], [70.7299, 92.2041]]`
- Compute similarity transform (rotation + uniform scale + translation) from detected landmarks to reference points
- Apply with `imageproc::geometric_transformations::warp()` using `Projection` (convert 2x3 affine to 3x3 by adding [0,0,1] row)

### Pattern 4: In-Memory Gallery with SQLite Persistence
**What:** Load all embeddings into `Vec<GalleryEntry>` at startup. Match against this Vec. Write new enrollments to SQLite and append to Vec.
**When to use:** For the embedding gallery at ~100 faces scale.
**Example:**
```rust
pub struct GalleryEntry {
    pub person_id: i64,
    pub person_name: String,
    pub embedding: [f32; 512],
}

pub struct Gallery {
    entries: RwLock<Vec<GalleryEntry>>,
    db: Mutex<rusqlite::Connection>,
}

impl Gallery {
    pub async fn find_match(&self, query: &[f32; 512], threshold: f32) -> Option<(i64, String, f32)> {
        let entries = self.entries.read().await;
        let mut best_score = 0.0_f32;
        let mut best_match = None;
        for entry in entries.iter() {
            let score = cosine_similarity(query, &entry.embedding);
            if score > best_score {
                best_score = score;
                best_match = Some((entry.person_id, entry.person_name.clone(), score));
            }
        }
        best_match.filter(|_| best_score >= threshold)
    }
}
```

### Pattern 5: Face Tracker with Cooldown
**What:** HashMap<person_id, Instant> tracking when each person was last recognized. Skip recognition broadcast if within cooldown.
**When to use:** After successful gallery match, before emitting recognition event.
**Example:**
```rust
pub struct FaceTracker {
    last_seen: Mutex<HashMap<i64, Instant>>,
    cooldown: Duration,  // 60 seconds
}

impl FaceTracker {
    pub fn should_report(&self, person_id: i64) -> bool {
        let mut last_seen = self.last_seen.lock().unwrap();
        let now = Instant::now();
        match last_seen.get(&person_id) {
            Some(t) if now.duration_since(*t) < self.cooldown => false,
            _ => {
                last_seen.insert(person_id, now);
                true
            }
        }
    }
}
```

### Anti-Patterns to Avoid
- **Running ArcFace on unaligned face crops:** Alignment to 112x112 with landmark-based warp is critical. Without it, embeddings are unreliable and cosine similarity becomes meaningless.
- **Applying CLAHE to the full frame:** Apply CLAHE only to the cropped face region (after bbox extraction, before alignment). Full-frame CLAHE wastes computation.
- **Storing embeddings as text/JSON in SQLite:** Store as BLOB (raw bytes). JSON serialization of 512 floats is 10x larger and slower to parse.
- **Computing Laplacian variance on the full frame:** Compute only on the face crop region. Background blur is irrelevant.
- **Using Euclidean distance for ArcFace:** ArcFace embeddings are L2-normalized. Cosine similarity is the standard metric. Euclidean distance works but threshold values are different and less intuitive.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| CLAHE algorithm | Custom tile-based histogram equalization | `clahe` crate 0.1.2 | Tile boundary interpolation is tricky. Crate handles it correctly. |
| Affine warp with interpolation | Manual bilinear sampling | `imageproc` warp() | Subpixel interpolation edge cases handled correctly |
| SQLite bindings | Raw FFI to sqlite3.dll | `rusqlite` with bundled | Standard, well-tested, compiles SQLite from source |
| Similarity transform estimation | Manual least-squares solver | Implement 2-point similarity (see Code Examples) | Only 4 unknowns (scale, rotation, tx, ty) -- solvable from 2+ point pairs with simple math. No external solver needed. |
| Cosine similarity | Custom dot product | Simple function (~5 lines) | So simple that a crate adds overhead. Inline implementation is correct and fast. |

**Key insight:** The computationally expensive parts (SCRFD inference, ArcFace inference) are already handled by ort. Everything else in this phase -- quality gates, alignment, CLAHE, gallery matching, tracking -- is straightforward image processing and data structure work.

## Common Pitfalls

### Pitfall 1: ArcFace Input Normalization Mismatch
**What goes wrong:** Using SCRFD normalization values (mean=127.5, std=128.0) for ArcFace input. ArcFace uses different normalization.
**Why it happens:** Both models use ONNX, easy to assume same preprocessing.
**How to avoid:** ArcFace models typically expect input normalized as `(pixel / 255.0 - 0.5) / 0.5` which simplifies to `(pixel - 127.5) / 127.5`. However, the exact normalization depends on the specific model export. Inspect the model at load time and document the normalization used. The glintr100.onnx from antelopev2 uses `(pixel - 127.5) / 127.5`.
**Warning signs:** All embeddings cluster together (cosine similarity always > 0.9 for different people), or embeddings are near-zero.

### Pitfall 2: Landmark Order Mismatch in Alignment
**What goes wrong:** Detected landmarks are in wrong order vs. reference points, producing a mirrored or rotated crop.
**Why it happens:** Different detectors may output landmarks in different order.
**How to avoid:** SCRFD (from InsightFace) outputs landmarks in the standard order: left_eye, right_eye, nose, left_mouth, right_mouth. Verify this matches the reference points order. A quick sanity check: left_eye x should be less than right_eye x in a frontal face.
**Warning signs:** Aligned face crops appear mirrored or tilted. Recognition accuracy is poor despite good detection.

### Pitfall 3: CLAHE on RGB Directly
**What goes wrong:** Applying CLAHE independently to R, G, B channels produces color distortion.
**Why it happens:** CLAHE operates on intensity. Applying to each channel independently changes color balance.
**How to avoid:** Convert face crop to grayscale (Luma8), apply CLAHE, then use the grayscale result. For ArcFace, some models accept grayscale-as-3-channel (replicate grayscale to R=G=B). Alternatively, convert to a luminance-based color space, apply CLAHE to luminance only, convert back. Since ArcFace is primarily interested in facial structure, grayscale CLAHE replicated to 3 channels works well in practice.
**Warning signs:** Face crops have unnatural color shifts (blue faces, green tint).

### Pitfall 4: Cosine Similarity Without L2 Normalization
**What goes wrong:** Raw ArcFace output may not be L2-normalized. Cosine similarity on unnormalized vectors gives wrong results.
**Why it happens:** Some ArcFace ONNX exports include the L2 normalization layer, some don't.
**How to avoid:** Always L2-normalize the embedding after extraction: `embedding /= embedding.norm()`. This makes cosine similarity equivalent to dot product, which is fast.
**Warning signs:** Cosine similarity values outside [0, 1] range, or threshold 0.45 rejects everyone.

### Pitfall 5: SQLite BLOB Byte Order
**What goes wrong:** Storing f32 embeddings as bytes with wrong endianness, loading garbage on retrieval.
**Why it happens:** `f32::to_ne_bytes()` uses native endian. If the database is ever read on a different architecture, it breaks.
**How to avoid:** Use `f32::to_le_bytes()` consistently for storage and `f32::from_le_bytes()` for loading. The James machine (.27) is x86_64 (little-endian), so native endian happens to be LE, but being explicit is safer.
**Warning signs:** Loaded embeddings produce random cosine similarities.

### Pitfall 6: Face Tracker Memory Leak
**What goes wrong:** HashMap of last-seen timestamps grows unbounded as more people are recognized.
**Why it happens:** Entries are added but never removed.
**How to avoid:** Periodically sweep the HashMap and remove entries older than cooldown period (e.g., every 5 minutes). Or use a bounded LRU cache.
**Warning signs:** Memory usage gradually increases over days of operation.

## Code Examples

### ArcFace Session Initialization
```rust
// Source: Same pattern as existing ScrfdDetector::new() in scrfd.rs
pub struct ArcfaceRecognizer {
    session: Arc<Mutex<Session>>,
}

impl ArcfaceRecognizer {
    pub fn new(model_path: &str) -> anyhow::Result<Self> {
        use ort::ep;

        let session = Session::builder()
            .map_err(|e| anyhow::anyhow!("ort session builder: {e}"))?
            .with_execution_providers([ep::CUDA::default().build().error_on_failure()])
            .map_err(|e| anyhow::anyhow!("ort CUDA EP: {e}"))?
            .commit_from_file(model_path)
            .map_err(|e| anyhow::anyhow!("ort commit model: {e}"))?;

        // Verify expected I/O: input [1, 3, 112, 112], output [1, 512]
        for input in session.inputs() {
            tracing::info!(name = %input.name(), dtype = ?input.dtype(), "ArcFace input");
        }
        for output in session.outputs() {
            tracing::info!(name = %output.name(), dtype = ?output.dtype(), "ArcFace output");
        }

        Ok(Self {
            session: Arc::new(Mutex::new(session)),
        })
    }

    pub async fn extract_embedding(&self, aligned_face: Array4<f32>) -> anyhow::Result<[f32; 512]> {
        let session = Arc::clone(&self.session);
        tokio::task::spawn_blocking(move || {
            let mut session = session.blocking_lock();
            let input = Tensor::from_array(aligned_face)
                .map_err(|e| anyhow::anyhow!("tensor: {e}"))?;
            let outputs = session.run(ort::inputs![input])
                .map_err(|e| anyhow::anyhow!("inference: {e}"))?;

            // Extract 512-D embedding from first output
            let (_shape, data) = outputs.values().next()
                .ok_or_else(|| anyhow::anyhow!("no output"))?
                .try_extract_tensor::<f32>()
                .map_err(|e| anyhow::anyhow!("extract: {e}"))?;

            let raw: Vec<f32> = data.to_vec();
            anyhow::ensure!(raw.len() == 512, "expected 512-D embedding, got {}", raw.len());

            // L2-normalize
            let mut embedding = [0.0_f32; 512];
            let norm: f32 = raw.iter().map(|x| x * x).sum::<f32>().sqrt();
            for (i, v) in raw.iter().enumerate() {
                embedding[i] = v / norm;
            }

            Ok(embedding)
        }).await?
    }
}
```

### Face Alignment (Similarity Transform from 5 Landmarks)
```rust
// Source: InsightFace face_align.py reference points
// https://github.com/deepinsight/insightface/blob/master/python-package/insightface/utils/face_align.py

/// InsightFace standard reference landmarks for 112x112 aligned face.
const ARCFACE_REF: [[f32; 2]; 5] = [
    [38.2946, 51.6963],  // left eye
    [73.5318, 51.5014],  // right eye
    [56.0252, 71.7366],  // nose
    [41.5493, 92.3655],  // left mouth
    [70.7299, 92.2041],  // right mouth
];

/// Estimate similarity transform from source landmarks to ARCFACE_REF.
///
/// Similarity transform has 4 DOF: scale, rotation angle, tx, ty.
/// We solve a linear system from the first 2 landmark pairs (eyes),
/// which gives us enough constraints for the 4 unknowns.
pub fn estimate_similarity_transform(src: &[[f32; 2]; 5]) -> [f32; 6] {
    // Use least-squares over all 5 points for robustness.
    // The similarity transform is: [a, -b, tx; b, a, ty]
    // where a = s*cos(theta), b = s*sin(theta).
    //
    // For each point pair (sx, sy) -> (dx, dy):
    //   dx = a*sx - b*sy + tx
    //   dy = b*sx + a*sy + ty
    //
    // Stack into Ax = d and solve via pseudo-inverse.
    let n = 5;
    let mut ata = [[0.0_f64; 4]; 4];
    let mut atb = [0.0_f64; 4];

    for i in 0..n {
        let sx = src[i][0] as f64;
        let sy = src[i][1] as f64;
        let dx = ARCFACE_REF[i][0] as f64;
        let dy = ARCFACE_REF[i][1] as f64;

        // Row 1: a*sx - b*sy + tx = dx  =>  [sx, -sy, 1, 0] * [a, b, tx, ty]' = dx
        // Row 2: b*sx + a*sy + ty = dy  =>  [sy,  sx, 0, 1] * [a, b, tx, ty]' = dy
        let rows = [[sx, -sy, 1.0, 0.0, dx], [sy, sx, 0.0, 1.0, dy]];

        for row in &rows {
            for j in 0..4 {
                for k in 0..4 {
                    ata[j][k] += row[j] * row[k];
                }
                atb[j] += row[j] * row[4];
            }
        }
    }

    // Solve 4x4 system (Cholesky or simple Gaussian elimination)
    let params = solve_4x4(&ata, &atb);
    let (a, b, tx, ty) = (params[0] as f32, params[1] as f32, params[2] as f32, params[3] as f32);

    // Return 2x3 affine matrix as flat array: [a, -b, tx, b, a, ty]
    [a, -b, tx, b, a, ty]
}
```

### CLAHE Application
```rust
// Source: clahe crate API + image crate color conversion
use clahe::clahe_image;
use image::{GrayImage, Luma, RgbImage};

/// Apply CLAHE to a face crop for lighting normalization.
/// Returns a 3-channel image (grayscale replicated to RGB) for ArcFace input.
pub fn apply_clahe(face_crop: &RgbImage) -> RgbImage {
    // Convert to grayscale
    let gray = image::DynamicImage::ImageRgb8(face_crop.clone()).into_luma8();

    // Apply CLAHE: 8x8 grid, clip_limit=40, tile_sample=1.0
    let enhanced: GrayImage = clahe_image(&gray, 8, 8, 40, 1.0)
        .unwrap_or_else(|_| gray.clone()); // fallback to original on error

    // Replicate grayscale to 3 channels for ArcFace
    let (w, h) = enhanced.dimensions();
    let mut rgb = RgbImage::new(w, h);
    for (x, y, Luma([v])) in enhanced.enumerate_pixels() {
        rgb.put_pixel(x, y, image::Rgb([*v, *v, *v]));
    }
    rgb
}
```

### Quality Gate: Laplacian Blur Detection
```rust
/// Compute Laplacian variance as a blur metric on a face crop.
///
/// The Laplacian highlights edges. High variance = sharp image. Low variance = blurry.
/// Threshold of 100.0 is standard for surveillance-quality cameras.
pub fn laplacian_variance(gray: &[u8], width: u32, height: u32) -> f64 {
    if width < 3 || height < 3 {
        return 0.0;
    }

    let w = width as usize;
    let h = height as usize;
    let mut sum = 0.0_f64;
    let mut sum_sq = 0.0_f64;
    let mut count = 0u64;

    // 3x3 Laplacian kernel: [0, 1, 0; 1, -4, 1; 0, 1, 0]
    for y in 1..(h - 1) {
        for x in 1..(w - 1) {
            let val = gray[y * w + x] as f64 * -4.0
                + gray[(y - 1) * w + x] as f64
                + gray[(y + 1) * w + x] as f64
                + gray[y * w + (x - 1)] as f64
                + gray[y * w + (x + 1)] as f64;
            sum += val;
            sum_sq += val * val;
            count += 1;
        }
    }

    let mean = sum / count as f64;
    (sum_sq / count as f64) - (mean * mean) // variance
}
```

### Quality Gate: Yaw Estimation from 5 Landmarks
```rust
/// Estimate yaw angle from 5-point facial landmarks.
///
/// Uses the ratio of left-eye-to-nose vs. nose-to-right-eye horizontal distances.
/// In a frontal face, these are roughly equal. As yaw increases, one side compresses.
///
/// Returns estimated absolute yaw in degrees (0 = frontal, 90 = full profile).
pub fn estimate_yaw(landmarks: &[[f32; 2]; 5]) -> f64 {
    let left_eye = landmarks[0];
    let right_eye = landmarks[1];
    let nose = landmarks[2];

    let left_dist = (nose[0] - left_eye[0]).abs();
    let right_dist = (right_eye[0] - nose[0]).abs();

    if left_dist + right_dist < 1.0 {
        return 90.0; // degenerate case
    }

    // Ratio: 1.0 = frontal, 0.0 = full profile
    let ratio = left_dist.min(right_dist) / left_dist.max(right_dist);

    // Map ratio to approximate yaw angle
    // ratio ~1.0 -> 0 degrees, ratio ~0.3 -> ~45 degrees, ratio ~0.0 -> 90 degrees
    let yaw_rad = (1.0 - ratio).acos().abs();
    let yaw_deg = yaw_rad.to_degrees();

    // Empirical calibration: the ratio-to-angle mapping is approximately:
    // yaw ~ arccos(ratio) * (90 / (PI/2)) -- but simpler linear approximation works:
    // yaw ~ (1 - ratio) * 90
    ((1.0 - ratio) as f64) * 90.0
}
```

### Cosine Similarity
```rust
/// Cosine similarity between two L2-normalized 512-D embeddings.
/// Returns value in [-1.0, 1.0]. Higher = more similar.
pub fn cosine_similarity(a: &[f32; 512], b: &[f32; 512]) -> f32 {
    // For L2-normalized vectors, cosine similarity = dot product
    let mut dot = 0.0_f32;
    for i in 0..512 {
        dot += a[i] * b[i];
    }
    dot
}
```

### SQLite Schema for Embedding Gallery
```rust
/// Create embedding gallery tables in SQLite.
pub fn create_tables(conn: &rusqlite::Connection) -> rusqlite::Result<()> {
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS persons (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            role TEXT NOT NULL DEFAULT 'customer',  -- 'staff' or 'customer'
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS face_embeddings (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            person_id INTEGER NOT NULL REFERENCES persons(id) ON DELETE CASCADE,
            embedding BLOB NOT NULL,  -- 512 x f32 = 2048 bytes, little-endian
            enrolled_at TEXT NOT NULL DEFAULT (datetime('now')),
            expires_at TEXT NOT NULL   -- 90 days from enrolled_at (DPDP retention)
        );

        CREATE INDEX IF NOT EXISTS idx_embeddings_person ON face_embeddings(person_id);
        CREATE INDEX IF NOT EXISTS idx_embeddings_expires ON face_embeddings(expires_at);
    ")?;
    Ok(())
}

/// Load all embeddings into memory for fast matching.
pub fn load_gallery(conn: &rusqlite::Connection) -> rusqlite::Result<Vec<GalleryEntry>> {
    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, e.embedding
         FROM face_embeddings e
         JOIN persons p ON p.id = e.person_id
         WHERE e.expires_at > datetime('now')"
    )?;

    let entries = stmt.query_map([], |row| {
        let person_id: i64 = row.get(0)?;
        let person_name: String = row.get(1)?;
        let blob: Vec<u8> = row.get(2)?;

        let mut embedding = [0.0_f32; 512];
        for (i, chunk) in blob.chunks_exact(4).enumerate() {
            embedding[i] = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }

        Ok(GalleryEntry { person_id, person_name, embedding })
    })?.collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(entries)
}
```

### ArcFace Preprocessing (112x112 Aligned Face to Tensor)
```rust
/// Convert an aligned 112x112 RGB face image to NCHW f32 tensor for ArcFace.
///
/// Normalization: (pixel - 127.5) / 127.5  (maps [0, 255] to [-1.0, 1.0])
pub fn preprocess_for_arcface(aligned_rgb: &image::RgbImage) -> ndarray::Array4<f32> {
    debug_assert_eq!(aligned_rgb.width(), 112);
    debug_assert_eq!(aligned_rgb.height(), 112);

    let mut tensor = ndarray::Array4::<f32>::zeros((1, 3, 112, 112));
    for y in 0..112u32 {
        for x in 0..112u32 {
            let pixel = aligned_rgb.get_pixel(x, y);
            tensor[[0, 0, y as usize, x as usize]] = (pixel[0] as f32 - 127.5) / 127.5;
            tensor[[0, 1, y as usize, x as usize]] = (pixel[1] as f32 - 127.5) / 127.5;
            tensor[[0, 2, y as usize, x as usize]] = (pixel[2] as f32 - 127.5) / 127.5;
        }
    }
    tensor
}
```

## ArcFace Model Details

**Recommended model:** `glintr100.onnx` from InsightFace's antelopev2 model pack.
**Download source:** https://huggingface.co/DIAMONIK7777/antelopev2 (same source as SCRFD model)
**Store at:** `C:\RacingPoint\models\glintr100.onnx`
**Size:** ~250 MB

**Input specification:**
- Shape: `[1, 3, 112, 112]` (NCHW)
- Type: f32
- Preprocessing: `(pixel - 127.5) / 127.5`
- Input name: verify at runtime (typically `"input.1"` or `"data"`)

**Output specification:**
- Shape: `[1, 512]`
- Type: f32
- Output name: verify at runtime (typically `"fc1"`)
- Post-processing: L2-normalize the 512-D vector

**Performance:** ~5ms per inference on RTX 4070 (from user's testing/decision context).

**Alternative source:** OpenVINO Model Zoo provides face-recognition-resnet100-arcface-onnx but uses BGR input order. The antelopev2 glintr100.onnx uses RGB and is consistent with the SCRFD model from the same pack.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| FaceNet (Google, 128-D) | ArcFace (InsightFace, 512-D) | 2019 | Better accuracy on all benchmarks. ArcFace margin-based loss produces more discriminative embeddings. |
| Global histogram equalization | CLAHE | Well-established | CLAHE preserves local contrast while preventing over-amplification. Critical for faces with mixed lighting (entrance doors). |
| OpenCV solvePnP for head pose | Landmark geometry ratio | Practical choice | PnP requires camera intrinsics and a 3D model. Landmark ratio is faster, simpler, and accurate enough for quality gating (not precise pose estimation). |
| Separate quality model (SER-FIQ) | Simple metrics (blur + size + pose) | Practical choice | Dedicated quality models add latency and another ONNX session. For a quality gate (accept/reject), simple metrics are sufficient. |

**Deprecated/outdated:**
- FaceNet embeddings: ArcFace is strictly better in accuracy
- dlib's face recognition: Outdated model, lower accuracy than ArcFace
- OpenFace: Superseded by InsightFace ecosystem
- Global histogram equalization: CLAHE is always preferred for face preprocessing

## Open Questions

1. **ArcFace input normalization for glintr100.onnx**
   - What we know: Standard InsightFace normalization is `(pixel - 127.5) / 127.5`. OpenVINO docs say the model uses BGR input.
   - What's unclear: Whether glintr100.onnx specifically uses RGB or BGR, and exact normalization.
   - Recommendation: At model load, log input/output names and shapes. Test with a known face pair to verify cosine similarity is in expected range. Start with RGB + `(px - 127.5) / 127.5`. If results are poor, try BGR.

2. **Yaw estimation accuracy from 5 landmarks**
   - What we know: The eye-nose-eye ratio provides a rough yaw estimate. With 45 degree threshold, we have margin for error.
   - What's unclear: Exact calibration of ratio-to-degrees mapping.
   - Recommendation: The 45-degree threshold is permissive enough that approximate yaw estimation works. Fine-tune empirically if needed after initial deployment.

3. **CLAHE parameters for entrance camera conditions**
   - What we know: Standard CLAHE params (clip_limit=40, grid=8x8) work well for general use.
   - What's unclear: Whether entrance backlighting requires different parameters.
   - Recommendation: Start with clip_limit=40, grid_size=8x8. These are configurable in TOML if tuning is needed.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | Cargo.toml per-crate |
| Quick run command | `cargo test -p rc-sentry-ai -- --nocapture` |
| Full suite command | `cargo test -p rc-sentry-ai` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| FACE-02a | ArcFace model loads with CUDA EP | integration | `cargo test -p rc-sentry-ai --test arcface_load -- --nocapture` | No -- Wave 0 |
| FACE-02b | Embedding extraction produces 512-D L2-normalized vector | unit | `cargo test -p rc-sentry-ai recognition::arcface::test -- --nocapture` | No -- Wave 0 |
| FACE-02c | Cosine similarity: same person > 0.45, different < 0.45 | integration | `cargo test -p rc-sentry-ai --test cosine_match -- --nocapture` | No -- Wave 0 |
| FACE-02d | Gallery load from SQLite returns valid entries | unit | `cargo test -p rc-sentry-ai recognition::db::test -- --nocapture` | No -- Wave 0 |
| FACE-03a | Blur gate rejects Laplacian var < 100 | unit | `cargo test -p rc-sentry-ai recognition::quality::test_blur -- --nocapture` | No -- Wave 0 |
| FACE-03b | Size gate rejects faces < 80x80 | unit | `cargo test -p rc-sentry-ai recognition::quality::test_size -- --nocapture` | No -- Wave 0 |
| FACE-03c | Pose gate rejects yaw > 45 degrees | unit | `cargo test -p rc-sentry-ai recognition::quality::test_pose -- --nocapture` | No -- Wave 0 |
| FACE-04a | CLAHE produces enhanced grayscale output | unit | `cargo test -p rc-sentry-ai recognition::clahe::test -- --nocapture` | No -- Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-sentry-ai`
- **Per wave merge:** `cargo test -p rc-sentry-ai && cargo test -p rc-common`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/rc-sentry-ai/tests/arcface_load.rs` -- integration test: ArcFace model loads, CUDA EP initializes
- [ ] `crates/rc-sentry-ai/tests/cosine_match.rs` -- integration test: same face -> high similarity, different face -> low similarity
- [ ] Test fixture: two face images of same person + one different person (112x112 aligned JPEGs)
- [ ] ArcFace model file must be present at `C:\RacingPoint\models\glintr100.onnx` for integration tests
- [ ] SQLite test database (in-memory `:memory:` for unit tests)

## Sources

### Primary (HIGH confidence)
- [InsightFace face_align.py](https://github.com/deepinsight/insightface/blob/master/python-package/insightface/utils/face_align.py) -- Reference alignment coordinates [[38.2946, 51.6963], [73.5318, 51.5014], [56.0252, 71.7366], [41.5493, 92.3655], [70.7299, 92.2041]]
- [OpenVINO ArcFace R100 docs](https://docs.openvino.ai/2023.3/omz_models_model_face_recognition_resnet100_arcface_onnx.html) -- Input: [1,3,112,112], output name fc1, shape [1,512]
- [ort crate docs](https://ort.pyke.io/) -- Session builder API, CUDA EP (already verified in Phase 113)
- [imageproc 0.26 docs](https://docs.rs/imageproc/latest/imageproc/) -- geometric_transformations::warp() for affine alignment
- [clahe 0.1.2 source](https://github.com/ykszk/clahe) -- clahe_image() API, works with image::GrayImage

### Secondary (MEDIUM confidence)
- [InsightFace alignment issue #1154](https://github.com/deepinsight/insightface/issues/1154) -- Confirmed reference points are stable across InsightFace versions
- [InsightFace threshold discussion #2239](https://github.com/deepinsight/insightface/issues/2239) -- LFW optimal threshold ~0.42 for ArcFace cosine similarity
- [HuggingFace antelopev2](https://huggingface.co/DIAMONIK7777/antelopev2) -- glintr100.onnx model download
- [rusqlite blob docs](https://docs.rs/rusqlite/latest/rusqlite/blob/) -- BLOB storage for embeddings

### Tertiary (LOW confidence)
- ArcFace glintr100.onnx exact normalization values -- sources differ on (px-127.5)/127.5 vs (px-127.5)/128.0. Must verify at runtime.
- Yaw estimation accuracy from landmark geometry -- empirical, no published calibration data for SCRFD 5-point landmarks specifically.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- ort reused from Phase 113, imageproc/clahe/rusqlite are well-established crates
- Architecture: HIGH -- follows existing rc-sentry-ai patterns (tokio tasks, Arc shared state, pipeline extension)
- ArcFace model integration: MEDIUM -- model I/O verified via OpenVINO docs, but exact normalization for glintr100.onnx needs runtime verification
- Quality gates: HIGH -- Laplacian variance and landmark geometry are textbook techniques
- CLAHE: MEDIUM -- clahe crate is small (0% documented on docs.rs) but source code confirms it works with image::GrayImage
- Pitfalls: HIGH -- identified from direct code examination and established face recognition literature

**Research date:** 2026-03-21
**Valid until:** 2026-04-21 (stable domain -- ArcFace is industry standard, all crates are stable)
