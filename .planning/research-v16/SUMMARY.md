# Project Research Summary

**Project:** v16.0 Security Camera AI & Attendance
**Domain:** Face Recognition Attendance + Security Camera Monitoring (Gaming Cafe / eSports Venue)
**Researched:** 2026-03-21
**Confidence:** MEDIUM-HIGH

## Executive Summary

v16.0 adds AI-powered face recognition to Racing Point's existing 13-camera Dahua setup, turning passive CCTV into an automated attendance system. The core pipeline is: RTSP frame capture from 3 entrance/reception cameras, local GPU-accelerated face detection (SCRFD), local face embedding (ArcFace), cosine similarity matching against ~100 enrolled faces, and attendance logging with Telegram alerts. The recommended approach is **fully local inference on James's RTX 4070** using ONNX Runtime, not cloud APIs. This eliminates recurring costs, removes internet dependency, achieves sub-10ms inference latency, and keeps biometric data on-premises -- critical for DPDP Act compliance.

The architecture should be a **single Rust crate (`rc-sentry-ai`)** running on James (.27) as a standalone service on port 8096. The STACK research strongly favors Rust with `retina` (RTSP), `ort` (ONNX inference), and `rusqlite` (embeddings/attendance), which aligns with the existing racecontrol ecosystem. The ARCHITECTURE research initially suggested Python/FastAPI, but given the STACK finding that `ort` with CUDA handles inference trivially in Rust, and that `retina` is a production-grade pure-Rust RTSP client, there is no compelling reason to introduce Python. Staying in Rust avoids a Python runtime dependency on James, keeps the deployment model consistent, and leverages the team's existing Rust toolchain. The one legitimate Python advantage -- OpenCV for RTSP -- is neutralized by `retina`.

The top risks are: (1) RTSP stream starvation from too many consumers hitting cameras directly -- mitigate with a local RTSP relay (go2rtc/mediamtx) as the very first infrastructure step; (2) duplicate attendance entries from overlapping camera coverage -- requires cross-camera deduplication from day one; (3) poor enrollment quality producing garbage recognition results -- enforce quality gates (minimum face size, pose angle, blur detection) during enrollment; (4) entrance camera lighting variance (backlight from doorway) degrading recognition accuracy -- test across all lighting conditions and consider using reception cameras as primary recognition points; and (5) DPDP Act compliance for biometric data -- consent signage, local-only storage, data retention policies must be in place before collecting any face data.

## Key Findings

### Recommended Stack

Local-first inference on the RTX 4070, entirely within the Rust ecosystem. No cloud APIs, no Python dependencies.

**Core technologies:**
- **`retina` 0.4.x**: RTSP client and H.264 frame extraction -- pure Rust, async/tokio-native, production-proven in Moonfire NVR
- **`ort` 2.0.0-rc.12 (CUDA)**: ONNX Runtime for GPU inference -- runs both SCRFD detection and ArcFace recognition models
- **SCRFD_10G_KPS.onnx**: Face detection with 5-point landmarks -- ~1ms/frame on RTX, outputs aligned landmarks for ArcFace preprocessing
- **ArcFace ResNet100 (ONNX)**: 512-D face embeddings -- industry SOTA, cosine similarity matching against enrolled faces
- **`rusqlite` 0.32.x**: SQLite for embeddings, persons, and attendance logs -- already proven in racecontrol ecosystem
- **FFmpeg CLI subprocess**: Video recording via `std::process::Command` with `-c copy` (no transcoding) -- segment-based recording with rolling retention
- **`teloxide` 0.13.x**: Telegram bot for alerts -- Uday already uses Telegram, free API, mature Rust framework
- **`axum` 0.8.x + `tokio`**: HTTP API and async runtime -- consistent with existing racecontrol architecture

**Critical version/setup requirements:**
- CUDA Toolkit 11.6+ on James (likely already present for Ollama)
- FFmpeg binary on PATH (Windows static build from gyan.dev)
- ONNX model files: SCRFD_10G_KPS (~17MB) and arcface_r100 (~250MB) from InsightFace model zoo

### Expected Features

**Must have (table stakes -- P1):**
- RTSP frame extraction from entrance/reception cameras (.8, .15, .154)
- Face detection on extracted frames (SCRFD on GPU)
- Face enrollment system (auto-capture from camera + manual upload, with quality gates)
- Face recognition via local ArcFace embeddings + cosine similarity matching
- Attendance logging (entry timestamp per recognized person with confidence score)
- Attendance dashboard (who is here now, recent arrivals)
- Unknown face auto-detection with "pending review" queue for staff
- Telegram alerts for unknown persons and staff arrivals
- Dashboard WebSocket notifications (existing infrastructure)

**Should have (differentiators -- P2):**
- Auto-detect and cluster unknown faces for passive database building
- Staff clock-in/clock-out with shift tracking
- Visit history and customer frequency analytics
- VIP/watchlist tagging with per-category alert rules
- Live camera feed viewing in dashboard (HLS via FFmpeg)
- Recording playback via NVR API proxy

**Defer (v2+):**
- Timeline scrubbing with event markers (complex UI)
- Racecontrol billing integration (separate milestone)
- Multi-camera live grid view (bandwidth-heavy)
- WhatsApp Business API alerts (Telegram sufficient)
- Cloud backup of flagged clips (storage cost analysis needed)
- Auto-start gaming sessions on face detection (dangerous edge cases)

### Architecture Approach

A new Rust crate `rc-sentry-ai` running on James (.27) at port 8096, structured as a standalone async service. It pulls RTSP frames via `retina`, runs face detection and recognition locally on the RTX 4070 via `ort`, maintains its own SQLite database for embeddings and attendance, pushes events to racecontrol (.23) via HTTP POST, and serves an API for enrollment and camera management. Racecontrol stores attendance records and broadcasts to the dashboard via existing WebSocket infrastructure. An RTSP relay (go2rtc or mediamtx) sits between cameras and all consumers to prevent stream starvation.

**Major components:**
1. **RTSP Relay (go2rtc/mediamtx)** -- single connection per camera, all consumers read from relay
2. **rc-sentry-ai camera module** -- `retina` RTSP client, frame extraction at 2-5 FPS per camera, FFmpeg recording subprocess
3. **rc-sentry-ai AI pipeline** -- SCRFD detection, face alignment (5-point landmark affine transform to 112x112), ArcFace embedding, cosine similarity matching (brute-force on ~100-face Vec)
4. **rc-sentry-ai attendance engine** -- cross-camera deduplication, recognition cooldown tracking, state machine (ABSENT/PRESENT for staff), SQLite storage
5. **rc-sentry-ai enrollment system** -- quality gates (min size, pose, blur), duplicate checking, staff confirmation workflow
6. **rc-sentry-ai alerts** -- Telegram bot (teloxide), WebSocket push to racecontrol, desktop notifications
7. **racecontrol API extensions** -- new /api/v1/attendance/* and /api/v1/sentry/* routes, DashboardEvent variants, config additions

### Critical Pitfalls

1. **RTSP stream starvation** -- Dahua cameras drop connections after 60-90 minutes with multiple consumers. Deploy an RTSP relay (go2rtc/mediamtx) on James as the very first step. All consumers (face detection, recording, live view, people tracker) read from the relay, never directly from cameras.

2. **Duplicate attendance from multi-camera overlap** -- Entrance and reception cameras see the same person within seconds. Implement a centralized deduplication window: after recognizing person X on any camera, suppress duplicates for 5-10 minutes across all cameras. This must be in the initial design, not patched later.

3. **Enrollment quality = recognition quality** -- Blurry, side-profile, or backlit enrollment images produce unreliable matching. Enforce quality gates: minimum 200x200px face, frontal pose (yaw < 30 degrees), blur detection (Laplacian variance), require 3-5 quality frames per person. Run duplicate check against existing gallery before confirming.

4. **Entrance lighting variance** -- Backlight from the doorway creates silhouettes during daytime. Test recognition at the entrance camera across all lighting conditions. Consider using reception cameras (.15, .154) as primary recognition points where lighting is controlled, with entrance camera as a person-detection trigger only.

5. **DPDP Act compliance** -- Face biometrics are sensitive personal data under India's Digital Personal Data Protection Act 2023 (penalties up to 250 crore INR). Requires: consent signage at entrance, explicit staff consent, embeddings-only storage (no raw face images in cloud), data deletion policy for inactive persons, audit logging of all biometric access. Must be addressed before collecting any face data.

6. **Disk full from recordings** -- 3 cameras at 4MP can generate 25-43 GB/day each. Use sub-stream for continuous recording, main stream only for event-triggered clips. Separate recording disk from OS. Daily retention cleanup with disk usage monitoring and alerts at 15% free.

## Implications for Roadmap

Based on research, suggested phase structure:

### Phase 1: RTSP Infrastructure and Camera Pipeline
**Rationale:** Everything depends on reliable frame access. RTSP relay prevents stream starvation (Pitfall 1) and protects the existing people tracker at :8095. This is the foundation with zero dependencies.
**Delivers:** RTSP relay running on James, `retina`-based frame extraction from 3 cameras, FFmpeg subprocess for segmented recording, basic health endpoint at :8096
**Addresses:** Camera feed extraction (P1), recording foundation
**Avoids:** RTSP stream starvation, disrupting existing people tracker
**Uses:** retina, FFmpeg CLI, go2rtc/mediamtx

### Phase 2: Face Detection and Recognition Pipeline
**Rationale:** The AI core. Depends on Phase 1 for frame access. Must include quality gates from day one to prevent garbage enrollments (Pitfall 3). Lighting testing must happen here (Pitfall 4).
**Delivers:** SCRFD face detection on GPU, ArcFace embedding generation, cosine similarity matching, in-memory face database, basic enrollment (manual upload with quality validation)
**Addresses:** Face detection (P1), face recognition (P1), face enrollment (P1)
**Avoids:** Poor enrollment quality, lighting-related accuracy failures
**Uses:** ort (CUDA), SCRFD model, ArcFace model, image/imageproc/ndarray

### Phase 3: Attendance Engine and Deduplication
**Rationale:** Connects recognition results to business logic. Cross-camera deduplication (Pitfall 2) must be designed in, not bolted on. State machine for staff attendance.
**Delivers:** Centralized attendance logging with cross-camera dedup, person tracking with recognition cooldown, attendance API (racecontrol routes), attendance dashboard panel, DPDP consent framework and data lifecycle policies
**Addresses:** Attendance logging (P1), attendance dashboard (P1), unknown face queue (P1), staff clock-in/clock-out (P2)
**Avoids:** Duplicate attendance entries, DPDP non-compliance
**Implements:** Attendance state machine, racecontrol API extensions, DashboardEvent variants

### Phase 4: Alerts, Enrollment UX, and Analytics
**Rationale:** Once attendance works end-to-end, add notification channels and the self-service enrollment workflow that builds the face database passively over time.
**Delivers:** Telegram bot alerts (teloxide), auto-detect unknown faces with clustering, staff confirmation workflow, VIP/watchlist tagging, visit frequency analytics, dashboard WebSocket notifications
**Addresses:** Telegram alerts (P1), unknown face auto-detection (P1), VIP alerts (P2), visit history (P2)
**Uses:** teloxide, existing WebSocket infrastructure

### Phase 5: Live Feeds, Recording Playback, and Operations
**Rationale:** Polish phase. Live streaming and recording playback are HIGH complexity but MEDIUM priority. Recording retention automation prevents Pitfall 6 (disk full).
**Delivers:** HLS live streaming via FFmpeg, NVR playback proxy, 30-day rolling retention with cleanup automation, disk usage monitoring, cloud backup of segments to Bono VPS
**Addresses:** Live camera feeds (P2), recording playback (P2)
**Avoids:** Disk full kills everything

### Phase Ordering Rationale

- **Phase 1 first** because every other phase depends on reliable RTSP frame access. The RTSP relay is the single most important infrastructure decision -- skipping it guarantees Pitfall 1 within weeks.
- **Phase 2 before Phase 3** because attendance logic cannot work without a functioning detection/recognition pipeline. The AI models and quality gates are prerequisites.
- **Phase 3 before Phase 4** because alerts and analytics are meaningless without correct attendance data. Deduplication must be proven before scaling notifications.
- **Phase 4 before Phase 5** because Telegram alerts and enrollment UX deliver more immediate value than live streaming. Uday wants to know "who arrived" before he wants to watch live feeds.
- **Phase 5 last** because the NVR at .18 already records everything. Dashboard playback and live feeds are enhancements, not prerequisites for the attendance system.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 1:** RTSP relay setup (go2rtc vs mediamtx configuration on Windows, integration with existing people tracker at :8095)
- **Phase 2:** ONNX model preprocessing specifics (SCRFD input format, ArcFace alignment transform, `ort` session configuration for CUDA on Windows). The pipeline steps are well-documented in InsightFace but Rust-specific integration details are sparse.
- **Phase 3:** DPDP Act compliance specifics -- may need legal review for consent mechanism and data retention policy wording

Phases with standard patterns (skip research-phase):
- **Phase 4:** Telegram bot via teloxide is well-documented, WebSocket broadcasting already exists in racecontrol
- **Phase 5:** FFmpeg HLS segmentation and retention cleanup are standard ops patterns

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | ArcFace + SCRFD + ort are industry-standard. retina is the only serious Rust RTSP crate. All production-proven. |
| Features | HIGH | Feature priorities clear from Uday's use cases. P1/P2/P3 tiers well-defined. |
| Architecture | MEDIUM-HIGH | Divergence between STACK (Rust/ort) and ARCHITECTURE (Python/FastAPI) researchers resolved in favor of Rust. Python path was reasonable but unnecessary given ort maturity. Minor uncertainty around retina's Windows RTSP reliability in production. |
| Pitfalls | HIGH | Pitfalls verified across multiple sources including Dahua-specific RTSP dropout reports and DPDP Act legal analysis. Cost explosion pitfall neutralized by choosing local inference. |

**Overall confidence:** MEDIUM-HIGH

### Gaps to Address

- **RTSP relay on Windows:** go2rtc and mediamtx are typically deployed on Linux. Need to verify Windows compatibility and configuration for Dahua cameras. Test during Phase 1 before committing.
- **ort CUDA on Windows:** The ort crate with CUDA execution provider needs CUDA Toolkit and cuDNN on Windows. Verify James already has the right CUDA version from Ollama, or document the installation steps.
- **retina on Windows:** retina is primarily tested on Linux. Verify RTSP frame extraction works reliably on Windows with Dahua cameras (authentication, sub-stream selection, reconnection behavior).
- **Entrance camera lighting:** Cannot be resolved with research alone. Requires physical testing at the venue across different times of day. May result in switching primary recognition to reception cameras.
- **DPDP consent mechanism:** The legal specifics of "implied consent via signage" for a commercial premises need verification. May warrant a brief legal consultation.
- **Disk capacity on James:** Need to check James's actual available disk space to confirm 30-day retention for 3 cameras is feasible, or determine if recording should be limited to entrance camera only.

## Sources

### Primary (HIGH confidence)
- [InsightFace GitHub](https://github.com/deepinsight/insightface) -- SCRFD + ArcFace model architecture and ONNX downloads
- [ort crate (pykeio)](https://github.com/pykeio/ort) -- ONNX Runtime Rust bindings, v2.0.0-rc.12, CUDA support
- [retina crate](https://crates.io/crates/retina) -- RTSP library, v0.4.19
- [AWS Rekognition pricing/docs](https://aws.amazon.com/rekognition/pricing/) -- cost modeling (used to justify local inference)
- [DPDP Act biometric regulation](https://ksandk.com/data-protection-and-data-privacy/regulation-of-biometric-data-under-the-dpdp-act/) -- legal requirements
- Existing racecontrol codebase and CLAUDE.md -- network map, camera info, deployment model

### Secondary (MEDIUM confidence)
- [Dahua RTSP dropout reports (Scrypted)](https://github.com/koush/scrypted/discussions/1591) -- connection stability issues
- [go2rtc buffer issues](https://github.com/AlexxIT/go2rtc/issues/383) -- relay lifecycle management
- [Montavue storage chart](https://montavue.com/blogs/news/storage-chart-for-2mp-1080p-4mp-2k-and-8mp-4k-ip-security-cameras) -- recording storage estimates
- [Frigate face recognition docs](https://docs.frigate.video/configuration/face_recognition/) -- enrollment quality guidance
- [Microsoft Face API characteristics](https://learn.microsoft.com/en-us/legal/cognitive-services/face/characteristics-and-limitations) -- head angle/quality requirements

### Tertiary (LOW confidence)
- [face-reidentification pipeline](https://github.com/yakhyo/face-reidentification) -- SCRFD + ArcFace + FAISS reference (Python, not Rust)
- [Medium: Facial Recognition Attendance Architecture](https://medium.com/thedevproject/facial-recognition-attendance-system-architecture-533e029a2dc1) -- general architecture patterns

---
*Research completed: 2026-03-21*
*Ready for roadmap: yes*
