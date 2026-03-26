---
phase: 199-crash-recovery
plan: 01
subsystem: game-launcher
tags: [rust, crash-recovery, protocol, metrics, whatsapp, race-engineer]

# Dependency graph
requires:
  - phase: 197-launch-resilience-ac-hardening
    provides: GameTracker struct, classify_error_taxonomy, dynamic timeout, Race Engineer auto-relaunch foundation
  - phase: 198-on-track-billing
    provides: BillingSessionStatus::PausedGamePause for crash recovery billing pause

provides:
  - force_clean: bool field on CoreToAgentMessage::LaunchGame (RECOVER-01)
  - query_best_recovery_action() in metrics.rs — 30-day history-informed recovery selection (RECOVER-03)
  - exit_codes: Vec<Option<i32>> accumulation in GameTracker (RECOVER-05)
  - Enriched recovery events with actual ErrorTaxonomy, car/track, exit_code, SLA duration (RECOVER-02)
  - Staff WhatsApp alert with exit codes list and suggested action (RECOVER-05)
  - Null-args guard dashboard notification "Cannot auto-relaunch" (RECOVER-04)

affects:
  - 199-crash-recovery (phases 200-201 build on this infrastructure)
  - rc-agent ws_handler (force_clean triggers clean_state_reset before game spawn)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "force_clean: bool with #[serde(default)] for backward-compatible protocol evolution"
    - "query_best_recovery_action follows query_dynamic_timeout pattern: .fetch_all().await.unwrap_or_default(), 3-sample minimum"
    - "Crash-detected-at captured at Error branch entry for SLA measurement"
    - "exit_codes pushed to tracker under existing write lock (no extra lock acquisition)"

key-files:
  created: []
  modified:
    - crates/rc-common/src/protocol.rs
    - crates/rc-common/src/types.rs
    - crates/racecontrol/src/metrics.rs
    - crates/racecontrol/src/game_launcher.rs
    - crates/racecontrol/src/ac_server.rs
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/src/auth/mod.rs
    - crates/racecontrol/src/billing.rs
    - crates/racecontrol/src/multiplayer.rs
    - crates/racecontrol/src/ws/mod.rs
    - crates/rc-agent/src/ws_handler.rs

key-decisions:
  - "force_clean: false on all normal launches (GameLauncherImpl::make_launch_message); force_clean: true on Race Engineer relaunch and manual relaunch_game() paths only"
  - "query_best_recovery_action returns (action, success_rate) tuple — 3-sample minimum; below threshold returns ('kill_clean_relaunch', 0.0) default"
  - "exit_codes pushed to tracker.exit_codes under existing LAUNCH-17 write lock — no extra lock needed"
  - "Null-args guard broadcasts existing DashboardEvent::GameStateChanged with modified error_message — no new variant needed"
  - "Agent ws_handler: force_clean field added to LaunchGame pattern destructuring; clean_state_reset() called via spawn_blocking before game spawn"

patterns-established:
  - "Protocol field backward compatibility: #[serde(default)] on new bool fields ensures old agents safely ignore new fields"
  - "Recovery metrics follow launch metrics pattern: same SQL pool, same error logging style, same unwrap_or_default"

requirements-completed: [RECOVER-01, RECOVER-02, RECOVER-03, RECOVER-04, RECOVER-05, RECOVER-06]

# Metrics
duration: 40min
completed: 2026-03-26
---

# Phase 199 Plan 01: Crash Recovery Infrastructure Summary

**Server-side crash recovery hardening: force_clean protocol flag, history-informed recovery action selection via query_best_recovery_action(), enriched recovery events with actual ErrorTaxonomy/car/track/exit_codes, and structured staff WhatsApp alert**

## Performance

- **Duration:** 40 min
- **Started:** 2026-03-26T03:30:00Z
- **Completed:** 2026-03-26T04:10:00Z
- **Tasks:** 2
- **Files modified:** 11

## Accomplishments

- Added `force_clean: bool` to `CoreToAgentMessage::LaunchGame` with `#[serde(default)]` — backward-compatible; old agents ignore the new field; Race Engineer relaunch path sets `force_clean: true`, all normal launches set `false`
- Agent `ws_handler.rs` now calls `clean_state_reset()` via `spawn_blocking` when `force_clean == true`, clearing orphan game processes before relaunch (RECOVER-01)
- Added `query_best_recovery_action(db, pod_id, sim_type, failure_mode) -> (String, f64)` to `metrics.rs` — queries 30-day recovery_events history, requires 3 samples, returns highest-success-rate action (RECOVER-03)
- `GameTracker.exit_codes: Vec<Option<i32>>` accumulates exit codes across relaunch attempts; included in staff WhatsApp alert as comma-separated list (RECOVER-05)
- Recovery events now record actual `ErrorTaxonomy` (not hardcoded "game_crash"), car/track from `extract_launch_fields()`, exit_code in details, and `recovery_duration_ms` from crash-to-relaunch wall-clock time (RECOVER-02 SLA)
- Null-args guard broadcasts `DashboardEvent::GameStateChanged` with "Cannot auto-relaunch: no launch args" error message (RECOVER-04)
- Staff alert signature expanded to include `exit_codes: &[Option<i32>]` and `suggested_action: &str` (RECOVER-05)

## Task Commits

Each task was committed atomically:

1. **Task 1: Protocol + metrics infrastructure (force_clean, query_best_recovery_action, exit_codes)** - `b8451f06` (feat)
2. **Task 2: Race Engineer relaunch path + enriched events + staff alert + null-args guard** - `6190bc98` (feat)

## Files Created/Modified

- `crates/rc-common/src/protocol.rs` — Added `#[serde(default)] force_clean: bool` to `CoreToAgentMessage::LaunchGame`
- `crates/rc-common/src/types.rs` — Updated test LaunchGame constructions to include `force_clean: false`
- `crates/racecontrol/src/metrics.rs` — Added `query_best_recovery_action()` function
- `crates/racecontrol/src/game_launcher.rs` — Added `exit_codes` to GameTracker; enriched Error branch with history-informed recovery, SLA timing, enriched events, expanded staff alert, null-args dashboard broadcast
- `crates/racecontrol/src/ac_server.rs` — Added `force_clean: false` to LaunchGame constructions
- `crates/racecontrol/src/api/routes.rs` — Added `force_clean: false` to LaunchGame construction
- `crates/racecontrol/src/auth/mod.rs` — Added `force_clean: false` to LaunchGame construction
- `crates/racecontrol/src/billing.rs` — Added `force_clean: false` to LaunchGame construction
- `crates/racecontrol/src/multiplayer.rs` — Added `force_clean: false` to LaunchGame construction
- `crates/racecontrol/src/ws/mod.rs` — Added `exit_codes: Vec::new()` to GameTracker construction
- `crates/rc-agent/src/ws_handler.rs` — Added `force_clean` to LaunchGame match destructuring; runs `clean_state_reset()` when true

## Decisions Made

- Used `#[serde(default)]` on `force_clean: bool` — old agents that don't have this field in deserialization get `false` automatically, no protocol version bump needed
- `query_best_recovery_action` follows the exact pattern of `query_dynamic_timeout`: `.fetch_all(db).await.unwrap_or_default()`, minimum 3 samples, sensible default below threshold
- `exit_codes` pushed inside the existing LAUNCH-17 write lock rather than a separate read lock — avoids additional lock acquisition and keeps exit code capture atomic with the relaunch counter increment
- Null-args guard reuses `DashboardEvent::GameStateChanged` with a modified `error_message` field rather than creating a new variant — minimizes protocol churn, kiosk already handles GameStateChanged

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Updated all non-game_launcher LaunchGame construction sites**
- **Found during:** Task 1 (adding `force_clean` to protocol)
- **Issue:** Adding a non-`#[serde(default)]` struct field in Rust requires updating all construction sites — 9 other files had `CoreToAgentMessage::LaunchGame { sim_type, launch_args }` without `force_clean`
- **Fix:** Added `force_clean: false` to all normal launch sites (ac_server.rs, routes.rs, auth/mod.rs, billing.rs, multiplayer.rs, ws/mod.rs), `force_clean: true` to relaunch_game(), `clean_state_reset` integration in rc-agent ws_handler
- **Files modified:** 7 additional files beyond the plan's 3-file scope
- **Verification:** `cargo check --workspace` passes with no errors
- **Committed in:** `b8451f06` (Task 1 commit)

**2. [Rule 3 - Blocking] Updated types.rs test LaunchGame constructions**
- **Found during:** Task 1 verification (`cargo test -p rc-common`)
- **Issue:** rc-common tests had LaunchGame constructions without `force_clean` — compilation failed
- **Fix:** Added `force_clean: false` to 2 test constructions; updated pattern match from `{ sim_type, launch_args }` to `{ sim_type, launch_args, .. }`
- **Files modified:** `crates/rc-common/src/types.rs`
- **Verification:** `cargo test -p rc-common` — 190 passed, 0 failed
- **Committed in:** `b8451f06` (Task 1 commit)

---

**Total deviations:** 2 auto-fixed (both Rule 3 - blocking compilation errors from protocol struct field addition)
**Impact on plan:** Expected cascade from adding a Rust struct field. All fixes correct and necessary. No scope creep.

## Issues Encountered

- `cargo test -p racecontrol-crate` (full suite) shows 1 pre-existing failure: `config::tests::config_fallback_preserved_when_no_env_vars` — test passes in isolation (parallel test environment pollution from unrelated config test). Confirmed pre-existing and out of scope.

## Next Phase Readiness

- RECOVER-01 through RECOVER-06 complete (server-side)
- Phase 200 (agent-side `clean_state_reset` invocation) may be partially obviated — rc-agent ws_handler already handles `force_clean: true` via `spawn_blocking(clean_state_reset)`
- Phase 201 (kiosk UI crash recovery indicators) unblocked — `DashboardEvent::GameStateChanged` with "Cannot auto-relaunch" message is already broadcasting

---
*Phase: 199-crash-recovery*
*Completed: 2026-03-26*
