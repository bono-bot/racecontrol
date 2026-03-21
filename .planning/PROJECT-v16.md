# v16.0 Security Camera AI & Attendance

## What This Is

An integrated security camera monitoring and face-recognition attendance system for Racing Point HQ. Builds on the existing 13x Dahua 4MP camera network and NVR (.18) to add live feed monitoring with motion/event alerts, continuous recording with 30-day retention, and automatic face-recognition-based attendance tracking for both customers and staff. Runs as a separate service but feeds data into the racecontrol dashboard.

## Core Value

Automatically identify and log every person entering Racing Point HQ — customers get recognized and their visit is logged without manual check-in, staff attendance is tracked hands-free.

## Requirements

### Validated

<!-- Existing infrastructure this milestone builds on -->

- ✓ 13x Dahua 4MP cameras on local network — existing
- ✓ NVR at 192.168.31.18 — existing
- ✓ People tracker (YOLOv8 + FastAPI) at port 8095 — existing
- ✓ RTSP streams available (subtype=1, auth: admin/Admin@123) — existing
- ✓ Entrance camera at .8, reception at .15/.154 — existing

### Active

- [ ] Live camera feed monitoring on dashboard
- [ ] Motion/event detection with alerts
- [ ] Continuous recording with 30-day local retention + cloud backup
- [ ] Face recognition via cloud API for known person identification
- [ ] Hybrid enrollment: auto-detect new faces, staff confirms/names them
- [ ] Customer attendance: auto-log entry timestamp on face recognition
- [ ] Staff attendance: auto-log clock-in/clock-out on face recognition (face only, no PIN)
- [ ] Attendance dashboard showing who is present, visit history
- [ ] Multi-channel alerts: racecontrol dashboard + James desktop notification + mobile push + Telegram/WhatsApp
- [ ] Customer profile with visit history and face embeddings
- [ ] Staff profile with shift history and face embeddings
- [ ] Recording playback with timeline scrubbing

### Out of Scope

- Auto-start gaming sessions on face detection — deferred, requires billing integration changes
- Face recognition on all 13 cameras — only entrance/reception cameras for attendance
- On-premise face recognition (RTX 4070) — using cloud API instead for accuracy/maintenance
- Biometric PIN verification for staff — face-only is sufficient

## Context

- **Existing camera infra:** 13x Dahua 4MP cameras, NVR at .18, RTSP streams with subtype=1
- **People tracker:** YOLOv8 + FastAPI already running at port 8095 for entry/exit counting
- **Key cameras for attendance:** Entrance (.8), Reception (.15, .154)
- **James machine (RTX 4070):** Available for local processing but face recognition will use cloud API
- **Bono VPS:** Cloud backup target for recordings and potentially face API proxy
- **Storage:** Local on James for 30 days, older footage to Bono VPS
- **Integration:** Separate service with its own UI, but attendance data feeds into racecontrol dashboard

## Constraints

- **Cloud API:** Face recognition via cloud service (to be determined during research — AWS Rekognition, Azure Face, or alternatives)
- **Network:** Cameras on 192.168.31.x subnet, all accessible from James (.27)
- **Privacy:** Face embeddings stored locally, not raw photos in cloud. Comply with Indian IT Act.
- **Storage:** 30-day retention locally, cloud backup for longer term. 6+ cameras recording = significant disk space
- **Latency:** Face recognition should complete within 2-3 seconds of person entering frame
- **Existing systems:** Must not disrupt existing people tracker at :8095 or NVR recording

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Cloud API for face recognition | Better accuracy, no model maintenance, Uday prefers managed service | — Pending |
| Hybrid enrollment (auto-detect + staff confirm) | Balances automation with accuracy — avoids false enrollments | — Pending |
| Separate service feeding into racecontrol | Decoupled for independent development/deployment, but unified data | — Pending |
| Face-only staff auth (no PIN) | Simpler UX, cameras already at entry points | — Pending |
| 30-day local + cloud backup retention | Balances storage costs with compliance needs | — Pending |
| Multi-channel alerts (dashboard + desktop + mobile + Telegram) | Uday wants to be notified everywhere, especially on phone | — Pending |

---
*Last updated: 2026-03-21 after initialization*
