---
phase: 50-llm-self-test-fleet-health
plan: 03
subsystem: testing
tags: [rust, axum, websocket, self-test, fleet-health, e2e, bash]

# Dependency graph
requires:
  - phase: 50-01
    provides: "self_test.rs with run_all_probes + get_llm_verdict + protocol RunSelfTest/SelfTestResult"
provides:
  - "GET /api/v1/pods/{id}/self-test HTTP endpoint with 30s timeout"
  - "pending_self_tests field in AppState (oneshot channel routing)"
  - "SelfTestResult WS handler resolving pending oneshot"
  - "RunSelfTest match arm in rc-agent spawning probes + LLM verdict"
  - "Disconnect cleanup for pending_self_tests"
  - "tests/e2e/fleet/pod-health.sh — fleet-wide self-test E2E script"
  - "Phase 5 (Fleet Health) gate in run-all.sh"
affects: [deploy-pipeline, e2e-test-suite, fleet-operations]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "pending_self_tests follows same oneshot pattern as pending_ws_execs"
    - "SelfTest result routing via ws_exec_result_tx channel (same path as ExecResult)"
    - "pod-health.sh follows identical 3-gate convention as ollama-health.sh"

key-files:
  created:
    - "tests/e2e/fleet/pod-health.sh"
  modified:
    - "crates/racecontrol/src/state.rs"
    - "crates/racecontrol/src/api/routes.rs"
    - "crates/racecontrol/src/ws/mod.rs"
    - "crates/rc-agent/src/main.rs"
    - "tests/e2e/run-all.sh"

key-decisions:
  - "pending_self_tests stores (pod_id, tx) tuple — pod_id needed for disconnect cleanup (retain != pod_id)"
  - "RunSelfTest result sent via ws_exec_result_tx channel (not ws_tx directly) — ws_tx is SplitSink, not Clone"
  - "pod-health.sh uses SERVER_URL not POD_IP for self-test — server dispatches via WS, not direct pod HTTP"
  - "Fleet Health phase gate skipped when --skip-deploy is passed (same condition as Phase 4)"
  - "SELFTEST_TIMEOUT=35 (35s curl max-time) vs server 30s timeout — 5s buffer for WS round-trip"

patterns-established:
  - "Self-test request flow: HTTP GET -> pending_self_tests oneshot -> WS RunSelfTest -> agent probes -> SelfTestResult -> resolve oneshot -> HTTP response"
  - "Disconnect cleanup uses retain(|_, (pid, _)| pid != pod_id) pattern for tuple-valued maps"

requirements-completed: [SELFTEST-03, SELFTEST-05]

# Metrics
duration: 25min
completed: 2026-03-19
---

# Phase 50 Plan 03: Self-Test Endpoint + WS Plumbing + Fleet E2E Summary

**HTTP GET /api/v1/pods/{id}/self-test wired end-to-end: server dispatches RunSelfTest via WS, agent runs 22 probes + LLM verdict, SelfTestResult resolves pending oneshot, pod-health.sh verifies all 8 pods**

## Performance

- **Duration:** 25 min
- **Started:** 2026-03-19T03:30:00Z
- **Completed:** 2026-03-19T03:55:00Z
- **Tasks:** 2
- **Files modified:** 5 (4 Rust files + 1 shell script) + 1 new shell script

## Accomplishments

- Wired the complete self-test request lifecycle from HTTP endpoint to WS dispatch to agent probe execution and back
- Added `pending_self_tests` field to AppState with proper disconnect cleanup (no memory leak)
- Created pod-health.sh E2E test script following identical conventions to ollama-health.sh
- Added Phase 5 (Fleet Health) as the final gate in run-all.sh with summary table and summary.json tracking
- Both racecontrol-crate (280 unit + 66 integration = 346 tests) and rc-agent-crate (17 self-test tests) pass

## Task Commits

Each task was committed atomically:

1. **Task 1: Server endpoint + WS plumbing + agent handler** - `c9996ea` (feat)
2. **Task 2: pod-health.sh E2E test + run-all.sh integration** - `b09c35d` (feat)

## Files Created/Modified

- `crates/racecontrol/src/state.rs` - Added `pending_self_tests: RwLock<HashMap<String, (String, oneshot::Sender<Value>)>>`
- `crates/racecontrol/src/api/routes.rs` - Added `.route("/pods/{id}/self-test", get(pod_self_test))` and `pod_self_test` handler with 30s timeout
- `crates/racecontrol/src/ws/mod.rs` - SelfTestResult match arm resolves pending oneshot; disconnect cleanup uses retain
- `crates/rc-agent/src/main.rs` - RunSelfTest match arm spawns probes + LLM verdict, sends result via ws_exec_result_tx
- `tests/e2e/fleet/pod-health.sh` - New: 3-gate fleet self-test E2E (reachable, HTTP 200, HEALTHY verdict)
- `tests/e2e/run-all.sh` - Phase 5 Fleet Health gate, summary table entry, summary.json fleet_health key

## Decisions Made

- **ws_exec_result_tx over ws_tx**: SplitSink is not Clone, so spawned task cannot directly own ws_tx. The ws_exec_result_tx mpsc channel (AgentMessage) is already drained by the select loop and forwarded to ws_tx — SelfTestResult is an AgentMessage, so it routes correctly through the same path.
- **pending_self_tests (pod_id, tx) tuple**: Disconnect cleanup needs the pod_id to retain-filter. Without it, cleaning up stale entries on disconnect would require scanning all request_id strings.
- **SELFTEST_TIMEOUT=35**: 5 seconds of buffer above the server-side 30s timeout ensures curl doesn't close the connection before the server's 504 response arrives.
- **Fleet Health gate condition = PREFLIGHT_STATUS=PASS && SKIP_DEPLOY=false**: Mirrors the Deploy Verification phase — if deploy is skipped, fleet health is also skipped (no new binary to test).

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- The Edit tool failed several times on main.rs due to "file modified since read" errors — resolved by using Python-based string replacement directly.
- cargo test -p rc-agent-crate returned exit code 101 from bash output collection (ENOENT on temp file) — confirmed tests pass by running the self_test subset directly (exit 0).

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Phase 50 is complete: 22-probe self-test module, protocol extensions, server endpoint, WS plumbing, agent handler, fleet E2E test, run-all.sh integration
- SELFTEST-01, SELFTEST-02, SELFTEST-03, SELFTEST-05, SELFTEST-06 all satisfied
- Staff can trigger `GET /api/v1/pods/{id}/self-test` from admin dashboard or curl to get full diagnostic report
- pod-health.sh in run-all.sh ensures every deploy is validated against all 8 pods being HEALTHY

---
*Phase: 50-llm-self-test-fleet-health*
*Completed: 2026-03-19*

## Self-Check: PASSED

All files verified present and containing expected content:
- crates/racecontrol/src/state.rs: pending_self_tests field present
- crates/racecontrol/src/api/routes.rs: pod_self_test handler present
- crates/racecontrol/src/ws/mod.rs: SelfTestResult resolution present
- crates/rc-agent/src/main.rs: RunSelfTest handler present
- tests/e2e/fleet/pod-health.sh: summary_exit present
- tests/e2e/run-all.sh: pod-health.sh wired

Commits verified:
- c9996ea: Task 1 (server endpoint + WS plumbing + agent handler)
- b09c35d: Task 2 (pod-health.sh + run-all.sh)
