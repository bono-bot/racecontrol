---
phase: 03-sync-hardening
plan: 01
subsystem: database
tags: [sqlite, migrations, cloud-sync, origin-tagging]

# Dependency graph
requires: []
provides:
  - "reservations table with PIN-based booking and status state machine"
  - "debit_intents table for wallet debit tracking with origin field"
  - "origin_id config field on CloudConfig for sync loop prevention"
  - "SCHEMA_VERSION 3 and SYNC_TABLES updated with new tables"
affects: [03-sync-hardening]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "origin_id tagging on sync payloads to prevent cloud-to-cloud loops"
    - "debit_intents pattern: pending -> processing -> completed/failed for 2-phase wallet operations"

key-files:
  created: []
  modified:
    - "crates/racecontrol/src/db/mod.rs"
    - "crates/racecontrol/src/config.rs"
    - "crates/racecontrol/src/cloud_sync.rs"

key-decisions:
  - "Placed new table migrations at end of run_migrations() before final Ok(())"
  - "origin_id defaults to 'local' via serde default function"

patterns-established:
  - "Origin tagging: every sync payload carries origin_id to identify sender"
  - "Reservation status state machine: pending_debit -> confirmed -> redeemed/expired/cancelled/failed"

requirements-completed: [SYNC-01, SYNC-02, SYNC-03]

# Metrics
duration: 3min
completed: 2026-03-21
---

# Phase 03 Plan 01: Schema Foundation Summary

**Reservations and debit_intents tables with origin_id config, SCHEMA_VERSION 3, and SYNC_TABLES update for cloud booking sync**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-21T12:07:44Z
- **Completed:** 2026-03-21T12:11:11Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Added reservations table with PIN, status state machine, driver FK, and 3 covering indexes
- Added debit_intents table with amount_paise, reservation_id, origin field, and 2 indexes
- Added origin_id field to CloudConfig with "local" default for sync origin tagging
- Bumped SCHEMA_VERSION from 2 to 3, added reservations and debit_intents to SYNC_TABLES
- Added origin tag to collect_push_payload for sync loop prevention

## Task Commits

Each task was committed atomically:

1. **Task 1: Add reservations and debit_intents table migrations** - `d71b9f6` (feat)
2. **Task 2: Add origin_id to CloudConfig, bump SCHEMA_VERSION, update SYNC_TABLES and payload** - `05457f1` (feat)

## Files Created/Modified
- `crates/racecontrol/src/db/mod.rs` - Added reservations and debit_intents CREATE TABLE migrations with indexes
- `crates/racecontrol/src/config.rs` - Added origin_id field to CloudConfig with default_origin_local function
- `crates/racecontrol/src/cloud_sync.rs` - Updated SYNC_TABLES, bumped SCHEMA_VERSION to 3, added origin to push payload

## Decisions Made
None - followed plan as specified

## Deviations from Plan
None - plan executed exactly as written.

## Issues Encountered
- Cargo package name is `racecontrol-crate` not `racecontrol` -- adjusted cargo check command accordingly. Pre-existing warnings about unused imports (unrelated to this plan).

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Schema foundation complete: reservations and debit_intents tables ready for Plan 02 sync integration
- CloudConfig.origin_id ready for both venue ("local") and VPS ("cloud") deployments
- SCHEMA_VERSION 3 ensures cloud side can gate on schema compatibility

---
*Phase: 03-sync-hardening*
*Completed: 2026-03-21*
