# Architecture Patterns

**Domain:** Security Camera AI & Face Recognition Attendance
**Researched:** 2026-03-21

## Recommended Architecture

A new standalone Python/FastAPI service (`rc-sentry`) running on James (.27) that handles RTSP frame capture, face detection, cloud face recognition, and attendance logging. It exposes a REST API consumed by racecontrol (.23) which surfaces attendance data on the dashboard via existing WebSocket infrastructure.

```
                          Cloud Face API
                          (AWS Rekognition)
                               ^
                               | HTTPS (face search/index)
                               |
  Dahua Cameras ──RTSP──> rc-sentry (James .27, :8096)
  (.8, .15, .154)         Python/FastAPI + OpenCV
                          - Frame capture (threaded, per camera)
                          - Face detection (local, YOLO/RetinaFace)
                          - Face crop + cloud recognition
                          - Attendance state machine
                          - Enrollment queue
                               |
                               | REST API (HTTP)
                               v
                      racecontrol (.23, :8080)
                      - New /api/v1/attendance/* routes
                      - New /api/v1/sentry/* routes
                      - Stores attendance in SQLite
                      - Broadcasts to dashboard via WS
                               |
                               | WebSocket (DashboardEvent)
                               v
                      Dashboard (.23, :3200)
                      - Attendance panel
                      - Live camera feeds (RTSP→HLS/MJPEG proxy)
                      - Enrollment UI
                               |
                               | Push notifications
                               v
                      Alerts (Telegram, Desktop, Mobile)
```

### Why a Separate Python Service (Not Rust)

1. **OpenCV + RTSP**: Python OpenCV is battle-tested for RTSP frame capture. Rust OpenCV bindings exist but are painful to build on Windows with CUDA.
2. **Face detection libraries**: RetinaFace, InsightFace, ultralytics YOLO -- all Python-first with GPU support via PyTorch/ONNX.
3. **Cloud API SDKs**: boto3 (AWS Rekognition) is mature Python. Rust SDK exists but adds build complexity for a service that's not latency-critical at the network boundary.
4. **Existing pattern**: The people tracker at :8095 is already Python/FastAPI/YOLOv8. Same deployment model, same team familiarity.
5. **James has RTX 4070**: Local face detection (not recognition) runs on GPU via Python trivially. Rust would require ONNX Runtime C bindings.

### Why NOT Merge Into People Tracker (:8095)

The people tracker does anonymous entry/exit counting. Face recognition is a fundamentally different pipeline (crop faces, generate embeddings, match against known faces, track identity across frames). Separate concerns, separate service. They can share the same RTSP streams since both use subtype=1 (sub-stream).

## Component Boundaries

| Component | Responsibility | Runs On | Port | Communicates With |
|-----------|---------------|---------|------|-------------------|
| **rc-sentry** | RTSP capture, face detection, cloud recognition, attendance events | James .27 | 8096 | Cameras (RTSP), Cloud API (HTTPS), racecontrol (HTTP) |
| **racecontrol** | Attendance data storage, API, dashboard broadcasting | Server .23 | 8080 | rc-sentry (HTTP), Dashboard (WS), Alerts (HTTP) |
| **Dashboard** | Attendance UI, live feeds, enrollment interface | Server .23 | 3200 | racecontrol (WS/HTTP) |
| **Cloud Face API** | Face embedding comparison, face collection storage | AWS/Azure | 443 | rc-sentry (HTTPS) |
| **NVR** | Continuous recording (existing, unchanged) | .18 | - | Cameras (existing) |
| **People Tracker** | Entry/exit people counting (existing, unchanged) | James .27 | 8095 | Cameras (RTSP) |

### rc-sentry Internal Components

```
rc-sentry/
  capture/          # Threaded RTSP frame grabbers (one per camera)
  detection/        # Local face detection (RetinaFace or YOLO-face)
  recognition/      # Cloud API client (search_faces_by_image)
  tracking/         # Face tracker across frames (avoid re-recognition every frame)
  attendance/       # State machine: detected -> recognized -> logged
  enrollment/       # New face queue, admin confirmation flow
  api/              # FastAPI routes for racecontrol integration
  config.py         # Camera URLs, API keys, thresholds
```

## Data Flow

### Flow 1: Real-Time Attendance Recognition

```
1. rc-sentry captures RTSP frames from entrance camera (.8) at ~2-5 FPS
2. Local face detection (GPU) extracts face bounding boxes + crops
3. Face tracker assigns temporary IDs to avoid re-processing same person
4. For each NEW face (not tracked in last N seconds):
   a. Crop face region, resize to 160x160 or similar
   b. Send to Cloud Face API (search_faces_by_image)
   c. Cloud returns match confidence + person_id (or no match)
5. If match found (confidence > threshold):
   a. rc-sentry POSTs to racecontrol: POST /api/v1/attendance/log
      { person_id, camera_id, timestamp, confidence, type: "entry" }
   b. racecontrol stores in SQLite, broadcasts DashboardEvent::AttendanceUpdate
   c. Dashboard shows "Rahul just checked in" notification
6. If no match found:
   a. rc-sentry POSTs to racecontrol: POST /api/v1/enrollment/queue
      { face_crop_b64, camera_id, timestamp }
   b. Staff sees "Unknown person detected" in enrollment queue
   c. Staff names the person -> POST /api/v1/enrollment/confirm
   d. racecontrol tells rc-sentry to index the face in cloud collection
```

### Flow 2: Staff Clock-In/Clock-Out

```
1. Same as Flow 1 steps 1-5
2. When staff member recognized:
   a. Check last attendance record for this staff_id
   b. If no record today or last was "clock_out" -> log "clock_in"
   c. If last was "clock_in" and enough time passed -> log "clock_out"
   d. Push notification to Uday: "Amit clocked in at 10:02 AM"
```

### Flow 3: Live Camera Feeds on Dashboard

```
1. Dashboard requests camera feed via racecontrol proxy
2. racecontrol proxies RTSP->MJPEG or uses rc-sentry's /stream/{camera_id} endpoint
3. rc-sentry uses OpenCV to decode RTSP and serve as MJPEG over HTTP
4. Dashboard renders <img src="/api/v1/sentry/stream/entrance">
   (MJPEG works natively in <img> tags, no player needed)
```

### Flow 4: Face Enrollment

```
1. Admin opens enrollment UI on dashboard
2. Can upload photo OR select from "unknown faces" queue
3. POST /api/v1/enrollment/register { name, phone, role, face_images[] }
4. racecontrol forwards to rc-sentry which:
   a. Runs face detection on each image to extract clean crops
   b. Indexes faces in cloud collection (index_faces API)
   c. Returns face_id from cloud
5. racecontrol stores person profile with face_id reference
```

## Patterns to Follow

### Pattern 1: Deduplication via Face Tracking

**What:** Use a simple centroid/IoU tracker to avoid sending the same face to the cloud API every frame. Only recognize a face when it first appears or after a cooldown period.

**When:** Always. Cloud API calls cost money and add latency.

**Implementation:**
```python
class FaceTracker:
    def __init__(self, max_disappeared=30, recognition_cooldown=60):
        self.tracked_faces = {}  # track_id -> {bbox, last_recognized, person_id}
        self.recognition_cooldown = recognition_cooldown  # seconds

    def update(self, detections: list[BBox]) -> list[tuple[int, BBox, bool]]:
        """Returns (track_id, bbox, needs_recognition) for each detection."""
        # IoU matching against existing tracks
        # Only needs_recognition=True if:
        #   - New track (never recognized)
        #   - Cooldown expired since last recognition
```

### Pattern 2: Push Events from rc-sentry to racecontrol

**What:** rc-sentry pushes attendance events to racecontrol via HTTP POST, not polling. Racecontrol is the source of truth for attendance records.

**When:** Every recognized face event, unknown face event, health heartbeat.

**Why:** Matches existing pattern -- rc-agent pushes to racecontrol, people tracker is polled. For attendance, push is better because events are infrequent but time-sensitive.

### Pattern 3: Shared Authentication Token

**What:** rc-sentry authenticates to racecontrol using a shared secret in config (same pattern as rc-process-guard).

**When:** All rc-sentry -> racecontrol API calls.

**Example:** racecontrol.toml already has `guard_secret` for process guard. Add `sentry_secret` for rc-sentry.

### Pattern 4: Graceful Degradation

**What:** If cloud API is unreachable, rc-sentry continues capturing and detecting faces locally, queuing recognition requests for retry. Attendance logging degrades to "person detected at entrance" without identity.

**When:** Network outage, API rate limit, API downtime.

## Anti-Patterns to Avoid

### Anti-Pattern 1: Sending Every Frame to Cloud

**What:** Calling cloud face recognition on every captured frame.
**Why bad:** At 5 FPS across 3 cameras = 15 API calls/second = 1.3M calls/day. Costs ~$1,300/day on AWS Rekognition.
**Instead:** Detect faces locally (free, GPU), track across frames, only send NEW faces to cloud. Expect ~50-200 cloud calls/day for a cafe.

### Anti-Pattern 2: Storing Raw Face Images in Cloud

**What:** Uploading full photos to AWS/Azure for storage.
**Why bad:** Privacy concerns under Indian IT Act. Embeddings are not reversible to faces; raw images are.
**Instead:** Store face embeddings in cloud collection (AWS manages this opaquely). Store reference photos locally on James only, encrypted at rest.

### Anti-Pattern 3: Running Face Recognition in racecontrol (Rust)

**What:** Adding RTSP capture and face recognition directly into the racecontrol Rust binary.
**Why bad:** Adds heavy dependencies (OpenCV, ONNX, CUDA) to a clean Axum server. Different failure modes. Different update cadence. Would need to run on James instead of server, breaking deployment model.
**Instead:** Separate Python service on James. Clean HTTP interface to racecontrol.

### Anti-Pattern 4: Using Main Stream for Face Detection

**What:** Pulling RTSP main stream (4MP, subtype=0) for face detection.
**Why bad:** 4MP frames are unnecessary for face detection at entrance distance. Wastes GPU memory and bandwidth. May conflict with NVR recording.
**Instead:** Use sub-stream (subtype=1, typically 640x480 or 720p). Sufficient for face detection at 2-5m distance. NVR continues recording main stream independently.

### Anti-Pattern 5: RTSP Direct in Browser

**What:** Trying to play RTSP streams directly in the browser.
**Why bad:** Browsers don't support RTSP. WebRTC requires a signaling server. HLS adds 5-10s latency.
**Instead:** MJPEG proxy from rc-sentry. Zero latency, works in `<img>` tag, no player library needed. Low bandwidth for monitoring (1-2 FPS is fine for live view).

## Database Schema Additions (racecontrol SQLite)

```sql
-- People (customers + staff) with face recognition
CREATE TABLE IF NOT EXISTS persons (
    id TEXT PRIMARY KEY,           -- UUID
    name TEXT NOT NULL,
    phone TEXT,                    -- links to existing drivers table
    role TEXT NOT NULL DEFAULT 'customer',  -- 'customer' | 'staff'
    face_collection_id TEXT,       -- cloud face collection reference
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Face images stored locally (not in cloud)
CREATE TABLE IF NOT EXISTS person_faces (
    id TEXT PRIMARY KEY,
    person_id TEXT NOT NULL REFERENCES persons(id),
    cloud_face_id TEXT,            -- AWS Rekognition face ID in collection
    image_path TEXT,               -- local file path on James (encrypted)
    created_at TEXT NOT NULL
);

-- Attendance log
CREATE TABLE IF NOT EXISTS attendance_log (
    id TEXT PRIMARY KEY,
    person_id TEXT REFERENCES persons(id),  -- NULL if unrecognized
    event_type TEXT NOT NULL,       -- 'entry' | 'exit' | 'clock_in' | 'clock_out'
    camera_id TEXT NOT NULL,
    confidence REAL,               -- recognition confidence 0.0-1.0
    timestamp TEXT NOT NULL,
    created_at TEXT NOT NULL
);

-- Enrollment queue (unrecognized faces awaiting staff confirmation)
CREATE TABLE IF NOT EXISTS enrollment_queue (
    id TEXT PRIMARY KEY,
    face_crop_path TEXT NOT NULL,   -- local temp file
    camera_id TEXT NOT NULL,
    detected_at TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',  -- 'pending' | 'confirmed' | 'dismissed'
    confirmed_person_id TEXT REFERENCES persons(id),
    confirmed_at TEXT
);
```

## Integration with Existing racecontrol

### New API Routes

```
# Attendance (rc-sentry pushes here)
POST   /api/v1/attendance/log          -- log attendance event
GET    /api/v1/attendance/today         -- who's present right now
GET    /api/v1/attendance/history       -- attendance history with filters

# Enrollment
POST   /api/v1/enrollment/queue        -- rc-sentry pushes unknown faces
GET    /api/v1/enrollment/pending       -- staff views pending enrollments
POST   /api/v1/enrollment/confirm      -- staff confirms identity
POST   /api/v1/enrollment/register     -- register new person with photos
DELETE /api/v1/enrollment/dismiss      -- dismiss false detection

# Sentry (camera management)
GET    /api/v1/sentry/status           -- rc-sentry health + camera status
GET    /api/v1/sentry/stream/:camera   -- proxied MJPEG stream

# Staff
GET    /api/v1/staff/attendance        -- staff clock-in/clock-out report
```

### New DashboardEvent Variants

```rust
// Add to rc_common::protocol::DashboardEvent
AttendanceUpdate { person_name: String, event_type: String, camera: String, timestamp: String }
UnknownPersonDetected { enrollment_id: String, camera: String, timestamp: String }
SentryHealthUpdate { cameras_online: u8, cameras_total: u8, recognition_active: bool }
```

### Config Additions (racecontrol.toml)

```toml
[sentry]
enabled = true
sentry_url = "http://192.168.31.27:8096"
sentry_secret = "shared-secret-here"
recognition_confidence_threshold = 0.85
staff_clock_out_min_hours = 4  # minimum hours before auto-clock-out
```

## Scalability Considerations

| Concern | Current (13 cameras, 3 for faces) | Future (20+ cameras) |
|---------|-----------------------------------|----------------------|
| RTSP capture | 3 threads on James, ~5% CPU | Add cameras = add threads, James handles 20+ easily |
| Face detection | GPU (RTX 4070), <50ms/frame | RTX 4070 handles 20+ streams at 5 FPS |
| Cloud API calls | ~100-500/day, <$1/day | Linear with unique visitors, not cameras |
| Storage (attendance) | SQLite rows, negligible | SQLite handles millions of rows fine |
| Storage (face crops) | ~1KB/crop, ~500 crops/day = 500KB/day | Negligible even at 10x scale |
| Live streams (MJPEG) | 3 streams, ~1-2 Mbps total | Bandwidth-limited, add streams as needed |
| Dashboard updates | WS broadcast, same as pod events | No scaling concern |

## Suggested Build Order (Dependencies)

```
Phase 1: RTSP Capture + Local Face Detection
  - rc-sentry skeleton (FastAPI on James :8096)
  - RTSP frame capture from entrance camera (.8)
  - Local face detection (RetinaFace/YOLO-face on GPU)
  - MJPEG proxy endpoint for dashboard
  - Health endpoint
  Depends on: nothing (standalone)
  Enables: everything else

Phase 2: Cloud Face Recognition + Attendance Logging
  - AWS Rekognition (or chosen API) integration
  - Face collection management (create, index, search)
  - Face tracker (deduplication across frames)
  - Push attendance events to racecontrol
  - racecontrol attendance API routes + SQLite tables
  - Dashboard attendance panel (basic: who's here today)
  Depends on: Phase 1 (face detection pipeline)
  Enables: Phase 3 (enrollment), Phase 4 (staff)

Phase 3: Enrollment Pipeline
  - Unknown face queue (rc-sentry -> racecontrol)
  - Staff enrollment UI on dashboard
  - Photo upload + cloud indexing flow
  - Link persons to existing drivers table (by phone)
  Depends on: Phase 2 (recognition working)
  Enables: growing the face collection

Phase 4: Staff Attendance + Alerts
  - Staff clock-in/clock-out state machine
  - Staff attendance reports
  - Multi-channel alerts (Telegram, desktop notification)
  - Uday mobile push notifications
  Depends on: Phase 2 + 3 (recognition + enrollment)

Phase 5: Recording + Playback (if needed beyond NVR)
  - Only if NVR playback is insufficient
  - HLS recording segments on James disk
  - 30-day retention with cleanup
  - Timeline playback in dashboard
  Depends on: Phase 1 (RTSP capture)
  Note: NVR at .18 already records everything. This phase may be unnecessary.
```

## Sources

- [AWS Rekognition face detection video stream architecture](https://github.com/aws-samples/amazon-rekognition-face-detection-video-stream) - MEDIUM confidence
- [AWS Rekognition pricing](https://aws.amazon.com/rekognition/pricing/) - HIGH confidence
- [Real-time face identification with Kinesis + Rekognition](https://medium.com/zenofai/real-time-face-identification-on-live-camera-feed-using-amazon-rekognition-video-and-kinesis-video-52b0a59e8a9) - MEDIUM confidence
- [Roboflow RTSP computer vision guide](https://blog.roboflow.com/computer-vision-rtsp-camera/) - MEDIUM confidence
- [Facial Recognition Attendance System Architecture](https://medium.com/thedevproject/facial-recognition-attendance-system-architecture-533e029a2dc1) - MEDIUM confidence
- Existing racecontrol codebase (state.rs, ws/mod.rs, routes.rs) - HIGH confidence
- Existing CLAUDE.md network map and camera info - HIGH confidence
