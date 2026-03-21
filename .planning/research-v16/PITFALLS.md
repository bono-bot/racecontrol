# Pitfalls Research

**Domain:** Face Recognition Attendance + Security Camera Monitoring (Dahua 4MP / RTSP / Cloud Face API)
**Researched:** 2026-03-21
**Confidence:** HIGH (most pitfalls verified across multiple sources and Dahua-specific reports)

## Critical Pitfalls

### Pitfall 1: RTSP Stream Starvation — NVR/Camera Connection Limits

**What goes wrong:**
Dahua cameras and NVRs have a practical limit on simultaneous RTSP connections. The existing NVR at .18 already serves the people tracker at :8095 and potentially the NVR's own recording. Adding 6+ new RTSP consumers (live view, recording, face detection) exhausts connections. Documented Dahua RTSP dropouts occur after 60-90 minutes of sustained multi-stream access, and pipeline create/destroy cycles cause buffer overflow crashes after ~150 reconnection cycles.

**Why it happens:**
Developers treat RTSP like HTTP -- open a new connection per consumer. Each camera has finite decode/encode bandwidth for simultaneous sub-streams. The NVR compounds this by proxying streams, adding its own connection overhead.

**How to avoid:**
- Pull each camera's RTSP stream ONCE into a local relay (go2rtc or mediamtx) running on James (.27). All consumers read from the relay, not the camera directly.
- Use sub-stream (subtype=1, lower resolution) for face detection -- main stream only for recording.
- Implement exponential backoff reconnection with jitter, not fixed-interval retry.
- Monitor connection count per camera -- alert if approaching limits.

**Warning signs:**
- Intermittent black frames or frozen video on dashboard after 1-2 hours uptime.
- People tracker at :8095 starts missing detections (its stream got dropped).
- Log entries showing repeated RTSP TEARDOWN/SETUP cycles.

**Phase to address:**
Phase 1 (Camera Infrastructure) -- the RTSP relay must be the very first thing built. Everything else depends on reliable stream access.

---

### Pitfall 2: Face Recognition Cost Explosion from Continuous Frame Analysis

**What goes wrong:**
Sending every frame (or even every Nth frame) from 3 entrance/reception cameras to a cloud face API generates thousands of API calls per hour. At AWS Rekognition's ~$1/1000 calls, a naive implementation analyzing 1 frame/second from 3 cameras = 10,800 calls/hour = ~$260/day = ~$7,800/month. This bankrupts a small eSports cafe.

**Why it happens:**
Developers build the "happy path" first -- detect face, call API, get result. They don't model the steady-state cost of continuous surveillance. Demo with 1 camera looks fine; production with 3 cameras 12 hours/day is 100x the cost.

**How to avoid:**
- Gate cloud API calls behind local pre-filtering: (1) motion detection, then (2) person detection (existing YOLOv8 at :8095 or local face detection via OpenCV Haar/DNN), then (3) face quality check (size > 200x200px, not blurred, frontal), THEN (4) cloud API call.
- Implement a "cooldown" per tracked person -- once recognized, skip re-recognition for N minutes using a local tracking ID (bounding box tracker like SORT/DeepSORT).
- Cache face embeddings locally. Only call cloud API for genuinely unknown faces or periodic re-verification.
- Set a hard daily API call budget with circuit breaker. Alert at 80% of budget.

**Warning signs:**
- Cloud billing alerts within first week of deployment.
- API rate limit errors (429s) during peak hours.
- Response latency spikes as you approach TPS limits.

**Phase to address:**
Phase 2 (Face Recognition Pipeline) -- the pre-filtering pipeline must be designed before any cloud API integration. Never start with "send everything to cloud, optimize later."

---

### Pitfall 3: Duplicate Attendance Entries from Multi-Camera Overlap

**What goes wrong:**
Person walks from entrance (.8) past reception (.15, .154). All 3 cameras detect and recognize them within seconds. System logs 3 separate "arrival" events. Over a day, attendance data becomes unusable -- every customer shows 2-3 check-ins per visit. Staff clock-in/clock-out becomes unreliable.

**Why it happens:**
Each camera pipeline runs independently. Without cross-camera deduplication, the same person generates multiple recognition events. This is especially bad at Racing Point where entrance and reception cameras likely have overlapping coverage of the same corridor/doorway.

**How to avoid:**
- Implement a recognition deduplication window: after recognizing person X on any camera, suppress duplicate recognition events for that person for 5-10 minutes across ALL cameras.
- Use a centralized recognition event queue (not per-camera). Events go through a dedup filter before becoming attendance records.
- For staff clock-in/clock-out, use state machine: ABSENT -> PRESENT (first recognition = clock-in), PRESENT -> ABSENT (no recognition for N hours or explicit exit detection = clock-out). Ignore all intermediate recognitions.
- Assign a "primary" camera for attendance (entrance .8) and use reception cameras only as fallback/confirmation.

**Warning signs:**
- Attendance logs show the same person arriving 2-3 times within minutes.
- Staff show multiple clock-in events per day without clock-outs.
- Dashboard "currently present" count is higher than physical headcount.

**Phase to address:**
Phase 3 (Attendance Logic) -- must be designed into the attendance engine from day one, not patched onto per-camera results.

---

### Pitfall 4: Face Enrollment Garbage In, Garbage Out

**What goes wrong:**
Hybrid enrollment (auto-detect new face, staff confirms/names) sounds elegant but produces terrible face galleries. Staff enrolls a blurry side-profile captured from 3 meters away. Or enrolls the same person twice under different names. Or confirms a face crop that includes two people. Recognition accuracy degrades to the point where the system is less reliable than a sign-in sheet.

**Why it happens:**
Enrollment quality is invisible -- a bad enrollment "works" until it causes a false match or miss weeks later. Staff have no training in what makes a good face photo. Auto-captured frames from surveillance cameras are inherently lower quality than dedicated enrollment photos (distance, angle, lighting, motion blur).

**How to avoid:**
- Enforce enrollment quality gates: minimum face size (200x200px per AWS/Azure recommendations), frontal pose (yaw < 30 degrees, pitch < 15 degrees), adequate brightness, no blur (Laplacian variance check), single face in crop.
- Require 3-5 quality frames from different moments (not consecutive frames from same second) for enrollment. Store best 3 as gallery.
- Before confirming a new enrollment, run it against existing gallery to check for duplicates ("This looks like existing person X -- merge or create new?").
- Provide a simple enrollment UI that shows quality score and explains why an image was rejected.
- Allow re-enrollment: periodically prompt to update gallery with fresh high-quality captures.

**Warning signs:**
- Recognition confidence scores trending downward over weeks.
- Increasing "unknown person" detections for people who should be enrolled.
- Staff complaining "it never recognizes me" -- check their enrollment photos.

**Phase to address:**
Phase 2 (Face Recognition Pipeline) for quality gates; Phase 3 (Attendance Logic) for the enrollment UI and duplicate checking workflow.

---

### Pitfall 5: Storage Disk Full Kills Everything

**What goes wrong:**
6 cameras at 4MP with H.265 at 15fps generate roughly 7-8 GB per camera per day (sub-stream) or 30-40 GB per camera per day (main stream). For 6 cameras on main stream, that is 180-240 GB/day, or 5.4-7.2 TB for 30 days. If recording all 13 cameras, double it. James's disk fills silently, recording stops, face recognition frames stop being processed, and the entire service crashes with disk I/O errors.

**Why it happens:**
Storage math is done once at planning time, then never revisited. Actual bitrates vary with scene complexity (busy cafe = higher bitrate than empty room). Nobody monitors disk usage in production. The 30-day retention policy requires active garbage collection, which is easy to get wrong (off-by-one = keeps 31 days, slowly accumulates).

**How to avoid:**
- Do actual storage math before deployment. Record 1 camera for 24 hours, measure actual file size, multiply by camera count and retention days. Add 30% buffer.
- Record sub-stream (subtype=1) for continuous recording. Only record main stream on motion/event triggers.
- Implement daily retention cleanup as a cron job with verification (check that old files were actually deleted). Alert if free disk drops below 15%.
- Use separate disk/partition for recordings so a full recording disk does not crash the OS or the face recognition service.
- Monitor: disk usage, oldest recording age, newest recording age, recording gap detection.

**Warning signs:**
- Disk usage growing faster than calculated.
- Gaps in recording timeline (cleanup ran but also deleted too-new files, or recording silently stopped).
- System slowdown or OOM errors (OS using swap because disk is full).

**Phase to address:**
Phase 1 (Camera Infrastructure) for storage architecture and recording pipeline. Phase 4 (Operations) for monitoring and retention automation.

---

### Pitfall 6: Indian DPDP Act Non-Compliance with Face Biometrics

**What goes wrong:**
Face recognition data is classified as sensitive personal data under India's Digital Personal Data Protection Act 2023. Collecting and storing face embeddings without proper consent, purpose limitation, and data retention policies violates the Act. Penalties reach up to 250 crore INR for inadequate security safeguards and 200 crore INR for failure to report breaches.

**Why it happens:**
Small businesses treat face recognition as a technical feature, not a legal obligation. "It's our own cafe, we can record whoever we want" is a common misconception. The DPDP Act requires specific consent (not bundled), clear purpose disclosure, and deletion when purpose is fulfilled.

**How to avoid:**
- Display clear signage at entrance: "This premises uses facial recognition for attendance and security. By entering, you consent to facial data processing." (Simplified consent for physical premises.)
- For staff: obtain explicit written consent as part of employment agreement. Keep consent separate from other terms (DPDP requires unbundled consent).
- Store face embeddings (mathematical representations) only -- never store raw face photos in cloud or in any location accessible outside the local network.
- Implement data deletion: when a customer has not visited for N months, purge their face data. When staff leaves, delete their biometric data within 30 days.
- Document your data processing purposes, retention periods, and security measures. This is your "reasonable security practices" defense.
- Never share face data with third parties (cloud API calls should send images for matching but not store them in the cloud provider's systems -- verify API provider's data retention policies).

**Warning signs:**
- No consent mechanism exists at go-live.
- Face images stored in cloud storage (S3/Azure Blob) alongside embeddings.
- No data deletion process for former staff or inactive customers.
- Cloud API provider's ToS allows them to retain/train on submitted images.

**Phase to address:**
Phase 1 (Camera Infrastructure) for signage and consent framework design. Phase 3 (Attendance Logic) for data lifecycle and deletion policies. Must be verified before any face data is collected.

---

### Pitfall 7: Lighting and Environmental Variance at Entrance

**What goes wrong:**
The entrance camera (.8) faces the door. During daytime, strong backlight from outside sun washes out faces entering the building -- they appear as dark silhouettes. At night, low ambient light produces noisy, grainy captures. Recognition accuracy swings wildly between 95% (indoor, good lighting) and below 50% (backlit entrance), making the system unreliable at the exact point where it matters most.

**Why it happens:**
Developers test with good indoor lighting. Real entrance cameras deal with dynamic range extremes -- bright outdoor light behind subjects walking in. Dahua 4MP cameras have decent WDR (Wide Dynamic Range) but the sub-stream used for bandwidth savings may not preserve WDR quality.

**How to avoid:**
- Test recognition accuracy specifically at the entrance camera under all lighting conditions: morning sun, afternoon sun, overcast, night, with door open vs closed.
- Use the main stream (not sub-stream) specifically for the entrance camera's face detection frames, since WDR detail matters here.
- Set camera's WDR/BLC (Back Light Compensation) mode aggressively for the entrance camera.
- Consider face detection only from the reception cameras (.15, .154) where lighting is controlled, using entrance camera only for person detection (trigger).
- Implement confidence thresholds: if recognition confidence is below X%, log as "low confidence match" and do not auto-confirm attendance. Queue for staff review.

**Warning signs:**
- Recognition success rate drops at specific times of day (correlates with sun position).
- High "unknown person" rate at entrance camera but not reception cameras.
- Face quality scores from entrance camera consistently lower than reception cameras.

**Phase to address:**
Phase 2 (Face Recognition Pipeline) -- must include real-world testing across lighting conditions before going live.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Skip RTSP relay, connect directly to cameras | Faster initial setup | Stream drops, camera overload, disrupts existing people tracker | Never -- relay is essential infrastructure |
| Store raw face images instead of embeddings only | Easier debugging, re-enrollment | Privacy liability, storage bloat, DPDP violation | Never in production -- use only in development with synthetic/consented test data |
| Single-threaded frame processing | Simpler code | Cannot keep up with multiple camera streams, frames dropped | Only during initial prototyping of a single camera |
| No face quality filtering before API call | Higher recall (catches more faces) | 10x cloud API cost, lower precision from bad enrollments | Never -- quality gate is mandatory |
| Hardcoded camera IPs | Faster development | Camera IP changes break everything, no hot-add | Only in Phase 1 prototype; must be configurable by Phase 2 |
| No attendance dedup window | Simpler logic | Duplicate entries corrupt attendance data | Never -- dedup must be in v1 |

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| AWS Rekognition / Azure Face API | Sending full-resolution 4MP images (4-8MB each) | Resize to 1024px max dimension before API call. APIs work fine at 640px for face matching. Saves bandwidth and cost. |
| AWS Rekognition | Creating one monolithic collection for all faces | Separate collections: staff vs customers. Different confidence thresholds per use case. Staff needs higher accuracy. |
| Dahua RTSP | Using main stream URL (subtype=0) for all consumers | Use sub-stream (subtype=1) for live preview and face detection. Main stream only for recording and entrance camera face detection. |
| Existing people tracker (:8095) | Not coordinating with it -- running duplicate YOLO inference | Share the RTSP relay stream. Consider extending people tracker to emit face crops, avoiding duplicate person detection work. |
| Cloud API | Not handling API timeouts/errors gracefully | Cloud APIs have 5-15s occasional spikes. Use async calls with 3s timeout, retry once, then skip frame. Never block the video pipeline on API response. |
| NVR at .18 | Pulling RTSP from NVR instead of directly from cameras | NVR adds latency and is a single point of failure for all streams. Connect directly to camera IPs for the attendance cameras. Use NVR only for its own recording/playback. |

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Decoding all camera frames on CPU | James CPU at 100%, frame drops, face detection lag | Use hardware-accelerated decode (NVDEC on RTX 4070 via ffmpeg/GStreamer). Only decode frames needed for face detection. | Immediately with 3+ cameras |
| Unbounded face detection queue | Memory grows, latency increases, eventual OOM | Fixed-size queue with drop-oldest policy. If face detection cannot keep up, drop frames rather than buffering indefinitely. | Within hours under sustained load |
| Synchronous cloud API calls in frame pipeline | Video freezes while waiting for API response | Async pipeline: frame capture -> local face detect -> async cloud API call -> result callback. Video display never waits for API. | First API latency spike (>1s) |
| Recording to same disk as OS/database | Disk I/O contention, database queries slow, recording gaps | Separate physical disk or partition for video recordings. NVMe for OS/DB, HDD array or large SSD for recordings. | When recording 3+ cameras continuously |
| Loading all 30 days of recordings into memory for playback | OOM, slow startup | Stream from disk on demand. Index files by timestamp for fast seek. Never load entire recording into memory. | Immediately with any meaningful retention period |

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Exposing RTSP relay on network without auth | Anyone on LAN can view all camera feeds, including customers' faces | RTSP relay must require authentication. Bind to localhost if only local consumers. |
| Storing face embeddings in plain database without encryption | Data breach exposes biometric data that cannot be changed (unlike passwords) | Encrypt face embeddings at rest. Use a separate encryption key from the main database key. |
| Cloud API credentials in config file on James | Credential theft gives attacker access to face recognition API and billing | Use environment variables or OS keyring. Never commit credentials. Rotate keys quarterly. |
| Camera credentials (admin/Admin@123) never changed | Default credentials = any LAN user can access/modify cameras | Change camera passwords. Even on an internal network, default credentials are unacceptable for a system handling biometric data. |
| No audit log for face data access | Cannot prove compliance with DPDP Act, cannot detect unauthorized access | Log every face enrollment, deletion, recognition event, and gallery access with timestamp and actor. |

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Silent recognition failures -- person walks in, nothing happens | Staff and customers lose trust in the system, revert to manual check-in | Show a "Welcome [Name]" on a reception display. If recognition fails, show "Please check in at reception." Visible feedback for every entry attempt. |
| Enrollment requires dedicated photo session | Staff resist enrollment ("too much hassle"), coverage stays low | Auto-enroll from surveillance frames with quality gate. Staff only needs to confirm name, not pose for photos. |
| Attendance dashboard shows raw timestamps without context | Uday has to mentally calculate hours worked, visit duration | Show derived data: hours worked today, average visit duration, "currently present" list with time-in. |
| Alert fatigue from too many notifications | Uday ignores all alerts, misses genuine security events | Categorize alerts by severity. Only push critical alerts (unknown person after hours, camera offline). Batch routine summaries (daily attendance report). |
| No fallback when system is down | Nobody can check in, attendance data has gaps | Manual check-in option always available. System should degrade gracefully, not fail completely. |

## "Looks Done But Isn't" Checklist

- [ ] **Face recognition works:** Often missing handling of partial occlusion (masks, sunglasses, hats) -- verify recognition accuracy with common accessories worn by customers.
- [ ] **Attendance logging works:** Often missing clock-OUT detection -- verify staff departure is actually tracked (re-recognition stops != reliable clock-out).
- [ ] **Recording is continuous:** Often missing gap detection -- verify there are no silent gaps in recordings by checking file timestamps span 24 hours without holes.
- [ ] **Cloud API integration works:** Often missing error budget/circuit breaker -- verify system continues functioning when cloud API is unreachable for 5+ minutes.
- [ ] **Storage cleanup works:** Often missing edge case where cleanup deletes currently-being-written file -- verify cleanup and recording do not race on the same files.
- [ ] **Multi-camera dedup works:** Often missing time synchronization between camera pipelines -- verify all camera processing pipelines use the same clock source for dedup windows.
- [ ] **Notifications work:** Often missing rate limiting on alerts -- verify that 10 motion events in 1 minute produce 1 summary alert, not 10 individual pushes.
- [ ] **Dashboard shows live feeds:** Often missing reconnection after network blip -- verify dashboard auto-recovers camera feeds without manual page refresh.

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| RTSP stream starvation | LOW | Deploy RTSP relay, reconfigure all consumers to use relay. No data loss. |
| Cloud API cost explosion | MEDIUM | Implement pre-filtering retroactively. May need to eat 1-2 months of inflated bills. Add budget alerts immediately. |
| Duplicate attendance entries | MEDIUM | Write migration script to deduplicate historical data. Deploy dedup logic. Manually verify corrected data with staff. |
| Bad face enrollments | HIGH | Must re-enroll all affected people. Cannot retroactively fix past recognition results. May need to wipe gallery and start fresh. |
| Disk full from recordings | LOW | Free space immediately (delete oldest). Add monitoring. No face data lost if embeddings are in database, not on recording disk. |
| DPDP non-compliance discovered | HIGH | Retroactive consent collection is legally questionable. May need to purge all collected data and restart with proper consent framework. Legal consultation needed. |
| Entrance lighting issues | LOW | Adjust camera settings, add physical lighting, or switch primary recognition to reception cameras. Software change + potentially minor hardware (LED light). |

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| RTSP stream starvation | Phase 1: Camera Infrastructure | All consumers use relay; no direct camera RTSP connections; people tracker at :8095 unaffected |
| Cloud API cost explosion | Phase 2: Face Recognition Pipeline | Daily API call count < budget; pre-filter rejection rate > 90% of frames |
| Duplicate attendance entries | Phase 3: Attendance Logic | Same person recognized on 3 cameras within 1 minute produces exactly 1 attendance entry |
| Face enrollment quality | Phase 2 + Phase 3 | Enrollment UI shows quality score; rejects sub-200px faces; duplicate check runs before confirming new person |
| Disk full from recordings | Phase 1 + Phase 4: Operations | Disk usage monitoring exists; retention cleanup verified over 30+ day period; recording disk is separate from OS |
| DPDP compliance | Phase 1 (consent) + Phase 3 (data lifecycle) | Signage deployed; consent documented; deletion policy implemented and tested; no raw face images in cloud |
| Entrance lighting variance | Phase 2: Face Recognition Pipeline | Recognition accuracy tested at entrance camera across morning/afternoon/evening/night; confidence threshold enforced |

## Sources

- [AWS Rekognition image recommendations](https://docs.aws.amazon.com/rekognition/latest/dg/recommendations-facial-input-images.html) -- enrollment best practices, minimum face size
- [AWS Rekognition pricing](https://aws.amazon.com/rekognition/pricing/) -- cost modeling ($1/1000 API calls)
- [AWS Rekognition limits and quotas](https://docs.aws.amazon.com/rekognition/latest/dg/limits.html) -- TPS limits, collection sizes
- [Microsoft Face API characteristics and limitations](https://learn.microsoft.com/en-us/legal/cognitive-services/face/characteristics-and-limitations) -- head angle limits, image quality requirements
- [Dahua RTSP dropout reports](https://github.com/koush/scrypted/discussions/1591) -- connection drops after 60-90 minutes
- [go2rtc buffer overflow with RTSP pipelines](https://github.com/AlexxIT/go2rtc/issues/383) -- pipeline lifecycle issues
- [mediamtx RTSP buffer tuning](https://github.com/bluenviron/mediamtx/discussions/697) -- relay configuration
- [Montavue storage chart for 4MP cameras](https://montavue.com/blogs/news/storage-chart-for-2mp-1080p-4mp-2k-and-8mp-4k-ip-security-cameras) -- 7.5TB for 8 cameras x 7 days at 4MP/H.265
- [DPDP Act biometric data regulation](https://ksandk.com/data-protection-and-data-privacy/regulation-of-biometric-data-under-the-dpdp-act/) -- consent requirements, penalties
- [Employee biometrics in India legal safeguards](https://ksandk.com/labour-employment/employee-biometrics-india-safeguards/) -- employer obligations
- [Multi-camera duplicate detection research](https://link.springer.com/article/10.1007/s00521-025-11716-2) -- overlapping field of view dedup strategies
- [Stanford facial recognition challenges](https://hai.stanford.edu/news/challenges-facial-recognition-technologies) -- environmental variance, equity concerns
- [Schneier on Security: Failures in Face Recognition](https://www.schneier.com/blog/archives/2025/10/failures-in-face-recognition.html) -- real-world failure modes
- [Frigate face recognition docs](https://docs.frigate.video/configuration/face_recognition/) -- practical enrollment quality guidance

---
*Pitfalls research for: Security Camera AI and Face Recognition Attendance at Racing Point eSports*
*Researched: 2026-03-21*
