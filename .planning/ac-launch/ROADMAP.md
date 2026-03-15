# Roadmap: AC Launch Reliability (v5.0)

## Overview

The billing engine and game launcher both work independently — but they're not wired together. When billing ends, the game keeps running. When the game crashes, billing keeps counting. When staff launches without billing, there's no guard. This milestone closes every gap between billing lifecycle and game process lifecycle.

No billing rewrite. No game launcher rewrite. Pure wiring between existing systems.

## Phases

- [ ] **Phase 1: Billing-Game Lifecycle** - Stop game on billing end, validate before launch, pod reset after session, anti-double-launch
- [ ] **Phase 2: Game Crash Recovery** - Detect crash, pause billing, show status, enable re-launch
- [ ] **Phase 3: Launch Resilience** - CM fallback improvements, failure reporting, billing pause on launch failure

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
- [x] 01-01-PLAN.md — rc-core: billing validation gate + double-launch guard fix in game_launcher.rs (LIFE-01, LIFE-02, LIFE-04)
- [ ] 01-02-PLAN.md — rc-agent: arm 15s blank_timer in SessionEnded + fix BillingStopped billing_active flag (LIFE-01, LIFE-03)

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
- [ ] 02-01-PLAN.md — rc-agent: game process monitor (poll PID every 2s), detect unexpected exit, send GameCrashed to rc-core
- [ ] 02-02-PLAN.md — rc-core: handle GameCrashed (auto-pause billing, update GameTracker), kiosk re-launch button for crashed pods

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
- [ ] 03-01-PLAN.md — rc-agent: improve CM fallback diagnostics (structured LaunchResult with error details), report to rc-core via enhanced GameStateChanged
- [ ] 03-02-PLAN.md — rc-core: handle LaunchFailed (auto-pause billing, store diagnostics), kiosk shows launch error details + retry button

## Progress

**Execution Order:**
Phase 1 first (most critical — revenue loss). Phase 2 depends on Phase 1's game state infrastructure. Phase 3 depends on Phase 2's billing pause infrastructure.

| Phase | Plans | Status | Completed |
|-------|-------|--------|-----------|
| 1. Billing-Game Lifecycle | 1/2 | In progress | Plan 01-01 done |
| 2. Game Crash Recovery | 0/2 | Not started | - |
| 3. Launch Resilience | 0/2 | Not started | - |

**Total: 1/6 plans complete**

## Dependency Graph

```
Phase 1: Billing-Game Lifecycle (CRITICAL — fixes revenue loss)
    |
    +---> Phase 2: Game Crash Recovery (uses StopGame + game state)
              |
              +---> Phase 3: Launch Resilience (uses crash detection + billing pause)
```

Execution order: 1 → 2 → 3 (strict dependency chain)
