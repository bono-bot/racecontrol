---
phase: 88-leaderboard-integration
plan: 01
subsystem: database
tags: [sqlite, rust, lap-tracker, catalog, track-normalization, sim-type, leaderboard, migration]

requires:
  - phase: 82-87-multi-game-adapters
    provides: sim_type stored in laps table via format!("{:?}", SimType::X).to_lowercase()

provides:
  - normalize_track_name(sim_type, raw_track) function in catalog.rs with cross-game mapping HashMap
  - personal_bests PRIMARY KEY extended to (driver_id, track, car, sim_type)
  - track_records PRIMARY KEY extended to (track, car, sim_type)
  - migrate_leaderboard_sim_type() idempotent migration function for existing DBs
  - sim_type scoping in all personal_bests and track_records SQL queries in persist_lap

affects:
  - 88-02-leaderboard-endpoints
  - any future phase reading personal_bests or track_records

tech-stack:
  added: [std::sync::LazyLock (Rust 1.80+, stdlib)]
  patterns:
    - v2-table rebuild pattern for SQLite PRIMARY KEY changes
    - TRACK_NAME_MAP static LazyLock<HashMap<(String,String), &'static str>> for cross-game normalization
    - sim_type_str computed once at top of persist_lap and reused for all queries

key-files:
  created: []
  modified:
    - crates/racecontrol/src/catalog.rs
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/lap_tracker.rs
    - crates/racecontrol/tests/integration.rs

key-decisions:
  - "TRACK_NAME_MAP keys use Debug-lowercased format (assettoCorsa, f125, iracing) not serde snake_case — must match stored DB values from format!(\"{:?}\", SimType::X).to_lowercase()"
  - "Migration DEFAULT is 'assettoCorsa' (not 'assetto_corsa') — matches stored format in live DB"
  - "normalize_track_name uses passthrough for unknown combos — never blocks lap storage"
  - "get_previous_record_holder gains sim_type parameter — track record emails are now per-game"
  - "normalized_track used for all downstream writes: laps INSERT, PB queries, TR queries, passport, hotlap event auto-entry"

patterns-established:
  - "Pattern: v2-table migration — CREATE v2, INSERT SELECT with DEFAULT, DROP old, RENAME v2. Guard with pragma_table_info check."
  - "Pattern: sim_type_str computed once via format!(\"{:?}\", lap.sim_type).to_lowercase() at top of persist_lap — reused everywhere"
  - "Pattern: normalize before persist — track name normalization happens in persist_lap before any DB write"

requirements-completed: [LB-01, LB-02]

duration: 11min
completed: 2026-03-21
---

# Phase 88 Plan 01: Leaderboard Integration Summary

**Cross-game track name normalization via TRACK_NAME_MAP + sim_type-scoped personal_bests and track_records PRIMARY KEYs with idempotent v2-table SQLite migration**

## Performance

- **Duration:** 11 min
- **Started:** 2026-03-21T07:57:44Z (13:27 IST)
- **Completed:** 2026-03-21T08:08:44Z (13:38 IST)
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- Added `normalize_track_name(sim_type, raw_track) -> String` in catalog.rs with static `TRACK_NAME_MAP` containing 28 cross-game mappings (F1 25, iRacing, LMU, Forza) keyed in Debug-lowercased format
- Migrated personal_bests and track_records to include sim_type in PRIMARY KEY via idempotent v2-table rebuild pattern; existing AC rows get DEFAULT 'assettoCorsa'
- Wired normalization and sim_type scoping into persist_lap — normalized_track used for all DB writes, PB/TR queries now scoped by sim_type, get_previous_record_holder gains sim_type param

## Task Commits

1. **Task 1: Track name normalization + sim_type schema migration** - `8ab3775` (feat)
2. **Task 2: Wire normalization and sim_type scoping into persist_lap** - `c754a9c` (feat)

## Files Created/Modified

- `crates/racecontrol/src/catalog.rs` - TRACK_NAME_MAP static HashMap, normalize_track_name(), unit test
- `crates/racecontrol/src/db/mod.rs` - personal_bests/track_records CREATE TABLE with sim_type in PK, migrate_leaderboard_sim_type() migration function
- `crates/racecontrol/src/lap_tracker.rs` - sim_type_str computed once, normalized_track wired through all queries, get_previous_record_holder gets sim_type param
- `crates/racecontrol/tests/integration.rs` - 3 call sites updated to pass sim_type="assettoCorsa" to get_previous_record_holder

## Decisions Made

- **Key pitfall avoided:** TRACK_NAME_MAP keys use Debug-lowercased format (`"assettoCorsa"`, `"f125"`, `"iracing"`) — not serde snake_case (`"assetto_corsa"`). The stored DB format is `format!("{:?}", SimType::X).to_lowercase()`. Using the wrong format would cause all lookups to miss.
- **Migration DEFAULT is 'assettoCorsa'**: Matches what is actually stored in the live DB for existing AC laps.
- **Passthrough for unknown:** `normalize_track_name` returns `raw_track.to_string()` for any unknown combination — lap storage is never blocked by a missing mapping.
- **normalized_track propagated everywhere:** After normalization, all downstream references (laps INSERT, LAP-02 check, PB queries, TR queries, passport, hotlap events) use `normalized_track` — raw `lap.track` is only used for normalization input.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Test module name conflict in catalog.rs**
- **Found during:** Task 1 verification
- **Issue:** Existing `mod tests` at line 907 in catalog.rs conflicted with new `mod tests` block added for normalize_track_name test
- **Fix:** Renamed new test module from `tests` to `catalog_normalize_tests`
- **Files modified:** crates/racecontrol/src/catalog.rs
- **Verification:** `cargo test --lib -- normalize_track_name` passes (1 test ok)
- **Committed in:** 8ab3775 (Task 1 commit)

**2. [Rule 3 - Blocking] Integration test call sites for get_previous_record_holder**
- **Found during:** Task 2 (after updating function signature)
- **Issue:** 3 call sites in tests/integration.rs used old 3-arg signature; new signature requires sim_type as 4th arg
- **Fix:** Added `"assettoCorsa"` as 4th argument to all 3 call sites (all test laps use AssettoCorsa)
- **Files modified:** crates/racecontrol/tests/integration.rs
- **Verification:** `cargo build --release --bin racecontrol` succeeds cleanly
- **Committed in:** c754a9c (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both auto-fixes necessary for compilation. No scope creep.

## Issues Encountered

None beyond the two auto-fixed blocking issues above.

## Next Phase Readiness

- Plan 88-02 can now add sim_type filtering to leaderboard endpoints
- TRACK_NAME_MAP is extensible — add new game mappings as they are discovered
- Migration is idempotent — safe to deploy to production server at 192.168.31.23

---
*Phase: 88-leaderboard-integration*
*Completed: 2026-03-21*
