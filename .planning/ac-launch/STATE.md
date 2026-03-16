---
gsd_state_version: 1.0
milestone: v5.0
milestone_name: AC Launch Reliability
status: complete
stopped_at: "Phase 5 Plan 02 complete — join failure recovery + config change done. ALL 5 PHASES COMPLETE. GSD milestone v5.0 AC Launch Reliability FINISHED."
last_updated: "2026-03-16"
last_activity: 2026-03-16 — Phase 5 Plan 02 complete (join failure recovery + mid-session config change)
progress:
  total_phases: 5
  completed_phases: 5
  total_plans: 10
  completed_plans: 10
  percent: 100
---

# Project State

## Project Reference

See: .planning/ac-launch/PROJECT.md (created 2026-03-15)

**Core value:** No customer ever plays for free and no customer ever pays for downtime — billing and game process always in sync.
**Current focus:** Phase 5 — Synchronized Group Play

## Current Position

Phase: 5 of 5 — Synchronized Group Play
Plan: 2 of 2
Status: ALL PLANS COMPLETE — AC Launch Reliability v5.0 milestone finished
Last activity: 2026-03-16 — Phase 5 Plan 02 complete (join failure recovery + mid-session config change)

Progress: [##########] 100%

## Performance Metrics

**Velocity:**
- Total plans completed: 8
- Average duration: 6min
- Total execution time: 44min

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 1. Billing-Game Lifecycle | 2/2 | 5min | 2.5min |
| 2. Game Crash Recovery | 2/2 | 8min | 4min |
| 3. Launch Resilience | 2/2 | 20min | 10min |
| 4. Multiplayer Server Lifecycle | 2/2 | 11min | 5.5min |
| 5. Synchronized Group Play | 2/2 | 38min | 19min |

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
- Replaced old multiplayer_lobby join/create UI with pod count selector for kiosk self-serve
- Success screen uses multiAssignments.length > 0 as multiplayer discriminator
- Review button text changes to "BOOK N RIGS" in multi mode
- GROUP-01: Use find_group_session_for_token(token_id) not pod+status query — token_id unambiguously identifies group membership at status='accepted'
- GROUP-01: AC server start removed from book_multiplayer/book_multiplayer_kiosk — deferred to on_member_validated()->start_ac_lan_for_group() when all PINs validated
- GROUP-02: Continuous mode monitor uses mutable current_session_id loop (not recursive spawn) — std::process::Child is !Send on Windows, recursive tokio::spawn rejected by compiler
- GROUP-02: Continuous mode guard in check_and_stop_multiplayer_server defers stop to monitor loop when flag active
- GROUP-03: join_failed is a top-level KioskPodCard block (not nested in on_track) — TypeScript narrows state to on_track inside that block
- GROUP-03: multiplayerGroup.pod_ids (group_session_all_validated) is source of truth for pod membership, not acServerInfo.connected_pods
- GROUP-04: update_session_config() mutates AcServerInstance.config in place; monitor loop re-reads on next restart iteration

### Existing Infrastructure (do NOT rebuild)

- BillingManager + BillingTimer in rc-core/billing.rs — full timer lifecycle
- GameTracker in rc-core/game_launcher.rs — launch FSM (Launching -> Running -> Error)
- ac_launcher.rs in rc-agent — 1,400+ lines, CM fallback already exists
- game_process.rs in rc-agent — PID tracking, orphan cleanup
- lock_screen.rs in rc-agent — state machine for lock screen
- Protocol messages: CoreToAgentMessage::LaunchGame, AgentToCoreMessage::GameStateChanged already exist
- AcServerManager in rc-core/ac_server.rs — full server lifecycle (start/stop/monitor/orphan cleanup)
- multiplayer.rs in rc-core — group booking, pod allocation, friend invites, PIN generation + kiosk booking
- kiosk/src/app/book/page.tsx — booking wizard with multiplayer "Play with Friends" flow
- DB tables: group_sessions (+ ac_session_id column), group_session_members, multiplayer_results, pod_reservations
- POST /kiosk/book-multiplayer endpoint (backend + kiosk UI both complete)
- api.kioskBookMultiplayer() in kiosk/src/lib/api.ts
- KioskMultiplayerAssignment + KioskMultiplayerResult in kiosk/src/lib/types.ts

### Pending Todos

- All plans complete. No pending todos.

### Blockers/Concerns

- Protocol changes need serde(default) for rolling deploy compatibility
- Agent-side LaunchDiagnostics is a separate struct from protocol type — converted explicitly at WebSocket send boundary
- get_cm_exit_code() returns Some(-1) for "exited but code unknown", None for "still running" (tasklist limitation)
- diagnostics: None on all non-CM-path GameLaunchInfo constructions — avoids false positives on direct launches

## Session Continuity

Last session: 2026-03-16
Stopped at: Phase 5 Plan 02 complete — ALL 10 PLANS DONE. Milestone v5.0 AC Launch Reliability complete.
Resume file: None
