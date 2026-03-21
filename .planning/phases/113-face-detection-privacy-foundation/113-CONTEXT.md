# Phase 113: Face Detection & Privacy Foundation - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Set up SCRFD face detection via ONNX Runtime with CUDA on the RTX 4070, integrated with rc-sentry-ai's camera frame pipeline. Implement DPDP Act 2023 consent framework with physical signage requirements, data retention policy, audit logging, and right-to-deletion API.

</domain>

<decisions>
## Implementation Decisions

### Face Detection Pipeline
- Use SCRFD-10GF model variant (best accuracy, ~7ms on RTX 4070)
- Decode H.264 NALs to RGB on GPU via CUDA for ONNX input
- Detection output: struct with bounding box, confidence score, and 5-point landmarks (standard InsightFace format)
- No-face frames: skip silently, only process/log when face detected — avoid log spam

### DPDP Compliance
- Physical signage at entrance + digital notice on dashboard for consent — standard for CCTV/biometric systems in India
- 90-day retention for face embeddings, auto-purge after expiry
- Audit log: append-only JSON file with timestamp, action, person_id, accessor fields — simple and grep-able
- Right to deletion: API endpoint to delete person + all embeddings + audit trail entry — DPDP Act requirement

### Claude's Discretion
- ONNX Runtime version and CUDA execution provider configuration
- Frame buffer integration details (how detection reads from Phase 112's FrameBuffer)
- Thread/task architecture for detection pipeline
- Audit log file location and rotation strategy

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- rc-sentry-ai crate from Phase 112 with frame.rs (FrameBuffer), stream.rs (RTSP extraction), config.rs
- go2rtc relay delivering H.264 NALs from 3 cameras
- Axum health endpoint at :8096 already in main.rs

### Established Patterns
- Tokio async runtime, Arc<RwLock> for shared state
- TOML config files (rc-sentry-ai.toml)
- Per-camera independent tasks with reconnect loops

### Integration Points
- Detection module reads from FrameBuffer (frame.rs)
- Detection results feed into Phase 114 (face recognition) and Phase 117 (alerts)
- Audit log accessible via API for dashboard display
- DPDP deletion endpoint on :8096

</code_context>

<specifics>
## Specific Ideas

- SCRFD ONNX model files should be stored in C:\RacingPoint\models\ directory
- Audit log at C:\RacingPoint\logs\face-audit.jsonl

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
