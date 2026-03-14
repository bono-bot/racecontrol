---
gsd_state_version: 1.0
milestone: v3.0
milestone_name: Leaderboards, Telemetry & Competitive
status: active
stopped_at: "Roadmap created — ready to plan Phase 12"
last_updated: "2026-03-14"
last_activity: 2026-03-14 — Roadmap created, 4 phases, 47 requirements mapped
progress:
  total_phases: 4
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
  percent: 0
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-14)

**Core value:** Customers see their lap times, compete on leaderboards, and compare telemetry — driving repeat visits and social sharing from a publicly accessible cloud PWA.
**Current focus:** Phase 12 — Data Foundation

## Current Position

Phase: 12 of 15 (Data Foundation)
Plan: — of — (not yet planned)
Status: Ready to plan
Last activity: 2026-03-14 — Roadmap created

Progress: [░░░░░░░░░░] 0%

## Performance Metrics

**Velocity:**
- Total plans completed: 0
- Average duration: —
- Total execution time: 0 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 12. Data Foundation | TBD | - | - |
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

### Pending Todos

None.

### Blockers/Concerns

- [Pre-Phase 15] Driver rating class boundaries (A=top X%?) are a product decision — need Uday sign-off before Phase 15 planning begins
- [Pre-Phase 14] Championship scoring edge cases (DNS/DNF, tiebreaker, round cancellation) need characterization tests written before implementing scoring logic

## Session Continuity

Last session: 2026-03-14
Stopped at: Roadmap created — 4 phases (12-15), 47 requirements mapped, ready to plan Phase 12
Resume file: None
