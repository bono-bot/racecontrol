---
phase: 69-health-monitor-failover-orchestration
plan: 02
subsystem: api
tags: [failover, split-brain, switchcontroller, reqwest, axum, orchestration]

# Dependency graph
requires:
  - phase: 68-failover-controller-wiring
    provides: SwitchController message type, active_url Arc<RwLock>, failover_url Option<String>, last_switch_ms AtomicU64

provides:
  - POST /api/v1/failover/broadcast endpoint on racecontrol (broadcasts SwitchController to all connected agents)
  - Split-brain guard in rc-agent SwitchController handler (probes .23 before switching)

affects: [69-03, deployment, failover-orchestration]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - failover_broadcast handler uses same x-terminal-secret auth pattern as sync_push and terminal endpoints
    - split_brain_probe reqwest::Client created once before outer reconnect loop (not per-message)
    - Split-brain guard uses fire-and-forget HTTP GET with 2s timeout — non-response = safe to switch

key-files:
  created: []
  modified:
    - crates/racecontrol/src/api/routes.rs
    - crates/rc-agent/src/main.rs

key-decisions:
  - "failover_broadcast uses simple != comparison for terminal_secret (consistent with all existing service routes — no subtle crate)"
  - "split_brain_probe reqwest::Client created once before the outer reconnect loop, not per-message — avoids repeated TLS handshake cost"
  - "Split-brain guard probes http://192.168.31.23:8090/ping (rc-agent port on server) not :8080 (racecontrol port) — rc-agent is the peer being displaced"
  - "If .23 is reachable, pod stays on current connection (no break) — allows natural reconnect without explicit disconnect"
  - "Route registered as /failover/broadcast in service_routes() — nested under /api/v1 prefix by api/mod.rs"

patterns-established:
  - "Failover broadcast: iterate agent_senders, send CoreToAgentMessage::SwitchController, return sent/total JSON"
  - "Split-brain guard: HTTP probe with timeout before acting on network-level commands"

requirements-completed: [ORCH-02, ORCH-03]

# Metrics
duration: 4min
completed: 2026-03-21
---

# Phase 69 Plan 02: Failover Broadcast + Split-Brain Guard Summary

**HTTP-triggered SwitchController broadcast via POST /api/v1/failover/broadcast with per-pod split-brain guard probing 192.168.31.23:8090/ping before URL switch**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-21T00:54:55Z (IST: 06:24)
- **Completed:** 2026-03-21T00:55:38Z (IST: 06:25)
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Added `POST /api/v1/failover/broadcast` to racecontrol — auth-protected endpoint that sends `SwitchController` to all connected agents and returns `{ ok, sent, total }`
- Added split-brain guard to rc-agent's `SwitchController` handler — probes `http://192.168.31.23:8090/ping` with 2s timeout; rejects switch if server still reachable from pod's perspective
- Both binaries compile cleanly; 129 rc-common tests pass with no regressions

## Task Commits

Each task was committed atomically:

1. **Task 1: Add POST /api/v1/failover/broadcast endpoint to racecontrol** - `92bd65b` (feat)
2. **Task 2: Add split-brain guard to rc-agent SwitchController handler** - `02030f4` (feat)

**Plan metadata:** (docs commit follows)

## Files Created/Modified

- `crates/racecontrol/src/api/routes.rs` - Added `FailoverBroadcastRequest` struct, `failover_broadcast` handler, route registration in `service_routes()`
- `crates/rc-agent/src/main.rs` - Added `split_brain_probe` reqwest::Client before outer loop, split-brain guard logic inside `SwitchController` else branch

## Decisions Made

- `failover_broadcast` uses `!=` for secret comparison (no `subtle` crate) — consistent with all 5+ existing service route auth checks in routes.rs
- `split_brain_probe` created once before `loop {}` to avoid per-reconnect client construction overhead
- Guard probes `:8090/ping` not `:8080` — the intent is to verify rc-agent is reachable on server, not racecontrol HTTP
- Pod stays in inner event loop (no break) when server is reachable — natural behavior, no forced disconnect

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- `POST /api/v1/failover/broadcast` is live and ready to test with `curl -X POST -H "x-terminal-secret: <secret>" -d '{"target_url":"ws://..."}' http://192.168.31.23:8080/api/v1/failover/broadcast`
- Split-brain guard active on all pods after next rc-agent deploy
- Phase 69 Plan 03 (health monitor trigger) can now wire into the broadcast endpoint

## Self-Check

---
*Phase: 69-health-monitor-failover-orchestration*
*Completed: 2026-03-21*
