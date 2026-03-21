---
phase: 118-live-camera-feeds
verified: 2026-03-22T01:15:00+05:30
status: human_needed
score: 8/8 must-haves verified
re_verification: false
human_verification:
  - test: "Open http://192.168.31.23:3200/cameras while rc-sentry-ai is running on James (.27)"
    expected: "Live camera feeds visible and updating in real-time with under 2-second latency"
    why_human: "MJPEG stream rendering and latency cannot be verified without running services and browser"
  - test: "Check browser console for CORS or mixed-content errors"
    expected: "No errors -- CORS headers from rc-sentry-ai allow cross-origin img loading"
    why_human: "Cross-origin behavior requires live browser verification"
  - test: "Verify detection pipeline performance is not degraded while streaming"
    expected: "Face detection latency and accuracy remain unchanged with active MJPEG viewers"
    why_human: "Performance impact requires live load testing"
---

# Phase 118: Live Camera Feeds Verification Report

**Phase Goal:** Staff can view live camera feeds directly in the racecontrol dashboard
**Verified:** 2026-03-22T01:15:00+05:30
**Status:** human_needed
**Re-verification:** No -- initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | GET /api/v1/cameras/:name/stream returns multipart/x-mixed-replace MJPEG stream | VERIFIED | mjpeg.rs:194-203 sets Content-Type header and returns Body::from_stream |
| 2 | GET /api/v1/cameras returns JSON list of camera names with stream URLs | VERIFIED | mjpeg.rs:50-78 iterates cameras, builds CameraInfo with stream_url, returns Json |
| 3 | MJPEG stream sends JPEG frames from FrameBuffer at camera's configured FPS | VERIFIED | mjpeg.rs:105-106 caps FPS to min(camera.fps, 10), unfold loop at line 125 sleeps frame_interval between frames, H.264 decode + JPEG encode at lines 148-164 |
| 4 | Stream does not hold FrameBuffer write lock -- read-only access | VERIFIED | mjpeg.rs:129 uses frame_buf.get() only (read lock), no update() calls anywhere in file |
| 5 | CORS headers allow cross-origin access from dashboard at :3200 | VERIFIED | mjpeg.rs:37-40 CorsLayer with allow_origin(Any), allow_methods GET, allow_headers Any |
| 6 | Dashboard has a Cameras page showing live feeds from all configured cameras | VERIFIED | web/src/app/cameras/page.tsx (135 lines) fetches camera list, renders grid of cards with MJPEG img tags |
| 7 | Each camera feed renders as an img tag with MJPEG src -- no video player library | VERIFIED | page.tsx:122-127 raw img tag with src=SENTRY_BASE+stream_url, eslint-disable for no-img-element |
| 8 | Sidebar has Cameras nav link between AI Insights and Settings | VERIFIED | Sidebar.tsx:22 has { href: "/cameras", label: "Cameras", icon: "&#128247;" } between AI Insights (line 21) and Settings (line 23) |

**Score:** 8/8 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-sentry-ai/src/mjpeg.rs` | MJPEG streaming handler and camera list endpoint | VERIFIED | 217 lines, exports mjpeg_router, CorsLayer, H.264 decode + JPEG encode pipeline |
| `crates/rc-sentry-ai/src/main.rs` | Router merge for MJPEG routes | VERIFIED | mod mjpeg at line 8, MjpegState construction at lines 286-290, .merge(mjpeg::mjpeg_router) at line 297 |
| `web/src/app/cameras/page.tsx` | Live camera feeds page | VERIFIED | 135 lines, fetches camera list, renders grid, handles loading/error/empty/offline states |
| `web/src/components/Sidebar.tsx` | Updated nav with Cameras link | VERIFIED | /cameras entry at line 22 |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| mjpeg.rs | frame.rs | frame_buf.get() read-only | WIRED | Lines 55, 129 call frame_buf.get(&name) |
| main.rs | mjpeg.rs | router merge | WIRED | Line 297: .merge(mjpeg::mjpeg_router(mjpeg_state)) |
| cameras/page.tsx | rc-sentry-ai :8096 | img src pointing to MJPEG stream URL | WIRED | Line 6: SENTRY_BASE = "http://192.168.31.27:8096", line 124: src={SENTRY_BASE + stream_url} |
| Sidebar.tsx | cameras/page.tsx | nav href /cameras | WIRED | Line 22: { href: "/cameras" } |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| MNTR-01 | 118-01, 118-02 | Live camera feed viewing in dashboard (MJPEG proxy) | SATISFIED | Backend MJPEG endpoint in mjpeg.rs + frontend page.tsx renders feeds in img tags |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| -- | -- | None found | -- | -- |

No TODO/FIXME/placeholder comments, no empty implementations, no stub handlers found in any modified files.

### Human Verification Required

### 1. Live Camera Feed Rendering

**Test:** Open http://192.168.31.23:3200/cameras while rc-sentry-ai is running on James (.27) with cameras connected
**Expected:** Camera feeds are visible and continuously updating (not frozen stills), with under 2-second latency matching real-time activity
**Why human:** MJPEG stream rendering, frame decode quality, and latency require a running browser with active camera connections

### 2. CORS and Mixed Content

**Test:** Open browser dev console on /cameras page, check for errors
**Expected:** No CORS blocked requests, no mixed-content warnings
**Why human:** Cross-origin behavior between :3200 (dashboard) and :8096 (sentry-ai) requires live browser verification

### 3. Detection Pipeline Performance

**Test:** Monitor rc-sentry-ai logs and detection stats while multiple browser tabs view MJPEG streams
**Expected:** Face detection latency and accuracy remain unchanged -- per-connection H.264 decoders do not contend with pipeline
**Why human:** Performance impact requires live load testing with active AI pipeline

### Gaps Summary

No gaps found. All automated checks pass across both plans (backend MJPEG endpoint and frontend dashboard page). Three items flagged for human verification: live stream rendering quality/latency, CORS behavior, and detection pipeline performance impact. The human-verify checkpoint in plan 118-02 was marked as approved, suggesting these were already verified during execution.

---

_Verified: 2026-03-22T01:15:00+05:30_
_Verifier: Claude (gsd-verifier)_
