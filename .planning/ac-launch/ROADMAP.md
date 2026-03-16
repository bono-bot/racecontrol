# Roadmap: AC Launch Reliability (v5.0)

## Overview

The billing engine and game launcher both work independently — but they're not wired together. When billing ends, the game keeps running. When the game crashes, billing keeps counting. When staff launches without billing, there's no guard. The AC server manager exists but isn't tied to billing. Multiplayer booking works from PWA but not from kiosk.

This milestone closes every gap between billing lifecycle and game process lifecycle, then extends to multiplayer server automation and self-serve kiosk multiplayer.

No billing rewrite. No game launcher rewrite. Pure wiring between existing systems.

## Phases

- [x] **Phase 1: Billing-Game Lifecycle** - Stop game on billing end, validate before launch, pod reset after session, anti-double-launch
- [x] **Phase 2: Game Crash Recovery** - Detect crash, pause billing, show status, enable re-launch
- [x] **Phase 3: Launch Resilience** - CM fallback improvements, failure reporting, billing pause on launch failure
- [x] **Phase 4: Multiplayer Server Lifecycle** - AC server auto-start/stop wired to billing, kiosk self-serve multiplayer booking
- [ ] **Phase 5: Synchronized Group Play** - Coordinated launch across pods, continuous race mode, failure recovery

## Phase Details

### Phase 1: Billing-Game Lifecycle
**Goal**: When billing ends, the game dies. When there's no billing, no game launches. When the session is over, the pod resets. No exceptions.
**Depends on**: Nothing (first phase)
**Requirements**: LIFE-01, LIFE-02, LIFE-03, LIFE-04
**Success Criteria** (what must be TRUE):
  1. Billing session expires on Pod 8 → game process (acs.exe) is killed within 10 seconds, verified via `tasklist`
  2. Staff clicks "Launch Game" on kiosk for a pod with no active billing → request is rejected with clear error message
  3. After billing ends on any pod, lock screen shows session summary for ~15 seconds then transitions to idle state (PIN entry)
  4. Staff rapidly clicks "Launch Game" twice → only one game process starts, second request is rejected
  5. Existing billing start/stop/pause flows still work unchanged (regression check)
**Plans**: 2 plans

Plans:
- [x] 01-01-PLAN.md — racecontrol: billing validation gate + double-launch guard fix in game_launcher.rs (LIFE-01, LIFE-02, LIFE-04)
- [x] 01-02-PLAN.md — rc-agent: arm 15s blank_timer in SessionEnded + fix BillingStopped billing_active flag (LIFE-01, LIFE-03)

### Phase 2: Game Crash Recovery
**Goal**: When a game crashes, billing pauses instantly, staff sees it on the dashboard, and they can restart the game without touching the pod or creating a new session
**Depends on**: Phase 1 (uses StopGame/game state infrastructure)
**Requirements**: CRASH-01, CRASH-02, CRASH-03, CRASH-04
**Success Criteria** (what must be TRUE):
  1. Game process (acs.exe) is killed externally → rc-agent detects exit within 5 seconds
  2. When game exit is unexpected (not triggered by billing end), billing timer pauses automatically
  3. Kiosk dashboard shows "Game Crashed" badge for the affected pod within 10 seconds of crash
  4. Staff clicks "Re-launch" on a crashed pod → game launches using same billing session (no new session created)
  5. If game crashes during free trial, the same crash recovery flow applies
**Plans**: 2 plans

Plans:
- [x] 02-01-PLAN.md — racecontrol: billing auto-pause on GameCrashed + POST /games/relaunch/:pod_id endpoint (CRASH-02, CRASH-04)
- [x] 02-02-PLAN.md — kiosk: "Game Crashed" badge + "Relaunch Game" button on pod cards (CRASH-03, CRASH-04)

### Phase 3: Launch Resilience
**Goal**: When Content Manager fails or AC won't start, the system recovers gracefully — falling back to direct launch, reporting diagnostics, and pausing billing until the issue is resolved
**Depends on**: Phase 2 (uses crash detection and billing pause infrastructure)
**Requirements**: LAUNCH-01, LAUNCH-02, LAUNCH-03
**Success Criteria** (what must be TRUE):
  1. Content Manager process hangs → AC falls back to direct acs.exe launch within 15 seconds (existing behavior verified + improved)
  2. Launch failure diagnostic info (CM exit code, CM log errors, acs.exe exit code) appears on kiosk dashboard for the pod
  3. When game launch fails completely (no acs.exe running after all attempts), billing is auto-paused
  4. After a failed launch, staff can retry from kiosk without creating a new billing session
**Plans**: 2 plans

Plans:
- [x] 03-01-PLAN.md — rc-agent: improve CM fallback diagnostics (structured LaunchResult with error details), report to racecontrol via enhanced GameStateChanged
- [x] 03-02-PLAN.md — racecontrol: handle LaunchFailed (auto-pause billing, store diagnostics), kiosk shows launch error details + retry button

### Phase 4: Multiplayer Server Lifecycle
**Goal**: When staff or customer books multiplayer, the AC server starts automatically. When billing ends, the server stops. Customers can book multiplayer directly from the kiosk without staff — friends walk in, pick a game, get PINs, and drive together.
**Depends on**: Phase 3 (uses launch resilience + billing pause infrastructure)
**Requirements**: MULTI-01, MULTI-02, MULTI-03, MULTI-04
**Success Criteria** (what must be TRUE):
  1. Staff books a multiplayer session from kiosk → acServer.exe starts automatically with the selected track/car config, verified via `tasklist` on server (.23)
  2. All pods in the multiplayer session have billing end → acServer.exe stops within 10 seconds, server process no longer running
  3. Customer walks to kiosk, taps "Play with Friends", selects 2 pods, picks a game → multiplayer booking created, 2 PINs generated, acServer.exe started — all without staff
  4. Each friend in the kiosk booking sees their PIN and pod number on the booking confirmation screen
  5. Existing single-player kiosk booking flow unchanged (regression check)
**Plans**: 2 plans

Plans:
- [x] 04-01-PLAN.md — racecontrol: wire book_multiplayer() → AcServerManager.start(), wire billing end → AcServerManager.stop(), add server lifecycle events to WebSocket dashboard
- [x] 04-02-PLAN.md — kiosk: add "Play with Friends" flow to booking wizard (pod count → experience → review → book), display PINs + pod assignments on confirmation

### Phase 5: Synchronized Group Play
**Goal**: Group events run smoothly — all pods launch and join the server at the same time, staff can run continuous races that auto-restart, and if a pod fails to join the server, staff can see and fix it without restarting everything
**Depends on**: Phase 4 (uses multiplayer server lifecycle)
**Requirements**: GROUP-01, GROUP-02, GROUP-03, GROUP-04
**Success Criteria** (what must be TRUE):
  1. All 3 pods in a multiplayer group validate their PINs → all 3 launch AC and join the server within 5 seconds of each other (coordinated, not sequential)
  2. Staff enables "continuous" mode on a multiplayer session → race ends → new race session starts automatically within 15 seconds — verified by watching acServer output
  3. Continuous mode auto-restart only fires if at least one pod still has active billing — when all billing expires, server stops
  4. Pod 3 fails to join the server → kiosk dashboard shows "Pod 3: Join Failed" with a "Retry" button → staff clicks retry → Pod 3 re-launches and joins
  5. Staff changes track from Monza to Spa between races in continuous mode → next race starts on Spa without stopping/restarting the full flow
**Plans**: 2 plans

Plans:
- [ ] 05-01-PLAN.md — racecontrol: coordinated launch trigger (wait for all PINs validated → send LaunchGame to all pods simultaneously), continuous mode flag on AcServerManager with auto-restart on session end
- [ ] 05-02-PLAN.md — racecontrol + kiosk: per-pod join status tracking, failure display on dashboard with retry button, mid-session config change (track/car swap between races)

## Progress

**Execution Order:**
Phase 1 first (most critical — revenue loss). Phase 2 depends on Phase 1's game state infrastructure. Phase 3 depends on Phase 2's billing pause infrastructure. Phase 4 depends on Phase 3 (launch resilience needed before automating multiplayer). Phase 5 depends on Phase 4 (server lifecycle needed before coordinated play).

| Phase | Plans | Status | Completed |
|-------|-------|--------|-----------|
| 1. Billing-Game Lifecycle | 2/2 | Complete | 2026-03-15 |
| 2. Game Crash Recovery | 2/2 | Complete | 2026-03-15 |
| 3. Launch Resilience | 2/2 | Complete | 2026-03-15 |
| 4. Multiplayer Server Lifecycle | 2/2 | Complete | 2026-03-15 |
| 5. Synchronized Group Play | 0/2 | Not started | - |

**Total: 8/10 plans complete**

## Dependency Graph

```
Phase 1: Billing-Game Lifecycle (CRITICAL — fixes revenue loss)
    |
    +---> Phase 2: Game Crash Recovery (uses StopGame + game state)
              |
              +---> Phase 3: Launch Resilience (uses crash detection + billing pause)
                        |
                        +---> Phase 4: Multiplayer Server Lifecycle (uses launch resilience + billing wiring)
                                  |
                                  +---> Phase 5: Synchronized Group Play (uses server lifecycle)
```

Execution order: 1 → 2 → 3 → 4 → 5 (strict dependency chain)
