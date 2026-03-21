---
phase: 85-lmu-telemetry
plan: 01
subsystem: telemetry
tags: [rust, winapi, shared-memory, rf2, lmu, le-mans-ultimate, sims, sector-splits]

requires:
  - phase: 84-iracing-telemetry
    provides: IracingAdapter with ShmHandle pattern, SimAdapter trait, connect lifecycle, first_read safety, test patterns

provides:
  - LmuAdapter struct implementing SimAdapter trait for Le Mans Ultimate
  - rF2 shared memory reader for $rFactor2SMMP_Scoring$ and $rFactor2SMMP_Telemetry$
  - sector_times_ms() free function for cumulative-to-differential sector derivation
  - 6 unit tests covering all key behaviors
  - pub mod lmu registration in sims/mod.rs

affects: [85-02-lmu-integration, event_loop, main.rs adapter wiring]

tech-stack:
  added: []
  patterns:
    - "rF2 fixed struct layout: named byte-offset constants sourced from rF2Data.cs (TheIronWolfModding, master 2024)"
    - "Torn-read guard: mVersionUpdateBegin/End equality check with 3-retry spin loop"
    - "sector_times_ms() rounds (not truncates) to avoid f64 precision loss"
    - "process_scoring() on each read_telemetry call — 5Hz scoring rate, 100ms polling harmless"

key-files:
  created:
    - crates/rc-agent/src/sims/lmu.rs
  modified:
    - crates/rc-agent/src/sims/mod.rs

key-decisions:
  - "Use .round() not `as u32` cast for sector/lap time ms conversion — (42.3 - 20.1) * 1000.0 = 22199.99... truncates to 22199, not 22200"
  - "pub mod lmu added to sims/mod.rs in Plan 01 (not deferred to Plan 02) — required for test compilation via cargo test sims::lmu"
  - "rF2VehicleScoring vehicle record: 368 bytes per entry, vehicle array starts at offset 2828 (12 header + 2816 rF2ScoringInfo)"
  - "Speed computed from mLocalVel magnitude (sqrt of x^2+y^2+z^2) — avoids needing orientation matrix multiplication"

patterns-established:
  - "ShmHandle Drop impl: UnmapViewOfFile + CloseHandle in Drop (not disconnect()) — prevents leaks if disconnect() not called"
  - "Two ShmHandles per adapter: scoring_shm + telemetry_shm — both opened in connect(), both dropped in disconnect()"

requirements-completed: [TEL-LMU-01, TEL-LMU-02, TEL-LMU-03]

duration: 25min
completed: 2026-03-21
---

# Phase 85 Plan 01: LMU Telemetry Adapter Summary

**LmuAdapter with rF2 fixed-struct shared memory reader: Scoring + Telemetry buffers, torn-read guard, sector splits via cumulative field derivation, first-packet safety, session transition reset, and 6 unit tests**

## Performance

- **Duration:** 25 min
- **Started:** 2026-03-21T06:35:00Z
- **Completed:** 2026-03-21T07:00:00Z
- **Tasks:** 1 (with TDD cycle: RED 0 failures fixed before write, GREEN on 2nd compile)
- **Files modified:** 2

## Accomplishments

- LmuAdapter struct with two ShmHandle fields (scoring_shm, telemetry_shm) for rF2 named file maps
- connect() opens both `$rFactor2SMMP_Scoring$` and `$rFactor2SMMP_Telemetry$` with clear error referencing rF2SharedMemoryMapPlugin
- process_scoring() reads player vehicle from vehicle array (mIsPlayer==1 scan), detects lap via mTotalLaps increment
- sector_times_ms() correctly derives S1/S2/S3 from cumulative rF2 fields using .round() for f64 precision
- First-packet safety: snapshots mTotalLaps on first read, no false lap emit on mid-session connect
- Session transition via mSession field change: resets last_lap_count, first_read, pending_lap
- read_is_on_track() trait override: Some(true) when mGamePhase >= 4 AND mIsPlayer found
- All 6 unit tests pass: connect_no_shm, sector_derivation, sector_guard, lap_completed_event, first_packet_safety, session_transition

## Task Commits

Each task was committed atomically:

1. **Task 1: LmuAdapter struct and rF2 shared memory connection** - `1161d80` (feat)

**Plan metadata:** (this SUMMARY commit — see final commit)

## Files Created/Modified

- `crates/rc-agent/src/sims/lmu.rs` — LmuAdapter + SimAdapter impl + sector_times_ms() + 6 unit tests + struct offset constants from rF2Data.cs
- `crates/rc-agent/src/sims/mod.rs` — added `pub mod lmu;` (needed for test compilation)

## Decisions Made

- Used `.round()` instead of raw `as u32` cast for sector/lap time millisecond conversion — f64 precision issue: `(42.3 - 20.1) * 1000.0` evaluates to `22199.999...` which truncates to `22199` instead of the expected `22200`
- Registered `pub mod lmu` in sims/mod.rs in this plan rather than deferring to Plan 02 — the plan verify command (`cargo test -p rc-agent sims::lmu`) requires module registration to compile tests
- Speed computed from mLocalVel magnitude (sqrt of x^2+y^2+z^2) converting m/s to km/h — avoids needing the full orientation matrix
- Byte offsets sourced from rF2Data.cs (TheIronWolfModding/rF2SharedMemoryMapPlugin master branch, 2024): vehicle record 368 bytes, vehicle array at offset 2828 in scoring buffer

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed f64 truncation error in sector time conversion**
- **Found during:** Task 1 (test run — test_sector_derivation and test_lap_completed_event failed)
- **Issue:** `(42.3 - 20.1) * 1000.0 as u32` truncates to `22199` due to f64 representation; expected `22200`
- **Fix:** Changed all time-to-milliseconds conversions to use `.round()` before `as u32` cast
- **Files modified:** crates/rc-agent/src/sims/lmu.rs (sector_times_ms, process_scoring, test)
- **Verification:** All 6 tests pass after fix
- **Committed in:** 1161d80 (Task 1 commit)

**2. [Rule 2 - Missing Critical] Added pub mod lmu to sims/mod.rs**
- **Found during:** Task 1 (cargo test returned 0 tests run — module not found)
- **Issue:** Without mod registration, cargo test filters out the module entirely
- **Fix:** Added `pub mod lmu;` to crates/rc-agent/src/sims/mod.rs
- **Files modified:** crates/rc-agent/src/sims/mod.rs
- **Verification:** cargo test sims::lmu runs 6 tests and all pass
- **Committed in:** 1161d80 (Task 1 commit)

---

**Total deviations:** 2 auto-fixed (1 bug, 1 missing critical)
**Impact on plan:** Both fixes necessary for correctness and testability. No scope creep.

## Issues Encountered

None beyond the two auto-fixed deviations above.

## User Setup Required

None - no external service configuration required. LMU requires rF2SharedMemoryMapPlugin (ships with game via Steam, active by default).

## Next Phase Readiness

- LmuAdapter is complete and fully tested
- Plan 02 (lmu-integration): wire adapter into main.rs adapter creation match + event_loop.rs PlayableSignal LMU arm
- pub mod lmu is already registered in sims/mod.rs (handled here, not needed again in Plan 02)

---
*Phase: 85-lmu-telemetry*
*Completed: 2026-03-21*
