---
phase: 17-websocket-exec
plan: 01
subsystem: protocol
tags: [serde, websocket, exec, rc-common, rust]

# Dependency graph
requires: []
provides:
  - CoreToAgentMessage::Exec variant with request_id, cmd, timeout_ms (default 10s)
  - AgentMessage::ExecResult variant with request_id, success, exit_code, stdout, stderr
  - default_exec_timeout_ms() free function for serde default
  - 5 serde roundtrip and wire format tests proving correct JSON shape
affects: [17-02-PLAN, 17-03-PLAN, rc-agent, racecontrol]

# Tech tracking
tech-stack:
  added: []
  patterns: [serde default function for optional timeout field, struct-style enum variants for exec protocol]

key-files:
  created: []
  modified: [crates/rc-common/src/protocol.rs]

key-decisions:
  - "Struct-style enum variants (not tuple) for both Exec and ExecResult, matching existing patterns like DrivingStateUpdate and AssistChanged"
  - "serde default function default_exec_timeout_ms() returns 10_000ms when timeout_ms is omitted from JSON"

patterns-established:
  - "Exec request/response correlation via request_id string field"
  - "ExecResult carries both stdout/stderr and optional exit_code for timeout/semaphore cases"

requirements-completed: [WSEX-01, WSEX-03]

# Metrics
duration: 3min
completed: 2026-03-15
---

# Phase 17 Plan 01: Protocol Extension Summary

**CoreToAgentMessage::Exec and AgentMessage::ExecResult variants with serde roundtrip tests proving snake_case wire format and default timeout**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-15T08:17:07Z
- **Completed:** 2026-03-15T08:20:13Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- Added `CoreToAgentMessage::Exec` with `request_id`, `cmd`, `timeout_ms` (serde default 10,000ms)
- Added `AgentMessage::ExecResult` with `request_id`, `success`, `exit_code`, `stdout`, `stderr`
- 5 new tests: roundtrip, wire format, default timeout, ExecResult roundtrip, success/error variants
- All 53 protocol tests pass (48 existing + 5 new)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add Exec and ExecResult variants** - `5f4eff5` (feat)
2. **Task 2: Add serde roundtrip and wire format tests** - `96ea7ea` (test)

## Files Created/Modified
- `crates/rc-common/src/protocol.rs` - Added Exec/ExecResult enum variants + default_exec_timeout_ms() + 5 test functions

## Decisions Made
- Used struct-style enum variants (not tuple), consistent with existing DrivingStateUpdate, AssistChanged patterns
- Placed default_exec_timeout_ms() as a free function after CoreToAgentMessage enum, before DashboardEvent — standard serde default pattern

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Protocol variants ready for Plan 17-02 (rc-agent exec handler) and Plan 17-03 (racecontrol dispatch)
- Wire format confirmed: `{"type":"exec","data":{"request_id":"...","cmd":"...","timeout_ms":10000}}`
- ExecResult wire format: `{"type":"exec_result","data":{"request_id":"...","success":true,...}}`

## Self-Check: PASSED

- [x] crates/rc-common/src/protocol.rs exists
- [x] Commit 5f4eff5 found (Task 1: Exec/ExecResult variants)
- [x] Commit 96ea7ea found (Task 2: serde tests)
- [x] 17-01-SUMMARY.md exists

---
*Phase: 17-websocket-exec*
*Completed: 2026-03-15*
