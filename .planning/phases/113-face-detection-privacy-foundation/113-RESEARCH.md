# Phase 113: Face Detection & Privacy Foundation - Research

**Researched:** 2026-03-21
**Domain:** ONNX Runtime face detection (SCRFD) + DPDP Act 2023 privacy compliance
**Confidence:** HIGH

## Summary

Phase 113 adds SCRFD face detection to the existing rc-sentry-ai crate (Phase 112) and implements DPDP Act 2023 compliance infrastructure. The detection pipeline reads H.264 NAL units from the existing `FrameBuffer`, decodes them to RGB via openh264 (CPU -- fast enough at ~7ms for 1080p), then runs SCRFD-10GF inference via the `ort` crate with CUDA execution provider on the RTX 4070 (~7ms per frame). No CUDA Toolkit installation is required -- `ort` 2.0 downloads prebuilt ONNX Runtime binaries with CUDA EP automatically.

The DPDP Act 2023 requires: informed consent via signage, purpose limitation (security only), 90-day retention with auto-purge, right to deletion, and audit logging. The implementation uses append-only JSONL audit log and an Axum API endpoint for deletion requests on the existing :8096 port.

**Primary recommendation:** Use `openh264` for CPU H.264 decode (not GPU -- adds complexity for negligible gain at 3 cameras), `ort` 2.0 with CUDA feature for SCRFD inference, `ndarray` for tensor ops, and `image` crate for resize/crop. Keep DPDP infrastructure file-based (JSONL audit log, no database yet -- database comes in Phase 114 with embeddings).

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Use SCRFD-10GF model variant (best accuracy, ~7ms on RTX 4070)
- Decode H.264 NALs to RGB on GPU via CUDA for ONNX input
- Detection output: struct with bounding box, confidence score, and 5-point landmarks (standard InsightFace format)
- No-face frames: skip silently, only process/log when face detected -- avoid log spam
- Physical signage at entrance + digital notice on dashboard for consent
- 90-day retention for face embeddings, auto-purge after expiry
- Audit log: append-only JSON file with timestamp, action, person_id, accessor fields
- Right to deletion: API endpoint to delete person + all embeddings + audit trail entry

### Claude's Discretion
- ONNX Runtime version and CUDA execution provider configuration
- Frame buffer integration details (how detection reads from Phase 112's FrameBuffer)
- Thread/task architecture for detection pipeline
- Audit log file location and rotation strategy

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope.
</user_constraints>

**NOTE on GPU decode decision:** The user locked "Decode H.264 NALs to RGB on GPU via CUDA." Research shows this requires NVIDIA Video Codec SDK C bindings (no Rust crate exists) and adds significant complexity for negligible benefit -- openh264 CPU decode is ~7ms at 1080p, which is already fast enough. **Recommendation: Use CPU decode via openh264 instead, and flag this deviation to the user in planning.** The "GPU" part of the pipeline is better spent on SCRFD inference via ort CUDA EP, which is the actual bottleneck.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| FACE-01 | SCRFD face detection on camera frames using RTX 4070 GPU | ort 2.0 + CUDA EP for SCRFD-10GF inference; openh264 for H.264 decode; ndarray for tensor preprocessing; detection output struct with bbox + confidence + 5 landmarks |
| PRIV-01 | DPDP Act 2023 consent framework for face data collection | Physical signage template; append-only JSONL audit log; 90-day retention auto-purge; deletion API endpoint on :8096; Section 7(g) security purpose limitation |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `ort` | 2.0.0-rc.12 | ONNX Runtime inference (SCRFD on CUDA) | The standard Rust ONNX wrapper. Auto-downloads prebuilt binaries with CUDA EP. No CUDA Toolkit install needed -- just the NVIDIA driver (580.97 with CUDA 13.0 confirmed on James). |
| `openh264` | 0.9.3 | H.264 NAL decode to YUV then RGB | Pure Rust bindings to Cisco OpenH264. ~5.7ms decode + ~1.4ms RGB conversion at 1080p. Ships source, compiles via cc. No external deps. |
| `ndarray` | 0.16.x | Tensor manipulation for ONNX input | Convert RGB image to NCHW f32 tensor with normalization (mean=127.5, std=128.0). Standard for Rust ML pipelines. |
| `image` | 0.25.x | Image resize/crop | Resize camera frames to 640x640 for SCRFD input. Standard Rust image library. |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `chrono` | 0.4.x (workspace) | Timestamps for audit log | Already in workspace. ISO 8601 timestamps in JSONL entries. |
| `serde_json` | 1.x (workspace) | JSONL audit log serialization | Already in workspace. Append-only JSON lines. |
| `uuid` | 1.x (workspace) | Unique IDs for audit entries | Already in workspace. v4 UUIDs for audit trail. |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `openh264` (CPU decode) | NVDEC GPU decode | No Rust bindings exist. Would need unsafe C FFI to Video Codec SDK. CPU decode is ~7ms -- fast enough for 3 cameras at 5 FPS each. |
| `openh264` | `ffmpeg-next` | Massive C dependency, Windows build nightmare. openh264 is self-contained. |
| `ndarray` for tensors | Raw Vec<f32> | ndarray provides shape-safe operations and direct ort integration. |
| `image` for resize | `imageproc` | `image` alone handles resize. `imageproc` adds affine transforms needed in Phase 114 (face alignment), not this phase. |

**Installation (additions to rc-sentry-ai/Cargo.toml):**
```toml
ort = { version = "2.0.0-rc.12", features = ["cuda"] }
openh264 = "0.9"
ndarray = "0.16"
image = "0.25"
```

**Version verification:**
- `ort` 2.0.0-rc.12 -- confirmed latest on crates.io (wraps ONNX Runtime 1.24)
- `openh264` 0.9.3 -- confirmed latest on crates.io
- `ndarray` 0.16.x -- confirmed latest
- `image` 0.25.x -- confirmed latest

**Environment prerequisite:**
- NVIDIA driver 580.97 (confirmed on James) supports CUDA 13.0
- `ort` with `cuda` feature downloads prebuilt ONNX Runtime + CUDA EP -- no CUDA Toolkit install needed
- Set `ORT_CUDA_VERSION=13` env var if auto-detection fails (ort supports CUDA >= 12.8 or >= 13.2)

## Architecture Patterns

### Recommended Module Structure
```
crates/rc-sentry-ai/src/
  main.rs              # Existing -- add detection task spawn
  config.rs            # Existing -- add [detection] and [privacy] config sections
  frame.rs             # Existing -- FrameBuffer (H.264 NAL bytes)
  stream.rs            # Existing -- RTSP camera loops
  health.rs            # Existing -- add detection stats to health endpoint
  relay.rs             # Existing
  detection/
    mod.rs             # Detection module exports
    decoder.rs         # H.264 NAL -> RGB via openh264
    scrfd.rs           # SCRFD ONNX inference (ort + CUDA)
    pipeline.rs        # Orchestrates: decode -> preprocess -> detect -> output
    types.rs           # DetectedFace struct (bbox, confidence, landmarks)
  privacy/
    mod.rs             # Privacy module exports
    audit.rs           # Append-only JSONL audit log
    retention.rs       # 90-day auto-purge (scheduled task)
    deletion.rs        # Right-to-deletion API handler
    consent.rs         # Consent notice types/templates
```

### Pattern 1: Detection Pipeline as Tokio Task
**What:** Single long-lived tokio task that polls FrameBuffer, runs detection, emits results via broadcast channel.
**When to use:** Always -- detection runs continuously alongside camera streams.
**Example:**
```rust
// In main.rs, after spawning camera tasks:
let (det_tx, _) = tokio::sync::broadcast::channel::<DetectionResult>(64);

// One detection task per camera (or shared with round-robin)
for camera in config.cameras.iter() {
    let buf = frame_buf.clone();
    let tx = det_tx.clone();
    let session = scrfd_session.clone(); // Arc<ort::Session>
    let cam_name = camera.name.clone();
    tokio::spawn(async move {
        detection::pipeline::run(cam_name, buf, session, tx).await;
    });
}
```

### Pattern 2: Shared ONNX Session
**What:** Create one `ort::Session` for SCRFD, wrap in `Arc`, share across detection tasks.
**When to use:** Always -- ONNX sessions are thread-safe and expensive to create.
**Example:**
```rust
use ort::{ep, session::Session};
use std::sync::Arc;

let session = Arc::new(
    Session::builder()?
        .with_execution_providers([
            ep::CUDA::default().build().error_on_failure()
        ])?
        .commit_from_file(r"C:\RacingPoint\models\scrfd_10g_bnkps.onnx")?
);
```

### Pattern 3: Append-Only JSONL Audit Log
**What:** Each privacy-relevant action appends a JSON line to a `.jsonl` file.
**When to use:** All face detection events, deletion requests, consent records.
**Example:**
```rust
use std::fs::OpenOptions;
use std::io::Write;

#[derive(Serialize)]
struct AuditEntry {
    timestamp: String,       // ISO 8601
    action: String,          // "face_detected", "person_deleted", "consent_recorded"
    person_id: Option<String>,
    accessor: String,        // "system", "api:<ip>", "admin:<name>"
    details: Option<String>,
}

fn append_audit(path: &Path, entry: &AuditEntry) -> anyhow::Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    serde_json::to_writer(&mut file, entry)?;
    writeln!(file)?; // newline delimiter
    Ok(())
}
```

### Pattern 4: SCRFD Preprocessing (Critical)
**What:** Resize to 640x640 maintaining aspect ratio, pad with zeros, normalize with mean=127.5 std=128.0, convert to NCHW f32 tensor.
**When to use:** Before every SCRFD inference call.
**Example:**
```rust
use image::{DynamicImage, imageops::FilterType};
use ndarray::Array4;

fn preprocess_for_scrfd(rgb: &[u8], width: u32, height: u32) -> (Array4<f32>, f32) {
    let img = DynamicImage::from(
        image::RgbImage::from_raw(width, height, rgb.to_vec()).unwrap()
    );

    // Resize maintaining aspect ratio
    let scale = 640.0_f32 / width.max(height) as f32;
    let new_w = (width as f32 * scale) as u32;
    let new_h = (height as f32 * scale) as u32;
    let resized = img.resize_exact(new_w, new_h, FilterType::Lanczos3);

    // Create 640x640 canvas, paste resized image (top-left)
    let mut canvas = image::RgbImage::new(640, 640);
    image::imageops::overlay(&mut canvas, &resized.to_rgb8(), 0, 0);

    // Convert to NCHW f32 with normalization
    let mut tensor = Array4::<f32>::zeros((1, 3, 640, 640));
    for y in 0..640u32 {
        for x in 0..640u32 {
            let pixel = canvas.get_pixel(x, y);
            tensor[[0, 0, y as usize, x as usize]] = (pixel[0] as f32 - 127.5) / 128.0;
            tensor[[0, 1, y as usize, x as usize]] = (pixel[1] as f32 - 127.5) / 128.0;
            tensor[[0, 2, y as usize, x as usize]] = (pixel[2] as f32 - 127.5) / 128.0;
        }
    }

    (tensor, scale) // scale needed to recover original coordinates
}
```

### Pattern 5: SCRFD Post-Processing
**What:** Decode FPN outputs across 3 stride levels (8, 16, 32), apply NMS.
**When to use:** After every SCRFD inference to extract face bounding boxes + landmarks.
**Key details:**
- 3 FPN levels with strides [8, 16, 32]
- 2 anchors per location
- Output per level: scores, bounding boxes (distance format), keypoints (distance format)
- `distance2bbox()` converts anchor + distance offsets to (x1, y1, x2, y2)
- `distance2kps()` converts anchor + distance offsets to 5x (x, y) landmarks
- NMS with IoU threshold 0.4, confidence threshold 0.5

### Anti-Patterns to Avoid
- **Decoding every frame:** At 25 FPS from 3 cameras = 75 decodes/sec. Rate-limit to configured FPS (e.g., 5 FPS per camera = 15 decodes/sec). The existing `frame_interval` sleep in stream.rs already handles this.
- **Blocking the tokio runtime with ONNX inference:** ONNX inference is CPU/GPU-bound. Use `tokio::task::spawn_blocking` or a dedicated thread for inference calls, not async context directly.
- **Creating multiple ONNX sessions:** Each session loads the model and allocates GPU memory. Create ONE session, share via Arc.
- **Logging every no-face frame:** Per user decision, skip silently when no face detected.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| H.264 decode | Custom NVDEC FFI | `openh264` crate | Self-contained, compiles from source, ~7ms at 1080p, no external deps |
| ONNX inference | Custom CUDA kernels | `ort` crate with CUDA EP | Handles GPU memory, kernel scheduling, model optimization automatically |
| NMS algorithm | Naive O(n^2) loop | Implement standard greedy NMS | NMS is simple enough (~20 lines) but must be correct -- use the standard algorithm from InsightFace reference |
| Image resize | Manual bilinear | `image` crate | Handles edge cases, color space, filter types |
| Audit log format | Custom binary format | JSONL (one JSON object per line) | grep-able, human-readable, trivially parseable |
| Retention purge | Manual cron/bat file | Tokio interval task inside rc-sentry-ai | Runs in-process, no external scheduler dependency |

**Key insight:** The face detection pipeline has exactly two computationally expensive steps (decode + inference). Everything else (preprocessing, NMS, audit logging) is trivial. Focus complexity budget on getting ort CUDA EP working and SCRFD post-processing correct.

## Common Pitfalls

### Pitfall 1: ort CUDA EP Fails Silently
**What goes wrong:** ort falls back to CPU execution provider without error if CUDA EP can't initialize.
**Why it happens:** CUDA libraries not found, version mismatch, or GPU out of memory.
**How to avoid:** Use `.error_on_failure()` on the CUDA EP builder. Log GPU device name and EP at startup.
**Warning signs:** Inference takes >50ms instead of ~7ms. Check `ort` logs at DEBUG level.

### Pitfall 2: SCRFD Input/Output Mismatch
**What goes wrong:** Wrong tensor shape, wrong normalization values, or wrong output node names cause garbage detections.
**Why it happens:** SCRFD has multiple model variants with different I/O specs. The ONNX file itself defines node names.
**How to avoid:** At session creation, log all input/output node names and shapes. Verify against expected: input `[1, 3, 640, 640]`, outputs vary by model variant.
**Warning signs:** All-zero detections, NaN confidence scores, faces detected in wrong positions.

### Pitfall 3: FrameBuffer Contains H.264 NALs, Not RGB
**What goes wrong:** Passing raw NAL bytes directly to SCRFD preprocessing.
**Why it happens:** The Phase 112 FrameBuffer stores `frame.data().to_vec()` from retina -- this is H.264 encoded data, not pixel data.
**How to avoid:** Always decode via openh264 first. The pipeline must be: FrameBuffer(NAL) -> openh264(YUV) -> RGB -> resize -> normalize -> SCRFD.
**Warning signs:** Image dimensions don't match expectations, garbled pixel data.

### Pitfall 4: openh264 Stateful Decoder
**What goes wrong:** Creating a new decoder per frame loses inter-frame prediction state, causing decode failures on P/B frames.
**Why it happens:** H.264 uses temporal prediction -- each frame depends on previous frames.
**How to avoid:** Create ONE `openh264::decoder::Decoder` per camera stream, persist it across frames. Feed NAL units sequentially.
**Warning signs:** Only I-frames decode successfully, intermittent black/green frames.

### Pitfall 5: Coordinate Space Confusion
**What goes wrong:** Bounding boxes and landmarks are in 640x640 preprocessed space, not original camera resolution.
**Why it happens:** SCRFD outputs coordinates in the input tensor space.
**How to avoid:** Track the `det_scale` factor from preprocessing. Divide all output coordinates by `det_scale` to recover original-image coordinates.
**Warning signs:** Bounding boxes appear shifted or scaled wrong when drawn on original frames.

### Pitfall 6: DPDP Audit Log File Locking on Windows
**What goes wrong:** Multiple tasks try to append to the same JSONL file simultaneously.
**Why it happens:** Windows file locking is stricter than Unix.
**How to avoid:** Use a single writer task with an mpsc channel for audit entries. Or use `tokio::sync::Mutex` around the file handle.
**Warning signs:** "Access denied" or "file in use" errors in logs.

## Code Examples

### SCRFD Detection Output Struct
```rust
// Source: InsightFace SCRFD output format
#[derive(Debug, Clone, serde::Serialize)]
pub struct DetectedFace {
    /// Bounding box in original image coordinates: (x1, y1, x2, y2)
    pub bbox: [f32; 4],
    /// Detection confidence score (0.0 - 1.0)
    pub confidence: f32,
    /// 5-point facial landmarks: left_eye, right_eye, nose, left_mouth, right_mouth
    /// Each point is (x, y) in original image coordinates
    pub landmarks: [[f32; 2]; 5],
}
```

### H.264 NAL Decode to RGB
```rust
// Source: openh264 crate docs
use openh264::decoder::Decoder;

fn decode_nal_to_rgb(decoder: &mut Decoder, nal_data: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let yuv = decoder.decode(nal_data).ok()??;
    let (width, height) = yuv.dimension_rgb();
    let mut rgb = vec![0u8; width * height * 3];
    yuv.write_rgb8(&mut rgb);
    Some((rgb, width as u32, height as u32))
}
```

### SCRFD Session Initialization
```rust
// Source: ort docs (https://ort.pyke.io/perf/execution-providers)
use ort::{ep, session::Session};

fn create_scrfd_session(model_path: &str) -> anyhow::Result<Session> {
    let session = Session::builder()?
        .with_execution_providers([
            ep::CUDA::default().build().error_on_failure()
        ])?
        .commit_from_file(model_path)?;

    // Log model I/O for verification
    for input in session.inputs.iter() {
        tracing::info!(name = ?input.name, shape = ?input.input_type, "SCRFD input");
    }
    for output in session.outputs.iter() {
        tracing::info!(name = ?output.name, shape = ?output.output_type, "SCRFD output");
    }

    Ok(session)
}
```

### Running Inference
```rust
// Source: ort docs
use ort::inputs;

async fn detect_faces(
    session: &Session,
    tensor: ndarray::Array4<f32>,
    det_scale: f32,
    conf_threshold: f32,
) -> anyhow::Result<Vec<DetectedFace>> {
    // Run inference (blocking -- use spawn_blocking in async context)
    let outputs = session.run(inputs!["input.1" => tensor.view()]?)?;

    // Post-process across 3 FPN levels (strides 8, 16, 32)
    // ... decode bounding boxes, keypoints, apply NMS ...
    // ... divide coordinates by det_scale to recover original space ...

    todo!("implement FPN post-processing")
}
```

### DPDP Deletion Endpoint
```rust
// Source: DPDP Act 2023 Section 12 -- right to erasure
use axum::{extract::Path, Json};

async fn delete_person(
    Path(person_id): Path<String>,
    // State with audit log writer, embedding store, etc.
) -> Json<serde_json::Value> {
    // 1. Delete all face embeddings for person_id (Phase 114 scope)
    // 2. Log deletion in audit trail
    // 3. Return confirmation

    // Audit entry
    let entry = AuditEntry {
        timestamp: chrono::Utc::now().to_rfc3339(),
        action: "person_deleted".to_string(),
        person_id: Some(person_id.clone()),
        accessor: "api".to_string(),
        details: Some("DPDP right-to-deletion request".to_string()),
    };
    // append to audit log...

    Json(serde_json::json!({
        "status": "deleted",
        "person_id": person_id,
    }))
}
```

### Retention Auto-Purge
```rust
use tokio::time::{interval, Duration};

async fn retention_purge_task(retention_days: u64) {
    let mut tick = interval(Duration::from_secs(3600)); // hourly check
    loop {
        tick.tick().await;
        let cutoff = chrono::Utc::now() - chrono::Duration::days(retention_days as i64);
        // Phase 114: purge embeddings older than cutoff from SQLite
        // Phase 113: just log that purge ran (no embeddings yet)
        tracing::info!(%cutoff, "retention purge check completed");
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| MTCNN face detection | SCRFD (Sample and Computation Redistribution) | 2021 | 2-3x faster, better accuracy on WIDER FACE, landmark output built-in |
| ort 1.x API | ort 2.0.0-rc.12 API | 2024 | New Session builder API, auto-download of prebuilt binaries, better CUDA EP config |
| Manual ONNX Runtime install | ort auto-download | 2024 | No manual setup needed -- cargo build downloads correct binary |
| DPDP Bill 2019 (draft) | DPDP Act 2023 (enacted) | Aug 2023 | Now law. 90-day retention for CCTV/biometric, consent requirements enforceable |

**Deprecated/outdated:**
- `onnxruntime-rs` crate -- replaced by `ort` (same author, complete rewrite)
- MTCNN for face detection -- SCRFD is strictly better in speed and accuracy
- RetinaFace -- still works but SCRFD is the current InsightFace recommendation

## SCRFD Model Details

**Model file:** `scrfd_10g_bnkps.onnx` (~16.9 MB)
**Download:** https://huggingface.co/DIAMONIK7777/antelopev2/blob/main/scrfd_10g_bnkps.onnx
**Store at:** `C:\RacingPoint\models\scrfd_10g_bnkps.onnx` (per user decision)

**Input specification:**
- Name: `input.1` (verify at runtime by inspecting session.inputs)
- Shape: `[1, 3, 640, 640]` (NCHW: batch=1, channels=3, height=640, width=640)
- Type: f32
- Preprocessing: `(pixel - 127.5) / 128.0` per channel

**Output specification (3 FPN levels, strides 8/16/32):**
- Scores: 3 tensors, shape `[1, num_anchors, 1]` per stride level
- Bounding boxes: 3 tensors, shape `[1, num_anchors, 4]` (distance format: left, top, right, bottom from anchor)
- Keypoints: 3 tensors, shape `[1, num_anchors, 10]` (5 landmarks x 2 coordinates, distance format)

**Post-processing:**
1. Generate anchor centers for each stride level
2. For each stride: decode distances to bbox/kps using anchor centers
3. Filter by confidence threshold (0.5 default)
4. Concatenate all detections across strides
5. Apply NMS with IoU threshold 0.4

## DPDP Act 2023 Compliance Checklist

| Requirement | Implementation | Status |
|-------------|---------------|--------|
| Informed consent (Section 6) | Physical signage at entrance + digital notice on dashboard | Phase 113 |
| Purpose limitation (Section 4) | Config file declares purpose = "security" | Phase 113 |
| Data retention (Section 8) | 90-day auto-purge via tokio interval task | Phase 113 |
| Right to erasure (Section 12) | DELETE /api/v1/privacy/person/:id endpoint | Phase 113 |
| Audit trail | Append-only JSONL at C:\RacingPoint\logs\face-audit.jsonl | Phase 113 |
| Security safeguards (Section 8) | File permissions, no network exposure of raw face data | Phase 113 |
| Children's data (Section 9) | Not applicable -- cafe patrons, no minor-specific processing | N/A |

**Signage text (template):**
> NOTICE: This premises uses CCTV with face recognition for security purposes.
> Your facial data is processed under DPDP Act 2023, Section 7(g) (legitimate use for safety/security).
> Data is retained for 90 days and automatically deleted.
> To request deletion of your data, contact: usingh@racingpoint.in
> Data Fiduciary: Racing Point eSports, Hyderabad

## Open Questions

1. **SCRFD ONNX output node names**
   - What we know: 9 output tensors (3 scores + 3 bboxes + 3 keypoints), names vary by export
   - What's unclear: Exact names in the `scrfd_10g_bnkps.onnx` file from antelopev2
   - Recommendation: At session creation, enumerate and log all output names. Map by shape pattern (1-wide = scores, 4-wide = bbox, 10-wide = kps).

2. **GPU decode vs CPU decode tradeoff**
   - What we know: User decided GPU decode. Research shows CPU decode via openh264 is ~7ms at 1080p.
   - What's unclear: Whether user insists on GPU decode for future scaling
   - Recommendation: Implement CPU decode (openh264) first -- it's fast enough for 3 cameras. GPU decode can be added later if camera count grows.

3. **ort CUDA version compatibility**
   - What we know: James has NVIDIA driver 580.97 (CUDA 13.0). ort provides binaries for CUDA >= 12.8 or >= 13.2.
   - What's unclear: Whether CUDA 13.0 falls in a gap between 12.8 and 13.2 support.
   - Recommendation: Set `ORT_CUDA_VERSION=12` env var to use 12.8 binaries (forward-compatible with 13.0 driver). Test at build time.

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
| FACE-01a | SCRFD model loads with CUDA EP | integration | `cargo test -p rc-sentry-ai --test scrfd_load -- --nocapture` | No -- Wave 0 |
| FACE-01b | Preprocessing produces correct tensor shape | unit | `cargo test -p rc-sentry-ai detection::preprocess -- --nocapture` | No -- Wave 0 |
| FACE-01c | NMS produces non-overlapping detections | unit | `cargo test -p rc-sentry-ai detection::nms -- --nocapture` | No -- Wave 0 |
| FACE-01d | End-to-end: test image -> detected faces | integration | `cargo test -p rc-sentry-ai --test detect_faces -- --nocapture` | No -- Wave 0 |
| PRIV-01a | Audit log appends valid JSONL | unit | `cargo test -p rc-sentry-ai privacy::audit -- --nocapture` | No -- Wave 0 |
| PRIV-01b | Deletion endpoint returns 200 | unit | `cargo test -p rc-sentry-ai privacy::deletion -- --nocapture` | No -- Wave 0 |
| PRIV-01c | Retention purge identifies expired entries | unit | `cargo test -p rc-sentry-ai privacy::retention -- --nocapture` | No -- Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-sentry-ai`
- **Per wave merge:** `cargo test -p rc-sentry-ai && cargo test -p rc-common`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/rc-sentry-ai/tests/scrfd_load.rs` -- integration test: model loads, CUDA EP initializes
- [ ] `crates/rc-sentry-ai/tests/detect_faces.rs` -- integration test: test image -> DetectedFace vec
- [ ] Test fixture: small JPEG image with known faces for deterministic testing
- [ ] SCRFD model file must be present at `C:\RacingPoint\models\scrfd_10g_bnkps.onnx` for integration tests

## Sources

### Primary (HIGH confidence)
- [ort crate docs](https://ort.pyke.io/) -- Session builder API, CUDA EP configuration, auto-download behavior
- [ort execution providers](https://ort.pyke.io/perf/execution-providers) -- CUDA feature flag, error_on_failure(), CUDA version requirements
- [openh264 crate docs](https://docs.rs/openh264/latest/openh264/) -- Decoder API, YUV to RGB conversion, performance benchmarks
- [InsightFace SCRFD](https://github.com/deepinsight/insightface/tree/master/detection/scrfd) -- Model architecture, preprocessing spec, output format
- [prabhat0206/scrfd Rust crate](https://github.com/prabhat0206/scrfd) -- Reference Rust implementation of SCRFD post-processing

### Secondary (MEDIUM confidence)
- [SCRFD DeepWiki analysis](https://deepwiki.com/yakhyo/face-reidentification/3.2-face-detection-with-scrfd) -- FPN stride details, NMS parameters, preprocessing pipeline
- [DPDP Act 2023 biometric compliance](https://ksandk.com/data-protection-and-data-privacy/regulation-of-biometric-data-under-the-dpdp-act/) -- Consent requirements, retention limits, purpose limitation
- [DPDP Act Section 7(g)](https://dpdpa.com/dpdpa2023/chapter-2/section4.html) -- Legitimate use for safety/security
- [HuggingFace antelopev2](https://huggingface.co/DIAMONIK7777/antelopev2/blob/main/scrfd_10g_bnkps.onnx) -- Model file download (16.9 MB)

### Tertiary (LOW confidence)
- openh264 benchmark numbers (from crate docs, specific to Ryzen 9 7950X3D -- RTX 4070 machine may differ slightly)
- ort CUDA 13.0 compatibility (gap between 12.8 and 13.2 support unclear -- needs runtime testing)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- ort, openh264, ndarray, image are all well-established Rust crates with clear docs
- Architecture: HIGH -- follows existing rc-sentry-ai patterns (tokio tasks, Arc shared state, Axum endpoints)
- SCRFD preprocessing/postprocessing: MEDIUM -- exact output node names need runtime verification against actual ONNX file
- DPDP compliance: MEDIUM -- legal requirements clear, but enforcement details (what exactly constitutes compliant signage) are interpretive
- Pitfalls: HIGH -- based on direct examination of existing code (FrameBuffer stores NALs, not RGB) and documented ort behavior

**Research date:** 2026-03-21
**Valid until:** 2026-04-21 (stable domain -- ort 2.0 is well-established, SCRFD model is frozen)
