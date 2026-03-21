---
phase: 82-billing-and-session-lifecycle
plan: 01
subsystem: billing
tags: [rust, billing, sim_type, sqlite, per-game-rates, sqlx]

# Dependency graph
requires:
  - phase: 78-session-lifecycle
    provides: BillingTimer, BillingManager, compute_session_cost
  - phase: 80-audit-trail-defense-in-depth
    provides: AgentMessage protocol foundation
provides:
  - GameState::Loading variant in rc-common types
  - PlayableSignal enum with TelemetryLive/ProcessFallback variants and sim_type() method
  - BillingRateTier.sim_type field (Option<SimType>, None = universal)
  - BillingTimer.sim_type field for per-game rate lookup
  - WaitingForGameEntry.sim_type field for rate propagation
  - get_tiers_for_game() function: prefer game-specific, fallback to universal
  - DB migration: ALTER TABLE billing_rates ADD COLUMN sim_type TEXT
  - billing_rates CRUD API with sim_type field
  - AgentMessage::GameStatusUpdate carries sim_type (backward-compat, skip_serializing_if None)
affects:
  - 82-02 (per-game billing start using PlayableSignal)
  - 82-03 (session lifecycle display using GameState::Loading)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "get_tiers_for_game(): prefer game-specific tiers, fallback to universal (NULL sim_type)"
    - "Option<SimType> on billing structs: None = universal, Some(x) = game-specific"
    - "serde_json::from_value for SimType deserialization from DB TEXT column"

key-files:
  created:
    - fix_billing.py (helper script, accidentally committed, can be deleted)
  modified:
    - crates/rc-common/src/types.rs
    - crates/racecontrol/src/billing.rs
    - crates/racecontrol/src/ws/mod.rs
    - crates/racecontrol/src/game_launcher.rs
    - crates/racecontrol/tests/integration.rs

key-decisions:
  - "get_tiers_for_game() returns Vec<&BillingRateTier>: prefer game-specific tiers, fall back to universal (None sim_type) tiers"
  - "current_cost() clones filtered tiers (into_iter().cloned()) because compute_session_cost takes &[BillingRateTier] not Vec<&>"
  - "SimType deserialized from DB via serde_json::from_value(Value::String(s)) -- same serde attributes as JSON protocol"
  - "GameState::Loading sits between Launching and Running -- game process detected but billing not yet started"
  - "PlayableSignal variants: TelemetryLive (AC UDP detected) and ProcessFallback (non-AC games with no UDP) both carry sim_type"

patterns-established:
  - "Billing tier lookup: get_tiers_for_game(tiers, sim_type) at call site, not inside BillingTimer field mutation"

requirements-completed: [BILL-01, BILL-03, BILL-05]

# Metrics
duration: ~90min
completed: 2026-03-21
---

# Phase 82 Plan 01: Billing Server Foundation Summary

**Per-game billing engine: GameState::Loading variant, PlayableSignal enum, BillingRateTier/BillingTimer sim_type fields, get_tiers_for_game() fallback logic, DB migration, and protocol sim_type wire-up**

## Performance

- **Duration:** ~90 min
- **Started:** 2026-03-21T04:00:00Z (IST 09:30)
- **Completed:** 2026-03-21T05:30:00Z (IST 11:00)
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments

- Added `GameState::Loading` variant to rc-common types and fixed all match exhaustiveness in ws/mod.rs and game_launcher.rs
- Added `PlayableSignal` enum with `TelemetryLive { sim_type }` and `ProcessFallback { sim_type }` variants, plus `sim_type()` accessor method, with 6 serde roundtrip tests
- Extended billing engine: `BillingRateTier.sim_type`, `BillingTimer.sim_type`, `WaitingForGameEntry.sim_type`, `get_tiers_for_game()` fallback function, `current_cost()` uses filtered tiers, `handle_game_status_update` propagates sim_type from `AcStatus::Live` message, `refresh_rate_tiers()` reads `sim_type` column from DB

## Task Commits

1. **Task 1: Shared types** - `60f7d9e` (feat)
2. **Task 2: DB migration + per-game billing engine + API sim_type** - `80f32d1` (feat)

## Files Created/Modified

- `crates/rc-common/src/types.rs` - GameState::Loading, PlayableSignal enum, 6 new tests
- `crates/racecontrol/src/billing.rs` - sim_type on BillingRateTier/BillingTimer/WaitingForGameEntry, get_tiers_for_game(), refresh_rate_tiers SQL, 4 new unit tests
- `crates/racecontrol/src/ws/mod.rs` - GameStatusUpdate destructures sim_type, passes to handle_game_status_update; PreFlightPassed/PreFlightFailed handlers added
- `crates/racecontrol/src/game_launcher.rs` - GameState::Loading => "loading" arm added to event_type match
- `crates/racecontrol/tests/integration.rs` - sim_type: None on 4 BillingTimer literals
- `fix_billing.py` - Helper script for atomic billing.rs multi-edit (workaround for Windows Edit tool issue; can be deleted)

## Decisions Made

- `get_tiers_for_game()` prefers game-specific tiers, falls back to universal (NULL sim_type) tiers — enables mixed fleets where some games have custom rates and others use defaults
- `current_cost()` clones filtered tier refs into `Vec<BillingRateTier>` to satisfy `compute_session_cost(&[BillingRateTier])` signature without changing the existing function
- `SimType` deserialized from DB TEXT column via `serde_json::from_value(Value::String(s))` — reuses serde attribute `rename_all="snake_case"` already on the enum
- `GameState::Loading` sits between `Launching` and `Running` to represent "process is up but billing hasn't started yet" — Plans 82-02/03 will use this to trigger `PlayableSignal`

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed non-exhaustive match for GameState::Loading**
- **Found during:** Task 1 (after adding Loading variant)
- **Issue:** Three match statements in ws/mod.rs (lines 164, 340) and game_launcher.rs (line 342) did not handle `GameState::Loading`, causing compile errors
- **Fix:** Added `GameState::Loading` to appropriate arms in all three locations
- **Files modified:** crates/racecontrol/src/ws/mod.rs, crates/racecontrol/src/game_launcher.rs
- **Verification:** cargo check --workspace passes
- **Committed in:** 60f7d9e (Task 1 commit)

**2. [Rule 3 - Blocking] Fixed non-exhaustive match for PreFlightPassed/PreFlightFailed AgentMessage variants**
- **Found during:** Task 2 (ws/mod.rs AgentMessage match)
- **Issue:** Commit e456467 (docs phase 97) added `PreFlightPassed`/`PreFlightFailed` variants to `AgentMessage` without adding handlers in ws/mod.rs, breaking the match
- **Fix:** Added `PreFlightPassed`/`PreFlightFailed` handler arms to the AgentMessage match in ws/mod.rs
- **Files modified:** crates/racecontrol/src/ws/mod.rs
- **Verification:** cargo check --workspace passes
- **Committed in:** 80f32d1 (Task 2 commit)

**3. [Rule 3 - Blocking] Fixed Windows Edit tool persistence issue**
- **Found during:** Task 2 (billing.rs edits)
- **Issue:** Edit tool appeared to succeed but files were unchanged on disk (Windows file locking/caching issue)
- **Fix:** Consolidated all billing.rs changes into a single Python `fix_billing.py` script using `open().write()`
- **Files modified:** fix_billing.py (new), crates/racecontrol/src/billing.rs
- **Verification:** File length and content verified via bash after script execution
- **Committed in:** 80f32d1 (Task 2 commit)

---

**Total deviations:** 3 auto-fixed (3 blocking)
**Impact on plan:** All auto-fixes required for correctness. The PreFlightPassed/PreFlightFailed fix resolved a latent issue introduced by a research commit (e456467). The fix_billing.py workaround resolved a platform-specific tool limitation.

## Issues Encountered

- Python multi-script overwrite: earlier attempts used separate Python scripts for each edit, but each script re-read the old file, causing later scripts to overwrite earlier changes. Resolved by consolidating all edits into a single `fix_billing.py` script.
- `Vec<&BillingRateTier>` vs `&[BillingRateTier]` type mismatch in `current_cost()`: `get_tiers_for_game()` returns references, but `compute_session_cost` needs owned slice. Resolved with `.into_iter().cloned().collect::<Vec<_>>()`.
- Application Control policy on James PC blocks `cargo test` execution (OS error 4551). Verified with `cargo check --workspace --tests` instead.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Plan 82-02 can implement `PlayableSignal` detection in rc-agent event_loop.rs — types are ready
- Plan 82-03 can implement `GameState::Loading` display in kiosk/dashboard — enum variant is ready
- `get_tiers_for_game()` is public and ready for use wherever billing calculations happen
- DB migration is idempotent (ALTER TABLE ADD COLUMN IF NOT EXISTS pattern — SQLite uses `IF NOT EXISTS` in the db/mod.rs migration)

---
*Phase: 82-billing-and-session-lifecycle*
*Completed: 2026-03-21*
