---
phase: 26-lap-filter-pin-security-telemetry-multiplayer
plan: 02
subsystem: database
tags: [lap-filter, sqlx, sqlite, migration, telemetry, session-type]

# Dependency graph
requires:
  - phase: 26-lap-filter-pin-security-telemetry-multiplayer plan 01
    provides: Wave 0 RED stubs in lap_tracker/auth/bot_coordinator
provides:
  - LapData.session_type required field in rc-common
  - catalog.TrackEntry.min_lap_time_ms per-track floor
  - catalog.get_min_lap_time_ms_for_track() pure static fn
  - persist_lap sets review_required=1 for laps below track floor
  - Idempotent DB migration: review_required + session_type columns on laps table
  - All 4 Wave 0 lap_tracker stubs GREEN (LAP-01, LAP-02, LAP-03)
affects: [26-03, 26-04, 26-05, lap_tracker, catalog, racecontrol-api]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Idempotent ALTER TABLE via let _ = ... ignore errors (SQLite duplicate column returns error, not no-op)"
    - "post-INSERT UPDATE for derived flags (review_required): avoids race conditions vs column defaults"
    - "Pure static catalog fn (not async) for min lap floor — static data, no DB needed"

key-files:
  created: []
  modified:
    - crates/rc-common/src/types.rs
    - crates/rc-agent/src/sims/assetto_corsa.rs
    - crates/rc-agent/src/sims/f1_25.rs
    - crates/racecontrol/src/catalog.rs
    - crates/racecontrol/src/lap_tracker.rs
    - crates/racecontrol/tests/integration.rs

key-decisions:
  - "session_type is non-optional on LapData — forces all construction sites to explicitly set it (no hidden defaults)"
  - "AC adapter hardcodes SessionType::Practice — AC shared memory has no session type field exposed"
  - "F1 adapter maps self.session_type (u8 from Packet 1) to SessionType at LapData construction"
  - "review_required is a post-INSERT UPDATE not a column flag — computed from catalog static data after lap is committed"
  - "Lap filter: game-reported isValidLap is authoritative; review_required is advisory staff flag, never deletes"

patterns-established:
  - "Lap floor check: catalog::get_min_lap_time_ms_for_track(&lap.track) — pure static lookup, no async"
  - "session_type serialized to lowercase debug string: format!(\"{:?}\", lap.session_type).to_lowercase()"

requirements-completed: [LAP-01, LAP-02, LAP-03]

# Metrics
duration: 9min
completed: 2026-03-16
---

# Phase 26 Plan 02: Lap Filter + Session Type Summary

**LapData gains required session_type field; catalog adds per-track minimum lap time floors; persist_lap sets review_required=1 for below-floor laps with idempotent DB migration**

## Performance

- **Duration:** 9 min
- **Started:** 2026-03-16T13:27:10Z
- **Completed:** 2026-03-16T13:36:23Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- LapData.session_type: SessionType is now a required field — all 12 construction sites updated
- AC adapter hardcodes Practice (correct per AC shared memory constraints), F1 adapter maps Packet 1 session_type integer
- catalog.rs: TrackEntry gains min_lap_time_ms with Monza=80s, Silverstone=90s, Spa=120s floors
- persist_lap: idempotent ALTER TABLE migration + session_type in INSERT + review_required UPDATE post-INSERT
- All 4 Wave 0 lap_tracker stubs GREEN (LAP-01 gate audit, LAP-02 floor check, LAP-02 above-floor, LAP-03 session_type)

## Task Commits

Each task was committed atomically:

1. **Task 1: LapData.session_type + adapter wiring (LAP-03)** - `43ae039` (feat)
2. **Task 2: catalog min_lap_time_ms + persist_lap review_required (LAP-01, LAP-02)** - `98eb3e5` (feat)

**Plan metadata:** (committed with state updates)

## Files Created/Modified
- `crates/rc-common/src/types.rs` - LapData gains `pub session_type: SessionType` (required, after valid field)
- `crates/rc-agent/src/sims/assetto_corsa.rs` - LapData construction: session_type: SessionType::Practice
- `crates/rc-agent/src/sims/f1_25.rs` - LapData construction: session_type mapped from self.session_type u8
- `crates/racecontrol/src/catalog.rs` - TrackEntry.min_lap_time_ms + get_min_lap_time_ms_for_track()
- `crates/racecontrol/src/lap_tracker.rs` - DB migration + session_type INSERT + review_required UPDATE + stubs GREEN
- `crates/racecontrol/tests/integration.rs` - All 10 LapData construction sites gain session_type: SessionType::Practice

## Decisions Made
- session_type is non-optional on LapData — forces all construction sites to explicitly set it
- AC adapter hardcodes SessionType::Practice (AC shared memory exposes no session type field)
- F1 adapter maps self.session_type (u8 from Packet 1 parse) to SessionType at lap construction
- review_required is a post-INSERT UPDATE (not a column default) — computed from catalog static data
- Lap filter philosophy: game-reported valid=false is hard gate; review_required is advisory staff flag

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Updated all 10 integration.rs LapData construction sites**
- **Found during:** Task 1 (LapData.session_type addition)
- **Issue:** integration.rs had 10 LapData constructions that would fail to compile without session_type
- **Fix:** Added session_type: rc_common::types::SessionType::Practice to all 10 integration test LapData structs
- **Files modified:** crates/racecontrol/tests/integration.rs
- **Verification:** cargo check -p racecontrol-crate passes with no "missing field" errors
- **Committed in:** 43ae039 (Task 1 commit, via prior session)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Necessary to update all LapData construction sites when adding a required field.

## Issues Encountered
- Previous execution had partially implemented the plan (43ae039 committed Task 1, catalog.rs and lap_tracker.rs had unstaged Task 2 changes). This execution completed Task 2 and committed it.
- rc-agent-crate `cargo test` hangs when executed on James's machine (lock_screen/overlay GUI code tries to initialize Windows UI). Build succeeds (`cargo test --no-run`), compile is verified clean.

## Next Phase Readiness
- LAP-01/02/03 complete — review_required flag is live in DB for below-floor laps
- Wave 0 stubs for MULTI-01 (multiplayer) and TELEM-01 (telemetry) remain RED — planned for Plans 26-04 and 26-05
- Plan 26-03 (PIN security) was already completed out-of-order (commit c4e47f5)

## Self-Check: PASSED

- FOUND: crates/rc-common/src/types.rs
- FOUND: crates/racecontrol/src/catalog.rs
- FOUND: crates/racecontrol/src/lap_tracker.rs
- FOUND: commit 43ae039 (feat: LapData.session_type field + adapter wiring)
- FOUND: commit 98eb3e5 (feat: catalog min_lap_time_ms + persist_lap review_required)

---
*Phase: 26-lap-filter-pin-security-telemetry-multiplayer*
*Completed: 2026-03-16*
