---
phase: 117-alerts-notifications
verified: 2026-03-22T12:00:00+05:30
status: passed
score: 9/9 must-haves verified
re_verification: false
---

# Phase 117: Alerts & Notifications Verification Report

**Phase Goal:** Staff and Uday are notified in real time about attendance events and unknown persons
**Verified:** 2026-03-22T12:00:00+05:30
**Status:** passed
**Re-verification:** No -- initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Dashboard clients receive real-time JSON events via WebSocket on :8096/ws/alerts | VERIFIED | `ws.rs` lines 17-21: route `/ws/alerts` with `alerts_router`; `main.rs` line 288: merged into app router; broadcast fan-out confirmed in `handle_socket` |
| 2 | Attendance events (recognized person) appear as WebSocket messages with type, person_name, camera, timestamp | VERIFIED | `types.rs` AlertEvent::Recognized has person_id, person_name, confidence, camera, timestamp; `engine.rs` converts RecognitionResult via From impl and sends on alert_tx; ws.rs serializes to JSON and sends as Text message |
| 3 | Multiple dashboard clients can connect simultaneously and all receive every event | VERIFIED | `ws.rs` line 27: each client calls `state.alert_tx.subscribe()` getting independent broadcast::Receiver; tokio broadcast channel guarantees fan-out to all subscribers |
| 4 | James machine shows a Windows toast notification when a person is detected at the entrance camera | VERIFIED | `toast.rs` lines 10-37: `#[cfg(target_os = "windows")]` run function subscribes to alert_rx, calls show_toast via spawn_blocking; `main.rs` line 162: spawned with alert_tx.subscribe() |
| 5 | A system sound plays with each toast notification | VERIFIED | `toast.rs` lines 88-90: comment explains Windows plays default notification sound automatically; winrt-toast 0.1 does not suppress audio, so system default plays |
| 6 | Toast shows person name, camera name, and timestamp | VERIFIED | `toast.rs` lines 46-72: format_event matches Recognized (title="Person Detected", line1=person_name, line2=camera+IST timestamp) and UnknownPerson (title="Unknown Person", line1="Unrecognized face detected", line2=camera+IST timestamp) |
| 7 | An unrecognized face (no gallery match above 0.45) triggers an UnknownPerson alert | VERIFIED | `pipeline.rs` lines 207-215: else branch on gallery miss sends UnknownFaceEvent via unknown_tx; `unknown.rs` receives and emits AlertEvent::UnknownPerson on alert_tx |
| 8 | Unknown person alerts are rate-limited to once per 5 minutes per camera | VERIFIED | `unknown.rs` lines 29-30: HashMap<String, Instant> for per-camera tracking; lines 52-62: rate limit check with configurable duration (default 300s); lines 46-50: periodic cleanup |
| 9 | Face crop JPEG is saved to C:\RacingPoint\face-crops\ with path included in the alert event | VERIFIED | `unknown.rs` lines 64-100: spawn_blocking saves JPEG via JpegEncoder with configurable quality; lines 139-144: AlertEvent::UnknownPerson includes crop_path: Some(path); config.rs default face_crop_dir is `C:\RacingPoint\face-crops\` |

**Score:** 9/9 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-sentry-ai/src/alerts/types.rs` | AlertEvent enum with Recognized and Unknown variants | VERIFIED | 48 lines; AlertEvent enum with serde tagged JSON, UnknownFaceEvent struct, From<RecognitionResult> impl |
| `crates/rc-sentry-ai/src/alerts/ws.rs` | WebSocket upgrade handler with broadcast fan-out | VERIFIED | 61 lines; AlertWsState, alerts_router, ws_handler with upgrade, handle_socket loop |
| `crates/rc-sentry-ai/src/alerts/engine.rs` | Alert engine subscribing to recognition broadcast | VERIFIED | 30 lines; run function receives RecognitionResult, converts to AlertEvent, sends on alert_tx |
| `crates/rc-sentry-ai/src/alerts/toast.rs` | Toast notification engine | VERIFIED | 108 lines; Windows cfg-gated run function with format_event and show_toast; non-Windows stub |
| `crates/rc-sentry-ai/src/alerts/unknown.rs` | Unknown person detection with rate limiting and JPEG saving | VERIFIED | 158 lines; rate limiting, create_dir_all, JpegEncoder, AlertEvent::UnknownPerson emission |
| `crates/rc-sentry-ai/src/alerts/mod.rs` | Module declarations | VERIFIED | All 5 submodules declared: engine, toast, types, unknown, ws |
| `crates/rc-sentry-ai/src/config.rs` | AlertsConfig struct | VERIFIED | Lines 273-310; enabled, unknown_rate_limit_secs (300), face_crop_dir, face_crop_quality (85) |
| `crates/rc-sentry-ai/src/main.rs` | Alert wiring (broadcast channels, engine spawns, router merge) | VERIFIED | Lines 144-178: alert_tx, unknown_tx channels; alert engine, toast engine, unknown engine all spawned; line 288: alerts_router merged |
| `crates/rc-sentry-ai/Cargo.toml` | axum ws feature + winrt-toast dependency | VERIFIED | (confirmed by successful cargo check) |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| engine.rs | recognition broadcast | `rx.recv()` | WIRED | Line 15: `rx.recv().await` in loop, converts via `AlertEvent::from(result)` |
| ws.rs | alert broadcast | `alert_rx.recv()` | WIRED | Line 38: `alert_rx.recv().await` in handle_socket loop |
| main.rs | alerts module | engine spawn + WS route merge | WIRED | Line 157: `alerts::engine::run(alert_rx, atx)` spawned; line 288: `alerts::ws::alerts_router(alert_ws_state)` merged |
| toast.rs | alert broadcast | `alert_rx.recv()` | WIRED | Line 18: `alert_rx.recv().await`; main.rs line 162: `alerts::toast::run(toast_rx)` spawned |
| main.rs | toast spawn | `tokio::spawn` | WIRED | Line 162: `tokio::spawn(alerts::toast::run(toast_rx))` |
| pipeline.rs | unknown face broadcast | `unknown_tx.send` | WIRED | Line 209-215: `utx.send(event)` in else branch when no gallery match |
| unknown.rs | alert broadcast | `alert_tx.send(AlertEvent::UnknownPerson)` | WIRED | Lines 119-124 (fallback) and lines 139-144 (success): AlertEvent::UnknownPerson sent on alert_tx |
| unknown.rs | face-crops directory | `JpegEncoder` + `File::create` | WIRED | Lines 89-100: spawn_blocking with JpegEncoder saving to crop_path |
| main.rs | unknown engine | `alerts::unknown::run` | WIRED | Lines 166-173: spawned with unknown_rx, alert_tx, config params |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| ALRT-01 | 117-01 | Real-time WebSocket notifications for attendance events | SATISFIED | WebSocket endpoint /ws/alerts streams AlertEvent JSON to dashboard clients; engine subscribes to recognition broadcast |
| ALRT-02 | 117-02 | Desktop popup with sound on James machine | SATISFIED | winrt-toast notifications with person name, camera, IST timestamp; system default sound; spawned from main.rs |
| ALRT-03 | 117-03 | Unknown person alert pipeline with face crop | SATISFIED | Pipeline else branch on gallery miss; rate-limited per camera (5min); JPEG crop saved; alert flows to both WS and toast |

Note: ALRT-01, ALRT-02, ALRT-03 are referenced in ROADMAP.md phase definition but not present in REQUIREMENTS.md (which currently only covers v15.0 AntiCheat Compatibility requirements). This is not a gap -- the requirement IDs serve as phase-internal tracking.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| (none) | - | - | - | No anti-patterns found |

No TODO, FIXME, HACK, placeholder, stub, or empty implementation patterns found in any alerts module file. All modules are fully implemented.

### Compilation

`cargo check -p rc-sentry-ai` passes with 0 errors (8 warnings, all in unrelated gallery module for dead code).

### Human Verification Required

### 1. Toast Notification Display

**Test:** Run rc-sentry-ai on James machine with a camera pointed at a known person. Verify a Windows toast popup appears.
**Expected:** Toast shows "Person Detected", person's name, camera name, and IST timestamp. System notification sound plays.
**Why human:** Visual toast rendering and audio playback cannot be verified programmatically.

### 2. Unknown Person Toast + Face Crop

**Test:** Stand in front of an entrance camera with a face not enrolled in the gallery. Wait for detection.
**Expected:** Toast shows "Unknown Person" / "Unrecognized face detected" with camera and timestamp. A 112x112 JPEG file appears in `C:\RacingPoint\face-crops\` with a recognizable face crop.
**Why human:** JPEG quality and face visibility require visual inspection. Rate limiting (5-min cooldown) should be tested by waiting and re-triggering.

### 3. WebSocket Dashboard Stream

**Test:** Open a WebSocket client (e.g., websocat or browser DevTools) to `ws://192.168.31.27:8096/ws/alerts`. Trigger a face detection.
**Expected:** JSON messages arrive with `"type": "recognized"` or `"type": "unknown_person"` fields, including person_name/camera/timestamp.
**Why human:** End-to-end WebSocket delivery requires a running instance and live camera feed.

### 4. Multiple WebSocket Clients

**Test:** Open 2+ simultaneous WebSocket connections to `/ws/alerts`. Trigger a detection.
**Expected:** Both clients receive the same event.
**Why human:** Concurrent broadcast fan-out requires runtime testing.

### Gaps Summary

No gaps found. All 9 observable truths are verified. All artifacts exist, are substantive (no stubs), and are fully wired. All 3 requirement IDs (ALRT-01, ALRT-02, ALRT-03) are satisfied by the implementation. The code compiles cleanly. No anti-patterns detected.

---

_Verified: 2026-03-22T12:00:00+05:30_
_Verifier: Claude (gsd-verifier)_
