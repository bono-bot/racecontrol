---
phase: 45-close-wait-fix-connection-hygiene
plan: 02
subsystem: infra
tags: [reqwest, connection-pooling, close-wait, e2e-testing, fleet-health, tcp]

# Dependency graph
requires:
  - phase: 45-01
    provides: Connection:close middleware on server side (axum layer)
provides:
  - reqwest probe client with connection pooling disabled (pool_max_idle_per_host(0))
  - tests/e2e/fleet/close-wait.sh E2E verification script for CLOSE_WAIT hygiene
affects: [45-close-wait-fix-connection-hygiene, fleet-health, e2e-testing]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "pool_max_idle_per_host(0) on reqwest::ClientBuilder eliminates idle connection pooling — forces TCP close after each request"
    - "E2E shell scripts under tests/e2e/fleet/ source lib/common.sh and lib/pod-map.sh, follow pass/fail/skip/summary_exit pattern"

key-files:
  created:
    - tests/e2e/fleet/close-wait.sh
  modified:
    - crates/racecontrol/src/fleet_health.rs

key-decisions:
  - "pool_max_idle_per_host(0) on probe_client is belt-and-suspenders alongside server-side Connection: close — both ends actively close TCP after each probe"
  - "E2E close-wait.sh uses rc-agent /exec endpoint to run netstat on the pod itself — avoids needing SSH or external tooling"
  - "THRESHOLD=5 matches self_monitor.rs internal threshold — consistent alerting boundary across runtime and test suite"
  - "close-wait.sh skips pods where rc-agent :8090/ping does not return pong — non-reachable pods are informational skips, not failures"

patterns-established:
  - "Fleet E2E tests live in tests/e2e/fleet/ subdirectory, following same lib sourcing pattern as tests/e2e/smoke.sh"

requirements-completed: [CONN-HYG-01]

# Metrics
duration: 8min
completed: 2026-03-19
---

# Phase 45 Plan 02: Close-Wait Fix Connection Hygiene Summary

**reqwest probe client connection pooling disabled via pool_max_idle_per_host(0), plus CLOSE_WAIT E2E verification script that checks all 8 pods via netstat over rc-agent /exec**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-19T00:35:00Z
- **Completed:** 2026-03-19T00:43:00Z
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments
- Added `pool_max_idle_per_host(0)` to reqwest probe client in fleet_health.rs — connections close immediately after each health probe response
- Created tests/e2e/fleet/close-wait.sh — polls all 8 pods via rc-agent /exec, runs netstat, counts CLOSE_WAIT on :8090, asserts count <5
- All 13 existing fleet_health unit tests pass with the change

## Task Commits

Each task was committed atomically:

1. **Task 1: fleet_health.rs pool_max_idle_per_host(0) + close-wait.sh E2E test** - `ad3fae7` (feat)

**Plan metadata:** (docs commit follows)

## Files Created/Modified
- `crates/racecontrol/src/fleet_health.rs` - Added `.pool_max_idle_per_host(0)` to probe_client builder in start_probe_loop()
- `tests/e2e/fleet/close-wait.sh` - New E2E script: checks CLOSE_WAIT socket count on :8090 for all 8 pods via rc-agent /exec endpoint

## Decisions Made
- pool_max_idle_per_host(0) complements the server-side Connection: close middleware from Plan 01 — both sides actively close TCP after each request, eliminating CLOSE_WAIT accumulation from either end
- close-wait.sh uses /exec on the pod to run netstat locally rather than pulling socket stats through the server — matches how self_monitor.rs counts internally
- Pods that fail the /ping reachability check are SKIP (not FAIL) — offline pods are informational, not a hygiene failure

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Both client-side (Plan 02) and server-side (Plan 01) CLOSE_WAIT fixes are complete
- Phase 45 is fully complete — deploy fleet_health.rs change to server and run close-wait.sh against live pods to confirm <5 CLOSE_WAIT sockets per pod after 30-minute soak

---
*Phase: 45-close-wait-fix-connection-hygiene*
*Completed: 2026-03-19*
