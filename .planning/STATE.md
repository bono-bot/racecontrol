---
gsd_state_version: 1.0
milestone: v34.0
milestone_name: Time-Series Metrics & Operational Dashboards
status: planning
stopped_at: Milestone initialized, defining requirements
last_updated: "2026-04-01T10:01:00.000Z"
last_activity: 2026-04-01 — Milestone v34.0 started
progress:
  total_phases: 0
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
  percent: 0
---

## Current Position

Phase: Not started (defining requirements)
Plan: —
Status: Defining requirements
Last activity: 2026-04-01 — Milestone v34.0 started

Progress: [░░░░░░░░░░] 0%

## Project Reference

**Milestone:** v34.0 Time-Series Metrics & Operational Dashboards
**Core value:** Make autonomous action loops observable and queryable with time-series depth
**Roadmap:** .planning/ROADMAP.md (pending)
**Requirements:** .planning/REQUIREMENTS.md (pending)

See: .planning/PROJECT.md (project context)
See: COGNITIVE-GATE-PROTOCOL.md (operations protocol v3.1)

## Accumulated Context

### Key Architectural Decisions

- **SQLite TSDB** — extend existing SQLite WAL pattern, no new database dependencies
- **Metrics captured** — CPU, GPU temp, FPS, billing revenue, WS connections, pod health
- **Rollup strategy** — 1-min raw (7 days), hourly rollups (90 days), daily rollups (90 days)
- **Dashboard in racingpoint-admin** — port 3201, Next.js /metrics page with recharts sparklines
- **Prometheus export is passive** — exposition format endpoint only, no Prometheus server deployed
- **Alert thresholds in TOML** — evaluated every 60s against TSDB, fires existing WhatsApp alerter
- **Extends alert_engine.rs** — not replacing existing alert system, adding TSDB-backed thresholds
- **Backlog design doc** — .planning/backlog/infrastructure-roadmap-v34-v37.md has full specs

### From v32.0 (carried forward)

- v32.0 paused at Phase 273 (3/4 plans done) — will resume after v34.0
- v33.0 billing integrity: Phase 280 (deferred billing) + Phase 281 (crash recovery) committed
- OpenRouter client trait in rc-common only — trait definition, no reqwest dependency

### Blockers/Concerns

- None — v34.0 is infrastructure-only, no dependencies on v32/v33 shipping first

## Session Continuity

Last session: 2026-04-01
Stopped at: Milestone initialized, defining requirements
Resume file: None
