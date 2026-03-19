---
phase: 46-crash-safety-panic-hook
plan: 01
subsystem: infra
tags: [ffb, hid, protocol, serde, fleet-health, startup-report, crash-safety]

# Dependency graph
requires:
  - phase: 45-close-wait-fix-connection-hygiene
    provides: Remote ops port 8090, connection hygiene patterns
provides:
  - zero_force_with_retry() on FfbController (sync-safe, panic hook ready)
  - StartupReport protocol extended with 4 boot verification fields (#[serde(default)])
  - Server-side FleetHealthStore + store_startup_report accepting and logging boot verification data
  - Backward compat: old agents without new fields deserialize to safe defaults
affects: [46-02, panic-hook, port-bind-signaling, fleet-health-dashboard]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "#[serde(default)] on protocol fields for backward-compat extension without versioning"
    - "sync-safe retry loop pattern for panic hook use (no async, thread::sleep)"
    - "Device-not-found vs HID-error distinction: Ok(false) is not retried, Err is"

key-files:
  created: []
  modified:
    - crates/rc-agent/src/ffb_controller.rs
    - crates/rc-common/src/protocol.rs
    - crates/racecontrol/src/fleet_health.rs
    - crates/racecontrol/src/ws/mod.rs
    - crates/rc-agent/src/main.rs

key-decisions:
  - "Ok(false) from zero_force means device not found — permanent, not retried; Err means HID write failed — retried"
  - "zero_force_with_retry uses thread::sleep not tokio::sleep — sync-safe for panic hook use"
  - "4 new StartupReport fields all use #[serde(default)] — backward compat without version negotiation"
  - "rc-agent main.rs StartupReport construction uses false defaults for Phase 46 fields — Plan 02 wires real values"
  - "Backward compat test JSON uses type/data adjacently-tagged format matching serde(tag,content) on AgentMessage"

patterns-established:
  - "Protocol extension pattern: add new optional fields with #[serde(default)], update sender last"
  - "boot verification fields: plan 01 adds protocol layer, plan 02 wires real port-bind checks"

requirements-completed: [SAFETY-03, SAFETY-04, SAFETY-05]

# Metrics
duration: 18min
completed: 2026-03-19
---

# Phase 46 Plan 01: Crash Safety — FFB Retry + StartupReport Protocol Extension Summary

**zero_force_with_retry(3, 100) on FfbController plus StartupReport extended with 4 #[serde(default)] boot verification fields wired to FleetHealthStore**

## Performance

- **Duration:** 18 min
- **Started:** 2026-03-19T06:00:00Z
- **Completed:** 2026-03-19T06:18:00Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Added `zero_force_with_retry(attempts, delay_ms) -> bool` to FfbController — sync-safe for panic hook use, device-not-found (Ok(false)) is not retried, HID write errors (Err) are retried up to `attempts` times with `delay_ms` delay
- Extended `AgentMessage::StartupReport` with 4 boot verification fields: `lock_screen_port_bound`, `remote_ops_port_bound`, `hid_detected`, `udp_ports_bound` — all with `#[serde(default)]` for backward compat
- Updated FleetHealthStore to store all 4 new fields; `clear_on_disconnect` clears them; `store_startup_report` signature extended
- WS handler in ws/mod.rs logs full boot verification status and emits BOOT WARNING if lock screen or remote ops ports failed to bind
- All tests pass: 1 new FFB test, 2 new protocol tests (roundtrip + backward compat), 1 new fleet_health test

## Task Commits

Each task was committed atomically:

1. **Task 1: Add zero_force_with_retry + extend StartupReport protocol** - `f6324f4` (feat, TDD)
2. **Task 2: Update server-side StartupReport handler and fleet_health store** - `9d3e00a` (feat)

## Files Created/Modified
- `crates/rc-agent/src/ffb_controller.rs` - Added zero_force_with_retry() method + test
- `crates/rc-common/src/protocol.rs` - Extended StartupReport with 4 boot verification fields + 2 new tests, updated 2 existing test constructions
- `crates/racecontrol/src/fleet_health.rs` - Extended FleetHealthStore struct, store_startup_report, clear_on_disconnect + new test; updated 8 existing test calls
- `crates/racecontrol/src/ws/mod.rs` - Updated StartupReport pattern match to destructure and log new fields, pass to store_startup_report
- `crates/rc-agent/src/main.rs` - Added new fields to StartupReport construction with false defaults (Plan 02 wires real values)

## Decisions Made
- `Ok(false)` from `zero_force()` means device not found — permanent condition, not retried. Only `Err` (HID write failure) is retried.
- `zero_force_with_retry` uses `std::thread::sleep` not `tokio::time::sleep` — must be sync-safe for panic hook use.
- All 4 new StartupReport fields use `#[serde(default)]` — old agents send the old JSON, new fields default to `false`/`vec![]`. No version negotiation needed.
- Backward compat test JSON format is `{"type":"startup_report","data":{...}}` (adjacently tagged with `tag = "type", content = "data"` on AgentMessage enum).
- rc-agent main.rs sets all new fields to `false`/`vec![]` for now. Plan 02 will wire actual port-bind results at startup.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed rc-agent main.rs compile error: missing new StartupReport fields**
- **Found during:** Task 2 (build verification after extending protocol)
- **Issue:** rc-agent/src/main.rs constructed `AgentMessage::StartupReport` without the 4 new fields added in Task 1, causing `E0063: missing fields` compile error
- **Fix:** Added `lock_screen_port_bound: false, remote_ops_port_bound: false, hid_detected: false, udp_ports_bound: vec![]` to the StartupReport construction in main.rs with comment noting Plan 02 wires real values
- **Files modified:** `crates/rc-agent/src/main.rs`
- **Verification:** `cargo build -p rc-agent-crate -p racecontrol-crate -p rc-common` succeeds; `cargo build --release` succeeds
- **Committed in:** `9d3e00a` (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Fix necessary — protocol extension requires all construction sites updated. Plan correctly leaves actual port-bind logic to Plan 02.

## Issues Encountered
- Backward compat test in plan used `{"startup_report":{...}}` JSON format (internally-tagged style), but AgentMessage uses `serde(tag = "type", content = "data")` (adjacently-tagged). Corrected to `{"type":"startup_report","data":{...}}` before writing test.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Plan 02 can now use `zero_force_with_retry(3, 100)` in panic hook — sync-safe method is available
- Plan 02 can populate `lock_screen_port_bound`, `remote_ops_port_bound`, `hid_detected`, `udp_ports_bound` in the StartupReport construction in main.rs
- Server will log BOOT WARNING for any pod that sends `lock_screen_port_bound=false` or `remote_ops_port_bound=false`
- All 3 crates build cleanly at release profile

---
*Phase: 46-crash-safety-panic-hook*
*Completed: 2026-03-19*
