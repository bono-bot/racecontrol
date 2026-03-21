---
phase: 84-iracing-telemetry
plan: "01"
subsystem: rc-agent/sims
tags: [telemetry, iracing, shared-memory, winapi, lap-detection]
dependency_graph:
  requires:
    - crates/rc-agent/src/sims/mod.rs (SimAdapter trait)
    - rc_common::types::{LapData, TelemetryFrame, SessionInfo, SimType, SessionType}
    - winapi 0.3 (OpenFileMappingW, MapViewOfFile, CloseHandle, UnmapViewOfFile)
    - dirs-next 2 (document_dir for pre-flight app.ini)
  provides:
    - crates/rc-agent/src/sims/iracing.rs (IracingAdapter + SimAdapter impl)
    - check_iracing_shm_enabled() / check_iracing_shm_enabled_at()
  affects:
    - crates/rc-agent/src/sims/mod.rs (pub mod iracing added; read_is_on_track on trait)
    - Plan 84-02 event_loop wiring (read_is_on_track via dyn SimAdapter)
tech_stack:
  added: []
  patterns:
    - ShmHandle pattern from assetto_corsa.rs (winapi memory-mapped file wrapper)
    - Dynamic variable lookup by name (not fixed offsets) via irsdk_varHeader scan
    - Double-buffer tick-lock read (highest tickCount, retry 3x on torn read)
    - Session transition via SessionUniqueID change + YAML re-parse
    - First-packet safety: snapshot LapCompleted at connect time
    - Manual YAML key scan (no serde_yaml — iRacing uses ISO-8859-1)
key_files:
  created:
    - crates/rc-agent/src/sims/iracing.rs
  modified:
    - crates/rc-agent/src/sims/mod.rs (already had pub mod iracing + read_is_on_track at HEAD)
decisions:
  - "Dynamic variable lookup: scan irsdk_varHeader by name at connect() time — offsets cached in VarOffsets struct. Fixed offsets break across iRacing SDK updates."
  - "read_is_on_track() override inside impl SimAdapter for IracingAdapter — required for Plan 02 dyn SimAdapter dispatch. Inherent method alone would not be reachable through trait object."
  - "No serde_yaml: iRacing YAML is ISO-8859-1 non-standard; manual key scan (find key: in string, extract rest of line) is correct and simpler."
  - "Sector splits set to None for v1 — no sector split variables in iRacing real-time IRSDK telemetry."
  - "LapLastLapTime is seconds (f32) * 1000.0 = lap_time_ms (u32). Unit confusion is the #1 iRacing adapter bug risk."
  - "apply_session_transition() and record_lap() exposed as pub methods for direct unit test access."
  - "check_iracing_shm_enabled_at(path) accepts explicit path for testability (tempfile in tests)."
metrics:
  duration_minutes: 20
  completed_date: "2026-03-21"
  tasks_completed: 1
  tasks_total: 1
  files_created: 1
  files_modified: 0
---

# Phase 84 Plan 01: iRacing Adapter — Shared Memory, Lap Detection, Pre-Flight Summary

**One-liner:** IracingAdapter implementing SimAdapter via irsdk shared memory with dynamic variable lookup, double-buffer tick-lock, SessionUniqueID transition detection, and app.ini pre-flight check.

---

## Tasks Completed

| # | Task | Commit | Status |
|---|------|--------|--------|
| 1 | IracingAdapter core — shared memory, variable lookup, session transitions, lap detection, pre-flight | 651249d | Done |

---

## What Was Built

### IracingAdapter (`crates/rc-agent/src/sims/iracing.rs`)

A complete `SimAdapter` implementation for iRacing via the IRSDK shared memory protocol:

**Core structs:**
- `IracingAdapter` — main adapter with all state fields
- `VarOffsets` — cached row offsets for each telemetry variable, all default to -1 (not found)
- `IrsdkHeader` — mirrors the 112-byte fixed header layout (status, session info, var header, buffer slots)
- `ShmHandle` — Windows memory-mapped file wrapper (same pattern as AC adapter), `Send + Sync`

**Shared memory functions (Windows only):**
- `open_iracing_shm()` — opens `Local\IRSDKMemMapFileName` via OpenFileMappingW + MapViewOfFile
- `read_header()` — reads IrsdkHeader from fixed offsets
- `is_iracing_active(status)` — checks `status & 1 != 0`
- `find_var_offset()` — scans irsdk_varHeader (144 bytes each) for variable by name
- `build_var_offsets()` — builds VarOffsets by looking up each required variable name
- `read_latest_row_offset()` — double-buffer tick-lock: finds highest tickCount buffer, verifies tick unchanged (retries 3x)
- `read_var_i32/f32/bool()` — unsafe reads from selected buffer row + variable offset

**YAML parsing:**
- `extract_yaml_value()` — manual key scan (no serde_yaml; ISO-8859-1 compatible)
- `parse_session_type()` — maps iRacing session type strings to `SessionType` enum
- `parse_session_yaml()` — extracts track, car, session type from shm YAML

**SimAdapter trait implementation:**
- `connect()` — opens shm, builds var offsets, parses YAML, snapshots LapCompleted (first-packet safety)
- `read_telemetry()` — tick-lock read, session UID change detection, lap completion detection
- `poll_lap_completed()` — `self.pending_lap.take()` semantics
- `session_info()` — returns current track/car/session_type
- `disconnect()` — UnmapViewOfFile + CloseHandle
- `read_is_on_track()` — **explicit trait override** calling `read_is_on_track_from_shm()`, required for Plan 02 dyn SimAdapter dispatch

**Pre-flight:**
- `check_iracing_shm_enabled()` — reads Documents/iRacing/app.ini via dirs_next::document_dir()
- `check_iracing_shm_enabled_at(path)` — same but with explicit path for testability

### Tests (8 passing)

| Test | Requirement |
|------|-------------|
| test_connect_no_shm | connect() fails without iRacing (any platform) |
| test_session_transition_resets_lap | Session UID change resets last_lap_count + sector_times |
| test_lap_completed_event | LapCompleted 1->2 with 62.5s produces LapData with lap_time_ms=62500, sim_type=IRacing |
| test_first_packet_safety | LapCompleted already >0 on first read does NOT fire a lap |
| test_preflight_missing_ini | Missing app.ini returns false |
| test_preflight_ini_enabled | app.ini with "irsdkEnableMem=1" returns true |
| test_session_type_mapping | All iRacing session type strings map correctly |
| test_extract_yaml_value | YAML key scan works for track, car, session type |

---

## Deviations from Plan

### Auto-discovered: mod.rs already had pub mod iracing + read_is_on_track

**Found during:** Task 1 setup
**Issue:** `crates/rc-agent/src/sims/mod.rs` already had `pub mod iracing;` and `read_is_on_track` trait method from a prior docs commit (dad850c).
**Fix:** No action needed — my edits were confirmed correct but were already at HEAD. Only `iracing.rs` itself was new.
**Commit:** N/A — mod.rs unchanged

None of the deviation rules (1-4) triggered for the implementation itself. Plan executed as written.

---

## Requirements Addressed

| ID | Description | Status |
|----|-------------|--------|
| TEL-IR-01 | iRacing shared memory read via OpenFileMappingW during active sessions | Complete |
| TEL-IR-02 | Session transitions handled via SessionUniqueID change detection | Complete |
| TEL-IR-03 | LapCompleted counter increment produces LapData with lap_time_ms from LapLastLapTime * 1000 | Complete |
| TEL-IR-04 | Pre-flight check for irsdkEnableMem=1 in app.ini via dirs_next | Complete |

---

## Self-Check: PASSED

- crates/rc-agent/src/sims/iracing.rs: FOUND
- commit 651249d: FOUND
- 8/8 unit tests pass
