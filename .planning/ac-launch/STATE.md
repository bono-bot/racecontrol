---
gsd_state_version: 1.0
milestone: v5.0
milestone_name: AC Launch Reliability
status: active
stopped_at: "Phase 3 Plan 01 complete — LaunchDiagnostics pipeline"
last_updated: "2026-03-15"
last_activity: 2026-03-15 — Phase 3 Plan 01 complete (LaunchDiagnostics struct + GameLaunchInfo field + diagnostics threading)
progress:
  total_phases: 5
  completed_phases: 2
  total_plans: 10
  completed_plans: 5
  percent: 50
---

# Project State

## Project Reference

See: .planning/ac-launch/PROJECT.md (created 2026-03-15)

**Core value:** No customer ever plays for free and no customer ever pays for downtime — billing and game process always in sync.
**Current focus:** Phase 3 — Launch Resilience (next)

## Current Position

Phase: 3 of 5 — Launch Resilience (in progress)
Plan: 1 of 2
Status: Phase 3 Plan 01 complete — diagnostics pipeline done, moving to Plan 02
Last activity: 2026-03-15 — Phase 3 Plan 01 complete

Progress: [#####░░░░░] 50%

## Performance Metrics

**Velocity:**
- Total plans completed: 4
- Average duration: 3min
- Total execution time: 13min

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 1. Billing-Game Lifecycle | 2/2 | 5min | 2.5min |
| 2. Game Crash Recovery | 2/2 | 8min | 4min |
| 3. Launch Resilience | 1/2 | 15min (so far) | 15min |

## Accumulated Context

### Decisions

- Billing-game lifecycle is a separate GSD from v4.0 (Pod Fleet Self-Healing)
- 5 phases: Lifecycle first (revenue), Crash Recovery, Launch Resilience, Multiplayer Server Lifecycle, Synchronized Group Play
- Phases 4-5 added after VMS analysis — wiring existing ac_server.rs + multiplayer.rs to billing lifecycle
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
- AcServerManager in rc-core/ac_server.rs — full server lifecycle (start/stop/monitor/orphan cleanup)
- multiplayer.rs in rc-core — group booking, pod allocation, friend invites, PIN generation
- kiosk/src/app/book/page.tsx — booking wizard (single-player only, needs multiplayer flow)
- DB tables: group_sessions, group_session_members, multiplayer_results, pod_reservations

### Pending Todos

- Phase 3 Plan 02: rc-core LaunchFailed handler + billing auto-pause + kiosk error display + retry button
- Phase 4: Multiplayer Server Lifecycle (after Phase 3 completes)
- Phase 5: Synchronized Group Play (after Phase 4 completes)

### Blockers/Concerns

- Protocol changes need serde(default) for rolling deploy compatibility
- Agent-side LaunchDiagnostics is a separate struct from protocol type — converted explicitly at WebSocket send boundary
- get_cm_exit_code() returns Some(-1) for "exited but code unknown", None for "still running" (tasklist limitation)
- diagnostics: None on all non-CM-path GameLaunchInfo constructions — avoids false positives on direct launches

## Session Continuity

Last session: 2026-03-15
Stopped at: Phase 3 Plan 01 complete — moving to Plan 02
Resume file: None
