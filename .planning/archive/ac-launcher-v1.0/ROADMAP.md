# Roadmap: AC Launcher

## Overview

Transform the existing racecontrol custom experience booking into a complete Assetto Corsa session management system. The work is primarily gap-filling and integration -- roughly 70% of the infrastructure exists. The journey moves from enabling new session types (race with AI), through billing accuracy and safety enforcement, content validation and filtering, mid-session controls, preset curation, and finally multiplayer enhancement. Each phase delivers a verifiable capability on the pods.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 1: Session Types & Race Mode** - Enable all single-player session types including race vs AI with configurable grid
- [x] **Phase 2: Difficulty Tiers** - Racing-themed difficulty presets that map to AC parameters and assist defaults
- [x] **Phase 3: Billing Synchronization** - Billing starts on-track (not at launch) with time-remaining overlay
- [x] **Phase 4: Safety Enforcement** - Grip/damage always enforced, FFB zeroed safely on session end (completed 2026-03-13)
- [x] **Phase 5: Content Validation & Filtering** - Only valid car/track/session combos shown, per-pod content scanning
- [x] **Phase 6: Mid-Session Controls** - Transmission, ABS, TC, FFB adjustable while driving (SC excluded -- no runtime mechanism)
- [x] **Phase 7: Curated Presets** - Popular car/track/session packages for quick launch
- [x] **Phase 8: Staff & PWA Integration** - Staff kiosk configuration and PWA/QR launch flow working end-to-end (completed 2026-03-14)
- [x] **Phase 9: Multiplayer Enhancement** - AI grid fillers, synchronized billing, lobby UI for multi-pod races

## Phase Details

### Phase 1: Session Types & Race Mode
**Goal**: Customers can launch every single-player session type from the system, including the core missing feature -- racing against AI opponents with a configurable grid
**Depends on**: Nothing (first phase)
**Requirements**: SESS-01, SESS-02, SESS-03, SESS-04, SESS-05, SESS-08
**Success Criteria** (what must be TRUE):
  1. Customer can launch Practice mode and hot-lap solo on any track
  2. Customer can launch Race vs AI mode and race against a configurable number of AI opponents
  3. Customer can launch Hotlap mode with timed lap tracking
  4. Customer can launch Track Day mode with mixed AI traffic
  5. Customer can launch Race Weekend mode that sequences through Practice, Qualify, and Race
**Plans**: 2 plans

Plans:
- [ ] 01-01-PLAN.md -- Extend AcLaunchParams, refactor write_race_ini into composable builder, implement Practice + Hotlap
- [ ] 01-02-PLAN.md -- Implement AI grid generation for Race vs AI, Track Day mixed traffic, and Race Weekend multi-session

### Phase 2: Difficulty Tiers
**Goal**: Customers choose a racing-themed difficulty level (Rookie/Amateur/Semi-Pro/Pro/Alien) that controls AI strength via AI_LEVEL, with a slider for fine-tuning. Assists are independent -- not bundled with tiers.
**Depends on**: Phase 1
**Requirements**: DIFF-01, DIFF-02, DIFF-03, DIFF-04, DIFF-05
**Success Criteria** (what must be TRUE):
  1. Customer sees 5 named difficulty tiers (Rookie / Amateur / Semi-Pro / Pro / Alien) when selecting a session
  2. Each tier maps to a specific AI_LEVEL range (assists remain independent per user decision)
  3. Assists (ABS, TC, SC, transmission, ideal line) are completely independent of tier selection
  4. Advanced customer can bypass tiers and set AI_LEVEL directly via slider (0-100)
**Plans**: 1 plan

Plans:
- [x] 02-01-PLAN.md -- DifficultyTier enum, session-wide ai_level on AcLaunchParams, INI builder wiring, TDD tests

### Phase 3: Billing Synchronization
**Goal**: Customers are billed only for time spent actually driving on-track, not loading screens or DirectX initialization
**Depends on**: Phase 1
**Requirements**: BILL-01, BILL-02, BILL-06
**Success Criteria** (what must be TRUE):
  1. Billing timer starts only when AC shared memory reports STATUS=LIVE (car is on-track)
  2. No billable time accrues during game startup, loading screens, or DirectX initialization
  3. Customer can see remaining session time as an overlay while driving
**Plans**: 3 plans

Plans:
- [x] 03-01-PLAN.md -- AcStatus enum, GameStatusUpdate protocol, BillingTimer count-up refactor, compute_session_cost (rc-common + rc-core)
- [x] 03-02-PLAN.md -- Overlay taxi meter display, AC STATUS reading from shared memory, main loop wiring, LaunchState machine (rc-agent)
- [x] 03-03-PLAN.md -- Core-side billing lifecycle: WebSocket GameStatusUpdate handler, auth decoupling, launch timeout handling (rc-core)

### Phase 4: Safety Enforcement
**Goal**: Safety-critical settings are always enforced regardless of session type, and force feedback is handled safely at session boundaries
**Depends on**: Phase 1
**Requirements**: BILL-03, BILL-04, BILL-05
**Success Criteria** (what must be TRUE):
  1. Tyre Grip is always 100% in every session -- verified in race.ini and server config, no customer override possible
  2. Damage Multiplier is always 0% in every session -- verified in race.ini and server config, no customer override possible
  3. When a session ends, FFB torque is zeroed on the wheelbase BEFORE the game process is killed
**Plans**: 2 plans

Plans:
- [ ] 04-01-PLAN.md -- Hardcode DAMAGE=0 in all INI writers, post-write verification, server config overrides, FfbZeroed/GameCrashed protocol messages
- [ ] 04-02-PLAN.md -- Fix FFB zeroing order in all session-end paths, add FFB to StopGame + crash detection, Pod 8 verification

### Phase 5: Content Validation & Filtering
**Goal**: Customers never see a car, track, or session option that would fail to launch -- every displayed option is guaranteed valid
**Depends on**: Phase 1
**Requirements**: SESS-07, CONT-01, CONT-02, CONT-04, CONT-05, CONT-06, CONT-07
**Success Criteria** (what must be TRUE):
  1. Customer browsing cars in PWA only sees cars actually installed on their pod
  2. Customer browsing tracks in PWA only sees tracks actually installed on their pod
  3. Tracks without AI line data (ai/ folder) do not show Race vs AI or Track Day session types
  4. Maximum AI opponent count for a track is capped by that track's pit stall count
  5. No invalid car/track/session combination can be selected -- invalid options are hidden, not just greyed out
**Plans**: 2 plans

Plans:
- [ ] 05-01-PLAN.md -- ContentManifest types in rc-common, content_scanner.rs in rc-agent (filesystem scanning, AI line detection, pit count parsing)
- [ ] 05-02-PLAN.md -- Core-side manifest caching, catalog filtering with per-pod content, launch validation gate, agent manifest sending

### Phase 6: Mid-Session Controls
**Goal**: Customers can adjust driving assists (transmission, ABS, TC) and force feedback while actively driving, without pausing or restarting the session. Stability control excluded -- AC has no runtime mechanism for it.
**Depends on**: Phase 2
**Requirements**: DIFF-06, DIFF-07, DIFF-08, DIFF-09, DIFF-10
**Success Criteria** (what must be TRUE):
  1. Customer can switch between automatic and manual transmission mid-session via PWA
  2. Customer can toggle ABS on/off mid-session via PWA
  3. Customer can toggle traction control on/off mid-session via PWA
  4. Stability control is NOT offered in the UI (AC has no runtime toggle -- excluded by design per user decision)
  5. Customer can adjust force feedback intensity (10-100%) mid-session via PWA
**Plans**: 3 plans

Plans:
- [x] 06-01-PLAN.md -- Protocol messages (SetAssist/SetFfbGain/QueryAssistState), SendInput helpers, FFB set_gain(), shared memory assist reading, overlay toast, main.rs handlers
- [x] 06-02-PLAN.md -- Core-side API routes (POST /assists, updated POST /ffb, GET /assist-state), WebSocket handler for new AgentMessage variants, CachedAssistState in AppState
- [x] 06-03-PLAN.md -- PWA bottom sheet controls: gear icon, ABS/TC/transmission toggles, FFB slider with 500ms debounce, inline confirmation

### Phase 7: Curated Presets
**Goal**: Customers can pick from popular pre-configured experiences for a fast path to driving
**Depends on**: Phase 5
**Requirements**: CONT-08, CONT-09
**Success Criteria** (what must be TRUE):
  1. PWA shows a "Popular" or "Quick Start" section with curated car/track/session combos
  2. Presets include real-world popular AC combinations (e.g. Spa GT3, Nurburgring Hot Lap, Monza F1)
  3. Selecting a preset fills in all fields and lets the customer launch with one tap
**Plans**: 2 plans

Plans:
- [x] 07-01-PLAN.md -- PresetEntry struct, PRESETS static array (14 curated combos), catalog integration with manifest filtering, TDD tests, TypeScript type updates
- [x] 07-02-PLAN.md -- PWA preset landing screen (hero Staff Picks, categorized browsing, Custom Experience), kiosk preset quick-picks, visual verification

### Phase 8: Staff & PWA Integration
**Goal**: Both customer (PWA/QR) and staff (kiosk) launch paths work end-to-end with the new session system
**Depends on**: Phase 5, Phase 6
**Requirements**: SESS-06, CONT-03
**Success Criteria** (what must be TRUE):
  1. Staff can configure any session type, car, track, difficulty, and grid size from the kiosk for any pod
  2. Customer scanning QR on a rig and entering PIN can complete full session selection and launch via PWA
  3. Staff-configured sessions and customer-configured sessions use the same backend validation
**Plans**: 2 plans

Plans:
- [x] 08-01-PLAN.md -- TypeScript type updates (SessionType 5-value union, CustomBookingPayload session_type), Rust backend session_type in CustomBookingOptions + build_custom_launch_args
- [x] 08-02-PLAN.md -- Kiosk SetupWizard 5 types + GameConfigurator session_type step, PWA SessionType step replacing Mode, track filtering by session type

### Phase 9: Multiplayer Enhancement
**Goal**: Multi-pod multiplayer races have AI grid fillers, synchronized billing, and a lobby experience
**Depends on**: Phase 3, Phase 4, Phase 8
**Requirements**: MULT-01, MULT-02, MULT-03, MULT-04, MULT-05, MULT-06
**Success Criteria** (what must be TRUE):
  1. Multiple customers on different pods can join the same race on the AC dedicated server
  2. AI opponents fill remaining grid spots in a multiplayer race
  3. Billing starts and stops simultaneously for all participants in a multiplayer session
  4. PWA shows a lobby/waiting screen with who has joined and race countdown status
  5. Multiplayer uses existing ac_server.rs infrastructure (generate_server_cfg_ini, generate_entry_list_ini, start_ac_server)
**Plans**: 3 plans

Plans:
- [x] 09-01-PLAN.md -- AI grid fillers via AssettoServer, fix LaunchGame JSON format, enrich GroupSessionInfo, move AI names to rc-common
- [ ] 09-02-PLAN.md -- Synchronized billing coordinator: group-aware billing start waits for all LIVE, individual disconnect stops
- [ ] 09-03-PLAN.md -- PWA lobby enrichment: track/car/AI info cards, remaining player count, TypeScript type updates

## Progress

**Execution Order:**
Phases execute in numeric order: 1 -> 2 -> 3 -> 4 -> 5 -> 6 -> 7 -> 8 -> 9

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Session Types & Race Mode | 2/2 | Complete | 2026-03-13 |
| 2. Difficulty Tiers | 1/1 | Complete | 2026-03-13 |
| 3. Billing Synchronization | 3/3 | Complete | 2026-03-14 |
| 4. Safety Enforcement | 2/2 | Complete   | 2026-03-13 |
| 5. Content Validation & Filtering | 2/2 | Complete |  2026-03-14 |
| 6. Mid-Session Controls | 3/3 | Complete | 2026-03-14 |
| 7. Curated Presets | 2/2 | Complete | 2026-03-14 |
| 8. Staff & PWA Integration | 2/2 | Complete | 2026-03-14 |
| 9. Multiplayer Enhancement | 2/3 | In progress | - |
