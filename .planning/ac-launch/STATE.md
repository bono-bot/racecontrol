---
gsd_state_version: 1.0
milestone: v5.0
milestone_name: AC Launch Reliability
status: active
stopped_at: "Requirements defined, roadmap created"
last_updated: "2026-03-15"
last_activity: 2026-03-15 — Milestone v5.0 created with 3 phases, 11 requirements
progress:
  total_phases: 3
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
  percent: 0
---

# Project State

## Project Reference

See: .planning/ac-launch/PROJECT.md (created 2026-03-15)

**Core value:** No customer ever plays for free and no customer ever pays for downtime — billing and game process always in sync.
**Current focus:** Phase 1 — Billing-Game Lifecycle (stop game on billing end, validate before launch, pod reset)

## Current Position

Phase: 1 of 3 — Billing-Game Lifecycle (not started)
Plan: —
Status: Ready for planning
Last activity: 2026-03-15 — Milestone created

Progress: [░░░░░░░░░░] 0%

## Performance Metrics

**Velocity:**
- Total plans completed: 0
- Average duration: -
- Total execution time: -

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| TBD | - | - | - |

## Accumulated Context

### Decisions

- Billing-game lifecycle is a separate GSD from v4.0 (Pod Fleet Self-Healing)
- 3 phases: Lifecycle first (revenue impact), Crash Recovery second, Launch Resilience third
- rc-core is billing-authoritative — rc-agent reports game state, rc-core decides billing actions
- No research needed — all issues documented in customer-journey-gaps.md with known code paths

### Existing Infrastructure (do NOT rebuild)

- BillingManager + BillingTimer in rc-core/billing.rs — full timer lifecycle
- GameTracker in rc-core/game_launcher.rs — launch FSM (Launching → Running → Error)
- ac_launcher.rs in rc-agent — 1,400+ lines, CM fallback already exists
- game_process.rs in rc-agent — PID tracking, orphan cleanup
- lock_screen.rs in rc-agent — state machine for lock screen
- Protocol messages: CoreToAgentMessage::LaunchGame, AgentToCoreMessage::GameStateChanged already exist

### Pending Todos

- Phase 1: Billing-Game Lifecycle (next)

### Blockers/Concerns

- Need to verify current GameTracker states before adding new transitions
- Protocol changes need serde(default) for rolling deploy compatibility

## Session Continuity

Last session: 2026-03-15
Stopped at: Milestone created, ready for /gsd:plan-phase 1
Resume file: None
