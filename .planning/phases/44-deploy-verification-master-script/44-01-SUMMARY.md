---
phase: 44-deploy-verification-master-script
plan: 01
subsystem: testing
tags: [bash, e2e, deploy, fleet-health, rc-sentry, ai-debugger]

requires:
  - phase: 41-e2e-test-foundation
    provides: lib/common.sh pass/fail/skip/summary_exit helpers and lib/pod-map.sh pod_ip()
  - phase: 43-wizard-flows-api-pipeline-tests
    provides: Confirmed rc-sentry port is 8091 (not 8090) and fleet/health endpoint shape

provides:
  - Gate-based deploy verification script covering DEPL-01, DEPL-02, and DEPL-04
  - Binary swap detection via rc-sentry remote exec on canary pod
  - Port conflict detection for kiosk :3300 with 30s EADDRINUSE poll loop
  - Fleet-wide ws_connected and build_id consistency checks
  - AI debugger log routing on every failure path

affects:
  - phase-44-run-all (run-all.sh will invoke this script as deploy gate)

tech-stack:
  added: []
  patterns:
    - "Gate-based verification: numbered gates 0-7 with early exit only on server-down"
    - "Fleet/health fetched once and reused across Gates 5-7 to avoid redundant HTTP calls"
    - "log_to_ai_debugger() helper appends structured failure lines to results/ai-debugger-input.log"
    - "EADDRINUSE detection via 30s poll loop (6 x 5s curl probes)"

key-files:
  created:
    - tests/e2e/deploy/verify.sh
  modified: []

key-decisions:
  - "Fleet/health response fetched once and cached in BASH variable for Gates 5-7 — avoids 3 redundant HTTP calls"
  - "Gate 3 polls :3300 up to 30s with 5s intervals to detect EADDRINUSE vs clean down — matches DEPL-01 spec"
  - "build_id missing from fleet/health results in skip (not fail) — older rc-agent versions may not report it"
  - "POLL_ATTEMPT loop uses sleep 5 per iteration for 30s total coverage — same pattern as game-launch.sh Gate 5"
  - "Binary size extracted with python3 regex fallback in case rc-sentry /exec returns non-JSON"
  - "summary_exit at end of Gate 3 poll failure replaced with regular fail() — script continues to collect all gate results"

patterns-established:
  - "verify.sh pattern: fetch fleet/health once, reuse for multiple gate checks"
  - "log_to_ai_debugger(gate_name, failure_msg) — consistent schema for AI debugger input across all gates"

requirements-completed: [DEPL-01, DEPL-02, DEPL-04]

duration: 2min
completed: 2026-03-19
---

# Phase 44 Plan 01: Deploy Verification Master Script Summary

**Gate-based deploy verification script with binary swap check (rc-sentry :8091), EADDRINUSE port-conflict polling on :3300, fleet ws_connected/build_id consistency, and per-failure AI debugger log routing**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-19T13:30:15Z
- **Completed:** 2026-03-19T13:32:30Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments
- Created `tests/e2e/deploy/verify.sh` with 8 gates covering all three requirements: DEPL-01 (binary swap + port conflicts + health), DEPL-02 (fleet WS connectivity + build_id + installed_games), and DEPL-04 (AI debugger log routing)
- Every `fail()` call is paired with a `log_to_ai_debugger()` call — failures append to `results/ai-debugger-input.log` with gate name, timestamp, and diagnostic detail
- Gate 3 detects EADDRINUSE on kiosk :3300 with a 30-second poll loop (6 probes x 5s), differentiating a stuck port from a clean service restart
- Fleet health fetched once and reused across Gates 5, 6, and 7 — no redundant HTTP calls

## Task Commits

Each task was committed atomically:

1. **Task 1: Create deploy/verify.sh with binary swap, port conflict, fleet health, and AI debugger routing** - `c6d5ac4` (feat)

**Plan metadata:** (docs commit follows)

## Files Created/Modified
- `tests/e2e/deploy/verify.sh` — 8-gate deploy verification script sourcing lib/common.sh + lib/pod-map.sh

## Decisions Made
- Fetched fleet/health once before Gate 5 and stored in `$FLEET_RESP` — reused for Gates 6 and 7 to avoid three identical 10s HTTP calls
- Gate 3 uses `sleep 5` in poll loop rather than `sleep 1` — avoids hammering the server during a restart; 30s total window is sufficient for Node.js startup
- `build_id` absence from fleet response is a `skip()` not a `fail()` — older rc-agent versions predate this field, and the absence alone doesn't indicate a failed deploy
- python3 regex fallback in Gate 2 for binary size: rc-sentry /exec may return stdout as a field or as raw text depending on version

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- `tests/e2e/deploy/verify.sh` is complete and syntactically valid
- Phase 44 can now create `run-all.sh` to invoke this script alongside smoke.sh, game-launch.sh, and the API tests
- No blockers — script follows established common.sh gate pattern and is independently runnable

---
*Phase: 44-deploy-verification-master-script*
*Completed: 2026-03-19*
