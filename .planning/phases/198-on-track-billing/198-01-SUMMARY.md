---
phase: 198-on-track-billing
plan: 01
subsystem: billing
tags: [rust, billing, ac, false-live, process-fallback, types, config]

# Dependency graph
requires: []
provides:
  - "BillingSessionStatus::CancelledNoPlayable variant in rc-common types"
  - "BillingConfig struct with 5 configurable timeout fields in racecontrol config"
  - "Config struct billing field with serde default"
  - "AC False-Live guard (5s speed+steer gate) in rc-agent event_loop"
  - "Process fallback crash guard (game.is_running() check before billing) in rc-agent event_loop"
affects:
  - "198-on-track-billing plan 02 (server-side billing logic depends on CancelledNoPlayable + BillingConfig)"
  - "racecontrol billing session state machine"
  - "rc-agent AC telemetry path"

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "serde default functions for configurable timeouts (5 fns + impl Default)"
    - "AC False-Live guard: Option<Instant> + bool gate on speed/steer before billing emit"
    - "Crash guard: game.is_running() check before process-fallback Live emit"

key-files:
  created: []
  modified:
    - "crates/rc-common/src/types.rs"
    - "crates/racecontrol/src/config.rs"
    - "crates/rc-agent/src/event_loop.rs"

key-decisions:
  - "False-Live guard uses 5s window with speed>0 OR |steer|>0.02 threshold -- same telemetry fields used by AC adapter (speed_kmh, steering)"
  - "Process fallback crash guard emits AcStatus::Off (not GameState::Error) when game is dead -- matches existing Off-handling path on server"
  - "BillingConfig placed after CafeConfig in Config struct for non-breaking append"
  - "Config default() initializer needed explicit billing: BillingConfig::default() addition -- serde default alone is not enough for the test/impl initializer"

patterns-established:
  - "AC False-Live guard pattern: Option<Instant> guards expensive emit, bool tracks qualifying condition, cleared on Off"
  - "Process fallback crash guard: always check game.is_running() before emitting Live via fallback path"

requirements-completed: [BILL-01, BILL-02, BILL-03, BILL-04, BILL-08, BILL-12]

# Metrics
duration: 25min
completed: 2026-03-26
---

# Phase 198 Plan 01: On-Track Billing Foundational Types Summary

**AC False-Live guard (5s speed+steer gate), CancelledNoPlayable status variant, and BillingConfig (5 configurable timeouts) -- billing foundation for Plan 02 server-side logic**

## Performance

- **Duration:** 25 min
- **Started:** 2026-03-26T07:00:00Z
- **Completed:** 2026-03-26T07:25:00Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Added `BillingSessionStatus::CancelledNoPlayable` variant (BILL-06: no charge if PlayableSignal never arrives)
- Added `BillingConfig` struct with 5 configurable timeout fields + serde defaults + `impl Default` (BILL-12)
- Added `billing: BillingConfig` field to `Config` struct with `#[serde(default)]`
- Added AC False-Live guard: 5s window after AC 1s stability, gates Live emit on speed>0 OR |steer|>0.02 (BILL-01/02)
- Added process fallback crash guard: checks `game.is_running()` before emitting Live at 90s; dead game emits `AcStatus::Off` instead (BILL-08)
- Verified F1 25 (BILL-03) and iRacing (BILL-04) playable signal flows are already correct -- no changes needed

## Task Commits

Each task was committed atomically:

1. **Task 1: CancelledNoPlayable variant + BillingConfig struct** - `ad7ab774` (feat)
2. **Task 2: AC False-Live guard + process fallback crash guard** - `e9558961` (feat)

**Plan metadata:** (this summary commit)

## Files Created/Modified
- `crates/rc-common/src/types.rs` - Added CancelledNoPlayable to BillingSessionStatus enum
- `crates/racecontrol/src/config.rs` - Added BillingConfig struct (5 fields, 5 defaults, impl Default) + billing field on Config + initializer
- `crates/rc-agent/src/event_loop.rs` - Added ac_live_since + ac_live_has_input fields, False-Live guard in telemetry tick, crash guard in process fallback

## Decisions Made
- False-Live guard uses `Option<Instant>` (reset to None when guard triggers or passes) rather than a separate FSM state -- keeps the telemetry tick block self-contained
- Process fallback emits `AcStatus::Off` when game is dead (not a new `GameState::Error` variant) -- the Off path on the server already handles session cancel correctly
- BillingConfig placed at the end of Config struct -- non-breaking, consistent with other optional sections like CafeConfig
- The Config test initializer at line ~767 required explicit `billing: BillingConfig::default()` -- `#[serde(default)]` only applies to deserialization, not struct literals

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Missing billing field in Config struct test initializer**
- **Found during:** Task 1 (cargo check after adding billing field to Config struct)
- **Issue:** `cargo check` reported `error[E0063]: missing field 'billing' in initializer of 'config::Config'` -- the Config test/default initializer at config.rs:767 had all fields listed explicitly but was missing the new billing field
- **Fix:** Added `billing: BillingConfig::default()` to the initializer block
- **Files modified:** `crates/racecontrol/src/config.rs`
- **Verification:** `cargo check --manifest-path crates/racecontrol/Cargo.toml` passes with no errors
- **Committed in:** `ad7ab774` (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 - bug: missing field in struct initializer)
**Impact on plan:** Necessary for compilation. Zero scope creep.

## Issues Encountered
None beyond the auto-fixed initializer issue above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All foundational types and guards are in place for Plan 02 (server-side billing state machine)
- `CancelledNoPlayable` is ready for DB insert path in BILL-06
- `BillingConfig` can be read from `state.config.billing.*` in server-side billing handlers
- AC False-Live guard active on all pods after next rc-agent deploy
- Process fallback crash guard active after next rc-agent deploy

---
*Phase: 198-on-track-billing*
*Completed: 2026-03-26*
