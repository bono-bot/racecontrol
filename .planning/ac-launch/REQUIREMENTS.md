# Requirements: AC Launch Reliability

**Defined:** 2026-03-15
**Core Value:** No customer ever plays for free and no customer ever pays for downtime — billing and game process always in sync.

## v5.0 Requirements

### Billing-Game Lifecycle (LIFE)

- [x] **LIFE-01**: When billing session expires or is manually stopped, the running game is force-closed within 10 seconds
- [x] **LIFE-02**: Staff cannot launch a game on a pod that has no active billing session
- [ ] **LIFE-03**: After session ends, pod shows a brief session summary (15s) then returns to the idle lock screen automatically
- [x] **LIFE-04**: Rapid "launch game" requests are deduplicated — only one game launch per active billing session

### Game Crash Recovery (CRASH)

- [ ] **CRASH-01**: rc-agent detects game process exit within 5 seconds of the process ending
- [ ] **CRASH-02**: Billing timer auto-pauses when the game process crashes or closes unexpectedly
- [ ] **CRASH-03**: Staff sees "Game Crashed" status on kiosk dashboard for the affected pod
- [ ] **CRASH-04**: Staff can re-launch the game from kiosk after a crash without starting a new billing session

### Launch Resilience (LAUNCH)

- [ ] **LAUNCH-01**: When Content Manager hangs or fails, AC falls back to direct acs.exe launch within 15 seconds
- [ ] **LAUNCH-02**: Game launch failure details (exit code, CM log errors) are reported to rc-core and visible on the dashboard
- [ ] **LAUNCH-03**: When game launch fails entirely, billing is auto-paused until staff takes action

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

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| LIFE-01 | Phase 1 | Complete (01-01) |
| LIFE-02 | Phase 1 | Complete (01-01) |
| LIFE-03 | Phase 1 | Pending |
| LIFE-04 | Phase 1 | Complete (01-01) |
| CRASH-01 | Phase 2 | Pending |
| CRASH-02 | Phase 2 | Pending |
| CRASH-03 | Phase 2 | Pending |
| CRASH-04 | Phase 2 | Pending |
| LAUNCH-01 | Phase 3 | Pending |
| LAUNCH-02 | Phase 3 | Pending |
| LAUNCH-03 | Phase 3 | Pending |

**Coverage:**
- v5.0 requirements: 11 total
- Mapped to phases: 11
- Unmapped: 0

---
*Requirements defined: 2026-03-15*
*Last updated: 2026-03-15 after Plan 01-01 completion*
