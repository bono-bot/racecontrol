---
phase: 147-cameras-html-dashboard-rewrite
plan: 03
subsystem: ui
tags: [vanilla-js, webrtc, go2rtc, cameras, nvr, dashboard, fullscreen, pre-warm]

# Dependency graph
requires:
  - phase: 147-02
    provides: cameras.html with drag-to-rearrange, zone grouping, layout persistence, and openFullscreen stub
  - phase: 145-01
    provides: go2rtc at ws://192.168.31.27:1984/api/ws with stream names ch1-ch13
provides:
  - cameras.html: complete WebRTC fullscreen with singleton pattern, pre-warm, and go2rtc signaling
affects:
  - live verification (Task 2 checkpoint — human verify on hardware)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "WebRTC singleton: teardownRtc() defined before connectWebRTC(), activePc/activeWs enforces one connection"
    - "go2rtc signaling: WebSocket ws://{host}/api/ws?src={stream} with webrtc/offer + webrtc/answer + webrtc/candidate JSON messages"
    - "Pre-warm pattern: 500ms mouseenter setTimeout starts WebRTC negotiation before click, mouseleave clears timer but keeps connection"
    - "Fullscreen lifecycle: openFullscreen -> teardownRtc(prev) -> show overlay -> connectWebRTC or promote preWarm -> startControlsAutoHide"
    - "Controls auto-hide: setTimeout 3s -> add auto-hidden class; mousemove resets; :hover overrides opacity via !important"
    - "Failure fallback: updateFsStatus('failed') -> showFallbackMessage() creates div, appends to fs, removes after 5s"

key-files:
  created: []
  modified:
    - crates/rc-sentry-ai/cameras.html

key-decisions:
  - "teardownRtc() must be defined before connectWebRTC() to satisfy singleton safety — no forward reference risk"
  - "buildFullscreenDOM() IIFE at script start creates fs structure via createElement (no innerHTML per security requirement)"
  - "Pre-warm: mouseleave does NOT teardown pre-warm connection — keeps it warm for potential immediate click"
  - "Snapshot poster set on video element before WebRTC connects — provides immediate visual fallback if connection fails"
  - "closeFullscreen() calls teardownPreWarm() in addition to teardownRtc() — cleans up any dangling pre-warm on explicit close"
  - "activePc.close() (not pc.close()) in teardownRtc — variable name is activePc throughout singleton pattern"

requirements-completed: [STRM-01, STRM-02, STRM-03, STRM-04, UIUX-03]

# Metrics
duration: 3min
completed: 2026-03-22
---

# Phase 147 Plan 03: WebRTC Fullscreen Streaming Summary

**RTCPeerConnection via go2rtc WebSocket signaling (ws://192.168.31.27:1984/api/ws) with singleton pattern, 500ms hover pre-warm, auto-hiding controls, and snapshot fallback on failure**

## Performance

- **Duration:** ~3 min
- **Started:** 2026-03-22T13:25:10+05:30
- **Completed:** 2026-03-22T13:28:14+05:30
- **Tasks:** 1/1 complete (Task 2 is human-verify checkpoint)
- **Files modified:** 1

## Accomplishments

- Full WebRTC lifecycle: connectWebRTC() opens WebSocket to go2rtc, creates RTCPeerConnection, negotiates offer/answer/ICE candidates
- Singleton pattern: teardownRtc() clears all event handlers and calls .close() before any new connection opens
- Pre-warm on 500ms hover: connectWebRTC starts before click; openFullscreen promotes the pre-warmed connection if channel matches
- Fullscreen overlay: 200ms fade-in animation, controls (camera name + status dot + X button) auto-hide after 3s, reappear on mousemove
- Loading spinner shown during WebRTC negotiation; snapshot poster set immediately as visual fallback
- WebRTC failure (failed/disconnected/closed state): showFallbackMessage() appends "Live unavailable — showing snapshot" badge for 5s
- beforeunload and visibilitychange event listeners ensure connections are torn down on page leave and tab hide
- Zero innerHTML assignments in entire file — all DOM via createElement
- cargo check -p rc-sentry-ai exits 0 (9 pre-existing warnings, identical to plan 02)

## Task Commits

1. **Task 1: WebRTC fullscreen with singleton pattern, pre-warm, and go2rtc signaling** - `58c624c5` (feat)

## Files Created/Modified

- `crates/rc-sentry-ai/cameras.html` - Added full WebRTC implementation: ~390 lines added (CSS + JS), replaced 31-line stub

## Decisions Made

- `buildFullscreenDOM()` runs as an IIFE at script initialization so the fs element is fully built before any event listeners reference it
- Pre-warm connection is NOT torn down on mouseleave — kept warm for potential immediate click. Only torn down if a different camera is hovered (teardownPreWarm called in mouseenter when preWarmChannel !== current ch)
- `closeFullscreen()` calls both `teardownRtc()` and `teardownPreWarm()` — ensures no dangling connections after explicit close
- Video element gets `poster` set to snapshot URL before WebRTC connects — gives immediate visual content and serves as WebRTC failure fallback without extra code
- Controls bar uses `opacity: 1 !important` on `:hover` to override the `auto-hidden` class opacity:0, ensuring controls are always accessible

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered

None — cargo check passed on first attempt. All acceptance criteria met.

## User Setup Required

None.

## Next Phase Readiness

- Task 2 is a `checkpoint:human-verify` — requires hardware verification on live go2rtc at http://192.168.31.27:8096/cameras/live
- Build and deploy rc-sentry-ai before verification: `cargo build --release --bin rc-sentry-ai`
- Verify: WebRTC fullscreen, pre-warm green border, controls auto-hide, singleton switching, Escape/X/backdrop close, 0 viewers after close

---
*Phase: 147-cameras-html-dashboard-rewrite*
*Completed: 2026-03-22*
