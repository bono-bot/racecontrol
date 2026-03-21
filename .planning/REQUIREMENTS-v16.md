# Requirements: v16.0 Security Camera AI & Attendance

**Defined:** 2026-03-21
**Core Value:** Automatically identify and log every person entering Racing Point HQ — customers get recognized and their visit is logged without manual check-in, staff attendance is tracked hands-free.

## v1 Requirements

Requirements for initial release. Each maps to roadmap phases.

### Camera Infrastructure

- [ ] **CAM-01**: RTSP relay service prevents Dahua stream starvation with auto-reconnect
- [ ] **CAM-02**: Multi-camera stream management for entrance (.8) and reception (.15/.154) cameras
- [ ] **CAM-03**: Stream health monitoring with auto-reconnect on failure
- [ ] **CAM-04**: Integration with existing YOLOv8 people tracker at :8095

### Face Recognition

- [ ] **FACE-01**: SCRFD face detection on camera frames using RTX 4070 GPU
- [ ] **FACE-02**: ArcFace embedding extraction for identity matching
- [ ] **FACE-03**: Quality gates to reject blurry, side-profile, and backlit captures
- [ ] **FACE-04**: Lighting normalization for entrance camera conditions

### Enrollment

- [ ] **ENRL-01**: Face profile management (add/remove/update face photos)
- [ ] **ENRL-02**: Multi-angle enrollment capture for better recognition accuracy

### Attendance

- [ ] **ATTN-01**: Auto-log entry timestamp on face recognition
- [ ] **ATTN-02**: Staff clock-in/clock-out tracking with shift history

### Alerts

- [ ] **ALRT-01**: Dashboard notifications in racecontrol for attendance events
- [ ] **ALRT-02**: Desktop popup/sound notification on James machine
- [ ] **ALRT-03**: Unknown person alert when unrecognized face detected

### Monitoring

- [ ] **MNTR-01**: Live camera feed viewing in dashboard (MJPEG proxy)
- [ ] **MNTR-02**: Timeline-based recording playback with event markers

### Privacy

- [ ] **PRIV-01**: DPDP Act 2023 consent framework for face data collection

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Notifications

- **NOTF-01**: Telegram bot alerts for attendance and security events
- **NOTF-02**: Mobile push notifications to Uday's phone

### Recording

- **RECD-01**: 30-day continuous recording with local retention
- **RECD-02**: Cloud backup of recordings to Bono VPS

### Attendance (Extended)

- **ATTN-03**: Cross-camera deduplication (entrance + reception overlap)
- **ATTN-04**: Customer visit history with frequency analytics

### Enrollment (Extended)

- **ENRL-03**: Hybrid auto-detect enrollment (auto-detect new faces, queue for staff naming)
- **ENRL-04**: Staff-confirmed enrollment workflow with approval UI

## Out of Scope

| Feature | Reason |
|---------|--------|
| Auto-start gaming sessions on face detection | Requires billing integration changes, deferred |
| Face recognition on all 13 cameras | Only entrance/reception needed for attendance |
| Cloud face recognition API | Local RTX 4070 inference is faster, free, and more reliable |
| WhatsApp Business API | Meta verification, per-message costs, template approval overhead |
| Biometric PIN verification for staff | Face-only is sufficient for clock-in/out |
| On-camera face detection (Dahua AI) | Inconsistent across camera models, prefer unified pipeline |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| CAM-01 | — | Pending |
| CAM-02 | — | Pending |
| CAM-03 | — | Pending |
| CAM-04 | — | Pending |
| FACE-01 | — | Pending |
| FACE-02 | — | Pending |
| FACE-03 | — | Pending |
| FACE-04 | — | Pending |
| ENRL-01 | — | Pending |
| ENRL-02 | — | Pending |
| ATTN-01 | — | Pending |
| ATTN-02 | — | Pending |
| ALRT-01 | — | Pending |
| ALRT-02 | — | Pending |
| ALRT-03 | — | Pending |
| MNTR-01 | — | Pending |
| MNTR-02 | — | Pending |
| PRIV-01 | — | Pending |

**Coverage:**
- v1 requirements: 18 total
- Mapped to phases: 0
- Unmapped: 18

---
*Requirements defined: 2026-03-21*
*Last updated: 2026-03-21 after initial definition*
