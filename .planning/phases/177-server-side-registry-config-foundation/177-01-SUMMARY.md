---
phase: 177-server-side-registry-config-foundation
plan: 01
subsystem: api
tags: [feature-flags, sqlite, websocket, axum, audit-log, runtime-config]

requires: []
provides:
  - "SQLite tables: feature_flags, config_push_queue, config_audit_log"
  - "flags.rs module: FeatureFlagRow, list_flags, create_flag, update_flag handlers"
  - "AppState.feature_flags RwLock in-memory cache"
  - "AppState.config_push_seq AtomicU64 sequence counter"
  - "AppState.load_feature_flags() startup population"
  - "AppState.broadcast_flag_sync() FlagSync broadcast to pods"
  - "REST: GET/POST /api/v1/flags, PUT /api/v1/flags/{name}"
affects: [177-02, 177-03, rc-agent flag cache sync, admin dashboard flag UI]

tech-stack:
  added: []
  patterns:
    - "Flag cache pattern: DB is source of truth, RwLock cache serves reads, mutations write DB then update cache then broadcast"
    - "Audit trail pattern: every mutation inserts into config_audit_log with pushed_by from StaffClaims.sub"
    - "FlagSync broadcast: iterates agent_senders, sends CoreToAgentMessage::FlagSync on every mutation"

key-files:
  created:
    - crates/racecontrol/src/flags.rs
  modified:
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/state.rs
    - crates/racecontrol/src/lib.rs
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/src/main.rs

key-decisions:
  - "FeatureFlagRow declared in flags.rs, imported into state.rs — circular module dependency within crate is fine in Rust"
  - "validate_flag_name enforces alphanumeric+underscore max-64 rule; validate_overrides enforces pod_N key pattern"
  - "update_flag reads current state from cache (not DB) for old_value in audit log, then re-fetches from DB for new_value"
  - "FlagSync version = max(version) across all flags in cache — simple monotonic proxy for fleet-wide version"

requirements-completed: [FF-01, FF-02, FF-03, CP-05]

duration: 25min
completed: 2026-03-24
---

# Phase 177 Plan 01: Feature Flag Registry Summary

**SQLite-backed feature flag registry with REST CRUD (GET/POST/PUT /flags), in-memory RwLock cache, real-time FlagSync WS broadcast to pods, and config_audit_log audit trail sourced from StaffClaims.sub**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-03-24T08:00:00Z
- **Completed:** 2026-03-24T08:25:00Z
- **Tasks:** 1 of 1
- **Files modified:** 6

## Accomplishments
- 3 new SQLite tables (feature_flags, config_push_queue, config_audit_log) added to the migrate() chain in db/mod.rs
- flags.rs created with FeatureFlagRow, CreateFlagRequest, UpdateFlagRequest, and three async handlers
- AppState gains feature_flags RwLock cache + config_push_seq AtomicU64, both initialized at startup via load_feature_flags()
- broadcast_flag_sync() sends CoreToAgentMessage::FlagSync to all connected pods via agent_senders on every mutation
- Every create/update writes a config_audit_log entry with pushed_by = StaffClaims.sub; no .unwrap() anywhere

## Task Commits

1. **Task 1: DB migration + AppState additions + flags.rs module + routes** - `35bff468` (feat)

**Plan metadata:** (docs commit — created after task)

## Files Created/Modified
- `crates/racecontrol/src/flags.rs` - Feature flag CRUD handlers (list, create, update), FeatureFlagRow type, validation logic
- `crates/racecontrol/src/db/mod.rs` - Added feature_flags, config_push_queue, config_audit_log CREATE TABLE blocks
- `crates/racecontrol/src/state.rs` - feature_flags RwLock, config_push_seq AtomicU64, load_feature_flags(), broadcast_flag_sync()
- `crates/racecontrol/src/lib.rs` - Added `pub mod flags;`
- `crates/racecontrol/src/api/routes.rs` - Imported `crate::flags`, registered /flags and /flags/{name} routes in staff_routes()
- `crates/racecontrol/src/main.rs` - Added `state.load_feature_flags().await` after seed_pods_on_startup

## Decisions Made
- FeatureFlagRow declared in flags.rs and imported into state.rs — circular module dependency within the same Rust crate is valid
- Flag name validation enforces alphanumeric+underscore, max 64 chars; override keys must match pod_N pattern
- update_flag reads old state from RwLock cache for audit log old_value (avoids extra DB read)
- FlagSync version sent to pods is max(row.version) across all flags — simple proxy for fleet-wide version

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None - compiled cleanly on first build attempt.

## Next Phase Readiness
- Feature flag registry foundation complete; 177-02 (ConfigPush delivery queue) can build on config_push_seq and config_push_queue table
- GET /api/v1/flags returns current state; PUT /api/v1/flags/:name supports partial updates (only provided fields change)
- FlagCacheSync handling (agent reconnect) to be added in 177-03 when rc-agent integration is implemented

## Self-Check: PASSED

- flags.rs: FOUND
- SUMMARY.md: FOUND
- Commit 35bff468: FOUND
- cargo build --bin racecontrol: Finished (no errors)
- All acceptance criteria grep checks: PASS

---
*Phase: 177-server-side-registry-config-foundation*
*Completed: 2026-03-24*
