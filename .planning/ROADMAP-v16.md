# Roadmap: v16.0 Security Camera AI & Attendance

## Overview

Transform Racing Point's existing 13-camera Dahua setup into an automated face-recognition attendance system. Starting with reliable RTSP frame access through a relay, then building the local AI pipeline (SCRFD detection + ArcFace recognition on RTX 4070 via ort), enrollment and attendance logging, alerts, and finally dashboard camera monitoring with NVR playback proxying. All inference runs locally -- zero cloud API cost, zero internet dependency, sub-10ms latency.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [ ] **Phase 1: RTSP Infrastructure & Camera Pipeline** - Relay service, multi-camera management, health monitoring, and people tracker integration
- [ ] **Phase 2: Face Detection & Privacy Foundation** - SCRFD face detection on GPU via ort, plus DPDP Act consent framework
- [ ] **Phase 3: Face Recognition & Quality Gates** - ArcFace embedding extraction, cosine similarity matching, quality filtering, and lighting normalization
- [ ] **Phase 4: Face Enrollment System** - Profile management, multi-angle capture, and face database population
- [ ] **Phase 5: Attendance Engine** - Auto-log entry timestamps on recognition, staff clock-in/clock-out with shift tracking
- [ ] **Phase 6: Alerts & Notifications** - Dashboard notifications, desktop popups, and unknown person alerts
- [ ] **Phase 7: Live Camera Feeds** - MJPEG proxy for real-time camera viewing in dashboard
- [ ] **Phase 8: NVR Playback Proxy** - Query Dahua NVR API for stored footage, serve through dashboard with event markers

## Phase Details

### Phase 1: RTSP Infrastructure & Camera Pipeline
**Goal**: Reliable frame access from all attendance cameras with zero disruption to existing systems
**Depends on**: Nothing (first phase)
**Requirements**: CAM-01, CAM-02, CAM-03, CAM-04
**Success Criteria** (what must be TRUE):
  1. RTSP relay (go2rtc or mediamtx) runs on James and proxies streams from entrance (.8) and reception (.15, .154) cameras without dropping connections over 24+ hours
  2. rc-sentry-ai crate exists with retina-based frame extraction pulling frames at 2-5 FPS from each camera via the relay
  3. Stream health endpoint at :8096 reports per-camera status and auto-reconnects within 30 seconds of a camera dropout
  4. Existing people tracker at :8095 continues working unaffected -- it reads from the relay instead of directly from cameras
**Plans**: TBD

Plans:
- [ ] 01-01: RTSP relay setup and camera wiring
- [ ] 01-02: rc-sentry-ai crate scaffold and retina frame extraction
- [ ] 01-03: Stream health monitoring and auto-reconnect
- [ ] 01-04: People tracker migration to relay

### Phase 2: Face Detection & Privacy Foundation
**Goal**: Detect faces in camera frames on the GPU, with legal compliance for biometric data collection
**Depends on**: Phase 1
**Requirements**: FACE-01, PRIV-01
**Success Criteria** (what must be TRUE):
  1. SCRFD model runs via ort with CUDA on the RTX 4070 and detects faces in live camera frames with bounding boxes and 5-point landmarks
  2. Detection completes in under 10ms per frame and handles multiple simultaneous faces
  3. DPDP Act consent mechanism is implemented -- consent signage requirements documented, data retention policy enforced, and audit logging records all biometric data access
**Plans**: TBD

Plans:
- [ ] 02-01: ONNX Runtime setup with CUDA and SCRFD model loading
- [ ] 02-02: Face detection pipeline integration with camera frames
- [ ] 02-03: DPDP consent framework and audit logging

### Phase 3: Face Recognition & Quality Gates
**Goal**: Identify detected faces by matching embeddings against enrolled faces, rejecting poor-quality captures
**Depends on**: Phase 2
**Requirements**: FACE-02, FACE-03, FACE-04
**Success Criteria** (what must be TRUE):
  1. ArcFace model generates 512-D embeddings from aligned face crops, and cosine similarity matching correctly identifies enrolled persons with confidence above threshold (~0.4-0.5)
  2. Quality gates reject blurry frames (Laplacian variance below threshold), extreme side-profile poses (yaw > 30 degrees), and faces smaller than 200x200px before sending to recognition
  3. Lighting normalization handles entrance camera backlight conditions -- recognition accuracy remains consistent across morning, midday, and evening lighting
  4. Face tracker deduplicates across frames so the same person walking through is recognized once per cooldown period, not on every frame
**Plans**: TBD

Plans:
- [ ] 03-01: ArcFace model loading and embedding extraction
- [ ] 03-02: Face alignment via 5-point landmark affine transform
- [ ] 03-03: Quality gates (blur, pose, size filtering)
- [ ] 03-04: Lighting normalization and cross-condition testing
- [ ] 03-05: Face tracker with recognition cooldown

### Phase 4: Face Enrollment System
**Goal**: Staff can add, update, and remove face profiles to build the recognition database
**Depends on**: Phase 3
**Requirements**: ENRL-01, ENRL-02
**Success Criteria** (what must be TRUE):
  1. Staff can create a person profile (name, role, phone) and associate face photos via an API endpoint
  2. Multi-angle enrollment captures 3-5 quality frames from different angles, rejecting images that fail quality gates, and stores embeddings in SQLite
  3. Staff can update or delete a person's face profile, and the in-memory embedding gallery reflects changes immediately
  4. Duplicate detection prevents enrolling the same person twice by checking new embeddings against existing ones
**Plans**: TBD

Plans:
- [ ] 04-01: Person and embedding database schema (SQLite)
- [ ] 04-02: Enrollment API endpoints and photo processing
- [ ] 04-03: Multi-angle capture with quality validation
- [ ] 04-04: In-memory gallery sync and duplicate detection

### Phase 5: Attendance Engine
**Goal**: Automatically log attendance when recognized faces appear on camera, with staff shift tracking
**Depends on**: Phase 4
**Requirements**: ATTN-01, ATTN-02
**Success Criteria** (what must be TRUE):
  1. When an enrolled person is recognized at an entrance/reception camera, an attendance entry is logged with person ID, camera, timestamp, and confidence -- without any manual action
  2. Cross-camera deduplication prevents duplicate attendance entries when the same person is seen by entrance (.8) and then reception (.15/.154) cameras within a configurable window (default 5-10 minutes)
  3. Staff members have automatic clock-in on first recognition of the day and clock-out after configurable minimum hours, with shift history queryable via API
  4. Attendance API serves "who is present now" and "attendance history" endpoints that racecontrol can consume for dashboard display
**Plans**: TBD

Plans:
- [ ] 05-01: Attendance logging and cross-camera deduplication
- [ ] 05-02: Staff clock-in/clock-out state machine
- [ ] 05-03: Attendance API endpoints and racecontrol integration

### Phase 6: Alerts & Notifications
**Goal**: Staff and Uday are notified in real time about attendance events and unknown persons
**Depends on**: Phase 5
**Requirements**: ALRT-01, ALRT-02, ALRT-03
**Success Criteria** (what must be TRUE):
  1. Attendance events (customer arrival, staff clock-in/out) appear as real-time notifications in the racecontrol dashboard via WebSocket broadcast
  2. James machine displays a desktop popup with sound when a person is detected at the entrance camera
  3. Unknown (unrecognized) faces trigger a distinct alert that appears in both the dashboard and as a desktop notification, with the face crop visible for staff review
**Plans**: TBD

Plans:
- [ ] 06-01: Dashboard WebSocket notifications (DashboardEvent variants)
- [ ] 06-02: Desktop popup and sound notifications on James
- [ ] 06-03: Unknown person alert pipeline with face crop display

### Phase 7: Live Camera Feeds
**Goal**: Staff can view live camera feeds directly in the racecontrol dashboard
**Depends on**: Phase 1
**Requirements**: MNTR-01
**Success Criteria** (what must be TRUE):
  1. Dashboard displays live MJPEG streams from entrance and reception cameras with under 2-second latency
  2. MJPEG proxy endpoint at rc-sentry-ai serves frames that render natively in browser img tags -- no video player library required
  3. Live feed does not degrade face detection performance -- frame serving is independent of the AI pipeline
**Plans**: TBD

Plans:
- [ ] 07-01: MJPEG proxy endpoint serving camera frames
- [ ] 07-02: Dashboard live feed UI component

### Phase 8: NVR Playback Proxy
**Goal**: Staff can review past footage from the Dahua NVR through the dashboard without accessing the NVR directly
**Depends on**: Phase 7
**Requirements**: MNTR-02
**Success Criteria** (what must be TRUE):
  1. Dashboard provides a time-range selector that queries the Dahua NVR at .18 for stored footage and streams it through rc-sentry-ai
  2. Attendance event markers overlay on the playback timeline so staff can jump to moments when specific persons were detected
  3. Playback works for all 3 attendance cameras and does not interfere with the NVR's ongoing recording
**Plans**: TBD

Plans:
- [ ] 08-01: Dahua NVR API integration and footage query
- [ ] 08-02: Playback proxy streaming through rc-sentry-ai
- [ ] 08-03: Event marker overlay on playback timeline

## Progress

**Execution Order:**
Phases execute in numeric order: 1 -> 2 -> 3 -> 4 -> 5 -> 6 -> 7 -> 8
(Phase 7 depends only on Phase 1, so it could run in parallel with 2-6 if needed)

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. RTSP Infrastructure & Camera Pipeline | 0/4 | Not started | - |
| 2. Face Detection & Privacy Foundation | 0/3 | Not started | - |
| 3. Face Recognition & Quality Gates | 0/5 | Not started | - |
| 4. Face Enrollment System | 0/4 | Not started | - |
| 5. Attendance Engine | 0/3 | Not started | - |
| 6. Alerts & Notifications | 0/3 | Not started | - |
| 7. Live Camera Feeds | 0/2 | Not started | - |
| 8. NVR Playback Proxy | 0/3 | Not started | - |
