---
gsd_state_version: 1.0
milestone: v5.0
milestone_name: AC Launch Reliability
status: active
stopped_at: "Completed 01-01-PLAN.md"
last_updated: "2026-03-15"
last_activity: 2026-03-15 — Completed Plan 01-01 (billing gate + double-launch guard)
progress:
  total_phases: 3
  completed_phases: 0
  total_plans: 6
  completed_plans: 1
  percent: 17
---

# Project State

## Project Reference

See: .planning/ac-launch/PROJECT.md (created 2026-03-15)

**Core value:** No customer ever plays for free and no customer ever pays for downtime — billing and game process always in sync.
**Current focus:** Phase 1 — Billing-Game Lifecycle (Plan 01-02 next: rc-agent SessionEnded + BillingStopped fixes)

## Current Position

Phase: 1 of 3 — Billing-Game Lifecycle (in progress)
Plan: 2 of 2 (Plan 01-01 complete, Plan 01-02 next)
Status: Executing Phase 1
Last activity: 2026-03-15 — Completed Plan 01-01

Progress: [##░░░░░░░░] 17%

## Performance Metrics

**Velocity:**
- Total plans completed: 1
- Average duration: 3min
- Total execution time: 3min

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 1. Billing-Game Lifecycle | 1/2 | 3min | 3min |

## Accumulated Context

### Decisions

- Billing-game lifecycle is a separate GSD from v4.0 (Pod Fleet Self-Healing)
- 3 phases: Lifecycle first (revenue impact), Crash Recovery second, Launch Resilience third
- rc-core is billing-authoritative — rc-agent reports game state, rc-core decides billing actions
- No research needed — all issues documented in customer-journey-gaps.md with known code paths
- Billing gate placed after catalog validation but before double-launch guard in launch_game()
- Double-launch guard error message generalized to "already has a game active" covering both Launching and Running

### Existing Infrastructure (do NOT rebuild)

- BillingManager + BillingTimer in rc-core/billing.rs — full timer lifecycle
- GameTracker in rc-core/game_launcher.rs — launch FSM (Launching -> Running -> Error)
- ac_launcher.rs in rc-agent — 1,400+ lines, CM fallback already exists
- game_process.rs in rc-agent — PID tracking, orphan cleanup
- lock_screen.rs in rc-agent — state machine for lock screen
- Protocol messages: CoreToAgentMessage::LaunchGame, AgentToCoreMessage::GameStateChanged already exist

### Pending Todos

- Phase 1 Plan 01-02: rc-agent SessionEnded + BillingStopped fixes (next)
- Phase 2: Game Crash Recovery (after Phase 1 completes)

### Blockers/Concerns

- Protocol changes need serde(default) for rolling deploy compatibility

## Session Continuity

Last session: 2026-03-15
Stopped at: Completed 01-01-PLAN.md
Resume file: None
