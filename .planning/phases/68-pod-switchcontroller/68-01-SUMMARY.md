---
phase: 68-pod-switchcontroller
plan: 01
subsystem: infra
tags: [rc-common, rc-agent, protocol, websocket, failover, atomic]

# Dependency graph
requires: []
provides:
  - "SwitchController { target_url: String } variant in CoreToAgentMessage (rc-common)"
  - "CoreConfig.failover_url: Option<String> with serde(default) (rc-agent)"
  - "validate_config rejects non-ws:// failover_url values"
  - "HeartbeatStatus.last_switch_ms: AtomicU64 initialized to 0 (rc-agent)"
affects: [68-02-pod-switchcontroller, 68-CONTEXT]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "AtomicU64 on HeartbeatStatus for epoch-millis timestamp tracking"
    - "serde(default) on Option<String> config fields for backward-compatible TOML"
    - "validate_config pattern: single-pass error accumulation, ws:// prefix check"

key-files:
  created: []
  modified:
    - crates/rc-common/src/protocol.rs
    - crates/rc-agent/src/main.rs
    - crates/rc-agent/src/udp_heartbeat.rs

key-decisions:
  - "SwitchController placed after RunSelfTest in CoreToAgentMessage — additive, no enum reorder needed"
  - "failover_url: Option<String> with #[serde(default)] — backward-compatible, existing configs deserialize without field"
  - "last_switch_ms: AtomicU64 on HeartbeatStatus (not a new struct) — minimal surface area for Plan 02 to wire"
  - "TOML test fixtures required sim field fix (Rule 1 auto-fix) — PodConfig.sim is required with no serde default"

patterns-established:
  - "Phase 68 SwitchController: JSON shape is {\"type\":\"switch_controller\",\"data\":{\"target_url\":\"ws://...\"}}"
  - "failover_url validation mirrors core.url validation — same ws:// / wss:// prefix check"

requirements-completed: [FAIL-01, FAIL-03, FAIL-04]

# Metrics
duration: 12min
completed: 2026-03-20
---

# Phase 68 Plan 01: Pod SwitchController — Data Contracts Summary

**SwitchController protocol variant, failover_url config field, and HeartbeatStatus.last_switch_ms AtomicU64 with 5 passing unit tests**

## Performance

- **Duration:** 12 min
- **Started:** 2026-03-20T13:48:04Z
- **Completed:** 2026-03-20T14:00:45Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Added `SwitchController { target_url: String }` variant to `CoreToAgentMessage` in rc-common, serializes as `{"type":"switch_controller","data":{"target_url":"..."}}`
- Added `failover_url: Option<String>` to `CoreConfig` with `#[serde(default)]` for zero-friction backward compatibility
- Extended `validate_config` to reject non-ws:// failover_url values with descriptive error mentioning "failover_url"
- Added `last_switch_ms: AtomicU64` to `HeartbeatStatus::new()` initialized to 0, ready for Plan 02 runtime wiring
- 5 new unit tests all pass: serde round-trip, 3 config validation variants, 1 heartbeat field init

## Task Commits

Each task was committed atomically:

1. **Task 1: Add SwitchController to CoreToAgentMessage + serde round-trip test** - `cccd7c9` (feat)
2. **Task 2: Add failover_url to CoreConfig + HeartbeatStatus.last_switch_ms + validation + tests** - `c26f939` (feat)

_Note: TDD tasks — implementation and tests committed together (Rust requires compilation for test coverage)_

## Files Created/Modified
- `crates/rc-common/src/protocol.rs` - SwitchController variant + switch_controller_serde_round_trip test
- `crates/rc-agent/src/main.rs` - CoreConfig.failover_url field, validate_config extension, 3 validation tests
- `crates/rc-agent/src/udp_heartbeat.rs` - AtomicU64 import, last_switch_ms field + constructor init, heartbeat init test

## Decisions Made
- SwitchController placed after RunSelfTest — no enum reorder, purely additive change
- `failover_url: Option<String>` with `#[serde(default)]` ensures existing `rc-agent.toml` files without the field continue to deserialize without errors
- `last_switch_ms` on HeartbeatStatus rather than a separate struct — minimal surface area, Plan 02 only needs to set and read one field
- TOML test fixtures required `sim = "assetto_corsa"` — `PodConfig.sim` has no `#[serde(default)]` so it must be explicit

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] TOML test fixtures missing required `sim` field**
- **Found during:** Task 2 (validate_config tests)
- **Issue:** The 3 TOML test strings for failover_url tests omitted `sim = "assetto_corsa"`. `PodConfig.sim` is a required field with no serde default, causing `toml::from_str` to return `Err("missing field sim")` instead of a valid config.
- **Fix:** Added `sim = "assetto_corsa"` to all 3 TOML test fixture strings
- **Files modified:** `crates/rc-agent/src/main.rs`
- **Verification:** All 4 rc-agent tests pass after fix
- **Committed in:** `c26f939` (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 - Bug)
**Impact on plan:** Required for tests to compile and pass. No scope creep — purely correctness fix for test infrastructure.

## Issues Encountered
- Windows Application Control policy blocks the rc-agent test binary when run without a specific filter (pre-existing environment constraint). Verified all new tests pass using targeted filter patterns. Build compiles cleanly.

## Next Phase Readiness
- All data contracts ready for Plan 02 runtime wiring
- `CoreToAgentMessage::SwitchController` available for `ws_handler.rs` match arm
- `CoreConfig.failover_url` available for reconnect loop to read target URL
- `HeartbeatStatus.last_switch_ms` available for self_monitor suppression guard
- Zero regressions in rc-common (129 tests pass) or rc-agent compilation

---
*Phase: 68-pod-switchcontroller*
*Completed: 2026-03-20*

## Self-Check: PASSED

- protocol.rs: FOUND
- main.rs: FOUND
- udp_heartbeat.rs: FOUND
- SUMMARY.md: FOUND
- Commit cccd7c9: FOUND
- Commit c26f939: FOUND
