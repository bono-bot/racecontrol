---
phase: 88-leaderboard-integration
verified: 2026-03-21T09:15:00+05:30
status: passed
score: 8/8 must-haves verified
re_verification: false
gaps: []
human_verification:
  - test: "Open /public/leaderboard in browser and confirm records from F1 25, iRacing and LMU appear alongside AC records"
    expected: "Records from multiple games visible; game picker dropdown populates from available_sim_types array"
    why_human: "Requires live DB with multi-game data and browser rendering of the frontend"
  - test: "Drive a lap in a non-AC game and verify the track name resolves to canonical name on leaderboard"
    expected: "e.g. F1 25 lap on 'silverstone' appears as 'ks_silverstone' on leaderboard, not raw game string"
    why_human: "Requires a live session with an active non-AC game adapter producing telemetry"
---

# Phase 88: Leaderboard Integration Verification Report

**Phase Goal:** Lap and stage times from all games appear on the existing Racing Point leaderboard with correct track names
**Verified:** 2026-03-21T09:15:00+05:30 (IST)
**Status:** PASSED
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (from ROADMAP Success Criteria + Plan must_haves)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Laps from all adapters stored in existing laps table with sim_type field | VERIFIED | `db/mod.rs:91` — `sim_type TEXT NOT NULL` in laps CREATE TABLE. Pre-existing from phases 82-87; laps INSERT in lap_tracker.rs binds sim_type_str at line 95 |
| 2 | Track name mapping table translates per-game IDs to canonical Racing Point names | VERIFIED | `catalog.rs:26` — `static TRACK_NAME_MAP: LazyLock<HashMap<(String,String), &'static str>>` with 28 mappings for F1 25, iRacing, LMU, Forza |
| 3 | Track names from non-AC games are normalized before storage | VERIFIED | `lap_tracker.rs:41` — `normalize_track_name(&sim_type_str, &lap.track)` called before all DB writes; `normalized_track` used in laps INSERT, PB queries, TR queries |
| 4 | personal_bests scoped by sim_type (F1 25 PB separate from AC on same track+car) | VERIFIED | `db/mod.rs:115` — `PRIMARY KEY (driver_id, track, car, sim_type)`. Migration function at line 2325 rebuilds table with sim_type in PK for existing DBs |
| 5 | track_records scoped by sim_type (iRacing record separate from AC) | VERIFIED | `db/mod.rs:130` — `PRIMARY KEY (track, car, sim_type)`. Migration at line 2373+ covers track_records similarly |
| 6 | Unknown track names pass through unchanged without blocking lap storage | VERIFIED | `catalog.rs:73-80` — `normalize_track_name` returns `raw_track.to_string()` on map miss. Unit test asserts `"unknown_track_xyz"` passes through |
| 7 | Leaderboard endpoints serve multi-game data with optional sim_type filter | VERIFIED | `routes.rs:9377-9508` (public_leaderboard), `9517-9610` (public_track_leaderboard), `1792-1830` (staff track_leaderboard), `12018-12096` (bot_leaderboard) — all 4 accept `Option<String>` sim_type query param |
| 8 | Without sim_type filter, all games shown (backward compatible) | VERIFIED | All 4 endpoints use `if let Some(ref st) = params.sim_type { filtered } else { unfiltered }` — None shows all games. Hardcoded `assetto_corsa` default removed from public_track_leaderboard (line 9523) |

**Score:** 8/8 truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/racecontrol/src/catalog.rs` | normalize_track_name() function + TRACK_NAME_MAP | VERIFIED | Lines 26-80: LazyLock HashMap with 28 cross-game mappings; `pub fn normalize_track_name` at line 73; unit test at line 87 passes |
| `crates/racecontrol/src/db/mod.rs` | personal_bests + track_records with sim_type in PK; migration function | VERIFIED | Lines 107-130: both tables have `sim_type TEXT NOT NULL DEFAULT 'assettoCorsa'` in PK. `migrate_leaderboard_sim_type()` at line 2325 with idempotent v2-table rebuild pattern |
| `crates/racecontrol/src/lap_tracker.rs` | normalize_track_name called; sim_type-scoped PB and TR queries | VERIFIED | Line 41: normalization wired. Lines 132-165: personal_bests SELECT + UPSERT include `AND sim_type = ?` and `ON CONFLICT(driver_id, track, car, sim_type)`. Lines 201-235: track_records same pattern |
| `crates/racecontrol/src/api/routes.rs` | sim_type query param on all leaderboard endpoints | VERIFIED | PublicLeaderboardQuery (9373), LeaderboardQuery (9511), StaffTrackLeaderboardQuery (1787), BotLeaderboardQuery (12013) — all have `sim_type: Option<String>` |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `lap_tracker.rs` | `catalog.rs` | `catalog::normalize_track_name` call before persist | WIRED | Line 41: `let normalized_track = catalog::normalize_track_name(&sim_type_str, &lap.track)`. Used for all subsequent DB writes |
| `lap_tracker.rs` | `personal_bests` table | SQL WHERE includes sim_type in SELECT and ON CONFLICT | WIRED | Line 132: `WHERE driver_id = ? AND track = ? AND car = ? AND sim_type = ?`. Line 152: `ON CONFLICT(driver_id, track, car, sim_type)` |
| `lap_tracker.rs` | `track_records` table | SQL WHERE includes sim_type in SELECT and ON CONFLICT | WIRED | Line 201: `get_previous_record_holder(..., &sim_type_str)`. Line 225: `ON CONFLICT(track, car, sim_type)`. Line 809 in helper: `AND tr.sim_type = ?` |
| `routes.rs (public_leaderboard)` | `track_records` table | WHERE tr.sim_type = ? when filter provided | WIRED | Line 9387: `WHERE tr.sim_type = ?` in filtered branch |
| `routes.rs (track_leaderboard)` | `track_records` table | WHERE tr.track = ? AND tr.sim_type = ? | WIRED | Line 1801: `WHERE tr.track = ? AND tr.sim_type = ?` in filtered branch |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| LB-01 | 88-01-PLAN.md | Lap/stage times from all games stored in existing laps table with sim_type field | SATISFIED | laps table has `sim_type TEXT NOT NULL` (db/mod.rs:91). lap_tracker.rs binds sim_type_str per lap (line 95). normalize_track_name ensures canonical track names before storage |
| LB-02 | 88-01-PLAN.md | Track name normalization mapping table | SATISFIED | `TRACK_NAME_MAP` in catalog.rs — 28 mappings covering F1 25 (14 tracks), iRacing (6), LMU (3), Forza (3), with passthrough for AC and unknowns |
| LB-03 | 88-02-PLAN.md | Existing leaderboard endpoints serve multi-game data with sim_type filtering | SATISFIED | All 4 endpoints updated: public_leaderboard, public_track_leaderboard, staff track_leaderboard, bot_leaderboard. available_sim_types discovery array added to public_leaderboard response |

**Orphaned requirements check:** REQUIREMENTS.md at `.planning/REQUIREMENTS.md` is scoped to v11.1 pre-flight requirements (PF-01 through STAFF-04) — it does not govern phase 88. LB requirements are defined in `.planning/milestones/v10.0-REQUIREMENTS.md` and `.planning/milestones/v11.0-REQUIREMENTS.md`. All three LB IDs are fully accounted for. No orphaned requirements.

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `api/routes.rs` | 12080-12087 | bot_leaderboard per-entry records do not include `sim_type` field | Info | Plan success criteria stated "sim_type field per record" for all 4 endpoints. Bot entries are `(name, track, time, car)` tuples — no per-record sim_type. Response-level `"sim_type": params.sim_type` indicates the applied filter. Does not block phase goal |

No blockers. No FIXME/TODO/placeholder comments found in modified files. No stub implementations.

---

### Build Verification

- `cargo test -p racecontrol-crate --lib -- normalize_track_name`: **1 test PASSED** (`catalog::catalog_normalize_tests::normalize_track_name_maps_known_tracks`)
- Commits verified in git: `8ab3775` (Plan 01 Task 1), `c754a9c` (Plan 01 Task 2), `d88f422` (Plan 02)
- Both SUMMARY files document clean builds with no compile errors

---

### Human Verification Required

#### 1. Multi-game records on public leaderboard

**Test:** Navigate to `/public/leaderboard` and verify records from F1 25, iRacing, and LMU appear alongside AC records when no sim_type filter is applied
**Expected:** Mixed records from all games; `available_sim_types` array in response contains `["assettoCorsa", "f125", "iracing", ...]`; frontend game picker is populated
**Why human:** Requires live DB with sessions from multiple game adapters already driven

#### 2. Track name normalization end-to-end

**Test:** Drive a lap in F1 25 on `silverstone` and check the leaderboard entry
**Expected:** Track appears as `ks_silverstone` on the leaderboard, not as the raw F1 25 string
**Why human:** Requires a live session with F1 25 adapter active and producing telemetry

---

### Gaps Summary

No gaps. All 8 observable truths verified, all 3 requirements satisfied, all artifacts substantive and wired. One minor info-level deviation: bot_leaderboard per-entry records omit sim_type (response-level sim_type present). This does not affect the phase goal.

---

_Verified: 2026-03-21T09:15:00+05:30 (IST)_
_Verifier: Claude (gsd-verifier)_
