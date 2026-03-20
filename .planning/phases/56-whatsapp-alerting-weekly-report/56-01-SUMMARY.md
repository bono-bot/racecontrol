---
phase: 56-whatsapp-alerting-weekly-report
plan: 01
subsystem: monitoring
tags: [whatsapp, evolution-api, alerting, broadcast, chrono-tz, sqlite]

# Dependency graph
requires:
  - phase: 54-structured-logging-error-rate-alerting
    provides: ErrorCountLayer, error_rate.rs mpsc channel, MonitoringConfig
provides:
  - whatsapp_alerter.rs module with P0 detection and Evolution API delivery
  - AlertingConfig struct in config.rs
  - broadcast channel migration in error_rate.rs (mpsc -> broadcast)
  - pod_uptime_samples and alert_incidents SQLite tables
affects: [56-02-weekly-report, monitoring, alerting]

# Tech tracking
tech-stack:
  added: [chrono-tz 0.9]
  patterns: [broadcast channel for multi-subscriber alerting, P0 state machine with cooldown]

key-files:
  created:
    - crates/racecontrol/src/whatsapp_alerter.rs
  modified:
    - crates/racecontrol/src/config.rs
    - crates/racecontrol/src/error_rate.rs
    - crates/racecontrol/src/main.rs
    - crates/racecontrol/src/lib.rs
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/Cargo.toml

key-decisions:
  - "broadcast::channel replaces mpsc for error rate alerts -- enables both email and WhatsApp alerters to subscribe independently"
  - "P0State is internal to whatsapp_alerter (not shared with AppState) -- keeps alert state isolated"
  - "2-second debounce on PodOffline before counting online pods -- absorbs cascading disconnects"
  - "enable_time() added to test runtime builders -- broadcast recv requires tokio time driver"

patterns-established:
  - "Broadcast channel pattern: subscribe before passing sender to layer, both alerter tasks get independent receivers"
  - "P0 state machine: track since/last_alert per event type, cooldown gating, resolved detection via periodic timer"

requirements-completed: [MON-06]

# Metrics
duration: 8min
completed: 2026-03-20
---

# Phase 56 Plan 01: WhatsApp P0 Alerter Summary

**WhatsApp P0 alerter with all-pods-offline + error-rate detection, Evolution API delivery, IST timestamps, rate limiting, and incident recording in SQLite**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-20T10:39:04Z
- **Completed:** 2026-03-20T10:47:18Z
- **Tasks:** 2
- **Files modified:** 8

## Accomplishments
- Created whatsapp_alerter.rs with P0 detection for all-pods-offline (via bono_event_tx broadcast) and error rate spikes (via broadcast channel), Evolution API delivery, IST timestamps, cooldown-based rate limiting, and SQLite incident recording
- Added AlertingConfig to config.rs with enabled, uday_phone, cooldown_secs fields
- Converted error_rate.rs alert channel from mpsc to broadcast so both email and WhatsApp alerters can subscribe independently
- Added pod_uptime_samples and alert_incidents SQLite tables for Plan 02 weekly report

## Task Commits

Each task was committed atomically:

1. **Task 1: AlertingConfig + whatsapp_alerter.rs module + SQLite tables** - `dc9fbf5` (feat)
2. **Task 2: Convert error_rate mpsc to broadcast + wire whatsapp_alerter in main.rs** - `309e218` (feat)
3. **Cargo.lock update** - `11b0cf7` (chore)

## Files Created/Modified
- `crates/racecontrol/src/whatsapp_alerter.rs` - P0 alerter module: pod offline detection, error rate monitoring, Evolution API WhatsApp delivery, IST timestamps, incident recording
- `crates/racecontrol/src/config.rs` - Added AlertingConfig struct (enabled, uday_phone, cooldown_secs)
- `crates/racecontrol/src/error_rate.rs` - Converted mpsc to broadcast channel for multi-subscriber alerting
- `crates/racecontrol/src/main.rs` - Wired whatsapp_alerter_task spawn with broadcast subscriber and alerting.enabled guard
- `crates/racecontrol/src/lib.rs` - Registered whatsapp_alerter module
- `crates/racecontrol/src/db/mod.rs` - Added pod_uptime_samples and alert_incidents tables with indexes
- `crates/racecontrol/Cargo.toml` - Added chrono-tz 0.9 dependency
- `Cargo.lock` - Updated for chrono-tz dependency tree

## Decisions Made
- broadcast::channel replaces mpsc for error rate alerts -- enables both email and WhatsApp alerters to subscribe independently without channel contention
- P0State is internal to whatsapp_alerter (not shared with AppState) -- keeps alert state isolated and avoids polluting shared state
- 2-second debounce on PodOffline before counting online pods -- absorbs cascading disconnects that happen in quick succession
- enable_time() added to error_rate test runtime builders -- broadcast recv requires tokio time driver even when value is already in channel

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added enable_time() to error_rate test runtime builders**
- **Found during:** Task 2 (broadcast migration)
- **Issue:** broadcast::Receiver::recv() needs tokio time driver; existing tests used new_current_thread() without enable_time()
- **Fix:** Added .enable_time() to both test runtime builders
- **Files modified:** crates/racecontrol/src/error_rate.rs
- **Verification:** All 4 error_rate tests pass
- **Committed in:** 309e218 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Necessary fix for broadcast migration to work with existing test patterns. No scope creep.

## Issues Encountered
None beyond the auto-fixed deviation above.

## User Setup Required
None - no external service configuration required. AlertingConfig defaults to enabled=false; Uday's phone number and Evolution API credentials are configured in racecontrol.toml when ready.

## Next Phase Readiness
- whatsapp_alerter.rs ready for production use once [alerting] section added to racecontrol.toml
- pod_uptime_samples and alert_incidents tables created and ready for Plan 02 (weekly report)
- broadcast channel pattern established for future multi-subscriber alerting

---
*Phase: 56-whatsapp-alerting-weekly-report*
*Completed: 2026-03-20*
