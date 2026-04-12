---
gsd_state_version: 1.0
milestone: v48.0
milestone_name: "Codebase Architecture — Department-Driven Event Mesh"
status: Ready to plan Phase 369
last_updated: "2026-04-13"
progress:
  total_phases: 14
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
  percent: 0
---

# Project State — v48.0 Codebase Architecture — Department-Driven Event Mesh

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-13)

**Core value:** A customer walks in, launches a game, drives, and their laps appear on the leaderboard. Every time. For every supported game.
**Current focus:** Phase 369 — AC Launch Rewrite (P0)

## Current Position

Phase: 369 of 382 (AC Launch Rewrite)
Plan: Not started
Status: Ready to plan
Last activity: 2026-04-13 — Roadmap created, 14 phases defined (P0→P1→P2)

Progress: [░░░░░░░░░░] 0%

## Performance Metrics

**Velocity:**
- Total plans completed: 0
- Average duration: —
- Total execution time: 0 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| — | — | — | — |

*Updated after each plan completion*

## Accumulated Context

### Key Decisions

- P0 phases (369-373) MUST be complete and verified before any P1 work begins
- P1 phases (374-378) MUST be complete and verified before any P2 work begins
- Exception: P2 decomposition that directly unblocks a P0 req may run in parallel with P0
- AC launch rewrites to VMS-parity (<500 lines) replacing the 19,597-line path
- Staff Launch (kiosk) and PWA Launch (PIN) are separate code paths — only converge at "validate funds -> debit -> launch"
- Per-minute billing (not post-session) — debit at game start, pause on crash

### From v46.0 + v47.0 (shipped 2026-04-12)

- All code merged to main, deployed to venue + cloud (build `8e8c07ba`)
- 419K lines, 335K touched by debug, 36K net fix bloat
- 141 files over 500 lines, routes.rs at 26,459 lines
- AC launch spans 19,597 lines across 12 files
- Phase 363 code-complete but NOT deployed (F-05 refund bug still live on prod)

### Blockers/Concerns

- Phase 363 (v46.0) deploy is pending — F-05 refund bug and GLD-C-04 lap-reject race still live on production. Should be resolved before starting v48.0 work or carried into Phase 372 billing fix scope.

## Session Continuity

Last session: 2026-04-13
Stopped at: Roadmap written, requirements mapped, ready to plan Phase 369
Resume file: None
