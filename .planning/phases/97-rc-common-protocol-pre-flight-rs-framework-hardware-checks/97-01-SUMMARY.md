---
phase: 97-rc-common-protocol-pre-flight-rs-framework-hardware-checks
plan: 01
subsystem: api
tags: [rust, protocol, serde, rc-common, rc-agent, pre-flight]

requires:
  - phase: 74-rc-agent-decomposition
    provides: AgentConfig struct with serde(default) fields pattern (KioskConfig)

provides:
  - AgentMessage::PreFlightPassed { pod_id: String } variant in rc-common protocol
  - AgentMessage::PreFlightFailed { pod_id, failures, timestamp } variant in rc-common protocol
  - CoreToAgentMessage::ClearMaintenance variant in rc-common protocol
  - PreflightConfig struct in rc-agent config.rs with enabled: bool (default true)
  - AgentConfig.preflight field wired with serde(default)

affects:
  - 97-02 (pre_flight.rs logic + ws_handler gate reads AgentConfig.preflight.enabled, sends PreFlightPassed/PreFlightFailed)
  - 97-03 (racecontrol MaintenanceRequired state + ClearMaintenance handler)
  - 98-rc-common-protocol (future protocol additions using same pod_id: String convention)

tech-stack:
  added: []
  patterns:
    - "New AgentMessage variants use pod_id: String (NOT u32) — matches all existing variants"
    - "PreflightConfig follows KioskConfig serde(default) pattern exactly"
    - "Phase 97+ match arms in racecontrol ws/mod.rs use log-only stubs; full handler wired in later plan"

key-files:
  created: []
  modified:
    - crates/rc-common/src/protocol.rs
    - crates/rc-agent/src/config.rs
    - crates/racecontrol/src/ws/mod.rs

key-decisions:
  - "pod_id: String (not u32) for PreFlightPassed/PreFlightFailed — CONTEXT.md specified u32 but RESEARCH.md identified deserialization-breaking mismatch; String matches all existing AgentMessage variants"
  - "ClearMaintenance is a unit variant (no fields) — pod ID not needed since CoreToAgentMessage is always addressed to a specific pod via its WebSocket connection"
  - "Match arms for new variants in racecontrol ws/mod.rs are log-only stubs — full MaintenanceRequired FSM deferred to Phase 98 per plan scope"

patterns-established:
  - "Phase 97+ protocol additions: add serde round-trip tests in rc-common protocol::tests module"
  - "Non-exhaustive match on AgentMessage in ws/mod.rs: always add log-only arm when extending protocol, handler logic in dedicated phase"

requirements-completed: [PF-07]

duration: 11min
completed: 2026-03-21
---

# Phase 97 Plan 01: Protocol Variants + PreflightConfig Summary

**Three AgentMessage/CoreToAgentMessage enum variants added to rc-common + PreflightConfig struct wired into AgentConfig enabling Plan 02 pre-flight logic to compile**

## Performance

- **Duration:** 11 min
- **Started:** 2026-03-21T03:53:37Z
- **Completed:** 2026-03-21T04:04:20Z (IST: 09:23:50 — 09:34:50)
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- `AgentMessage::PreFlightPassed { pod_id: String }` and `AgentMessage::PreFlightFailed { pod_id, failures, timestamp }` added to protocol.rs with round-trip serde tests
- `CoreToAgentMessage::ClearMaintenance` (unit variant) added — server clears pod MaintenanceRequired state
- `PreflightConfig { enabled: bool }` struct with `Default` impl (enabled=true) added to rc-agent config.rs, wired into `AgentConfig.preflight` with `#[serde(default)]`
- All 135 rc-common tests pass; rc-sentry, rc-agent, and racecontrol all compile cleanly

## Task Commits

Each task was committed atomically:

1. **Task 1: Add PreFlightPassed, PreFlightFailed, and ClearMaintenance protocol variants** - `be61b1f` (feat)
2. **Task 2: Add PreflightConfig struct and wire into AgentConfig** - `70612c2` (feat)

**Plan metadata:** (docs commit below)

## Files Created/Modified

- `crates/rc-common/src/protocol.rs` - Three new enum variants + two round-trip serde tests
- `crates/rc-agent/src/config.rs` - PreflightConfig struct + AgentConfig.preflight field + test helper fix
- `crates/racecontrol/src/ws/mod.rs` - Log-only match arms for PreFlightPassed/PreFlightFailed (auto-added by rust-analyzer; required for racecontrol to compile)

## Decisions Made

- `pod_id: String` (not u32) for new AgentMessage variants — CONTEXT.md had u32 but RESEARCH.md correctly identified this as a deserialization-breaking mismatch with all existing variants which use String
- `ClearMaintenance` is a unit variant with no fields — CoreToAgentMessage is routed to a specific pod via its WS connection, pod_id redundant
- Log-only stubs in racecontrol ws/mod.rs are sufficient for Phase 97-01 scope; full handler FSM wired in Phase 98

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Non-exhaustive match on AgentMessage in racecontrol ws/mod.rs**
- **Found during:** Task 1 (protocol variant insertion)
- **Issue:** `cargo build --bin racecontrol` failed with E0004: PreFlightPassed and PreFlightFailed not covered in match at ws/mod.rs:132
- **Fix:** Log-only match arms added to ws/mod.rs (rust-analyzer pre-populated, verified manually). `tracing::info!` for PreFlightPassed, `tracing::warn!` for PreFlightFailed
- **Files modified:** crates/racecontrol/src/ws/mod.rs
- **Verification:** `cargo build --bin racecontrol` succeeds
- **Committed in:** be61b1f (Task 1 commit)

**2. [Rule 3 - Blocking] Missing `preflight` field in valid_config() test helper**
- **Found during:** Task 2 (PreflightConfig insertion)
- **Issue:** E0063 missing field `preflight` in initializer of `config::AgentConfig` in test module
- **Fix:** Added `preflight: PreflightConfig::default()` to `valid_config()` struct literal
- **Files modified:** crates/rc-agent/src/config.rs
- **Verification:** rc-agent builds without error
- **Committed in:** 70612c2 (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (1 bug, 1 blocking)
**Impact on plan:** Both fixes were direct consequences of adding new struct fields — expected struct literal and match exhaustiveness maintenance. No scope creep.

## Issues Encountered

- `cargo test -p rc-agent-crate` output files were cleaned up by another Claude Code process during the session — verified correctness via `cargo build --bin rc-agent` (succeeded) and `grep` acceptance criteria checks instead.

## Next Phase Readiness

- Plan 02 (pre_flight.rs + ws_handler gate) can now compile against: `AgentMessage::PreFlightPassed`, `AgentMessage::PreFlightFailed`, `CoreToAgentMessage::ClearMaintenance`, and `AgentConfig.preflight.enabled`
- rc-sentry stdlib-only constraint verified: still builds without tokio after rc-common changes
- No blockers for Plan 02

---
*Phase: 97-rc-common-protocol-pre-flight-rs-framework-hardware-checks*
*Completed: 2026-03-21*
