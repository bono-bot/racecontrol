---
gsd_state_version: 1.0
milestone: v3.0
milestone_name: Leaderboards, Telemetry & Competitive
status: active
stopped_at: "Completed 12-01-PLAN.md"
last_updated: "2026-03-15"
last_activity: 2026-03-15 — Completed Phase 12 Plan 01 (Data Foundation schema)
progress:
  total_phases: 4
  completed_phases: 0
  total_plans: 2
  completed_plans: 1
  percent: 12
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-14)

**Core value:** Customers see their lap times, compete on leaderboards, and compare telemetry — driving repeat visits and social sharing from a publicly accessible cloud PWA.
**Current focus:** Phase 12 — Data Foundation

## Current Position

Phase: 12 of 15 (Data Foundation)
Plan: 1 of 2 complete
Status: Executing
Last activity: 2026-03-15 — Completed Plan 01 (v3.0 database foundation schema)

Progress: [█░░░░░░░░░] 12%

## Performance Metrics

**Velocity:**
- Total plans completed: 1
- Average duration: 6 min
- Total execution time: 0.1 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 12. Data Foundation | 1/2 | 6 min | 6 min |
| 13. Leaderboard Core | TBD | - | - |
| 14. Events and Championships | TBD | - | - |
| 15. Telemetry and Driver Rating | TBD | - | - |

*Updated after each plan completion*

## Accumulated Context

### Decisions

- v3.0 scope: Leaderboards + Telemetry + Group Events + Championships on cloud PWA
- Car classes: Both vehicle-based (leaderboard filter) AND driver skill rating (percentile A/B/C/D)
- Hotlap events: Staff-created, auto-entry when lap matches track+car class+date range
- Group event scoring: F1-style auto-score (25/18/15/12/10/8/6/4/2/1)
- Telemetry sync: Event laps only — not all laps (bounded volume)
- Public access: All pages fully public, no login required
- Driver rating thresholds: Algorithm settled (percentile), specific boundaries need Uday sign-off before Phase 15 planning
- Championship edge cases: Tiebreaker/DNS/DNF need characterization tests before Phase 14 scoring implementation
- [12-01] idx_telemetry_lap_offset added alongside existing idx_telemetry_lap (no drop) to avoid production table locking
- [12-01] cloud_driver_id column added as schema plumbing only; enforcement logic deferred to Phase 14
- [12-01] hotlap_events.car is free-text display field; Phase 14 auto-entry matches on car_class

### Pending Todos

None.

### Blockers/Concerns

- [Pre-Phase 15] Driver rating class boundaries (A=top X%?) are a product decision — need Uday sign-off before Phase 15 planning begins
- [Pre-Phase 14] Championship scoring edge cases (DNS/DNF, tiebreaker, round cancellation) need characterization tests written before implementing scoring logic

## Session Continuity

Last session: 2026-03-15
Stopped at: Completed 12-01-PLAN.md — v3.0 database foundation schema with WAL tuning, covering indexes, cloud_driver_id, 6 competitive tables
Resume file: None
