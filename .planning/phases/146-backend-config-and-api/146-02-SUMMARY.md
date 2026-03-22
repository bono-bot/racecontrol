---
phase: 146-backend-config-and-api
plan: 02
subsystem: rc-sentry-ai
tags: [camera, layout, api, persistence, atomic-write]
dependency_graph:
  requires: [146-01]
  provides: [CameraLayout, LayoutState, /api/v1/cameras/layout-GET, /api/v1/cameras/layout-PUT, camera-layout.json]
  affects: [crates/rc-sentry-ai/src/mjpeg.rs, crates/rc-sentry-ai/src/main.rs]
tech_stack:
  added: []
  patterns: [atomic-write-tmp-rename, tokio-mutex-shared-state, serde-default-fn, file-backed-state]
key_files:
  created: []
  modified:
    - crates/rc-sentry-ai/src/mjpeg.rs
    - crates/rc-sentry-ai/src/main.rs
decisions:
  - "LayoutState uses Mutex<CameraLayout> (not RwLock) because PUT updates are rare and the simpler lock avoids writer starvation concerns"
  - "Layout file path derived from config_path parent so camera-layout.json lives beside rc-sentry-ai.toml — no separate config entry needed"
  - "Atomic write uses write-to-.json.tmp then tokio::fs::rename so partial writes never corrupt the layout file"
metrics:
  duration_minutes: 2
  completed_date: "2026-03-22T12:52:00+05:30"
  tasks_completed: 2
  tasks_total: 2
  files_modified: 2
requirements: [LYOT-04]
---

# Phase 146 Plan 02: Backend Config and API — Camera Layout Persistence Summary

One-liner: Added GET/PUT /api/v1/cameras/layout endpoints backed by atomic file writes to camera-layout.json, with startup load from disk so layout survives restarts.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Add layout persistence types, state, and GET/PUT endpoints | 8267f3f8 | crates/rc-sentry-ai/src/mjpeg.rs |
| 2 | Wire LayoutState into MjpegState in main.rs | 66cb98c8 | crates/rc-sentry-ai/src/main.rs |

## What Was Built

**Task 1 — mjpeg.rs:**
- Added `CameraLayout` struct with `grid_mode: String` (default "3x3"), `camera_order: Vec<u32>` (default empty), `zone_filter: Option<String>` (default None)
- Added `default_grid_mode()` serde default fn and `Default` impl for `CameraLayout`
- Added `LayoutState` struct with `layout: Mutex<CameraLayout>` and `file_path: PathBuf`
- Added `LayoutState::load(path)` — reads JSON from file, falls back to `CameraLayout::default()` on any error
- Added `layout_get_handler` — locks Mutex, returns JSON with grid_mode/camera_order/zone_filter
- Added `layout_put_handler` — updates Mutex, serializes to pretty JSON, writes to `.json.tmp`, renames to final path; returns 500 with error JSON on any failure step
- Added route `GET /api/v1/cameras/layout` and `PUT /api/v1/cameras/layout` via `.put()` method chaining on MethodRouter
- Added `layout_state: Arc<LayoutState>` field to `MjpegState`

**Task 2 — main.rs:**
- Derives `layout_file_path` from `config_path.parent()` joined with `"camera-layout.json"`, fallback parent `C:\RacingPoint`
- Calls `mjpeg::LayoutState::load(layout_file_path)` and wraps in `Arc`
- Logs loaded path at INFO level
- Passes `layout_state` into `MjpegState` initializer

## Verification

All acceptance criteria passed:
- All 7 mjpeg.rs checks: CameraLayout, LayoutState, layout_get_handler, layout_put_handler, camera-layout reference, json.tmp, route registration
- All 3 main.rs checks: layout_state, LayoutState::load, camera-layout.json
- `cargo check -p rc-sentry-ai` exits 0 (9 pre-existing warnings, zero errors)
- No `.unwrap()` in any new handler code

## Deviations from Plan

None — plan executed exactly as written.

## Self-Check: PASSED

- crates/rc-sentry-ai/src/mjpeg.rs: modified with all required types, handlers, and route
- crates/rc-sentry-ai/src/main.rs: modified with LayoutState initialization and MjpegState wiring
- Commit 8267f3f8: exists (Task 1)
- Commit 66cb98c8: exists (Task 2)
