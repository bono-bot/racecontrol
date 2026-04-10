---
phase: 352-health-whatsapp-alerts
plan: 01
subsystem: api, monitoring
tags: [health-probes, whatsapp, alerting, sqlite, sysinfo, dedup]

requires:
  - phase: 343
    provides: alert_incidents table, whatsapp_alerter send_whatsapp()
provides:
  - subsystem_health.rs module with 7 real probes and cached state
  - Extended /api/v1/health endpoint with subsystems object
  - alert_incidents schema with subsystem/severity/correlation_id columns
  - 10-minute dedup for subsystem degradation alerts
affects: [354-ui-hardening, 352-02-plan, admin-dashboard, deploy-scripts]

tech-stack:
  added: []
  patterns:
    - "LazyLock<RwLock<HashMap>> for cached probe results (zero-latency health reads)"
    - "10-min dedup via HashMap<(String, String), Instant> with eviction on each cycle"
    - "Transition detection (ok->degraded, degraded->ok) with alert dispatch"
    - "ALTER TABLE ADD COLUMN with duplicate-column error suppression for idempotent migrations"

key-files:
  created:
    - crates/racecontrol/src/subsystem_health.rs
  modified:
    - crates/racecontrol/src/lib.rs
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/src/main.rs
    - scripts/deploy/check-health.sh
    - scripts/deploy/bono-server-monitor.sh
    - scripts/bono-auto-detect.sh
    - scripts/auto-detect.sh
    - test/e2e/run-e2e.sh

key-decisions:
  - "Overall health status derived from subsystem probes (not hardcoded 'ok') -- closes 'probes that lie' standing rule"
  - "Updated 5 monitoring scripts to accept 'degraded' status alongside 'ok' for backward compatibility"
  - "admin_db probe uses file existence check (racecontrol has no admin_db pool)"
  - "cloud_sync probe treats missing sync_state table as ok (venue-only mode)"

patterns-established:
  - "SubsystemStatus { ok, latency_ms, error_code, detail } as standard probe output"
  - "Background probe task with transition detection and dedup alert dispatch"

requirements-completed: [OPS-01, OPS-02, OPS-04, OPS-05]

duration: 14min
completed: 2026-04-10
---

# Phase 352 Plan 01: Per-subsystem probes + health endpoint extension + alert_incidents migration Summary

**7 real subsystem probes (db_writable, rc_backend, disk_free, cloud_sync, whatsapp_api, fleet_connectivity, admin_db) with 10-minute dedup alert dispatch, cached in LazyLock for zero-latency health reads, and alert_incidents schema extended with subsystem/severity/correlation_id columns**

## Performance

- **Duration:** 14 min
- **Started:** 2026-04-10T08:47:06Z
- **Completed:** 2026-04-10T09:01:32Z
- **Tasks:** 2
- **Files modified:** 10

## Accomplishments
- Created subsystem_health.rs module (480+ lines) with 7 probe functions, background task (10s interval), transition detection, dedup, and WhatsApp alert dispatch
- Extended /api/v1/health with "subsystems" object and derived overall status (closes "probes that lie" standing rule)
- Added alert_incidents schema migration (subsystem, severity, correlation_id columns) for subsystem incident tracking
- Updated 5 monitoring scripts for backward compatibility with "degraded" status

## Task Commits

Each task was committed atomically:

1. **Task 1: Create subsystem_health.rs module with probes, dedup, and alert dispatch** - `dd7779ee` (feat)
2. **Task 2: Extend health endpoint and spawn probe task** - `1a92e749` (feat)

## Files Created/Modified
- `crates/racecontrol/src/subsystem_health.rs` - New module: 7 probes, LazyLock state, dedup, alert dispatch, incident recording, 9 unit tests
- `crates/racecontrol/src/lib.rs` - Added `pub mod subsystem_health`
- `crates/racecontrol/src/db/mod.rs` - ALTER TABLE migration for alert_incidents (subsystem, severity, correlation_id)
- `crates/racecontrol/src/api/routes.rs` - Extended health() handler with subsystems object, derived overall status
- `crates/racecontrol/src/main.rs` - Spawned subsystem_health task after app_health_monitor
- `scripts/deploy/check-health.sh` - Accept "degraded" status
- `scripts/deploy/bono-server-monitor.sh` - Accept "degraded" status
- `scripts/bono-auto-detect.sh` - Accept "degraded" status (venue + cloud checks)
- `scripts/auto-detect.sh` - Accept "degraded" status (server + bono checks)
- `test/e2e/run-e2e.sh` - Accept "degraded" status in inline health check

## Decisions Made
- Derived overall status from subsystem probes instead of hardcoded "ok" -- this makes the health endpoint truthful
- Updated monitoring scripts proactively (Rule 2: cascade updates) -- 5 scripts check `status == "ok"` and would have false-alarmed with "degraded"
- admin_db probe uses file existence check since racecontrol has no admin_db pool in AppState
- cloud_sync probe treats missing sync_state table or no records as ok (covers venue-only deployments)
- Used id=2 for subsystem health probe INSERT to avoid conflict with server_diagnostics which uses id=1

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Updated 5 monitoring scripts for backward compatibility**
- **Found during:** Task 2 (health endpoint extension)
- **Issue:** 5 scripts check `"status":"ok"` strictly -- changing status to "degraded" would trigger false alarms
- **Fix:** Updated grep patterns to accept both "ok" and "degraded" in check-health.sh, bono-server-monitor.sh, bono-auto-detect.sh, auto-detect.sh, run-e2e.sh
- **Files modified:** 5 shell scripts
- **Verification:** grep confirms updated patterns
- **Committed in:** 1a92e749 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 missing critical)
**Impact on plan:** Essential for backward compatibility. No scope creep.

## Issues Encountered
None

## Known Stubs
None -- all 7 probes perform real checks (DB write, disk space via sysinfo, HTTP to Evolution API, fleet WS count, file existence).

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Backend API ready for Phase 354 (/settings/health page) to consume subsystem probes
- Ready for Plan 352-02 (WhatsApp relay fallback, structured logging, log rsync)
- Deploy: binary rebuild required, db migration is idempotent (ALTER TABLE with duplicate-column suppression)

---
*Phase: 352-health-whatsapp-alerts*
*Completed: 2026-04-10*
