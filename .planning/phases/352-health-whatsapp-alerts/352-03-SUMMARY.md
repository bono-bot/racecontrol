---
phase: 352-health-whatsapp-alerts
plan: 03
subsystem: logging
tags: [tracing, tower-http, scp, rsync, jsonl, structured-logging]

requires:
  - phase: 352-01
    provides: subsystem_health.rs module and spawn pattern in main.rs
provides:
  - Structured JSON request logging via customized TraceLayer (method, route, status, latency_ms, correlation_id)
  - log_sync.rs background task for daily SCP of JSONL logs to Bono VPS
affects: [354-ui-hardening, deploy-pipeline]

tech-stack:
  added: []
  patterns: [TraceLayer customization with make_span_with/on_request/on_response, daily SCP sync with IST window check]

key-files:
  created: [crates/racecontrol/src/log_sync.rs]
  modified: [crates/racecontrol/src/main.rs, crates/racecontrol/src/lib.rs]

key-decisions:
  - "Used hardcoded Bono VPS IP constant in log_sync.rs (matches event_archive.rs default)"
  - "Correlation ID generated per-request via uuid in TraceLayer make_span_with"
  - "admin_api target name for structured log filtering"

patterns-established:
  - "TraceLayer customization: make_span_with + on_request + on_response for structured HTTP logging"
  - "Daily SCP sync pattern: hourly interval check with IST 02:00-04:00 window and once-per-day dedup"

requirements-completed: [OPS-06, OPS-07]

duration: 10min
completed: 2026-04-10
---

# Phase 352 Plan 03: Structured JSON Request Logging + Log Sync Summary

**Customized TraceLayer with per-request correlation_id, method/route/status/latency_ms structured logging, plus daily SCP rsync of JSONL logs to Bono VPS during IST 02:00-04:00 window**

## Performance

- **Duration:** 10 min
- **Started:** 2026-04-10T17:47:14Z
- **Completed:** 2026-04-10T17:57:20Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Customized TraceLayer with make_span_with (correlation_id), on_request (method, route), and on_response (status, latency_ms) callbacks
- Added admin_api=info to env_filter for structured log capture
- Created log_sync.rs module with background task for daily SCP of logs/*.jsonl to Bono VPS /root/backups/venue-logs/
- IST 02:00-04:00 sync window with once-per-day dedup (matching event_archive.rs pattern)

## Task Commits

Each task was committed atomically:

1. **Task 1: Customize TraceLayer for structured admin API request logging** - `4bb5fa77` (feat)
2. **Task 2: Create log_sync.rs background task for rsync to Bono VPS** - `712943e8` (feat)

## Files Created/Modified
- `crates/racecontrol/src/log_sync.rs` - Background task for daily SCP of JSONL logs to Bono VPS with IST window check
- `crates/racecontrol/src/main.rs` - Customized TraceLayer with structured callbacks, added log_sync::spawn, admin_api in env_filter
- `crates/racecontrol/src/lib.rs` - Registered pub mod log_sync

## Decisions Made
- Used hardcoded constants for Bono VPS IP and remote path (matching event_archive.rs defaults) rather than config-driven approach -- keeps the module simple and consistent with the plan
- Placed chrono::Timelike import at module top (not bottom as in plan template) for standard Rust style
- Used ok_or_else instead of ok_or for the filename conversion to avoid unnecessary String allocation

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Known Stubs
None - all functionality is fully wired.

## Next Phase Readiness
- Structured logging is active -- admin API requests now produce JSON log lines with correlation_id, method, route, status, latency_ms
- Log sync task will begin syncing during the next IST 02:00-04:00 window
- Ready for Phase 354 health page UI to consume the /api/v1/health subsystems data (delivered in 352-01)

## Self-Check: PASSED

- [x] crates/racecontrol/src/log_sync.rs exists
- [x] .planning/phases/352-health-whatsapp-alerts/352-03-SUMMARY.md exists
- [x] Commit 4bb5fa77 found in git history
- [x] Commit 712943e8 found in git history

---
*Phase: 352-health-whatsapp-alerts*
*Completed: 2026-04-10*
