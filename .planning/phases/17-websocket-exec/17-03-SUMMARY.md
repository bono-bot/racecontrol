---
phase: 17-websocket-exec
plan: 03
subsystem: infra
tags: [websocket, deploy, exec, fallback, oneshot, tokio]

# Dependency graph
requires:
  - phase: 17-websocket-exec P01
    provides: Exec/ExecResult protocol variants in rc-common
provides:
  - Core-side ExecResult handler resolving pending oneshot channels
  - ws_exec_on_pod() public function for sending commands via WebSocket
  - deploy.rs HTTP-first with WS fallback for all pod command execution
  - Disconnect cleanup sweep for stale pending_ws_execs entries
affects: [18-deploy-rollback, 19-rc-watchdog, pod_healer]

# Tech tracking
tech-stack:
  added: []
  patterns: [pod-prefixed request_id for stale entry identification, HTTP-first WS-fallback exec pattern, oneshot channel resolution]

key-files:
  created: []
  modified:
    - crates/racecontrol/src/state.rs
    - crates/racecontrol/src/ws/mod.rs
    - crates/racecontrol/src/deploy.rs

key-decisions:
  - "Pod-prefixed request_id format (pod_X:uuid) enables efficient disconnect cleanup via prefix matching"
  - "WS timeout = command timeout + 5s buffer for round-trip overhead"
  - "HTTP-first fallback: try direct HTTP to pod-agent, fall back to WS only on HTTP failure"
  - "Internal helper signatures changed (is_process_alive, is_lock_screen_healthy) but public API unchanged"

patterns-established:
  - "HTTP-first WS-fallback: try fast path (HTTP), degrade gracefully to WS when firewall/agent blocks HTTP"
  - "Oneshot channel resolution: register pending tx, match on response, clean up on disconnect/timeout"

requirements-completed: [WSEX-01, WSEX-03, WSEX-04]

# Metrics
duration: 9min
completed: 2026-03-15
---

# Phase 17 Plan 03: Core-Side Handler + Deploy Fallback Summary

**Core-side ExecResult handler with oneshot channel resolution, ws_exec_on_pod() public function, and deploy.rs HTTP-first WS-fallback for all pod commands**

## Performance

- **Duration:** 9 min
- **Started:** 2026-03-15T08:24:51Z
- **Completed:** 2026-03-15T08:33:33Z
- **Tasks:** 3
- **Files modified:** 3

## Accomplishments
- WsExecResult struct and pending_ws_execs HashMap added to AppState for tracking in-flight WS commands
- ExecResult match arm in ws/mod.rs resolves pending oneshot channels by request_id
- Disconnect cleanup sweeps all pending_ws_execs entries with the disconnected pod's prefix
- ws_exec_on_pod() public function sends Exec commands via WebSocket with timeout+5s buffer
- deploy.rs exec_on_pod now tries HTTP first, falls back to WS when HTTP is unreachable
- All 536 tests pass across rc-common (98), racecontrol (254), and rc-agent (184)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add pending_ws_execs to AppState and WsExecResult struct** - `88134ce` (feat)
2. **Task 2: Add ExecResult handler in ws/mod.rs and ws_exec_on_pod function** - `c4acce0` (feat)
3. **Task 3: Add deploy.rs fallback -- try HTTP first, fall back to WS** - `ecb87a4` (feat)

## Files Created/Modified
- `crates/racecontrol/src/state.rs` - WsExecResult struct + pending_ws_execs field on AppState
- `crates/racecontrol/src/ws/mod.rs` - ExecResult match arm, disconnect sweep, ws_exec_on_pod() function
- `crates/racecontrol/src/deploy.rs` - http_exec_on_pod rename, new exec_on_pod wrapper with WS fallback, updated helper signatures

## Decisions Made
- Pod-prefixed request_id format (pod_X:uuid) for efficient disconnect cleanup via prefix matching
- WS timeout = command timeout + 5s buffer for round-trip overhead
- HTTP-first fallback: try direct HTTP to pod-agent, fall back to WS only on HTTP failure
- Internal helper signatures changed (is_process_alive, is_lock_screen_healthy gain pod_id) but public deploy_pod/deploy_rolling API unchanged

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 17 (WebSocket Exec) is now complete: protocol (P01), agent handler (P02), core handler + deploy fallback (P03)
- deploy.rs can now reach pods even when HTTP :8090 is blocked by firewall -- WS fallback always works since agents initiate outbound connections
- Phase 18 (Deploy Rollback) can build on this infrastructure for reliable remote deploy commands

---
*Phase: 17-websocket-exec*
*Completed: 2026-03-15*
