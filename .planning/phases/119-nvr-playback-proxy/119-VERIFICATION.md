---
phase: 119-nvr-playback-proxy
verified: 2026-03-22T14:30:00+05:30
status: gaps_found
score: 8/9 must-haves verified
gaps:
  - truth: "Attendance event markers appear on the playback timeline for quick navigation"
    status: failed
    reason: "Frontend/backend response format mismatch -- backend returns {day, events, count} object but frontend expects flat AttendanceEntry[] array"
    artifacts:
      - path: "web/src/app/cameras/playback/page.tsx"
        issue: "Line 112: `const data: AttendanceEntry[] = await res.json()` treats response as flat array, but backend playback.rs returns `json!({day, events, count})` wrapper object"
      - path: "crates/rc-sentry-ai/src/playback.rs"
        issue: "Line 146-150: events_handler returns nested object `{day, events, count}` instead of flat array"
    missing:
      - "Either change frontend to `const data = await res.json(); setEvents(data.events || [])` OR change backend to return flat array `Json(json!(entries))`"
---

# Phase 119: NVR Playback Proxy Verification Report

**Phase Goal:** Staff can review past footage from the Dahua NVR through the dashboard without accessing the NVR directly
**Verified:** 2026-03-22T14:30:00+05:30
**Status:** gaps_found
**Re-verification:** No -- initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | rc-sentry-ai can search Dahua NVR for recordings by channel, start time, and end time | VERIFIED | nvr.rs implements 4-step mediaFileFind CGI flow (factory, findFile, findNextFile, close) with full response parsing |
| 2 | rc-sentry-ai can stream a recorded file from the NVR as raw video bytes | VERIFIED | nvr.rs stream_file() uses RPC_Loadfile endpoint, returns raw reqwest::Response |
| 3 | NVR channel numbers are mapped from camera config | VERIFIED | config.rs CameraConfig has `nvr_channel: Option<u32>` with serde(default) |
| 4 | Dashboard can search NVR recordings by camera, date, and time range via HTTP API | VERIFIED | playback.rs GET /api/v1/playback/search with camera name lookup, nvr_channel validation, NvrClient.search_files call, JSON result |
| 5 | Dashboard can stream recorded footage through rc-sentry-ai proxy | VERIFIED | playback.rs GET /api/v1/playback/stream uses Body::from_stream for zero-copy proxying with Content-Type: video/mp4 |
| 6 | Attendance events for a time range are available alongside playback search results | VERIFIED | playback.rs GET /api/v1/playback/events queries SQLite via spawn_blocking + get_attendance_for_day |
| 7 | Staff can select a camera, date, and time range to search NVR recordings | VERIFIED | page.tsx has camera dropdown (fetched from /api/v1/cameras), date input, start/end time inputs, Search button with fetch to /api/v1/playback/search |
| 8 | Staff can play back a selected recording in the browser | VERIFIED | page.tsx uses HTML5 `<video>` element with src pointing at /api/v1/playback/stream?file_path=... proxy URL |
| 9 | Attendance event markers appear on the playback timeline for quick navigation | FAILED | Frontend expects flat AttendanceEntry[] from events endpoint but backend returns wrapped object {day, events, count} -- timeline markers will never render |

**Score:** 8/9 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-sentry-ai/src/nvr.rs` | Dahua NVR CGI API client | VERIFIED | 361 lines, NvrClient with digest auth, search_files (4-step), stream_file, response parsers |
| `crates/rc-sentry-ai/src/config.rs` | NvrConfig + nvr_channel | VERIFIED | NvrConfig struct with defaults (host .18, port 80, admin creds), nvr_channel on CameraConfig |
| `crates/rc-sentry-ai/src/playback.rs` | Axum handlers for search/stream/events | VERIFIED | 168 lines, PlaybackState, 3 GET endpoints, CORS, proper error handling |
| `web/src/app/cameras/playback/page.tsx` | Playback page with search, player, timeline | VERIFIED | 405 lines, search form, file list, video player, event timeline with markers |
| `web/src/components/Sidebar.tsx` | Playback navigation link | VERIFIED | Line 23: `{href: "/cameras/playback", label: "Playback", icon: "&#9202;"}` |
| `crates/rc-sentry-ai/Cargo.toml` | md-5 dependency | VERIFIED | `md-5 = "0.10"` present |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| nvr.rs | http://192.168.31.18 | reqwest HTTP digest auth | WIRED | mediaFileFind CGI calls with digest challenge-response using md-5 |
| playback.rs | nvr.rs | NvrClient::search_files and stream_file | WIRED | search_handler calls nvr_client.search_files(), stream_handler calls nvr_client.stream_file() |
| playback.rs | attendance/db.rs | get_attendance_for_day | WIRED | events_handler calls attendance::db::get_attendance_for_day via spawn_blocking |
| main.rs | playback.rs | router merge | WIRED | mod playback declared, PlaybackState constructed conditionally on nvr.enabled, playback_router merged |
| page.tsx | /api/v1/playback/search | fetch on form submit | WIRED | handleSearch() builds URLSearchParams and fetches, response parsed as NvrFileInfo[] |
| page.tsx | /api/v1/playback/stream | video src attribute | WIRED | streamUrl constructed from SENTRY_BASE + encodeURIComponent(file_path), set as video src |
| page.tsx | /api/v1/playback/events | fetch for event markers | PARTIAL | fetch works but response parsing wrong -- expects flat array, gets {day, events, count} object |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| MNTR-02 | 119-01, 119-02, 119-03 | NVR playback proxy -- query Dahua NVR API for stored footage, serve through dashboard with event markers | PARTIAL | Core playback (search + stream) fully works. Event markers have wiring bug preventing display. |

### Roadmap Success Criteria

| # | Criterion | Status | Evidence |
|---|-----------|--------|---------|
| 1 | Dashboard provides a time-range selector that queries the Dahua NVR at .18 for stored footage and streams it through rc-sentry-ai | VERIFIED | Search form + playback proxy endpoints + video player all wired correctly |
| 2 | Attendance event markers overlay on the playback timeline so staff can jump to moments when specific persons were detected | FAILED | Frontend/backend response format mismatch prevents markers from rendering |
| 3 | Playback works for all 3 attendance cameras and does not interfere with the NVR's ongoing recording | NEEDS HUMAN | Camera selection works for any camera with nvr_channel; NVR interference cannot be verified without live test |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| (none found) | - | - | - | No TODOs, FIXMEs, placeholders, or empty implementations detected |

### Compilation

`cargo check -p rc-sentry-ai` succeeds (8 pre-existing warnings, none from phase 119 code).

### Human Verification Required

### 1. NVR Playback End-to-End

**Test:** Open http://192.168.31.23:3200/cameras/playback, select a camera/date/time, search, click a recording, verify video plays
**Expected:** Video streams through proxy and plays in browser
**Why human:** Requires live NVR with recordings, network connectivity, browser video codec support

### 2. NVR Recording Non-Interference

**Test:** While streaming playback, verify NVR at http://192.168.31.18 is still recording on all channels
**Expected:** NVR continues recording without interruption
**Why human:** Requires accessing NVR web interface during playback

### 3. All 3 Attendance Cameras Work

**Test:** Search recordings for each of the 3 attendance cameras
**Expected:** All return recordings (if NVR has them) and stream correctly
**Why human:** Requires NVR to have recordings from each camera

### Gaps Summary

One gap found: **frontend/backend response format mismatch on the events endpoint**.

The `events_handler` in `playback.rs` (line 146-150) wraps the attendance entries in a `{"day": ..., "events": [...], "count": N}` JSON object. However, the frontend `page.tsx` (line 112) does `const data: AttendanceEntry[] = await res.json()` expecting a flat array. This means `setEvents(data)` stores the wrapper object (not an array) into state, so `filteredEvents` will be empty and no timeline markers will render. The click-to-seek functionality also depends on this working.

**Fix:** Either change the frontend line 112 to `const data = await res.json(); setEvents(data.events || [])` or change the backend to return the flat array directly with `Json(json!(entries))`.

This is a small but blocking bug for the event timeline feature, which is one of the three roadmap success criteria.

---

_Verified: 2026-03-22T14:30:00+05:30_
_Verifier: Claude (gsd-verifier)_
