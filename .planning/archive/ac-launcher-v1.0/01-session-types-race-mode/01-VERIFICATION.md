---
phase: 01-session-types-race-mode
verified: 2026-03-13T07:00:00Z
status: passed
score: 5/5 must-haves verified
re_verification: false
human_verification:
  - test: "Launch Race vs AI on Pod 8 and verify AI cars appear on the grid"
    expected: "AI opponent cars visible and driving on grid, count matches configured grid size"
    why_human: "Requires AC installed on pod, RC-agent deployed, actual game runtime"
  - test: "Launch Race Weekend on Pod 8 and verify session transitions P->Q->R"
    expected: "Game sequences through Practice, Qualifying, then Race sessions automatically"
    why_human: "Multi-session sequencing is AC runtime behavior, cannot verify from INI content alone"
  - test: "Launch Track Day on Pod 8 and verify mixed-class AI traffic"
    expected: "12 AI cars from different GT3/supercar classes present and driving"
    why_human: "Requires AC runtime and installed car content to confirm all models load"
---

# Phase 1: Session Types & Race Mode Verification Report

**Phase Goal:** Customers can launch every single-player session type from the system, including the core missing feature -- racing against AI opponents with a configurable grid
**Verified:** 2026-03-13T07:00:00Z
**Status:** passed
**Re-verification:** No -- initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Customer can launch Practice mode and hot-lap solo on any track | VERIFIED | `write_session_blocks` generates TYPE=1/SPAWN_SET=PIT for `session_type="practice"`. `test_write_race_ini_practice` asserts TYPE=1, CARS=1, no CAR_1. |
| 2 | Customer can launch Race vs AI mode and race against a configurable number of AI opponents | VERIFIED | `write_session_blocks` generates TYPE=3 for `session_type="race"`. `write_ai_car_sections` generates [CAR_1]..[CAR_N] with AI=1. 19-car cap enforced in `effective_ai_cars`. Tests confirm 5 AI, 19 AI, 25 AI clamped to 19. |
| 3 | Customer can launch Hotlap mode with timed lap tracking | VERIFIED | `write_session_blocks` generates TYPE=4/SPAWN_SET=START for `session_type="hotlap"`. `test_write_race_ini_hotlap` asserts TYPE=4, SPAWN_SET=START, CARS=1. |
| 4 | Customer can launch Track Day mode with mixed AI traffic | VERIFIED | `effective_ai_cars` generates 12 default AI from `TRACKDAY_CAR_POOL` when `session_type="trackday"` and `ai_cars` is empty. `write_session_blocks` generates TYPE=1. Tests confirm 12 cars, mixed models, all AI=1. |
| 5 | Customer can launch Race Weekend mode that sequences through Practice, Qualify, and Race | VERIFIED | `write_session_blocks` "weekend" arm generates SESSION_0 TYPE=1, SESSION_1 TYPE=2, SESSION_2 TYPE=3. Sessions are skippable (practice_minutes=0 omits SESSION_0). Race gets remaining time with min 1 minute. Tests cover all cases. |

**Score:** 5/5 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-agent/src/ac_launcher.rs` | AiCarSlot struct, extended AcLaunchParams, composable INI builder, AI name pool | VERIFIED | File exists (25K+ tokens). Contains `pub struct AiCarSlot`, 6 new AcLaunchParams fields all with `serde(default)`, `build_race_ini_string()`, `write_race_ini()`, `AI_DRIVER_NAMES` (60 names), `pick_ai_names()`, `TRACKDAY_CAR_POOL`, `effective_ai_cars()`, 40 unit tests. |
| `crates/rc-agent/src/ac_launcher.rs` | Unit tests for practice, hotlap, and no-fallback verification | VERIFIED | `test_write_race_ini_practice` at line 1404, `test_write_race_ini_hotlap` at line 1421, `test_write_race_ini_no_phantom_ai` at line 1499 all exist. 40 test functions total confirmed. |
| `crates/rc-agent/Cargo.toml` | rand dependency for AI name shuffling | VERIFIED | Line 48: `rand = "0.8"` with comment "Random selection for AI driver name pool". |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `AcLaunchParams.session_type` | `write_session_blocks()` | `match params.session_type.as_str()` dispatches to practice/hotlap/race/trackday/weekend arms | WIRED | Lines 572-613. All 5 session types have explicit dispatch paths. Default falls through to Practice TYPE=1. |
| `AcLaunchParams.ai_cars` | `write_ai_car_sections()` | `effective_ai_cars(params)` returns capped/defaulted list, passed to `write_ai_car_sections(&mut ini, &ai_cars)` | WIRED | Lines 637 and 643 in `build_race_ini_string`. `effective_ai_cars` handles trackday defaults and 19-cap. `write_ai_car_sections` iterates and writes [CAR_N] AI=1. |
| `write_race_config_section()` | `[RACE] CARS=` field | `let total_cars = 1 + ai_count` where `ai_count = ai_cars.len()` from `effective_ai_cars` | WIRED | Line 517. Both `write_race_config_section` and `write_ai_car_sections` receive the same `ai_cars` slice from `build_race_ini_string`, ensuring CARS count matches actual CAR section count. |
| `AcLaunchParams` (from `launch_args` JSON) | `write_race_ini()` | `LaunchGame` message in `main.rs` deserializes `launch_args` into `AcLaunchParams`, then calls `ac_launcher::launch_ac(&params)` which calls `write_race_ini(params)?` at line 174 | WIRED | `main.rs` lines 903-995. Fallback struct literal also includes all 6 new fields with correct defaults. |
| `AcLaunchParams.session_type == "weekend"` | `write_session_blocks()` SESSION_0/1/2 | `match "weekend"` arm with `session_index` counter and `saturating_sub().max(1)` for race time | WIRED | Lines 586-608. Skippable sessions (minutes=0) correctly omit the block and advance `session_index`. |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| SESS-01 | 01-01-PLAN | Customer can select Practice mode (solo hot-lapping, no AI) | SATISFIED | TYPE=1, CARS=1, no CAR_1 section. `test_write_race_ini_practice` + `test_write_race_ini_solo_cars_count` verify. |
| SESS-02 | 01-02-PLAN | Customer can select Race vs AI mode with configurable grid size | SATISFIED | TYPE=3 session, [CAR_1]..[CAR_N] with AI=1, CARS=1+N. 8 dedicated tests: 5-AI grid, 0-AI, 19-AI max, 25-AI clamped, model/name/skin/AI_LEVEL/RACE_LAPS/formation_lap/starting_position. |
| SESS-03 | 01-01-PLAN | Customer can select Hotlap mode (timed laps) | SATISFIED | TYPE=4, SPAWN_SET=START, CARS=1. `test_write_race_ini_hotlap` verifies. |
| SESS-04 | 01-02-PLAN | Customer can select Track Day mode (open pit, mixed traffic) | SATISFIED | TYPE=1 with 12 default mixed-class AI from TRACKDAY_CAR_POOL, or explicit AI. 5 tests verify default count, mixed models, TYPE=1, explicit override, all AI=1. |
| SESS-05 | 01-02-PLAN | Customer can select Race Weekend mode (P->Q->R sequence) | SATISFIED | SESSION_0 TYPE=1, SESSION_1 TYPE=2, SESSION_2 TYPE=3. Skippable sessions, time allocation math, starting position, AI grid, insufficient-time minimum. 8 tests verify all cases. |
| SESS-08 | 01-01-PLAN | Game launches with exact config -- no silent fallbacks | SATISFIED | `effective_ai_cars()` never synthesizes phantom AI for race mode with empty ai_cars. `test_write_race_ini_no_phantom_ai` explicitly tests SESS-08. CARS=1 when ai_cars is empty for race. Weekend race minimum 1 minute tested. |

No orphaned requirements found. All 6 phase-1 requirements (SESS-01 through SESS-05, SESS-08) are claimed by the plans and verified in code.

### Anti-Patterns Found

No anti-patterns detected. Full scan of `crates/rc-agent/src/ac_launcher.rs` found:
- Zero TODO/FIXME/XXX/HACK/PLACEHOLDER comments
- No stub return values (return null, return {}, return [])
- No console.log-only implementations
- No empty handlers

### Human Verification Required

The following items pass all automated checks but require on-pod validation to fully confirm goal achievement:

#### 1. Race vs AI on-pod test

**Test:** Deploy current rc-agent to Pod 8, launch AC with `session_type="race"` and 5 AI cars configured
**Expected:** 5 AI opponent cars visible on grid at race start, driving correctly
**Why human:** The INI content is verified correct by unit tests. Whether AC actually loads all car models from TRACKDAY_CAR_POOL (installed content) and whether AI pathfinding works on the selected track requires AC runtime.

#### 2. Race Weekend session transition

**Test:** Deploy to Pod 8, launch Race Weekend with 10min Practice, 10min Qualifying, 30min duration
**Expected:** AC cycles through Practice session, then Qualifying, then Race in sequence; qualifying times carry into race grid
**Why human:** Multi-session INI format is verified correct. Actual AC behavior sequencing through multiple SESSION_N blocks is not testable from INI content alone.

#### 3. Track Day mixed-class traffic

**Test:** Deploy to Pod 8, launch Track Day with no custom AI cars specified
**Expected:** 12 AI cars appear on track from different car classes (not all the same model)
**Why human:** `TRACKDAY_CAR_POOL` car models must be installed on the pod. If any model is missing, AC may fail to load that car silently. Requires visual confirmation on-pod.

### Gaps Summary

No gaps found. All 5 success criteria from ROADMAP.md are met by the implementation:

1. Practice mode: TYPE=1, solo, no AI -- fully implemented and tested
2. Race vs AI: TYPE=3, configurable AI grid, 19-car cap -- fully implemented and tested
3. Hotlap mode: TYPE=4, SPAWN_SET=START -- fully implemented and tested
4. Track Day: TYPE=1 with mixed AI traffic (12 default from GT3/supercar pool) -- fully implemented and tested
5. Race Weekend: SESSION_0 TYPE=1 + SESSION_1 TYPE=2 + SESSION_2 TYPE=3, time allocation, skippable sessions -- fully implemented and tested

The composable INI builder architecture (`build_race_ini_string` -> 18 section writers) and backward-compatible serde defaults ensure SESS-08 (no silent fallbacks) is satisfied at the infrastructure level. The 40 unit tests across all session types (rc-agent: 100 passed total) provide high confidence in correctness.

Commits verified in racecontrol repo: `ec341f9`, `9708a48`, `707331d` -- all three plan commits present and accounted for.

---

_Verified: 2026-03-13T07:00:00Z_
_Verifier: Claude (gsd-verifier)_
