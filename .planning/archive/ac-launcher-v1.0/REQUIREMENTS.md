# Requirements: AC Launcher

**Defined:** 2026-03-13
**Core Value:** When a customer selects a session and hits go, the game launches with exactly the settings they chose, billing starts only when they're actually driving, and they never see an option that doesn't work.

## v1 Requirements

### Session Management

- [x] **SESS-01**: Customer can select Practice mode (solo hot-lapping, no AI) from PWA
- [x] **SESS-02**: Customer can select Race vs AI mode with configurable grid size from PWA
- [x] **SESS-03**: Customer can select Hotlap mode (timed laps) from PWA
- [x] **SESS-04**: Customer can select Track Day mode (open pit, mixed traffic) from PWA
- [x] **SESS-05**: Customer can select Race Weekend mode (Practice -> Qualify -> Race sequence) from PWA
- [x] **SESS-06**: Staff can configure any session type from kiosk for a pod
- [x] **SESS-07**: Only valid session/mode combinations are presented (invalid options hidden)
- [x] **SESS-08**: Game launches with the exact preset/config selected -- no silent fallbacks

### Difficulty & Assists

- [x] **DIFF-01**: 5 racing-themed difficulty tiers available: Rookie / Amateur / Semi-Pro / Pro / Alien
- [x] **DIFF-02**: Each tier maps to specific AC parameters (AI_LEVEL, AI_AGGRESSION, assist defaults)
- [x] **DIFF-03**: Rookie tier auto-enables all assists (ABS, TC, SC, auto-transmission, ideal line)
- [x] **DIFF-04**: Alien tier disables all assists (manual everything, no aids)
- [x] **DIFF-05**: Customer can set custom difficulty via slider (direct AI_LEVEL control) for advanced use
- [x] **DIFF-06**: Customer can toggle transmission auto/manual mid-session while driving
- [x] **DIFF-07**: Customer can toggle ABS on/off mid-session
- [x] **DIFF-08**: Customer can toggle traction control on/off mid-session
- [x] **DIFF-09**: Stability control excluded -- AC has no runtime toggle (by design, not offered in UI)
- [x] **DIFF-10**: Customer can adjust force feedback intensity mid-session

### Billing & Safety

- [x] **BILL-01**: Billing timer starts when AC shared memory STATUS=LIVE (on-track), not at game process launch
- [x] **BILL-02**: DirectX initialization delay does not count as billable time
- [x] **BILL-03**: Tyre Grip is always 100% -- enforced in race.ini and server config, not overridable
- [x] **BILL-04**: Damage Multiplier is always 0% -- enforced in race.ini and server config, not overridable
- [x] **BILL-05**: FFB torque zeroed on wheelbase BEFORE game process is killed (safety ordering)
- [x] **BILL-06**: Session time remaining displayed as overlay during gameplay

### Content & Presets

- [x] **CONT-01**: Customer can browse and select car from available catalog via PWA
- [x] **CONT-02**: Customer can browse and select track from available catalog via PWA
- [x] **CONT-03**: Staff can configure car/track/session from kiosk
- [x] **CONT-04**: Invalid car/track/session combinations are filtered out before display
- [x] **CONT-05**: Tracks without AI line data (ai/ folder) hide AI-related session types
- [x] **CONT-06**: Track pit count limits maximum AI opponents shown for that track
- [x] **CONT-07**: Per-pod content scanning -- only show cars/tracks installed on the target pod
- [x] **CONT-08**: Curated popular presets available (e.g. "Spa GT3 Race", "Nurburgring Hot Lap", "Monza F1")
- [x] **CONT-09**: Presets sourced from popular real-world AC community combinations

### Multiplayer

- [x] **MULT-01**: Multiple customers on different pods can race together on AC dedicated server
- [x] **MULT-02**: AI fills remaining grid spots in multiplayer races
- [x] **MULT-03**: Cross-pod billing synchronized -- all participants start/stop billing together
- [x] **MULT-04**: Multiplayer lobby/waiting UI in PWA shows who's joined and race status
- [x] **MULT-05**: Multiplayer uses existing ac_server.rs infrastructure (generate_server_cfg_ini, generate_entry_list_ini, start_ac_server)
- [x] **MULT-06**: Entry list includes real driver names and GUIDs (existing get_driver_entry_info)

## v2 Requirements

### Engagement & Polish

- **ENG-01**: Hotlap leaderboard tracking across sessions
- **ENG-02**: Weather presets (rain, dynamic weather)
- **ENG-03**: Replay recording accessible after session
- **ENG-04**: Post-race results screen in PWA with positions, lap times, gaps

### Advanced Multiplayer

- **AMLT-01**: Race Weekend multiplayer (group Practice -> Qualify -> Race sequence)
- **AMLT-02**: Spectator mode for waiting customers
- **AMLT-03**: Custom livery selection per customer

## Out of Scope

| Feature | Reason |
|---------|--------|
| F1/Forza/iRacing launch | AC-first; other sims are a separate project |
| Voice chat between pods | Hardware-dependent, separate scope |
| Custom livery upload | Content management complexity, defer |
| Online multiplayer (internet) | LAN-only venue; no public server needed |
| Drift/Drag session types | Niche, not customer-requested, defer |
| AI difficulty auto-adjust | Complexity; manual tier selection is sufficient for v1 |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| SESS-01 | Phase 1: Session Types & Race Mode | Complete |
| SESS-02 | Phase 1: Session Types & Race Mode | Complete |
| SESS-03 | Phase 1: Session Types & Race Mode | Complete |
| SESS-04 | Phase 1: Session Types & Race Mode | Complete |
| SESS-05 | Phase 1: Session Types & Race Mode | Complete |
| SESS-06 | Phase 8: Staff & PWA Integration | Complete |
| SESS-07 | Phase 5: Content Validation & Filtering | Complete |
| SESS-08 | Phase 1: Session Types & Race Mode | Complete |
| DIFF-01 | Phase 2: Difficulty Tiers | Complete |
| DIFF-02 | Phase 2: Difficulty Tiers | Complete |
| DIFF-03 | Phase 2: Difficulty Tiers | Complete |
| DIFF-04 | Phase 2: Difficulty Tiers | Complete |
| DIFF-05 | Phase 2: Difficulty Tiers | Complete |
| DIFF-06 | Phase 6: Mid-Session Controls | Complete |
| DIFF-07 | Phase 6: Mid-Session Controls | Complete |
| DIFF-08 | Phase 6: Mid-Session Controls | Complete |
| DIFF-09 | Phase 6: Mid-Session Controls | Complete (excluded by design) |
| DIFF-10 | Phase 6: Mid-Session Controls | Complete |
| BILL-01 | Phase 3: Billing Synchronization | Complete |
| BILL-02 | Phase 3: Billing Synchronization | Complete |
| BILL-03 | Phase 4: Safety Enforcement | Complete |
| BILL-04 | Phase 4: Safety Enforcement | Complete |
| BILL-05 | Phase 4: Safety Enforcement | Complete |
| BILL-06 | Phase 3: Billing Synchronization | Complete |
| CONT-01 | Phase 5: Content Validation & Filtering | Complete |
| CONT-02 | Phase 5: Content Validation & Filtering | Complete |
| CONT-03 | Phase 8: Staff & PWA Integration | Complete |
| CONT-04 | Phase 5: Content Validation & Filtering | Complete |
| CONT-05 | Phase 5: Content Validation & Filtering | Complete |
| CONT-06 | Phase 5: Content Validation & Filtering | Complete |
| CONT-07 | Phase 5: Content Validation & Filtering | Complete |
| CONT-08 | Phase 7: Curated Presets | Complete |
| CONT-09 | Phase 7: Curated Presets | Complete |
| MULT-01 | Phase 9: Multiplayer Enhancement | Complete |
| MULT-02 | Phase 9: Multiplayer Enhancement | Complete |
| MULT-03 | Phase 9: Multiplayer Enhancement | Complete |
| MULT-04 | Phase 9: Multiplayer Enhancement | Complete |
| MULT-05 | Phase 9: Multiplayer Enhancement | Complete |
| MULT-06 | Phase 9: Multiplayer Enhancement | Complete |

**Coverage:**
- v1 requirements: 31 total
- Mapped to phases: 31
- Unmapped: 0

---
*Requirements defined: 2026-03-13*
*Last updated: 2026-03-14 after plan 09-01 completion*
