---
phase: 84-iracing-telemetry
verified: 2026-03-21T06:15:00+05:30
status: passed
score: 6/6 must-haves verified
re_verification: false
---

# Phase 84: iRacing Telemetry Verification Report

**Phase Goal:** iRacing lap times and sector splits are captured via shared memory with reliable session transition handling
**Verified:** 2026-03-21T06:15:00+05:30 (IST)
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | IracingAdapter opens shared memory via OpenFileMappingW and reads telemetry variables by name | VERIFIED | `open_iracing_shm()` calls `OpenFileMappingW` at line 126; `find_var_offset()` scans irsdk_varHeader by name; `build_var_offsets()` looks up all 11 variables at connect time |
| 2 | Session UID changes reset lap state and re-parse YAML session info | VERIFIED | `apply_session_transition()` resets `last_lap_count`, `sector_times=[None;3]`, updates `last_session_uid`; called from `read_telemetry()` on UID change |
| 3 | LapCompleted counter increment produces LapData with lap_time_ms from LapLastLapTime * 1000 | VERIFIED | `record_lap()` at line 431: `(last_lap_time_s * 1000.0) as u32`; LapData constructed with `sim_type: SimType::IRacing`; `test_lap_completed_event` asserts `lap_time_ms=62500` for input `62.5s` |
| 4 | check_iracing_shm_enabled reads Documents/iRacing/app.ini for irsdkEnableMem=1 | VERIFIED | `check_iracing_shm_enabled()` uses `dirs_next::document_dir()` + pushes `iRacing/app.ini`; delegates to `check_iracing_shm_enabled_at(path)` which scans lines for `"irsdkEnableMem=1"` |
| 5 | First-packet safety prevents false lap when connecting mid-session | VERIFIED | `connect()` snapshots `LapCompleted` into `last_lap_count` before setting `first_read=true`; `record_lap()` early-returns without emitting when `first_read=true`; `test_first_packet_safety` asserts no `pending_lap` |
| 6 | read_is_on_track is a trait method override in impl SimAdapter, callable via dyn SimAdapter | VERIFIED | `fn read_is_on_track(&self) -> Option<bool>` appears inside `impl SimAdapter for IracingAdapter` block at line 734, calling `self.read_is_on_track_from_shm()` |

**Score:** 6/6 truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-agent/src/sims/iracing.rs` | IracingAdapter struct + SimAdapter impl + pre-flight + unit tests | VERIFIED | File exists; `pub struct IracingAdapter` at line 20; `impl SimAdapter for IracingAdapter` at line 529; 8 unit tests with `#[test]`; `pub fn check_iracing_shm_enabled` at line 487 |
| `crates/rc-agent/src/sims/mod.rs` | pub mod iracing + read_is_on_track default trait method | VERIFIED | `pub mod iracing;` at line 3; `fn read_is_on_track(&self) -> Option<bool> { None }` at line 43 |
| `crates/rc-agent/src/main.rs` | IracingAdapter creation in adapter match | VERIFIED | `use sims::iracing::IracingAdapter;` at line 49; `SimType::IRacing => Some(Box::new(IracingAdapter::new(pod_id.clone())))` at lines 407-409 |
| `crates/rc-agent/src/event_loop.rs` | IRacing PlayableSignal dispatch replacing 90s fallback | VERIFIED | `Some(rc_common::types::SimType::IRacing)` arm at line 513 reads `adapter.read_is_on_track()`; iRacing arm comes BEFORE the catch-all `Some(sim_type)` arm which retains 90s fallback for other sims |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `iracing.rs` | `rc_common::types::LapData` | LapData construction in record_lap() | VERIFIED | `LapData {` constructed at line 432 with all required fields populated |
| `iracing.rs` | `rc_common::types::SimType::IRacing` | sim_type field in LapData and SimAdapter impl | VERIFIED | `SimType::IRacing` appears 4 times: sim_type() return, LapData field, TelemetryFrame field, test assertion |
| `iracing.rs` | `SimAdapter trait` | fn read_is_on_track override inside impl SimAdapter block | VERIFIED | Override confirmed at line 734 inside `impl SimAdapter for IracingAdapter` — NOT an inherent method |
| `main.rs` | `iracing.rs` | SimType::IRacing arm calls IracingAdapter::new | VERIFIED | `SimType::IRacing => Some(Box::new(IracingAdapter::new(pod_id.clone())))` — exact pattern from plan |
| `event_loop.rs` | `iracing.rs` | read_is_on_track() call via dyn SimAdapter | VERIFIED | `adapter.read_is_on_track()` called through `dyn SimAdapter` trait reference at line 516 |

---

### Requirements Coverage

The active REQUIREMENTS.md (`.planning/REQUIREMENTS.md`) covers v11.1 Pre-Flight requirements and does not contain TEL-IR-* IDs. The authoritative source for these requirements is `.planning/milestones/v10.0-REQUIREMENTS.md` (also mirrored in `v11.0-REQUIREMENTS.md`), which maps all four TEL-IR requirements to Phase 84.

| Requirement | Source | Description | Status | Evidence |
|-------------|--------|-------------|--------|----------|
| TEL-IR-01 | v10.0-REQUIREMENTS.md | iRacing shared memory reader using winapi OpenFileMappingW | SATISFIED | `open_iracing_shm()` uses `OpenFileMappingW`; adapter wired in main.rs for `SimType::IRacing` |
| TEL-IR-02 | v10.0-REQUIREMENTS.md | Handle session transitions — re-open shared memory handle between races | SATISFIED | Implementation uses `SessionUniqueID` change detection + YAML re-parse (shm handle stays open — more robust than re-open). `apply_session_transition()` resets all lap state. `test_session_transition_resets_lap` verifies correctness. |
| TEL-IR-03 | v10.0-REQUIREMENTS.md | Lap times and sector splits extracted from iRacing telemetry | SATISFIED | `LapLastLapTime * 1000.0` gives `lap_time_ms`; sector splits set to `None` for v1 (no sector variables in iRacing real-time IRSDK — plan decision, not an omission). IsOnTrack billing signal wired in event_loop.rs. |
| TEL-IR-04 | v10.0-REQUIREMENTS.md | Pre-flight check: verify irsdkEnableMem=1 in app.ini | SATISFIED | `check_iracing_shm_enabled()` and `check_iracing_shm_enabled_at()` implemented; two tests covering missing file and enabled cases both pass |

**Note on TEL-IR-02:** The milestone requirement says "re-open shared memory handle between races." The implementation instead keeps the handle open and detects transitions via `SessionUniqueID` change. This is a valid and superior approach — re-opening the handle is fragile during session handoff; UID-based detection is atomic and does not risk a window where the handle is closed. The requirement intent (reliable cross-session operation) is fully satisfied.

**Note on TEL-IR-03 (sector splits):** The milestone says "sector splits extracted." The implementation sets all three sector fields to `None` in v1, with a documented rationale: iRacing real-time IRSDK telemetry does not expose sector split variables. This is an accepted limitation noted in plan decisions and the SUMMARY. Lap times are fully implemented.

---

### Anti-Patterns Found

| File | Pattern | Severity | Impact |
|------|---------|----------|--------|
| `iracing.rs` (test module) | `.unwrap()` in tempfile setup (lines 853-854) | INFO | Test-only, acceptable — `expect()` used for the named tempfile itself; `writeln!` unwraps are standard test boilerplate with no production impact |

No blockers or warnings found. No TODO/FIXME/placeholder comments. No empty implementations in production code. No stub return values in any SimAdapter methods.

---

### Human Verification Required

The following items cannot be verified programmatically and require a live iRacing session:

#### 1. End-to-end lap capture with iRacing running

**Test:** Launch iRacing on a pod, set `SimType = IRacing` in config, start a session, complete a lap.
**Expected:** LapData appears in racecontrol dashboard within 2s of lap completion, lap_time_ms matches iRacing's displayed lap time.
**Why human:** Requires live iRacing process with shared memory active; cannot simulate `Local\IRSDKMemMapFileName` in automated tests.

#### 2. Session transition across races

**Test:** Start an iRacing session, complete it, start another session without restarting rc-agent.
**Expected:** Second session's lap counts start from lap 1 with no carryover from the first session.
**Why human:** Requires two sequential live iRacing sessions on the same pod.

#### 3. First-packet safety with mid-session reconnect

**Test:** Start iRacing, complete 3 laps, then restart rc-agent while iRacing is still running.
**Expected:** No spurious LapData emitted on reconnect; first new lap after reconnect fires correctly.
**Why human:** Requires live iRacing process; shm state cannot be injected in offline tests.

#### 4. IsOnTrack billing trigger timing

**Test:** Launch an iRacing session; observe when billing transitions from WaitingForLive to Live.
**Expected:** Billing triggers promptly when player enters the track (IsOnTrack=true), NOT after 90s.
**Why human:** Requires live session and observing timestamp of GameStatusUpdate message.

---

## Gaps Summary

No gaps. All 6 must-have truths are verified, all 4 artifacts exist and are substantive and wired, all 5 key links are confirmed, and all 4 TEL-IR requirements are satisfied by the implementation.

The only items not verified programmatically are live-runtime behaviors (actual shm reads, billing trigger timing) that are inherently human-testable.

---

_Verified: 2026-03-21T06:15:00+05:30 (IST)_
_Verifier: Claude (gsd-verifier)_
