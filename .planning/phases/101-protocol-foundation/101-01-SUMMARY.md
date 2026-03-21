---
phase: 101-protocol-foundation
plan: 01
subsystem: infra
tags: [rust, serde, rc-common, protocol, process-guard, websocket]

# Dependency graph
requires: []
provides:
  - MachineWhitelist, ViolationType, ProcessViolation types in rc-common/types.rs
  - AgentMessage::ProcessViolation and AgentMessage::ProcessGuardStatus variants in rc-common/protocol.rs
  - CoreToAgentMessage::UpdateProcessWhitelist variant in rc-common/protocol.rs
affects:
  - 102-racecontrol-api (uses UpdateProcessWhitelist, reads ProcessViolation reports)
  - 103-agent-enforcement (uses all three AgentMessage variants, reads MachineWhitelist)
  - 104-james-reporter (uses ProcessViolation, MachineWhitelist via HTTP)
  - 105-james-standalone (same types)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Manual Default impl required when serde default functions must apply at struct construction time, not only on deserialization"
    - "New AgentMessage variants added additively with wildcard arm in existing match blocks — downstream handlers deferred to Phase 103/104"

key-files:
  created: []
  modified:
    - crates/rc-common/src/types.rs
    - crates/rc-common/src/protocol.rs
    - crates/racecontrol/src/ws/mod.rs

key-decisions:
  - "Manual Default impl for MachineWhitelist instead of #[derive(Default)] — serde default= functions are only called during deserialization, not by Rust's Default::default()"
  - "Wildcard arm added to racecontrol ws/mod.rs match on AgentMessage — Phase 101 adds types only, no server-side handling until Phase 102+"

patterns-established:
  - "Process guard types follow existing derive pattern: #[derive(Debug, Clone, Serialize, Deserialize)] with #[serde(rename_all = snake_case)]"
  - "ProcessViolation uses #[serde(default, skip_serializing_if = Option::is_none)] on exe_path — absent from JSON when None"

requirements-completed:
  - GUARD-04
  - GUARD-05

# Metrics
duration: 35min
completed: 2026-03-21
---

# Phase 101 Plan 01: Protocol Foundation Summary

**Three new process guard types (ViolationType, ProcessViolation, MachineWhitelist) and three new protocol variants (ProcessViolation, ProcessGuardStatus, UpdateProcessWhitelist) added to rc-common as the compile-time boundary for Phases 102-105**

## Performance

- **Duration:** ~35 min
- **Started:** 2026-03-21T12:00:00Z (approx)
- **Completed:** 2026-03-21T12:35:00Z (approx)
- **Tasks:** 3 of 3
- **Files modified:** 3

## Accomplishments
- `ViolationType` enum (Process, Port, AutoStart, WrongMachineBinary) with snake_case serde serialization
- `ProcessViolation` struct with optional `exe_path` that omits itself from JSON when `None`
- `MachineWhitelist` struct with `violation_action = "report_only"` and `warn_before_kill = true` defaults
- `AgentMessage::ProcessViolation(ProcessViolation)` and `AgentMessage::ProcessGuardStatus { ... }` variants
- `CoreToAgentMessage::UpdateProcessWhitelist { whitelist: MachineWhitelist }` variant
- 144 rc-common tests pass (139 pre-existing + 4 types tests + 5 protocol tests), zero warnings
- All three crates build cleanly: rc-common, rc-agent-crate, racecontrol-crate

## Task Commits

Each task was committed atomically:

1. **Task 1: Add MachineWhitelist, ViolationType, ProcessViolation to types.rs** - `c728074` (feat)
2. **Task 2: Add ProcessViolation, ProcessGuardStatus, UpdateProcessWhitelist to protocol.rs** - `be02757` (feat)
3. **Task 3: Confirm downstream crates compile — add wildcard arm to racecontrol ws** - `20d9c98` (fix)

## Files Created/Modified
- `crates/rc-common/src/types.rs` — appended ViolationType, ProcessViolation, MachineWhitelist, manual Default impl, 4 tests
- `crates/rc-common/src/protocol.rs` — updated import block, added 3 new variants, 5 protocol tests
- `crates/racecontrol/src/ws/mod.rs` — added `_ => {}` wildcard arm to AgentMessage match at line 742

## Decisions Made
- **Manual Default impl for MachineWhitelist:** `#[derive(Default)]` initializes strings to `""` and bools to `false`, completely ignoring serde's `default =` functions. The test `machine_whitelist_default_has_report_only_action` caught this in the RED phase. Replaced with explicit `impl Default` that calls `default_violation_action()` and `default_warn_before_kill()`.
- **Wildcard arm in racecontrol ws/mod.rs:** Rust non-exhaustive pattern error `E0004` — the existing match on `&AgentMessage` had no wildcard. Added `_ => { /* new process guard variants — handled in Phase 103/104 */ }` as specified in plan Task 3.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] #[derive(Default)] does not apply serde default functions**
- **Found during:** Task 1 (TDD RED/GREEN cycle)
- **Issue:** `MachineWhitelist::default().violation_action` was `""` instead of `"report_only"`. The `#[serde(default = "default_violation_action")]` attribute is only applied when deserializing from JSON with missing fields — not when calling `Default::default()`.
- **Fix:** Removed `#[derive(Default)]`, added `impl Default for MachineWhitelist` that explicitly calls `default_violation_action()` and `default_warn_before_kill()`. Also added `#[serde(default)]` to the other fields for deserialization safety.
- **Files modified:** `crates/rc-common/src/types.rs`
- **Verification:** `machine_whitelist_default_has_report_only_action` test passes — `violation_action == "report_only"`, `warn_before_kill == true`, `processes.is_empty()`
- **Committed in:** `c728074` (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 - Bug)
**Impact on plan:** Required for correctness — plan assumed `Default` would use the same values as serde defaults, but Rust does not link these two mechanisms. No scope creep.

## Issues Encountered
- racecontrol non-exhaustive match: expected per plan Task 3. Compiler error `E0004` on `AgentMessage` in `crates/racecontrol/src/ws/mod.rs:132`. Fixed by adding wildcard arm as instructed.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- rc-common types and protocol variants are fully published — all downstream crates compile against them
- Phase 102 (racecontrol API) can now add `GET /api/v1/guard/whitelist` and `POST /api/v1/guard/violations` endpoints referencing `MachineWhitelist` and `ProcessViolation` directly
- Phase 103 (rc-agent enforcement) can now implement the process scanner and send `AgentMessage::ProcessViolation` and `AgentMessage::ProcessGuardStatus`
- No blockers

---
*Phase: 101-protocol-foundation*
*Completed: 2026-03-21*
