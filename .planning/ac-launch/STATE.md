---
gsd_state_version: 1.0
milestone: v5.0
milestone_name: AC Launch Reliability
status: active
stopped_at: "Phase 4 Plan 01 complete — multiplayer server lifecycle wiring done, kiosk UI next"
last_updated: "2026-03-15"
last_activity: 2026-03-15 — Phase 4 Plan 01 complete (multiplayer AC server auto-start/stop + kiosk booking endpoint)
progress:
  total_phases: 5
  completed_phases: 3
  total_plans: 10
  completed_plans: 7
  percent: 70
---

# Project State

## Project Reference

See: .planning/ac-launch/PROJECT.md (created 2026-03-15)

**Core value:** No customer ever plays for free and no customer ever pays for downtime — billing and game process always in sync.
**Current focus:** Phase 4 Plan 02 — Kiosk multiplayer booking wizard UI

## Current Position

Phase: 4 of 5 — Multiplayer Server Lifecycle
Plan: 1 of 2
Status: Plan 01 complete — backend wiring done, kiosk UI next (Plan 02)
Last activity: 2026-03-15 — Phase 4 Plan 01 complete

Progress: [#######░░░] 70%

## Performance Metrics

**Velocity:**
- Total plans completed: 7
- Average duration: 6min
- Total execution time: 41min

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 1. Billing-Game Lifecycle | 2/2 | 5min | 2.5min |
| 2. Game Crash Recovery | 2/2 | 8min | 4min |
| 3. Launch Resilience | 2/2 | 20min | 10min |
| 4. Multiplayer Server Lifecycle | 1/2 | 8min | 8min |

## Accumulated Context

### Decisions

- Billing-game lifecycle is a separate GSD from v4.0 (Pod Fleet Self-Healing)
- 5 phases: Lifecycle first (revenue), Crash Recovery, Launch Resilience, Multiplayer Server Lifecycle, Synchronized Group Play
- Phases 4-5 added after VMS analysis — wiring existing ac_server.rs + multiplayer.rs to billing lifecycle
- rc-core is billing-authoritative — rc-agent reports game state, rc-core decides billing actions
- No research needed — all issues documented in customer-journey-gaps.md with known code paths
- Billing gate placed after catalog validation but before double-launch guard in launch_game()
- Double-launch guard error message generalized to "already has a game active" covering both Launching and Running
- Reused PausedGamePause status (from Phase 2) for launch failure billing pause — no new enum variant needed
- StateLabel component receives gameInfo prop for context-aware crashed/launch-failed label
- AC server start is fire-and-forget — booking succeeds even if server fails to start
- ac_session_id column added via idempotent ALTER TABLE (safe for rolling deploy)
- check_and_stop_multiplayer_server wired at three billing-end paths (tick-expired, manual, orphan)
- Kiosk multiplayer uses unique PINs per pod (not shared group PIN)

### Existing Infrastructure (do NOT rebuild)

- BillingManager + BillingTimer in rc-core/billing.rs — full timer lifecycle
- GameTracker in rc-core/game_launcher.rs — launch FSM (Launching -> Running -> Error)
- ac_launcher.rs in rc-agent — 1,400+ lines, CM fallback already exists
- game_process.rs in rc-agent — PID tracking, orphan cleanup
- lock_screen.rs in rc-agent — state machine for lock screen
- Protocol messages: CoreToAgentMessage::LaunchGame, AgentToCoreMessage::GameStateChanged already exist
- AcServerManager in rc-core/ac_server.rs — full server lifecycle (start/stop/monitor/orphan cleanup)
- multiplayer.rs in rc-core — group booking, pod allocation, friend invites, PIN generation + kiosk booking
- kiosk/src/app/book/page.tsx — booking wizard (single-player only, needs multiplayer flow)
- DB tables: group_sessions (+ ac_session_id column), group_session_members, multiplayer_results, pod_reservations
- POST /kiosk/book-multiplayer endpoint (backend ready, needs kiosk UI)

### Pending Todos

- Phase 4 Plan 02: Kiosk multiplayer booking wizard UI (next)
- Phase 5: Synchronized Group Play (after Phase 4 completes)

### Blockers/Concerns

- Protocol changes need serde(default) for rolling deploy compatibility
- Agent-side LaunchDiagnostics is a separate struct from protocol type — converted explicitly at WebSocket send boundary
- get_cm_exit_code() returns Some(-1) for "exited but code unknown", None for "still running" (tasklist limitation)
- diagnostics: None on all non-CM-path GameLaunchInfo constructions — avoids false positives on direct launches

## Session Continuity

Last session: 2026-03-15
Stopped at: Phase 4 Plan 01 complete — multiplayer server lifecycle wiring done, kiosk UI next
Resume file: None
