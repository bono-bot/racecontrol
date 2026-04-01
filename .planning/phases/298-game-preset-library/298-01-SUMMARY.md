---
phase: 298-game-preset-library
plan: "01"
subsystem: api
tags: [rust, axum, sqlite, sqlx, websocket, preset-library, reliability-scoring]

requires:
  - phase: 296-server-pushed-config
    provides: push_full_config_to_pod pattern, CoreToAgentMessage enum, WS Register handler wiring point

provides:
  - GamePreset struct (rc-common/types.rs)
  - GamePresetWithReliability struct with reliability_score, total_launches, flagged_unreliable
  - PresetPushPayload struct for WS push
  - CoreToAgentMessage::PresetPush WS variant
  - game_presets SQLite table migration
  - PresetsConfig with unreliable_threshold=0.6 default
  - preset_library.rs module: list_presets_with_reliability, push_presets_to_pod, REST CRUD handlers
  - GET /api/v1/presets (public), POST/PUT/DELETE /api/v1/presets (staff JWT)
  - WS push on pod connect after push_full_config_to_pod

affects: [298-02, ws/mod.rs, api/routes.rs, db migrations, rc-common types]

tech-stack:
  added: []
  patterns:
    - "GamePreset uses manual row mapping (not sqlx::FromRow) because rc-common has no sqlx dep"
    - "GET preset routes in public_routes (pods/kiosk need them without JWT), writes in staff_routes"
    - "Reliability scoring: AVG(success_rate) from combo_reliability with >= 5 launch minimum"

key-files:
  created:
    - crates/racecontrol/src/preset_library.rs
  modified:
    - crates/rc-common/src/types.rs
    - crates/rc-common/src/protocol.rs
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/config.rs
    - crates/racecontrol/src/lib.rs
    - crates/racecontrol/src/ws/mod.rs
    - crates/racecontrol/src/api/routes.rs

key-decisions:
  - "GamePreset does NOT derive sqlx::FromRow — rc-common has no sqlx dependency, rows mapped manually in preset_library.rs"
  - "GET /presets placed in public_routes so pods/kiosk can read presets without JWT (same as config/kiosk-allowlist pattern)"
  - "Reliability threshold 0.6 (60%) with minimum 5 launches matches INTEL-02 convention from combo_reliability"
  - "push_presets_to_pod called after push_full_config_to_pod in WS Register handler — non-fatal, logs warn on error"

requirements-completed: [PRESET-01, PRESET-02, PRESET-03]

duration: 20min
completed: "2026-04-01"
---

# Phase 298 Plan 01: Game Preset Library Backend Summary

**SQLite-backed game preset library with reliability scoring from combo_reliability, WS push on pod connect, and REST CRUD endpoints for admin management**

## Performance

- **Duration:** ~20 min
- **Started:** 2026-04-01T15:18:00Z
- **Completed:** 2026-04-01T15:37:00Z
- **Tasks:** 2
- **Files modified:** 7 + 1 created

## Accomplishments

- Added `GamePreset`, `GamePresetWithReliability`, `PresetPushPayload` to rc-common shared types
- Added `CoreToAgentMessage::PresetPush` WS variant
- Created `game_presets` SQLite table with indexes (migration in db/mod.rs)
- Created `preset_library.rs` with full CRUD, reliability scoring via LEFT JOIN on combo_reliability, and WS push
- Wired `push_presets_to_pod` into WS Register handler so pods receive presets on connect
- Registered REST routes: GET (public), POST/PUT/DELETE (staff JWT)
- 8 unit tests pass (4 in rc-common, 4 in preset_library)
- Release build succeeds

## Task Commits

1. **Task 1: GamePreset types, DB migration, PresetsConfig** - `95ef5f5f` (feat)
2. **Task 2: preset_library.rs, WS wiring, REST routes** - `8b64bf77` (feat)

## Files Created/Modified

- `crates/rc-common/src/types.rs` - Added GamePreset, GamePresetWithReliability, PresetPushPayload structs + 4 tests
- `crates/rc-common/src/protocol.rs` - Added CoreToAgentMessage::PresetPush variant
- `crates/racecontrol/src/db/mod.rs` - Added game_presets table migration + 2 indexes
- `crates/racecontrol/src/config.rs` - Added PresetsConfig struct, field on Config, default initializer
- `crates/racecontrol/src/preset_library.rs` - NEW: full preset library module
- `crates/racecontrol/src/lib.rs` - Added pub mod preset_library
- `crates/racecontrol/src/ws/mod.rs` - Wired push_presets_to_pod in Register handler
- `crates/racecontrol/src/api/routes.rs` - Added preset_library import + GET in public_routes + writes in staff_routes

## Decisions Made

- `GamePreset` does not derive `sqlx::FromRow` — `rc-common` has no sqlx dependency. Row mapping is done manually in `preset_library.rs` via `sqlx::Row::try_get`.
- GET routes for presets placed in `public_routes` so pods and kiosk can read without JWT (same pattern as `/config/kiosk-allowlist`).
- Reliability threshold 0.6 (60%) with minimum 5 launches matches the INTEL-02 convention already established in `combo_reliability`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Removed sqlx::FromRow derive from rc-common types**
- **Found during:** Task 1 (compile error)
- **Issue:** Plan specified `#[derive(sqlx::FromRow)]` on `GamePreset` in `rc-common/types.rs`, but `rc-common` has no sqlx dependency — would not compile
- **Fix:** Removed `sqlx::FromRow` derive; added manual row mapping in `preset_library.rs` using `sqlx::Row::try_get`
- **Files modified:** `crates/rc-common/src/types.rs`, `crates/racecontrol/src/preset_library.rs`
- **Committed in:** 95ef5f5f + 8b64bf77

**2. [Rule 1 - Bug] Fixed pre-existing test: CoreToAgentMessage uses snake_case type tags**
- **Found during:** Task 1 (cargo test failure)
- **Issue:** Pre-existing test at types.rs:1667 asserted `"PresetPush"` (PascalCase) but `CoreToAgentMessage` uses `rename_all = "snake_case"` so the actual type tag is `"preset_push"`
- **Fix:** Changed assertion to `"preset_push"` in both the pre-existing test and the new test
- **Files modified:** `crates/rc-common/src/types.rs`
- **Committed in:** 95ef5f5f

**3. [Rule 1 - Bug] Added missing PresetsConfig to Config default_config() initializer**
- **Found during:** Task 2 (compile error)
- **Issue:** `Config::default_config()` is a struct literal that listed all fields — after adding `presets: PresetsConfig` to the `Config` struct, the initializer was missing the field
- **Fix:** Added `presets: PresetsConfig::default()` to `default_config()`
- **Files modified:** `crates/racecontrol/src/config.rs`
- **Committed in:** 8b64bf77

---

**Total deviations:** 3 auto-fixed (3 bugs — compile errors and test correctness)
**Impact on plan:** All fixes necessary for correctness. No scope creep.

## Issues Encountered

Integration tests (`tests/integration.rs`) have pre-existing compile errors (`BillingTimer` missing `nonce` field) — these are out of scope and not caused by this plan. Unit tests and library tests all pass cleanly.

## Known Stubs

None — all endpoints are fully wired to the database. `GET /api/v1/presets` returns `[]` on a fresh database (correct behavior, not a stub).

## Next Phase Readiness

- Backend preset library complete. Plan 298-02 can proceed with admin UI.
- `GET /api/v1/presets` is public and returns `[]` on empty DB — admin UI can start with empty state.
- Reliability scores will auto-populate once combo_reliability has data from game launches.

---
*Phase: 298-game-preset-library*
*Completed: 2026-04-01*
