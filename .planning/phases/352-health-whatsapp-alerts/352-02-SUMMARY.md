---
phase: 352-health-whatsapp-alerts
plan: 02
subsystem: alerting
tags: [whatsapp, evolution-api, comms-link, relay, fallback, health-alerts]

# Dependency graph
requires:
  - phase: 352-01
    provides: subsystem_health.rs with dispatch_subsystem_alert, probes, dedup
provides:
  - POST /relay/alert endpoint on comms-link James relay server (:8766)
  - GET /relay/alert config status endpoint
  - Fallback chain in subsystem_health: direct Evolution API -> comms-link relay
affects: [352-03, admin-dashboard, deploy-comms-link]

# Tech tracking
tech-stack:
  added: []
  patterns: [fallback-chain-dispatch, relay-alert-forwarding]

key-files:
  created: []
  modified:
    - comms-link/james/index.js
    - crates/racecontrol/src/subsystem_health.rs

key-decisions:
  - "Import sendEvolutionText directly at top-level (ESM static import) rather than dynamic import -- james/index.js already imports AlertCooldown from the same module"
  - "try_direct_whatsapp() implemented inline in subsystem_health.rs rather than modifying send_whatsapp() signature -- avoids breaking 10+ existing callers"
  - "Relay fallback hardcoded to 192.168.31.27:8766 (James LAN IP) -- matches venue topology, configurable relay URL is out of scope"

patterns-established:
  - "Fallback chain dispatch: try primary path -> on failure try relay -> log both outcomes"
  - "Relay alert endpoint: validate message (400), check config (503), dispatch and return result"

requirements-completed: [OPS-03]

# Metrics
duration: 7min
completed: 2026-04-10
---

# Phase 352 Plan 02: Comms-Link /relay/alert Endpoint + Relay Fallback Summary

**POST /relay/alert endpoint on James relay with fallback chain in subsystem_health dispatch (direct Evolution API -> comms-link relay -> error logging)**

## Performance

- **Duration:** 7 min
- **Started:** 2026-04-10T17:47:42Z
- **Completed:** 2026-04-10T17:55:02Z
- **Tasks:** 2/2
- **Files modified:** 2

## Accomplishments
- Added POST /relay/alert and GET /relay/alert endpoints to comms-link James relay server with full validation (400 on missing message, 503 on unconfigured Evolution API)
- Implemented fallback chain in subsystem_health.rs: try_direct_whatsapp() -> try_relay_fallback() -> error logging, with incident always recorded
- Both dispatch paths log outcomes at appropriate tracing levels (info/warn/error) for audit trail

## Task Commits

Each task was committed atomically:

1. **Task 1: Add POST /relay/alert endpoint to James relay server** - `6bda9a2` (feat) [comms-link repo]
2. **Task 2: Add relay fallback to subsystem_health alert dispatch** - `e6de7791` (feat) [racecontrol repo]

## Files Created/Modified
- `comms-link/james/index.js` - Added GET+POST /relay/alert routes with validation, Evolution API dispatch via sendEvolutionText, audit logging
- `crates/racecontrol/src/subsystem_health.rs` - Added try_direct_whatsapp(), try_relay_fallback(), updated dispatch_subsystem_alert() with fallback chain

## Deviations from Plan

None - plan executed exactly as written.

## Known Stubs

None - all code paths are fully wired.

## Verification

- `grep -c "/relay/alert" comms-link/james/index.js` = 4 (GET route, POST route, URL refs)
- `grep -c "try_relay_fallback" crates/racecontrol/src/subsystem_health.rs` = 2 (definition + call)
- `cargo check -p racecontrol-crate` = success (0 errors, 1 pre-existing warning)
- `cargo test -p racecontrol-crate --lib -- subsystem_health` = 9 passed, 0 failed
- `node -c james/index.js` = syntax valid

## Notes

- Pre-existing `log_sync` module reference in main.rs:1163 causes `cargo test --bin` to fail, but this is from Plan 352-03 (not yet executed) and is out of scope. `cargo check` and `cargo test --lib` both succeed.
- The relay fallback URL is hardcoded to `http://192.168.31.27:8766/relay/alert` (James's LAN IP). This matches the venue network topology and is the simplest correct approach. A configurable relay URL could be added later if needed.
