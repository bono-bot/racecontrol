---
gsd_state_version: 1.0
milestone: v36.0
milestone_name: Config Management & Policy Engine
status: executing
stopped_at: Completed 300-02-PLAN.md
last_updated: "2026-04-01T14:54:07.632Z"
last_activity: 2026-04-01
progress:
  total_phases: 5
  completed_phases: 2
  total_plans: 10
  completed_plans: 3
  percent: 0
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-01)

**Core value:** Ensure operational data survives hardware failure and prepare the data layer for a potential second venue
**Current focus:** Phase 297 — config-editor-ui

## Current Position

Phase: 297 (config-editor-ui) — EXECUTING
Plan: 2 of 2
Status: Ready to execute
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

### Pending Todos

None yet.

### Blockers/Concerns

- Phase 304 (Fleet Deploy Automation) builds on OTA pipeline from v22.0 — verify ota_pipeline.rs extension points before planning
- Phase 303 venue_id migration must be audited against all INSERT/UPDATE queries in routes.rs (16K lines, known tech debt)

## Session Continuity

Last session: 2026-04-01T14:54:07.627Z
Stopped at: Completed 300-02-PLAN.md
Resume file: None
