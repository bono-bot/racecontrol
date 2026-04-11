---
phase: 366-fleet-intelligence
plan: 02
subsystem: api
tags: [rust, axum, sqlite, content-drift, websocket, whatsapp, cloud-sync, background-task]

# Dependency graph
requires:
  - phase: 366-fleet-intelligence
    plan: 01
    provides: fleet_intelligence module and fleet_intelligence_handler
  - phase: 361-inventory
    provides: GET :8090/debug/content-dirs endpoint on rc-agent + ContentDirsResponse type
  - phase: 301-cloud-sync
    provides: cloud_sync pipeline for pushing tables to cloud
provides:
  - ContentDriftDetected WS event variant in DashboardEvent enum (rc-common/protocol.rs)
  - content_drift.rs background task polling all 8 pods every 60 minutes
  - content_drift_events SQLite table (id, pod_id, detected_at, game_key, delta_type, item, resolved_at, resolution_note)
  - Cloud sync for content_drift_events via Phase 301 pipeline
affects: [366-03, 366-04, cloud_sync, protocol, admin-dashboard]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Background polling pattern: interval tick, skip first tick, silently skip offline pods"
    - "Drift detection: HashSet difference between TOML expected and live disk inventory"
    - "Selective alerting: WhatsApp only for game_removed (P2-10 severity), not all drift types"

key-files:
  created:
    - crates/racecontrol/src/content_drift.rs
  modified:
    - crates/rc-common/src/protocol.rs
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/lib.rs
    - crates/racecontrol/src/main.rs
    - crates/racecontrol/src/cloud_sync.rs

key-decisions:
  - "60-minute poll interval (not real-time) — content drift is a slow-moving problem, polling avoids agent load"
  - "Skip first tick to avoid run at startup before pods connect"
  - "WhatsApp alert only for game_removed — game additions are non-critical, removals risk session failure"
  - "Offline pods skipped silently — drift check for unreachable pod = false positives"

patterns-established:
  - "Content drift event: (game_key, delta_type, item) where delta_type in {game_added, game_removed, car_added, car_removed, track_added, track_removed}"
  - "Background task spawn pattern: spawn_X_task(Arc<AppState>) called from main.rs after AppState init"

requirements-completed: [GLD-F-03]

# Metrics
duration: 30min
completed: 2026-04-11
---

# Phase 366 Plan 02: Content Drift Detector Summary

**60-minute background poller comparing pod TOML inventory vs live :8090/debug/content-dirs disk state, emitting ContentDriftDetected WS events with WhatsApp alert for game_removed**

## Performance

- **Duration:** ~30 min
- **Started:** 2026-04-11T02:01:00Z
- **Completed:** 2026-04-11T02:10:00Z
- **Tasks:** 3
- **Files modified:** 6

## Accomplishments
- Added `ContentDriftDetected` variant to `DashboardEvent` enum in `rc-common/src/protocol.rs` with fields: pod_id, game_key, delta_type, item, detected_at
- Created `content_drift.rs` with `spawn_content_drift_task` (60-min interval, skip first tick), `check_all_pods_drift` (polls pods 1-8), and `emit_drift_events` (DB insert + WS broadcast + conditional WhatsApp)
- Added `content_drift_events` table migration and indexes in `db/mod.rs` Phase 366 block
- Wired `spawn_content_drift_task` into `main.rs`, declared `pub mod content_drift` in `lib.rs`, and added `content_drift_events` push to `cloud_sync.rs`

## Task Commits

1. **Task 1: Add content_drift_events migration** - `47a22520` (feat)
2. **Task 2: Add ContentDriftDetected variant + create content_drift.rs** - `47a22520` (feat)
3. **Task 3: Wire into lib.rs, main.rs, cloud_sync.rs** - `47a22520` (feat)

## Files Created/Modified
- `crates/racecontrol/src/content_drift.rs` — New module: background poll, TOML vs disk comparison, WS broadcast, WhatsApp gate
- `crates/rc-common/src/protocol.rs` — Added ContentDriftDetected variant to DashboardEvent enum
- `crates/racecontrol/src/db/mod.rs` — Phase 366 migration block: content_drift_events table + 2 indexes
- `crates/racecontrol/src/lib.rs` — Added `pub mod content_drift;`
- `crates/racecontrol/src/main.rs` — Added `spawn_content_drift_task(Arc::clone(&state))`
- `crates/racecontrol/src/cloud_sync.rs` — Added content_drift_events push to cloud sync pipeline

## Decisions Made
- Skip first tick of the polling interval to avoid running at startup before pods have connected
- WhatsApp alert fires only for `game_removed` delta — game additions are informational; removals risk silent session failure (P2-10 severity class)
- Offline pods (reqwest error) are skipped silently — unreachable pods would generate false drift events if not excluded

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- ContentDriftDetected WS event can be surfaced in admin dashboard (Phase 367 Staff Tools)
- content_drift_events table ready for admin UI query endpoint
- WhatsApp alerting live for game_removed events on next server deploy

---
*Phase: 366-fleet-intelligence*
*Completed: 2026-04-11*
