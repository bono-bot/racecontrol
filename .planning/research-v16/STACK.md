# Technology Stack

**Project:** v16.0 Security Camera AI & Attendance
**Researched:** 2026-03-21
**Overall confidence:** MEDIUM-HIGH

## Key Decision: Local-First Face Recognition (NOT Cloud API)

The PROJECT-v16.md lists "Cloud API for face recognition" as a pending decision. After research, I recommend **local inference on James's RTX 4070** instead. Here's why:

| Factor | Cloud API (Rekognition) | Local (ort + ArcFace) |
|--------|------------------------|----------------------|
| Latency | 200-500ms per call (network) | <10ms per inference (GPU) |
| Cost | $0.001/image = ~$30-90/mo at scale | $0 after setup |
| Internet dependency | YES - single point of failure | NO - works offline |
| Face DB size | Designed for millions | ~100 faces = trivial |
| Privacy | Faces sent to AWS | Everything stays local |
| RTX 4070 utilization | Wasted | Fully leveraged |
| Accuracy | 99.5%+ | 99.5%+ (ArcFace is SOTA) |

**Verdict:** Cloud APIs make sense for serverless apps or million-face databases. For a cafe with ~100 known faces and a dedicated GPU sitting idle, local inference is faster, cheaper, more reliable, and more private.

**Confidence: HIGH** -- ArcFace is the industry-standard model, ort is production-proven, RTX 4070 handles this trivially.

---

## Recommended Stack

### RTSP Ingestion

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| `retina` | 0.4.x (latest) | RTSP client, H.264 frame extraction | Production-proven in Moonfire NVR. Pure Rust, async/tokio-native. Only serious RTSP crate in the Rust ecosystem. Actively maintained (0.4.19 released March 2026). | HIGH |

**Why not GStreamer/FFmpeg bindings?** Retina is pure Rust, no C dependency headaches on Windows, integrates natively with tokio. GStreamer Rust bindings (`gstreamer-rs`) add massive build complexity for what we need (just pulling H.264 frames). FFmpeg bindings (`ffmpeg-next`) are an option for recording but overkill for frame extraction.

### Face Detection (Finding Faces in Frames)

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| SCRFD (ONNX model) | SCRFD_10G_KPS | Detect face bounding boxes + 5-point landmarks | From InsightFace. 95.16% accuracy on WIDER FACE Easy. Fast on GPU (~1ms/frame on RTX). Outputs aligned landmarks needed for ArcFace preprocessing. Multiple size variants available (0.5G for CPU, 10G for GPU). | HIGH |
| `ort` | 2.0.0-rc.12 | ONNX Runtime inference engine | The standard Rust ONNX wrapper. CUDA execution provider for RTX 4070. Production-ready despite "rc" tag (used widely). Supports dynamic input shapes. | HIGH |

**Why not `rust-faces`?** It wraps ort internally anyway. Using ort directly gives us control over session configuration, GPU memory, and batch inference. One fewer abstraction layer.

**Why not `rusty_scrfd`?** It's a thin wrapper -- fine for prototyping but we want direct ort control for shared session management across detection + recognition models.

### Face Recognition (Identifying Who)

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| ArcFace (ONNX model) | ResNet100 or MobileFaceNet | Generate 512-D face embeddings | Industry SOTA for face recognition. ONNX models freely available from InsightFace. ResNet100 for max accuracy, MobileFaceNet for speed (both trivial on RTX 4070). | HIGH |
| Cosine similarity | N/A | Match embeddings against known faces | Standard approach for 512-D ArcFace vectors. Threshold ~0.4-0.5 for same person. With 100 faces, brute-force cosine search takes microseconds. | HIGH |

**Why not a vector DB?** With ~100 enrolled faces, a `Vec<(PersonId, [f32; 512])>` in memory with brute-force cosine similarity is faster than any DB lookup. Vector DBs solve million-scale problems we don't have.

### Face Embedding Storage

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| SQLite via `rusqlite` | 0.32.x | Store face embeddings, person profiles, attendance records | Already proven in the racecontrol ecosystem. Embeddings stored as BLOB (512 x f32 = 2KB per face). No need for sqlite-vec extension at this scale. | HIGH |

**Schema concept:**
```sql
-- persons table: id, name, role (staff/customer), created_at
-- face_embeddings table: id, person_id, embedding BLOB, enrolled_at
-- attendance_log table: id, person_id, camera_id, timestamp, confidence, direction (in/out)
```

### Video Recording & Storage

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| FFmpeg (CLI via `std::process::Command`) | System FFmpeg | Record RTSP streams to MP4 segments | Spawning FFmpeg as a subprocess is the pragmatic approach. No Rust bindings needed -- `ffmpeg -i rtsp://... -c copy -f segment -segment_time 3600 output_%Y%m%d_%H.mp4` does copy-mux (no transcoding, minimal CPU). Battle-tested for RTSP recording. | HIGH |
| `notify` crate | 8.x | Watch recording directory for new segments | Trigger cloud backup when segments complete. | MEDIUM |

**Why `std::process::Command` over FFmpeg bindings?** FFmpeg C bindings (`ffmpeg-next`, `ffmpeg-sys-next`) are notoriously painful to build on Windows, version-sensitive, and add 50MB+ of C dependencies. Spawning the CLI binary is simpler, debuggable, and equally reliable. The NVR at .18 already records -- our recording is supplementary for cloud backup.

**Why not retina for recording?** Retina extracts decoded frames (for AI processing). For recording, we want raw H.264 copy-mux (no decode/re-encode). FFmpeg's `-c copy` handles this perfectly.

### Recording Retention & Cloud Backup

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| Local filesystem | N/A | 30-day rolling storage on James (.27) | Simple date-based directories. Cron/scheduled cleanup of files > 30 days. James has sufficient disk for ~6 cameras x 4MP x 30 days. | HIGH |
| `russh` or SCP via CLI | Latest | Upload segments to Bono VPS | Push completed segments to srv1422716 for cloud backup. SCP/rsync over Tailscale is simplest. | MEDIUM |

**Storage estimate:** 4MP H.264 camera at ~2-4 Mbps = ~1-1.8 GB/hour = ~25-43 GB/day per camera. 3 cameras (entrance + 2 reception) x 30 days = ~2.2-3.9 TB. James needs sufficient disk or we limit to entrance camera only for recording.

### Live Feed Streaming (Dashboard)

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| RTSP re-stream to HLS/MJPEG | Via FFmpeg | Serve live camera feeds to web dashboard | Two options: (1) FFmpeg RTSP-to-HLS for each camera, serve .m3u8 via Axum static files. (2) MJPEG re-stream from decoded frames. HLS has 2-5s latency but is browser-native. MJPEG is lower latency but higher bandwidth. | MEDIUM |
| `hls.js` | 1.x | Browser-side HLS playback | Standard library for HLS in browsers. Works with Next.js dashboard. | HIGH |

**Recommendation:** Use HLS for live viewing. The 2-5s latency is acceptable for a monitoring dashboard. MJPEG is an option if Uday wants near-real-time but adds complexity.

### Alerts & Notifications

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| `teloxide` | 0.13.x | Telegram bot for alerts | Mature async Rust Telegram bot framework. Uday already uses Telegram. Send face-match notifications, unknown person alerts, motion events. | HIGH |
| Desktop notifications | `notify-rust` 4.x | James desktop popup on events | Local notifications on James machine for staff awareness. | MEDIUM |
| WebSocket (existing) | Via Axum | Dashboard real-time alerts | racecontrol already has WebSocket infrastructure. Push attendance events and alerts to the dashboard in real-time. | HIGH |
| Web Push API | Via `web-push` crate | Mobile push notifications | For Uday's phone. Requires service worker in Next.js PWA. | LOW |

**WhatsApp:** Skip for now. WhatsApp Business API requires Meta business verification, costs money, and is complex. Telegram covers the "instant mobile notification" need.

### Image Processing

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| `image` | 0.25.x | Decode/resize/crop frames for face preprocessing | Standard Rust image library. Needed to resize face crops to 112x112 for ArcFace input. | HIGH |
| `imageproc` | 0.25.x | Image processing operations | Drawing bounding boxes on frames, affine transforms for face alignment using landmark points. | MEDIUM |
| `ndarray` | 0.16.x | Tensor manipulation | Convert image data to NCHW tensors for ONNX input. Normalize pixel values. | HIGH |

### Service Architecture

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| `axum` | 0.8.x | HTTP API for attendance service | Already the standard in racecontrol. Serve attendance API, enrollment endpoints, live feed proxying. | HIGH |
| `tokio` | 1.x | Async runtime | Already in workspace. Drive RTSP ingestion, inference pipeline, recording, and API server concurrently. | HIGH |
| `sqlx` or `rusqlite` | Latest | Database access | `rusqlite` for synchronous SQLite access (simpler for embedded). `sqlx` if we want async but adds complexity for SQLite. Recommend `rusqlite` with `tokio::task::spawn_blocking`. | HIGH |

---

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| Face Recognition | Local ArcFace via ort | AWS Rekognition | Slower (network latency), costs money, internet-dependent, unnecessary for ~100 faces. RTX 4070 is sitting right there. |
| Face Recognition | Local ArcFace via ort | Azure Face API | Same as Rekognition. Also: Azure Face API requires "Limited Access" approval for face identification since June 2023. |
| RTSP Client | `retina` | `gstreamer-rs` | Massive C dependency. Complex Windows build. Overkill for frame extraction. |
| RTSP Client | `retina` | OpenCV via `opencv-rust` | Huge build dependency. OpenCV's RTSP is just FFmpeg underneath anyway. |
| Recording | FFmpeg CLI subprocess | `retina` + manual muxing | Retina decodes frames; we want raw H.264 copy-mux. Don't decode just to re-encode. |
| Recording | FFmpeg CLI subprocess | `ffmpeg-next` bindings | Windows build nightmare. CLI subprocess is simpler and equally reliable. |
| Face Detection | SCRFD via ort | YOLOv8-face | SCRFD is specifically designed for faces with landmark output. YOLOv8-face works but SCRFD is more efficient and outputs the 5 landmarks needed for ArcFace alignment. |
| Face Detection | SCRFD via ort | MediaPipe Face Detection | No Rust bindings. Python/JS only. |
| Embedding Storage | SQLite (brute force) | sqlite-vec extension | 100 faces x 512 floats = 50ms brute force max. Vector index adds complexity for zero benefit at this scale. |
| Embedding Storage | SQLite (brute force) | FAISS / Qdrant | Designed for millions of vectors. Massive overkill. Adds deployment complexity. |
| Notifications | Telegram (`teloxide`) | WhatsApp Business API | Requires Meta business verification, paid messages, complex webhook setup. Telegram is free and `teloxide` is excellent. |
| Live Streaming | HLS via FFmpeg | WebRTC | WebRTC is low-latency but extremely complex (STUN/TURN, ICE, signaling). HLS "just works" in browsers. 2-5s latency is fine for monitoring. |

---

## ONNX Models Required

Download these before development:

| Model | Source | Size | Purpose |
|-------|--------|------|---------|
| SCRFD_10G_KPS.onnx | [InsightFace GitHub](https://github.com/deepinsight/insightface/tree/master/detection/scrfd) | ~17MB | Face detection + 5-point landmarks |
| arcface_r100.onnx | [InsightFace model zoo](https://github.com/deepinsight/insightface/tree/master/model_zoo) | ~250MB | Face embedding (512-D) |

**Fallback models (lighter):**
- SCRFD_2.5G_KPS.onnx (~3MB) -- if 10G is too slow for multi-camera
- mobilefacenet.onnx (~5MB) -- if ResNet100 is too slow (unlikely on RTX 4070)

---

## New Crate Structure

```
crates/rc-sentry-ai/       # New crate for v16.0
  src/
    main.rs               # Service entrypoint
    camera/
      rtsp.rs             # Retina RTSP client, frame extraction
      recorder.rs         # FFmpeg subprocess management
      hls.rs              # HLS segment serving
    ai/
      detector.rs         # SCRFD face detection
      recognizer.rs       # ArcFace embedding + matching
      pipeline.rs         # Detection -> alignment -> recognition pipeline
    attendance/
      tracker.rs          # Person tracking, entry/exit logic
      db.rs               # SQLite: persons, embeddings, attendance log
    enrollment/
      handler.rs          # New face enrollment flow
    alerts/
      telegram.rs         # Teloxide bot
      desktop.rs          # Desktop notifications
      websocket.rs        # Dashboard push
    api/
      routes.rs           # Axum API endpoints
```

**Why `rc-sentry-ai` not `rc-camera`?** Aligns with the existing `rc-sentry` naming. This is the AI-powered extension of sentry monitoring. The "AI" suffix distinguishes it from the existing `rc-sentry` crate.

---

## Workspace Dependencies (additions to Cargo.toml)

```toml
[workspace.dependencies]
# v16.0 additions
retina = "0.4"
ort = { version = "2.0.0-rc.12", features = ["cuda"] }
ndarray = "0.16"
image = "0.25"
imageproc = "0.25"
rusqlite = { version = "0.32", features = ["bundled"] }
teloxide = { version = "0.13", features = ["macros"] }
notify = "8"
```

### Dev/Build Requirements

```bash
# System requirements (on James .27)
# 1. CUDA Toolkit 11.6+ (for ort CUDA execution provider)
#    - James has RTX 4070, likely already has CUDA from Ollama
# 2. FFmpeg binary on PATH
#    - Download from https://www.gyan.dev/ffmpeg/builds/ (Windows static build)
# 3. ONNX model files in a known location
#    - e.g., C:\RacingPoint\models\scrfd_10g_kps.onnx
#    -       C:\RacingPoint\models\arcface_r100.onnx
```

---

## Pipeline Architecture (Data Flow)

```
RTSP Camera (.8, .15, .154)
    |
    v
retina (async RTSP client)
    |
    +--> Frame buffer (tokio channel)
    |        |
    |        v
    |    SCRFD face detection (ort + CUDA)
    |        |
    |        v
    |    Face alignment (5-point landmark -> affine transform -> 112x112)
    |        |
    |        v
    |    ArcFace embedding (ort + CUDA) -> 512-D vector
    |        |
    |        v
    |    Cosine similarity vs enrolled faces (brute force, in-memory)
    |        |
    |        +--> Match found: log attendance, push alert
    |        +--> No match: new face -> enrollment queue
    |
    +--> FFmpeg subprocess (copy-mux to MP4 segments)
            |
            v
         Local storage (30-day rolling)
            |
            v
         Cloud backup (SCP to Bono VPS)
```

---

## Cost Estimate

| Item | Cost | Notes |
|------|------|-------|
| Cloud API | $0/mo | Not using cloud -- local inference |
| ONNX models | Free | Open source from InsightFace |
| Telegram bot | Free | Telegram Bot API is free |
| Storage (local) | Disk only | ~2-4 TB for 30 days, 3 cameras |
| Storage (VPS backup) | Included | Bono VPS already provisioned |
| **Total recurring** | **$0/mo** | |

---

## Sources

- [retina crate - crates.io](https://crates.io/crates/retina) -- RTSP library, v0.4.19
- [ort crate - GitHub](https://github.com/pykeio/ort) -- ONNX Runtime for Rust, v2.0.0-rc.12
- [InsightFace - GitHub](https://github.com/deepinsight/insightface) -- SCRFD + ArcFace models
- [SCRFD face detection](https://github.com/deepinsight/insightface/tree/master/detection/scrfd) -- Detection model details
- [ArcFace ResNet100 ONNX](https://github.com/openvinotoolkit/open_model_zoo/blob/master/models/public/face-recognition-resnet100-arcface-onnx/README.md) -- Model spec
- [sqlite-vec Rust usage](https://alexgarcia.xyz/sqlite-vec/rust.html) -- Vector search in SQLite (not needed at our scale)
- [AWS Rekognition pricing](https://aws.amazon.com/rekognition/pricing/) -- $0.001/image baseline
- [teloxide - Telegram bot framework](https://crates.io/crates/teloxide) -- Rust Telegram bot
- [rusty_scrfd - Rust SCRFD](https://github.com/prabhat0206/scrfd) -- Reference implementation
- [face-reidentification pipeline](https://github.com/yakhyo/face-reidentification) -- SCRFD + ArcFace + FAISS reference architecture
