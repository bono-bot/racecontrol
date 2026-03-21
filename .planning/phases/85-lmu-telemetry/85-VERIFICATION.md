---
phase: 85-lmu-telemetry
verified: 2026-03-21T10:30:00+05:30
status: passed
score: 8/8 must-haves verified
re_verification: false
---

# Phase 85: LMU Telemetry Verification Report

**Phase Goal:** Le Mans Ultimate lap times are captured via rFactor 2 shared memory plugin
**Verified:** 2026-03-21T10:30:00+05:30 (IST)
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths (from ROADMAP.md Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | LMU shared memory is read using rFactor 2 shared memory plugin mapped files ($rFactor2SMMP_*) | VERIFIED | `open_lmu_shm("$rFactor2SMMP_Scoring$")` and `open_lmu_shm("$rFactor2SMMP_Telemetry$")` in `connect()` at lmu.rs:538-556 |
| 2 | Lap times and sector splits are extracted from rF2 scoring data after each completed lap | VERIFIED | `process_scoring()` reads `mTotalLaps`, `mLastLapTime`, `mLastSector1`, `mLastSector2` using fixed byte offsets; `sector_times_ms()` derives S1/S2/S3 correctly |
| 3 | Each completed lap emits a LapCompleted event with sim_type = LMU | VERIFIED | `pending_lap = Some(LapData { sim_type: SimType::LeMansUltimate, ... })` at lmu.rs:512; `poll_lap_completed()` at lmu.rs:738 takes and returns it |

**Score:** 3/3 truths verified

### Must-Have Truths (from PLAN 01 + PLAN 02 frontmatter)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | LmuAdapter connects to rF2 shared memory mapped files ($rFactor2SMMP_Scoring$, $rFactor2SMMP_Telemetry$) | VERIFIED | lmu.rs:538-556: both maps opened in `connect()`; error messages name the plugin |
| 2 | Sector splits correctly derived from cumulative rF2 fields (S1=mLastSector1, S2=mLastSector2-mLastSector1, S3=mLastLapTime-mLastSector2) | VERIFIED | `sector_times_ms()` at lmu.rs:285-297; uses `.round()` to avoid f64 truncation |
| 3 | Each completed lap emits LapData with sim_type=SimType::LeMansUltimate | VERIFIED | lmu.rs:512, 533; `sim_type()` returns `SimType::LeMansUltimate` |
| 4 | First-packet safety prevents spurious lap emission on mid-session connect | VERIFIED | `first_read` field at lmu.rs:43; snapshotted in `process_scoring()` at lmu.rs:483-489; test_first_packet_safety passes |
| 5 | Session transitions reset lap tracking state | VERIFIED | `process_scoring()` at lmu.rs:415-426: resets `last_lap_count`, `first_read`, `pending_lap` on `mSession` change; test_session_transition_resets_lap passes |
| 6 | LmuAdapter is created when rc-agent config specifies sim=lmu or sim=le_mans_ultimate | VERIFIED | main.rs:307: `"lmu" | "le_mans_ultimate" => SimType::LeMansUltimate`; main.rs:411: `SimType::LeMansUltimate => Some(Box::new(LmuAdapter::new(pod_id)))` |
| 7 | LMU PlayableSignal uses read_is_on_track() from shared memory instead of 90s process fallback | VERIFIED | event_loop.rs:533-551: dedicated `SimType::LeMansUltimate` arm; LMU does NOT fall through to the process-based 90s fallback at line 552+ |
| 8 | LMU laps are emitted through the standard poll_lap_completed pathway | VERIFIED | lmu.rs:738: `fn poll_lap_completed()` returns `Ok(self.pending_lap.take())`; same pattern as iRacing adapter |

**Score:** 8/8 truths verified

---

## Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-agent/src/sims/lmu.rs` | LmuAdapter struct + impl SimAdapter + unit tests | VERIFIED | 950 lines; `pub struct LmuAdapter`, `impl SimAdapter for LmuAdapter`, `#[cfg(test)] mod tests` all present |
| `crates/rc-agent/src/sims/mod.rs` | `pub mod lmu;` registration | VERIFIED | Line 4: `pub mod lmu;` |
| `crates/rc-agent/src/main.rs` | LmuAdapter creation in match arm | VERIFIED | Line 50: `use sims::lmu::LmuAdapter`; line 411: `SimType::LeMansUltimate => Some(Box::new(LmuAdapter::new(pod_id)))` |
| `crates/rc-agent/src/event_loop.rs` | LMU PlayableSignal dispatch arm | VERIFIED | Lines 533-551: dedicated `Some(rc_common::types::SimType::LeMansUltimate)` match arm using `adapter.read_is_on_track()` |

---

## Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `lmu.rs` | `$rFactor2SMMP_Scoring$` | `OpenFileMappingW + MapViewOfFile` | WIRED | `open_lmu_shm("$rFactor2SMMP_Scoring$")` at lmu.rs:538 |
| `lmu.rs` | `$rFactor2SMMP_Telemetry$` | `OpenFileMappingW + MapViewOfFile` | WIRED | `open_lmu_shm("$rFactor2SMMP_Telemetry$")` at lmu.rs:546 |
| `lmu.rs` | `rc_common::types::LapData` | `poll_lap_completed returns LapData` | WIRED | `SimType::LeMansUltimate` set at lmu.rs:512; trait method returns `Ok(self.pending_lap.take())` |
| `main.rs` | `sims/lmu.rs` | `use sims::lmu::LmuAdapter + match arm` | WIRED | main.rs:50 import + line 411 match arm |
| `event_loop.rs` | `adapter.read_is_on_track()` | `dyn SimAdapter trait dispatch` | WIRED | event_loop.rs:536: `adapter.read_is_on_track()` via `dyn SimAdapter`; trait override confirmed at lmu.rs:768 inside `impl SimAdapter for LmuAdapter` |

---

## Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| TEL-LMU-01 | 85-01, 85-02 | LMU adapter reads rF2 shared memory | SATISFIED | `open_lmu_shm()` opens both named maps; `LmuAdapter::new` created for `SimType::LeMansUltimate` in main.rs |
| TEL-LMU-02 | 85-01 | Sector splits extracted from cumulative rF2 fields | SATISFIED | `sector_times_ms()` function at lmu.rs:285; S2=(mLastSector2-mLastSector1)*1000, S3=(mLastLapTime-mLastSector2)*1000 |
| TEL-LMU-03 | 85-01, 85-02 | Completed laps emit LapCompleted with sim_type=LMU | SATISFIED | `pending_lap` set at lmu.rs:512 with `SimType::LeMansUltimate`; wired through `poll_lap_completed()` pathway |

### Note on Requirements Definition

TEL-LMU-01, TEL-LMU-02, and TEL-LMU-03 are referenced in `ROADMAP.md` Phase 85 and in the plan frontmatter, but are NOT defined in `REQUIREMENTS.md`. The current `REQUIREMENTS.md` covers the v11.1 Pre-Flight Session Checks milestone (PF/HW/SYS/NET/DISP/STAFF IDs). The LMU telemetry requirements belong to a different milestone (leaderboard/telemetry suite). The IDs are semantically clear from ROADMAP context and the implementation satisfies all three. This is a documentation gap in REQUIREMENTS.md but does not block goal achievement.

---

## Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `crates/rc-agent/src/sims/lmu.rs` | 141, 172 | Unused constants (`TEL_NUM_VEHICLES_OFF`, `TEL_VEH_ID_OFF`) | Info | Cargo warning only; dead code for telemetry constants not yet used in read path. No impact on goal. |

No blockers or warnings that prevent goal achievement. 35 compiler warnings were generated (mostly unused variables/constants in telemetry reading path), all below warning level — none are errors.

---

## Unit Test Results

All 6 required tests pass:

```
test sims::lmu::tests::test_first_packet_safety ... ok
test sims::lmu::tests::test_sector_guard ... ok
test sims::lmu::tests::test_sector_derivation ... ok
test sims::lmu::tests::test_connect_no_shm ... ok
test sims::lmu::tests::test_session_transition_resets_lap ... ok
test sims::lmu::tests::test_lap_completed_event ... ok

test result: ok. 6 passed; 0 failed; 0 ignored
```

---

## Human Verification Required

### 1. Live LMU Session — Lap Capture

**Test:** On a pod configured with `sim = "lmu"`, launch Le Mans Ultimate and complete a lap.
**Expected:** Lap appears on leaderboard with correct lap time and S1/S2/S3 sector splits. `sim_type = LeMansUltimate` in the stored lap record.
**Why human:** Requires physical LMU installation with rF2SharedMemoryMapPlugin active. Cannot verify shared memory contents programmatically without a running game.

### 2. Live LMU Session — Billing Start Accuracy

**Test:** On a pod configured for LMU, start a session and drive onto the track. Observe when billing transitions to Live state.
**Expected:** Billing starts when `mGamePhase >= 4` AND player vehicle found in scoring buffer — faster and more accurate than the previous 90s process fallback.
**Why human:** Requires live game state; timing cannot be verified statically.

### 3. Mid-Session Connect Safety

**Test:** While LMU is running mid-session (e.g., lap 5), restart rc-agent on the pod.
**Expected:** No spurious lap emission on reconnect; first lap after reconnect is captured correctly.
**Why human:** Requires physical pod restart during active game session.

---

## Summary

Phase 85 achieves its goal. All 8 must-have truths are verified against the actual codebase:

- `crates/rc-agent/src/sims/lmu.rs` (950 lines) implements the full `LmuAdapter` with rF2 fixed-struct shared memory reading, torn-read guard, sector derivation, first-packet safety, session reset, and all 6 unit tests passing.
- `sims/mod.rs` registers `pub mod lmu`.
- `main.rs` creates `LmuAdapter` for `SimType::LeMansUltimate` (matching both `"lmu"` and `"le_mans_ultimate"` config strings).
- `event_loop.rs` has a dedicated LMU `PlayableSignal` arm using `read_is_on_track()` via trait dispatch — LMU no longer falls through to the 90s process-based fallback.

The only documentation gap is that TEL-LMU-01/02/03 are referenced in ROADMAP.md but not formally defined in REQUIREMENTS.md. This does not affect goal achievement — the implementation satisfies all three per their ROADMAP descriptions.

---

_Verified: 2026-03-21T10:30:00+05:30 (IST)_
_Verifier: Claude (gsd-verifier)_
