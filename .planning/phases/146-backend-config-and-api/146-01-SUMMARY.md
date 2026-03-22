---
phase: 146-backend-config-and-api
plan: 01
subsystem: rc-sentry-ai
tags: [camera, config, api, metadata]
dependency_graph:
  requires: []
  provides: [CameraConfig.display_name, CameraConfig.display_order, CameraConfig.zone, /api/v1/cameras-extended]
  affects: [crates/rc-sentry-ai/src/config.rs, crates/rc-sentry-ai/src/mjpeg.rs]
tech_stack:
  added: []
  patterns: [serde-default-fn, option-fallback-unwrap_or]
key_files:
  created: []
  modified:
    - crates/rc-sentry-ai/src/config.rs
    - crates/rc-sentry-ai/src/mjpeg.rs
decisions:
  - "CameraConfig uses Option<String>/Option<u32> for display_name/display_order so None signals 'use default' to callers, avoiding a double-layer default"
  - "zone field uses serde default_zone() fn returning 'other' rather than Option so JSON always contains a string (no null in API response)"
  - "CORS Method::PUT added preemptively for layout PUT endpoint in plan 02"
metrics:
  duration_minutes: 5
  completed_date: "2026-03-22T12:45:00+05:30"
  tasks_completed: 2
  tasks_total: 2
  files_modified: 2
requirements: [INFRA-03, INFRA-04]
---

# Phase 146 Plan 01: Backend Config and API — Camera Metadata Summary

One-liner: Extended CameraConfig with display_name/display_order/zone TOML fields and updated /api/v1/cameras to return all 8 dashboard-required fields with sensible fallbacks.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Add display_name, display_order, zone to CameraConfig | 71bd2d1f | crates/rc-sentry-ai/src/config.rs |
| 2 | Update /api/v1/cameras to return extended metadata | 2f0d0572 | crates/rc-sentry-ai/src/mjpeg.rs |

## What Was Built

**Task 1 — config.rs:**
- Added `display_name: Option<String>` with `#[serde(default)]` — deserializes from TOML, None when absent
- Added `display_order: Option<u32>` with `#[serde(default)]` — None when absent
- Added `zone: String` with `#[serde(default = "default_zone")]` — always "other" when absent (never null)
- Added `default_zone()` fn returning `"other".to_string()`
- Added `effective_display_name(&self) -> &str` method — returns display_name if set, falls back to name

**Task 2 — mjpeg.rs:**
- Extended `CameraInfo` struct with `display_name: String`, `display_order: u32`, `zone: String`, `nvr_channel: Option<u32>`
- Updated `cameras_list_handler` to populate all 8 fields using `effective_display_name()` and `unwrap_or(0)` for display_order
- Added `Method::PUT` to CORS allow_methods for upcoming layout endpoint

## Verification

All acceptance criteria passed:
- All 5 config.rs checks: display_name, display_order, zone, default_zone, effective_display_name
- All 6 mjpeg.rs checks: display_name, display_order, nvr_channel, zone, effective_display_name, Method::PUT
- `cargo check -p rc-sentry-ai` exits 0 (9 pre-existing warnings, zero errors)
- No `.unwrap()` added in any modified code

## Deviations from Plan

None — plan executed exactly as written.

## Self-Check: PASSED

- crates/rc-sentry-ai/src/config.rs: modified with all required fields
- crates/rc-sentry-ai/src/mjpeg.rs: modified with all required fields
- Commit 71bd2d1f: exists
- Commit 2f0d0572: exists
