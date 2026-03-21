# Phase 114: Face Recognition & Quality Gates - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Add ArcFace face recognition (embedding extraction + cosine similarity matching) to rc-sentry-ai, with quality gates (blur, pose, size filtering), CLAHE lighting normalization, and a face tracker with 60-second recognition cooldown. Builds on Phase 113's SCRFD detection pipeline.

</domain>

<decisions>
## Implementation Decisions

### Recognition Pipeline
- ArcFace-R100 model variant (most accurate, ~5ms on RTX 4070)
- Cosine similarity threshold of 0.45 for matching (balanced precision/recall for ~100 faces)
- Embedding gallery: in-memory Vec + SQLite persistence — fast lookup at small scale

### Quality Gates
- Blur rejection: Laplacian variance below 100.0 (standard for surveillance cameras)
- Minimum face size: 80x80px (practical for 4MP cameras at entrance distance)
- Pose rejection: yaw > 45 degrees rejected (more permissive than roadmap's 30° to avoid over-rejection)
- Face tracker cooldown: 60 seconds (same person recognized once per minute)

### Lighting Normalization
- CLAHE (Contrast Limited Adaptive Histogram Equalization) applied always before ArcFace — consistent embeddings regardless of lighting conditions
- No conditional backlight detection needed — CLAHE is always beneficial

### Claude's Discretion
- ArcFace ONNX model source (HuggingFace/InsightFace official)
- Face alignment implementation details (affine transform from 5 landmarks)
- Face tracker data structure and tracking algorithm
- SQLite schema for embedding gallery

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- detection/scrfd.rs: ScrfdDetector with detect() returning Vec<DetectedFace> (bbox, landmarks, confidence)
- detection/types.rs: DetectedFace struct with 5-point landmarks
- detection/decoder.rs: H264Decoder for frame decoding
- detection/pipeline.rs: Per-camera detection loop reading from FrameBuffer
- privacy/audit.rs: AuditWriter for logging biometric access

### Established Patterns
- ort 2.0 with CUDA EP (Arc<Mutex<Session>> pattern from SCRFD)
- Per-camera tokio tasks with reconnect
- tokio::broadcast for detection results
- TOML config with nested structs

### Integration Points
- ArcFace runs after SCRFD in the detection pipeline
- Quality gates filter between SCRFD output and ArcFace input
- Face tracker wraps the full pipeline output
- Embedding gallery loads from SQLite on startup, syncs on changes
- Recognition results broadcast for downstream consumption (Phase 116 attendance)

</code_context>

<specifics>
## Specific Ideas

- ArcFace ONNX model stored in C:\RacingPoint\models\ alongside SCRFD model
- Face alignment uses standard InsightFace reference points for 112x112 crops

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
