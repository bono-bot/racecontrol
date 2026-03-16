---
phase: 23-protocol-contract-concurrency-safety
plan: 01
subsystem: protocol
tags: [rust, serde, rc-common, protocol, enum, agent-message]

# Dependency graph
requires: []
provides:
  - "PodFailureReason enum in rc-common/types.rs with 18 variants covering all 9 bot failure classes"
  - "5 new AgentMessage variants: HardwareFailure, TelemetryGap, BillingAnomaly, LapFlagged, MultiplayerFailure"
  - "Stub match arms in ws/mod.rs for all 5 new variants (logging only, bot wiring in Phase 25)"
  - "6 new roundtrip tests verifying snake_case wire format keys for all new variants"
affects: ["24-bot-detection", "25-bot-recovery", "26-bot-supervisor"]

# Tech tracking
tech-stack:
  added: []
  patterns: ["PodFailureReason as typed enum taxonomy for all bot failure classes", "AgentMessage struct variants with PodFailureReason fields", "snake_case serde wire format for all new variants"]

key-files:
  created: []
  modified:
    - "crates/rc-common/src/types.rs"
    - "crates/rc-common/src/protocol.rs"
    - "crates/racecontrol/src/ws/mod.rs"

key-decisions:
  - "PodFailureReason derives Copy (not Hash) — not needed as HashMap key in Phase 23"
  - "5 new AgentMessage variants added atomically with ws/mod.rs stub arms to avoid non-exhaustive match breakage"
  - "Stub arms are logging-only — bot_coordinator wiring deferred to Phase 25"

patterns-established:
  - "PodFailureReason pattern: Debug + Clone + Copy + PartialEq + Eq + Serialize + Deserialize, serde rename_all = snake_case"
  - "AgentMessage bot variant pattern: struct variant with pod_id + PodFailureReason reason + context fields"
  - "TDD commit order: RED (test) -> GREEN (impl + stubs) for cross-crate changes"

requirements-completed: [PROTO-01, PROTO-02]

# Metrics
duration: 15min
completed: 2026-03-16
---

# Phase 23 Plan 01: Protocol Contract + Concurrency Safety Summary

**PodFailureReason enum (18 variants, 9 classes) and 5 typed AgentMessage bot failure variants established as the shared protocol foundation for all Phase 24-26 bot detection code**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-03-16T10:00:00Z
- **Completed:** 2026-03-16T10:15:33Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- Added `PodFailureReason` enum with 18 variants covering all 9 bot failure classes (crash, launch, USB, billing, telemetry, multiplayer, PIN, lap) to `rc-common/types.rs`
- Added 5 new `AgentMessage` struct variants (`HardwareFailure`, `TelemetryGap`, `BillingAnomaly`, `LapFlagged`, `MultiplayerFailure`) to `rc-common/protocol.rs` with correct snake_case serde wire keys
- Added 5 roundtrip tests verifying exact JSON wire format (e.g. `"hardware_failure"` not `"HardwareFailure"`)
- Added 5 stub match arms to `ws/mod.rs` so workspace compiles without non-exhaustive match errors
- Full test suite: 606 tests green across all 3 crates (112 rc-common + 211 rc-agent + 283 racecontrol)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add PodFailureReason enum to types.rs** - `b58f22a` (feat)
2. **Task 2: Add 5 AgentMessage variants + ws/mod.rs stub arms** - `6cf453c` (feat)

## Files Created/Modified

- `crates/rc-common/src/types.rs` — PodFailureReason enum (18 variants, 9 failure classes)
- `crates/rc-common/src/protocol.rs` — 5 new AgentMessage variants + 5 roundtrip tests, PodFailureReason import
- `crates/racecontrol/src/ws/mod.rs` — 5 stub match arms for new AgentMessage variants

## Decisions Made

- `PodFailureReason` derives `Copy` but not `Hash` — won't be used as HashMap key in Phase 23; Hash can be added in Phase 24 if needed
- Task 2 commits protocol.rs and ws/mod.rs atomically — adding AgentMessage variants immediately makes ws/mod.rs non-exhaustive, so the commit must include both files
- Stub arms are logging-only (`tracing::info!`) — bot_coordinator wiring is Phase 25 work

## Deviations from Plan

None - plan executed exactly as written. All code was already implemented and committed (Task 1 from a prior session: `b58f22a`). Task 2 changes were staged but uncommitted — committed atomically as `6cf453c`.

## Issues Encountered

None. All 606 tests were green at the start of execution. The rc-agent-crate test output was lost multiple times due to bash output file cleanup conflicts; worked around by writing output to persistent file path.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Phase 23 Plan 01 complete: `PodFailureReason` enum and 5 `AgentMessage` variants are in rc-common
- Phase 24 bot detection code can now import `PodFailureReason` and send `HardwareFailure`, `TelemetryGap`, `BillingAnomaly`, `LapFlagged`, `MultiplayerFailure` messages
- Phase 25 bot recovery can wire `AgentMessage::BillingAnomaly` etc. to `bot_coordinator` in ws/mod.rs (replace stub arms)
- No blockers for Phase 24

---
*Phase: 23-protocol-contract-concurrency-safety*
*Completed: 2026-03-16*

## Self-Check: PASSED

- FOUND: `.planning/phases/23-protocol-contract-concurrency-safety/23-01-SUMMARY.md`
- FOUND: commit `b58f22a` (Task 1: PodFailureReason enum)
- FOUND: commit `6cf453c` (Task 2: 5 AgentMessage variants + ws/mod.rs stubs)
