# Requirements: AC Launch Reliability

**Defined:** 2026-03-15
**Core Value:** No customer ever plays for free and no customer ever pays for downtime — billing and game process always in sync.

## v5.0 Requirements

### Billing-Game Lifecycle (LIFE)

- [x] **LIFE-01**: When billing session expires or is manually stopped, the running game is force-closed within 10 seconds
- [x] **LIFE-02**: Staff cannot launch a game on a pod that has no active billing session
- [x] **LIFE-03**: After session ends, pod shows a brief session summary (15s) then returns to the idle lock screen automatically
- [x] **LIFE-04**: Rapid "launch game" requests are deduplicated — only one game launch per active billing session

### Game Crash Recovery (CRASH)

- [x] **CRASH-01**: rc-agent detects game process exit within 5 seconds of the process ending
- [x] **CRASH-02**: Billing timer auto-pauses when the game process crashes or closes unexpectedly
- [x] **CRASH-03**: Staff sees "Game Crashed" status on kiosk dashboard for the affected pod
- [x] **CRASH-04**: Staff can re-launch the game from kiosk after a crash without starting a new billing session

### Launch Resilience (LAUNCH)

- [x] **LAUNCH-01**: When Content Manager hangs or fails, AC falls back to direct acs.exe launch within 15 seconds
- [x] **LAUNCH-02**: Game launch failure details (exit code, CM log errors) are reported to rc-core and visible on the dashboard
- [x] **LAUNCH-03**: When game launch fails entirely, billing is auto-paused until staff takes action

### Multiplayer Server Lifecycle (MULTI)

- [x] **MULTI-01**: When a multiplayer booking is confirmed, acServer.exe auto-starts with the selected track/car/session config
- [x] **MULTI-02**: When billing ends for all pods in a multiplayer session, acServer.exe auto-stops within 10 seconds
- [x] **MULTI-03**: Customer can select "Play with Friends" on kiosk booking wizard to start a multiplayer session without staff
- [x] **MULTI-04**: Each friend in a kiosk multiplayer booking gets a unique PIN and assigned pod number

### Synchronized Group Play (GROUP)

- [ ] **GROUP-01**: All pods in a multiplayer group launch AC and join the server simultaneously (coordinated start)
- [ ] **GROUP-02**: Staff can enable "continuous" mode — when a race ends, a new session auto-starts while billing is active
- [ ] **GROUP-03**: If any pod fails to join the AC server, staff sees which pod failed and can retry from kiosk
- [ ] **GROUP-04**: Staff can change track/car between races in continuous mode without stopping the full AC server

## Future Requirements

### Session Intelligence

- **INTEL-01**: Experience (car/track) linked to billing session for revenue analytics
- **INTEL-02**: Auto-pause billing during 10s idle threshold (spec exists, currently disabled)

## Out of Scope

| Feature | Reason |
|---------|--------|
| F1 25 / Forza launch reliability | AC only for this milestone — other sims follow same patterns |
| Billing algorithm changes | Already done in credits migration (cc3da21) |
| HUD overlay changes | Separate milestone (archived) |
| Cloud dashboard game state | Separate GSD (billing-pos Phase 2) |
| Lock screen visual redesign | Only lifecycle state transitions, not visual changes |
| QR auth race conditions | Separate issue (customer-journey-gaps #3) |
| Per-pod scenario groups | All 8 pods identical — no hardware differentiation needed |
| Public lobby browser | Venue is invite-only multiplayer, not open lobbies |
| Tournament/championship scoring | Separate GSD (v3.0 Phase 14 — Events & Championships) |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| LIFE-01 | Phase 1 | Complete (01-01) |
| LIFE-02 | Phase 1 | Complete (01-01) |
| LIFE-03 | Phase 1 | Complete (01-02) |
| LIFE-04 | Phase 1 | Complete (01-01) |
| CRASH-01 | Phase 2 | Complete (pre-existing 2s polling) |
| CRASH-02 | Phase 2 | Complete (02-01) |
| CRASH-03 | Phase 2 | Complete (02-02) |
| CRASH-04 | Phase 2 | Complete (02-01 + 02-02) |
| LAUNCH-01 | Phase 3 | Complete (03-01) |
| LAUNCH-02 | Phase 3 | Complete (03-01) |
| LAUNCH-03 | Phase 3 | Complete (03-02) |
| MULTI-01 | Phase 4 | Complete (04-01) |
| MULTI-02 | Phase 4 | Complete (04-01) |
| MULTI-03 | Phase 4 | Complete (04-01 backend + 04-02 kiosk UI) |
| MULTI-04 | Phase 4 | Complete (04-01 backend + 04-02 kiosk UI) |
| GROUP-01 | Phase 5 | Pending |
| GROUP-02 | Phase 5 | Pending |
| GROUP-03 | Phase 5 | Pending |
| GROUP-04 | Phase 5 | Pending |

**Coverage:**
- v5.0 requirements: 19 total
- Mapped to phases: 19
- Unmapped: 0

---
*Requirements defined: 2026-03-15*
*Last updated: 2026-03-15 after Phase 4 Plan 02 (kiosk multiplayer UI)*
