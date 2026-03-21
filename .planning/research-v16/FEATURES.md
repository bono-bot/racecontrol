# Feature Research

**Domain:** Face Recognition Attendance & Security Camera Monitoring (Gaming Cafe / eSports Venue)
**Researched:** 2026-03-21
**Confidence:** HIGH

## Feature Landscape

### Table Stakes (Users Expect These)

Features that Uday and staff assume exist. Missing these = the system feels broken or incomplete.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Live camera feed in dashboard | Core purpose of a monitoring system; if you can't see cameras live, it's not a monitoring system | HIGH | RTSP cannot play in browsers natively. Must transcode to HLS or use WebRTC. FFmpeg sidecar per stream. 13 cameras = significant CPU. Consider showing 3-4 key cameras live, others on-demand. |
| Face detection on entrance cameras | Foundation for everything else. Without detecting faces in frames, no recognition can happen | MEDIUM | YOLOv8 people tracker already runs at :8095. Can reuse its detections or add a face-specific detection stage. Only entrance (.8) and reception (.15, .154) cameras need face detection. |
| Face recognition (known person ID) | The entire value proposition. Person walks in, system says "that's Rahul" | MEDIUM | Cloud API call (AWS Rekognition or similar) with face embedding comparison. Latency target: 2-3 seconds. Accuracy depends heavily on enrollment image quality and lighting at entrance. |
| Face enrollment (register new people) | Cannot recognize anyone without first enrolling them. Chicken-and-egg problem | MEDIUM | Hybrid workflow: system auto-detects unknown face, creates a pending entry with snapshot, staff names/confirms via dashboard. Need 3-5 good quality images per person for reliable matching. |
| Attendance logging (entry timestamp) | Primary business output. Customer walked in at 14:32 — that's the record | LOW | Simple DB insert once recognition confirms identity. Timestamp + person_id + camera_id + confidence_score. |
| Attendance dashboard (who is here now) | Uday's #1 use case: glance at phone, see who's in the venue right now | LOW | Real-time list of recognized people with entry times. Requires exit detection or timeout-based "departed" logic. |
| Visit history per person | "How often does Rahul come?" is a natural follow-up to knowing he's here | LOW | Query attendance logs grouped by person. Simple UI: person profile with visit list. |
| Staff clock-in / clock-out | Staff attendance is explicitly required. Distinct from customer visits — needs shift tracking | MEDIUM | Requires exit detection (harder than entry). Options: second recognition at exit, manual clock-out button, or inactivity timeout. Face-only auth (no PIN) per project requirements. |
| Motion detection alerts | Basic security feature. "Something moved after hours" is the minimum viable alert | LOW | Dahua cameras have built-in motion detection via ONVIF events. Subscribe to camera events rather than doing CV-based motion detection. Much simpler. |
| Recording with playback | "Show me what happened at 2am" is fundamental to any security system | HIGH | NVR already records continuously. For dashboard playback: either proxy NVR recordings (Dahua API) or run parallel recording via FFmpeg. 30-day retention at 4MP across 6+ cameras = ~6-12TB depending on codec/bitrate. |
| Multi-channel alerts (dashboard + Telegram) | Uday explicitly wants notifications everywhere, especially on phone | MEDIUM | Telegram Bot API is straightforward — send snapshot + message on events. Dashboard notifications via WebSocket (already have WS infrastructure in racecontrol). Start with Telegram + dashboard; add others later. |

### Differentiators (Competitive Advantage)

Features that make this system uniquely valuable for Racing Point vs buying an off-the-shelf VMS.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Auto-detect unknown faces with staff confirmation workflow | Most systems require manual enrollment one-by-one. This system spots new faces automatically and queues them for staff to name — building the face database passively over time | MEDIUM | Cluster unknown faces (same person appearing multiple times gets grouped). Staff sees "Unknown Person #7 — seen 3 times this week" and can name them. Dramatically reduces enrollment friction. |
| Customer visit frequency analytics | "Rahul has come 12 times this month, average stay 2.5 hours" — no off-the-shelf camera system provides this business intelligence tied to individual customers | LOW | Straightforward aggregation once attendance data exists. High value for Uday to understand customer patterns, identify regulars, spot churn. |
| Integration with racecontrol billing data | Linking "who is in the building" with "who has an active gaming session" is unique to Racing Point. No commercial product does this | MEDIUM | Cross-reference attendance with active billing sessions. Enables: "Rahul is here but hasn't started a session" alerts, automatic session attribution, walk-in tracking. |
| Stranger/VIP alert system | Instant notification when an unknown person enters (security) or when a VIP/regular arrives (hospitality). Configurable per-person alert rules | LOW | Tag people as VIP/staff/regular/watchlist during enrollment. Different alert channels per category. "Unknown person at entrance" goes to Telegram immediately. |
| Timeline scrubbing with event markers | Playback timeline shows face recognition events, motion events, and alerts as markers. Jump to "when Rahul arrived" instead of scrubbing through hours of footage | HIGH | Requires correlating recording timestamps with event database. Complex UI component but extremely useful for incident review. |
| Desktop notification on James machine | James (.27) is always on. Desktop toast notifications for security events provide immediate awareness without checking phone | LOW | Windows toast notifications via PowerShell or native Rust (winrt crate). Trivial to implement since James is the processing hub. |

### Anti-Features (Commonly Requested, Often Problematic)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Face recognition on all 13 cameras | "More coverage = better" | Massive CPU/API cost (13 streams of face detection + cloud API calls). Most cameras point at gaming pods where identity is irrelevant. Entrance/reception cameras already capture everyone entering. | Run face recognition only on entrance (.8) and reception (.15, .154) cameras. 3 cameras covers all entry points. |
| On-premise face recognition model (RTX 4070) | "No cloud dependency, lower latency, free" | Model maintenance burden, accuracy lower than cloud services, GPU already used for other tasks, model updates require manual intervention. InsightFace/ArcFace are good but require ML expertise to tune. | Use cloud API (AWS Rekognition at $0.10/face). At ~100-200 recognitions/day, cost is negligible (~$6-12/month). No maintenance. |
| Real-time face recognition on every frame | "Instant recognition" | Processing every frame at 25fps = 25 API calls/second per camera = cost explosion and API throttling. Recognition doesn't need to be instant — 2-3 second delay is imperceptible for attendance. | Sample frames: detect face, track it for 1-2 seconds to get best quality frame, then send one API call. One recognition per person per entry, not per frame. |
| Biometric PIN as backup auth | "What if face recognition fails?" | Adds hardware (PIN pad), complicates UX, defeats the "hands-free" value proposition. Face recognition failures are rare with good enrollment. | Manual override in dashboard: staff can mark attendance manually when recognition fails. Log it as "manual entry" for audit trail. |
| WhatsApp Business API alerts | "Everyone uses WhatsApp in India" | WhatsApp Business API requires Facebook business verification, costs per message (~$0.05/message), has strict template approval process, and 24-hour messaging window rules. Significant setup overhead for marginal benefit over Telegram. | Start with Telegram (free, instant, bot API is simple). Add WhatsApp later only if Uday specifically needs it. Most Indian tech users have both. |
| Emotion/sentiment detection | "Know if customers are happy" | Unreliable (academic accuracy ~60-70%), ethically questionable, actionable insights are minimal. "Customer looked angry" doesn't tell you why. | Track visit frequency and duration instead — actual behavioral signals of satisfaction. |
| Age/gender demographic analytics | "Understand our customer base" | Privacy-invasive, accuracy varies by demographic (bias issues), limited actionable value for a single-location gaming cafe. | Ask customers during enrollment or use billing data demographics. |
| Auto-start gaming sessions on face detection | "Walk in, session starts" | Requires billing integration changes (out of scope), creates accidental billing if someone walks past entrance, no consent mechanism. Edge cases are dangerous. | Defer to future milestone. Requires explicit customer opt-in and billing system changes. |
| Continuous cloud backup of all footage | "Never lose anything" | 13 cameras at 4MP = enormous bandwidth and cloud storage costs. 30 days of footage could be 6-12TB. Cloud upload at typical Indian broadband speeds (50-100 Mbps up) would saturate the connection. | Local 30-day retention on James's storage. Cloud backup only for flagged events/clips, not continuous footage. Use NVR as primary recorder. |

## Feature Dependencies

```
[Camera Feed Streaming (RTSP->HLS)]
    |
    +--enables--> [Live Dashboard View]
    +--enables--> [Recording & Playback]
    |                 +--enhances--> [Timeline Scrubbing with Event Markers]
    +--enables--> [Face Detection on Frames]
                      |
                      +--requires--> [Face Recognition (Cloud API)]
                      |                   |
                      |                   +--requires--> [Face Enrollment Database]
                      |                   |                   |
                      |                   |                   +--enables--> [Unknown Face Auto-Detection]
                      |                   |
                      |                   +--enables--> [Attendance Logging]
                      |                   |                 |
                      |                   |                 +--enables--> [Attendance Dashboard]
                      |                   |                 +--enables--> [Visit History]
                      |                   |                 +--enables--> [Staff Clock-in/out]
                      |                   |                 +--enables--> [Customer Frequency Analytics]
                      |                   |
                      |                   +--enables--> [Stranger/VIP Alerts]
                      |
                      +--enables--> [Motion Detection Alerts]

[Multi-Channel Alerts (Telegram + Dashboard)]
    +--consumed-by--> [Stranger/VIP Alerts]
    +--consumed-by--> [Motion Detection Alerts]
    +--consumed-by--> [Staff Clock-in/out Notifications]

[Racecontrol Integration]
    +--requires--> [Attendance Logging]
    +--requires--> [Existing racecontrol API]
```

### Dependency Notes

- **Face Detection requires Camera Feed Streaming:** Must extract frames from RTSP streams before any face processing can happen.
- **Face Recognition requires Face Enrollment Database:** Cannot identify anyone without a database of known face embeddings to compare against.
- **Attendance Logging requires Face Recognition:** Attendance is automatically recorded when a known face is recognized — this is the core pipeline.
- **All Alerts require Multi-Channel Alert Infrastructure:** Build the alert routing system (Telegram bot, WebSocket notifications) once, then all alert types plug into it.
- **Timeline Scrubbing requires both Recording and Attendance Logging:** Events from the attendance/alert system are overlaid on recorded footage — both must exist.
- **Racecontrol Integration requires Attendance Logging:** Cannot cross-reference with billing until attendance data exists.

## MVP Definition

### Launch With (v1)

Minimum viable: face recognition attendance works end-to-end on entrance cameras.

- [ ] Camera feed extraction (RTSP frame grabbing from entrance/reception cameras) -- foundation for all processing
- [ ] Face detection on extracted frames -- identify that a face exists in the frame
- [ ] Face enrollment system (manual upload + auto-capture from camera) -- seed the face database with staff and known regulars
- [ ] Face recognition via cloud API (AWS Rekognition) -- match detected faces against enrolled database
- [ ] Attendance logging (entry timestamp per recognized person) -- the core business output
- [ ] Attendance dashboard (who is here, recent arrivals) -- Uday's primary interface
- [ ] Unknown face detection with "pending review" queue -- passive database building
- [ ] Telegram bot alerts for unknown persons and staff arrivals -- immediate mobile notification
- [ ] Dashboard notifications via WebSocket -- in-app awareness

### Add After Validation (v1.x)

Features to add once core recognition pipeline is stable and accurate.

- [ ] Live camera feed viewing in dashboard (HLS/WebRTC) -- once frame extraction is proven stable, add full live view
- [ ] Staff clock-in/clock-out with shift tracking -- once recognition accuracy is validated on staff faces
- [ ] Visit history and frequency analytics -- once enough attendance data has accumulated (2+ weeks)
- [ ] Customer/staff profile management UI -- once enrollment workflow is proven
- [ ] VIP/watchlist tagging with per-category alert rules -- once base alerts are working
- [ ] Recording playback via NVR API proxy -- once dashboard is stable

### Future Consideration (v2+)

Features to defer until the system is running reliably for 1+ months.

- [ ] Timeline scrubbing with event markers -- complex UI, requires stable event data
- [ ] Racecontrol billing integration -- requires billing system changes, separate milestone
- [ ] Multi-camera live grid view (4-9 cameras simultaneously) -- heavy on bandwidth and CPU
- [ ] Cloud backup of flagged event clips -- needs storage cost analysis
- [ ] WhatsApp Business API alerts -- only if Telegram proves insufficient
- [ ] Desktop toast notifications on James -- nice-to-have, low priority vs mobile alerts
- [ ] Mobile push notifications (PWA) -- requires service worker setup

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Face recognition on entrance cameras | HIGH | MEDIUM | P1 |
| Attendance logging | HIGH | LOW | P1 |
| Attendance dashboard | HIGH | LOW | P1 |
| Face enrollment (hybrid auto+manual) | HIGH | MEDIUM | P1 |
| Unknown face auto-detection | HIGH | MEDIUM | P1 |
| Telegram alerts | HIGH | LOW | P1 |
| Camera frame extraction (RTSP) | HIGH | MEDIUM | P1 |
| Dashboard WebSocket notifications | MEDIUM | LOW | P1 |
| Staff clock-in/clock-out | HIGH | MEDIUM | P2 |
| Live camera feed in dashboard | MEDIUM | HIGH | P2 |
| Visit history & analytics | MEDIUM | LOW | P2 |
| Profile management UI | MEDIUM | MEDIUM | P2 |
| VIP/watchlist alerts | MEDIUM | LOW | P2 |
| Recording playback (NVR proxy) | MEDIUM | HIGH | P2 |
| Timeline event markers | LOW | HIGH | P3 |
| Racecontrol billing integration | MEDIUM | HIGH | P3 |
| Multi-camera grid view | LOW | HIGH | P3 |
| Cloud backup of clips | LOW | MEDIUM | P3 |

**Priority key:**
- P1: Must have for launch -- the recognition-to-attendance pipeline
- P2: Should have, add once pipeline is stable
- P3: Nice to have, future consideration

## Competitor Feature Analysis

| Feature | Off-the-shelf VMS (Dahua SmartPSS) | Cloud VMS (Verkada/Rhombus) | Our Approach |
|---------|-------------------------------------|-----------------------------|----|
| Face recognition | Built into AI cameras only (not all 13 are AI models) | Cloud-based, managed | Cloud API on entrance cameras only -- targeted and cost-effective |
| Attendance tracking | Not a feature -- security focus only | Some offer people counting, not named attendance | Core feature -- named individuals with timestamps |
| Customer profiles | Not applicable | Not applicable | Built-in -- visit history, frequency, preferences |
| Enrollment | Manual per-camera face database | Managed enrollment portal | Hybrid: auto-detect + staff confirm -- lowest friction |
| Alerts | Email/push from NVR app | Multi-channel (email, SMS, app) | Telegram + dashboard WebSocket -- fits Uday's workflow |
| Integration with business systems | None | API available but generic | Deep integration with racecontrol billing and sessions |
| Cost | Free (already own hardware) | $15-30/camera/month ($195-390/mo for 13 cameras) | Cloud API usage only (~$6-12/month for face recognition) |
| Recording | NVR handles it (already working) | Cloud storage ($$$) | Leverage existing NVR, proxy playback through dashboard |

## Sources

- [7 Best Face Recognition Attendance Systems 2026 - Timeero](https://timeero.com/post/best-face-recognition-attendance-system)
- [Best Face Recognition Attendance Systems 2025 - Workyard](https://www.workyard.com/compare/face-recognition-attendance-system)
- [Top 5 Facial Recognition Challenges - AIMultiple](https://research.aimultiple.com/facial-recognition-challenges/)
- [Microsoft Face API Enrollment Best Practices](https://learn.microsoft.com/en-us/azure/ai-services/computer-vision/enrollment-overview)
- [Frigate Face Recognition Docs](https://docs.frigate.video/configuration/face_recognition/)
- [Dahua Face Detection Setup](https://dahuawiki.com/Face_Detection)
- [RTSP to Browser Streaming - Dev.to](https://dev.to/foyzulkarim/how-to-stream-your-ip-camera-into-browser-using-ffmpeg-node-and-react-162k)
- [IP Camera RTSP to WebRTC - Red5](https://www.red5.net/blog/ip-camera-live-streaming-rtsp-to-webrtc/)
- [AWS Rekognition Documentation](https://docs.aws.amazon.com/rekognition/latest/dg/what-is.html)
- [Compare Rekognition vs Azure Face API - G2](https://www.g2.com/compare/amazon-rekognition-vs-azure-face-api)
- [Enterprise NVR Systems Guide 2025 - Spot AI](https://www.spot.ai/blog/enterprise-nvr-security-systems-guide-2025)
- [Visitor Management with Facial Recognition - VisitUs](https://visit-us.com/facial-recognition-app/)
- [IP Camera Telegram Integration](https://explore.st-aug.edu/exp/ip-camera-telegram-revolutionizing-smart-surveillance-with-instant-secure-real-time-monitoring)
- [Handling Unknown Faces in Recognition - Medium](https://medium.com/winkl-insights/face-recognition-how-to-deal-with-people-that-were-not-part-of-training-data-36fab27faabc)

---
*Feature research for: Security Camera AI & Attendance (v16.0)*
*Researched: 2026-03-21*
