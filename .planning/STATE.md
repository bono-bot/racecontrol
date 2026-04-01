---
gsd_state_version: 1.0
milestone: v36.0
milestone_name: Config Management & Policy Engine
status: verifying
stopped_at: Completed 298-02-PLAN.md
last_updated: "2026-04-01T15:45:42.786Z"
last_activity: 2026-04-01
progress:
  total_phases: 5
  completed_phases: 4
  total_plans: 10
  completed_plans: 7
  percent: 0
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-01)

**Core value:** Ensure operational data survives hardware failure and prepare the data layer for a potential second venue
**Current focus:** Phase 301 — cloud-data-sync-v2

## Current Position

Phase: 301 (cloud-data-sync-v2) — EXECUTING
Plan: 2 of 2
Status: Phase complete — ready for verification
Last activity: 2026-04-01

Progress: [░░░░░░░░░░] 0%

## Performance Metrics

**Velocity:**

- Total plans completed: 0 (v37.0)
- Average duration: -
- Total execution time: -

## Accumulated Context

### Decisions

- Phase numbering continues from v36.0 — phases 300-304
- Backup uses SQLite .backup API (WAL-safe, not file copy)
- venue_id default: 'racingpoint-hyd-001' — backward compatible, no functional change
- Fleet deploy extends existing OTA pipeline from v22.0 — no rewrite
- BACKUP completes before SYNC and EVENT (both depend on Phase 300)
- SYNC + EVENT both complete before VENUE schema (Phase 303 depends on 301 + 302)
- VENUE schema completes before DEPLOY automation (Phase 304 depends on 303)
- [Phase 296]: compute_config_hash_local in ws_handler.rs (not imported from racecontrol -- rc-agent cannot depend on racecontrol crate)
- [Phase 300]: VACUUM INTO used for WAL-safe backup (not file copy) per locked decision
- [Phase 300]: BackupConfig has serde defaults for all fields — backward compatible, no TOML change needed
- [Phase 300]: SCP transfer only during 02:00-04:00 IST window, once per day via NaiveDate tracking
- [Phase 300]: backup/status endpoint in staff_routes (JWT required) — backup health is internal operational data
- [Phase 300]: Remote reachability checked every tick so dashboard always shows current state
- [Phase 297-config-editor-ui]: Promise.all SWR fetcher for pod configs avoids React Rules of Hooks violations from conditional/loop hook calls
- [Phase 297-config-editor-ui]: ConfigEditorModal accepts initialConfig from parent SWR data — no additional fetch on modal open
- [Phase 301]: normalize_timestamp made pub(crate) for reuse in routes.rs
- [Phase 301]: metrics_rollups uses max-sample-count wins for avg_value conflict resolution (more data = more authoritative)
- [Phase 301]: SCHEMA_VERSION bumped 3->4 when fleet_solutions+model_evaluations+metrics_rollups added to push payload
- [Phase 298]: GamePreset does NOT derive sqlx::FromRow — rc-common has no sqlx dep, rows mapped manually
- [Phase 298]: GET /presets in public_routes (pods/kiosk need without JWT), POST/PUT/DELETE in staff_routes
- [Phase 298]: Admin UI uses rcFetch proxy (cookie JWT) not localStorage authHeaders — follows existing admin pattern

### Pending Todos

None yet.

### Blockers/Concerns

- Phase 304 (Fleet Deploy Automation) builds on OTA pipeline from v22.0 — verify ota_pipeline.rs extension points before planning
- Phase 303 venue_id migration must be audited against all INSERT/UPDATE queries in routes.rs (16K lines, known tech debt)

## Session Continuity

Last session: 2026-04-01T15:45:42.781Z
Stopped at: Completed 298-02-PLAN.md
Resume file: None
