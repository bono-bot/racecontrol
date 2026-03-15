---
phase: 18-startup-self-healing
plan: 02
subsystem: protocol
tags: [serde, websocket, startup-report, self-heal, protocol]

# Dependency graph
requires:
  - phase: 18-startup-self-healing P01
    provides: self_heal::run(), startup_log::detect_crash_recovery(), config_hash(), SelfHealResult struct, crash_recovery and heal_result variables in main.rs
provides:
  - AgentMessage::StartupReport variant in protocol.rs with pod_id, version, uptime_secs, config_hash, crash_recovery, repairs fields
  - rc-agent sends startup report once per process lifetime after first WS connection
  - rc-core handles StartupReport by logging and recording pod activity
affects: [21-fleet-dashboard, pod-monitoring, deploy-verification]

# Tech tracking
tech-stack:
  added: []
  patterns: [fire-and-forget startup report, once-per-lifetime flag pattern]

key-files:
  created: []
  modified:
    - crates/rc-common/src/protocol.rs
    - crates/rc-agent/src/main.rs
    - crates/rc-core/src/ws/mod.rs

key-decisions:
  - "StartupReport sent once per process lifetime using startup_report_sent bool flag -- not on every reconnect"
  - "Fire-and-forget from agent side -- if send fails, log warning, do not disconnect"
  - "Core logs at INFO with WARN for crash recovery and repairs, records in pod activity for dashboard"

patterns-established:
  - "Once-per-lifetime message pattern: bool flag before reconnection loop, checked inside loop, set on successful send"
  - "Startup report message ordering: Register -> StartupReport -> ContentManifest"

requirements-completed: [HEAL-02]

# Metrics
duration: 6min
completed: 2026-03-15
---

# Phase 18 Plan 02: Startup Report Protocol Summary

**AgentMessage::StartupReport with serde roundtrip tests, sent once per process lifetime from rc-agent to rc-core with version, uptime, config hash, crash recovery flag, and self-heal repairs**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-15T09:23:54Z
- **Completed:** 2026-03-15T09:30:22Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Added AgentMessage::StartupReport variant with 6 fields (pod_id, version, uptime_secs, config_hash, crash_recovery, repairs) and 2 serde roundtrip tests
- rc-agent sends StartupReport once after first WS connection, using Plan 01's heal_result and crash_recovery data
- rc-core handles StartupReport by logging version/uptime/config_hash and warning on crash recovery or self-heal repairs
- Full test suite green: 100 rc-common + 199 rc-agent + 213 rc-core unit + 41 rc-core integration = 553 tests passing

## Task Commits

Each task was committed atomically:

1. **Task 1: Add AgentMessage::StartupReport variant with serde tests** - `65e48e1` (test)
2. **Task 2: Send StartupReport from rc-agent and handle in rc-core** - `bf46902` (feat)

## Files Created/Modified
- `crates/rc-common/src/protocol.rs` - Added StartupReport variant to AgentMessage enum + 2 roundtrip tests
- `crates/rc-agent/src/main.rs` - Added startup_report_sent flag + StartupReport send block after registration
- `crates/rc-core/src/ws/mod.rs` - Added match arm for StartupReport with logging and pod activity recording

## Decisions Made
- StartupReport sent once per process lifetime using `startup_report_sent` bool flag -- avoids log spam during flaky connections (Pitfall 6 from research)
- Fire-and-forget from agent side -- if WS send fails, log warning and retry on next connection, do not disconnect
- Message ordering: Register -> StartupReport -> ContentManifest (startup report is second message after registration)
- Core-side handling is log-only for Phase 18 -- Phase 21 (Fleet Dashboard) will add state storage

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- StartupReport data flowing from all pods to core, ready for Phase 21 Fleet Dashboard to store and display
- Phase 18 (Startup Self-Healing) is now complete: Plan 01 (self-heal + startup log) + Plan 02 (startup report protocol)
- Ready to proceed to Phase 19 (Watchdog) or Phase 21 (Fleet Dashboard)

## Self-Check: PASSED

- All 3 modified source files exist on disk
- All 1 created file (SUMMARY.md) exists on disk
- Commit 65e48e1 (Task 1) found in git log
- Commit bf46902 (Task 2) found in git log

---
*Phase: 18-startup-self-healing*
*Completed: 2026-03-15*
