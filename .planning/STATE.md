---
gsd_state_version: 1.0
milestone: v3.0
milestone_name: Leaderboards, Telemetry & Competitive
status: active
stopped_at: "Completed 13-04-PLAN.md"
last_updated: "2026-03-15"
last_activity: 2026-03-15 — Completed Plan 04 (public driver search and profile endpoints)
progress:
  total_phases: 4
  completed_phases: 1
  total_plans: 7
  completed_plans: 6
  percent: 86
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-14)

**Core value:** Customers see their lap times, compete on leaderboards, and compare telemetry — driving repeat visits and social sharing from a publicly accessible cloud PWA.
**Current focus:** Phase 13 — Leaderboard Core

## Current Position

Phase: 13 of 15 (Leaderboard Core)
Plan: 4 of 5 complete
Status: In Progress
Last activity: 2026-03-15 — Completed Plan 04 (public driver search and profile endpoints)

Progress: [████████░░] 86%

## Performance Metrics

**Velocity:**
- Total plans completed: 6
- Average duration: 6.2 min
- Total execution time: 0.6 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 12. Data Foundation | 2/2 | 11 min | 5.5 min |
| 13. Leaderboard Core | 4/5 | 26 min | 6.5 min |
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
- [12-02] No backfill of historical laps: NULL car_class is sentinel for pre-v3.0 data
- [12-02] car_class lookup uses driver_id + status='active' (not pod_id) to find billing session
- [12-02] kiosk_experiences table added to test migrations for JOIN query validation
- [13-01] Suspect flag is orthogonal to valid: valid=1 AND suspect=1 means game says ok but time/sectors are suspicious
- [13-01] Zero sectors treated as absent (not flagged) since some sims report zeros instead of null
- [13-01] Sector sum tolerance is 500ms for rounding across different sim telemetry systems
- [13-01] Pre-migration laps get suspect=0 via DEFAULT, treating historical data as clean
- [13-02] sim_type defaults to assetto_corsa for backward compatibility with existing PWA consumers
- [13-02] Suspect laps always hidden from public endpoints regardless of show_invalid toggle
- [13-02] Circuit records query from laps table (not track_records) to include sim_type dimension
- [13-02] Vehicle records grouped by (track, sim_type) to prevent cross-sim contamination
- [13-03] Previous record holder data fetched BEFORE UPSERT to avoid reading back new holder's data
- [13-03] Notification is fire-and-forget via tokio::spawn -- failure never blocks lap persistence
- [13-03] New holder display name uses nickname if show_nickname_on_leaderboard=1 (NTF-02)
- [13-03] NULL email silently skips notification; first record has no notification attempt
- [13-04] PII exclusion by construction: SELECT only safe fields (never SELECT * then filter)
- [13-04] Sector times <= 0 mapped to SQL NULL via CASE expression, not application-level filtering
- [13-04] class_badge: null hardcoded in response — Phase 15 RAT-01 will populate with driver rating class
- [13-04] Search queries both name AND nickname columns with COLLATE NOCASE for case-insensitive matching
- [13-04] Driver profile returns 404 JSON error for non-existent IDs (not 500)

### Pending Todos

None.

### Blockers/Concerns

- [Pre-Phase 15] Driver rating class boundaries (A=top X%?) are a product decision — need Uday sign-off before Phase 15 planning begins
- [Pre-Phase 14] Championship scoring edge cases (DNS/DNF, tiebreaker, round cancellation) need characterization tests written before implementing scoring logic

## Session Continuity

Last session: 2026-03-14T21:55:07Z
Stopped at: Completed 13-04-PLAN.md
Resume file: None
