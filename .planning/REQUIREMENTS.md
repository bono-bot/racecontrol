# Requirements: v41.0 Game Intelligence System

**Defined:** 2026-04-03
**Core Value:** Proactive game availability management — stop showing customers games they can't play, flag broken combos before launch, surface failures instantly through Meshed Intelligence.

## v41.0 Requirements

Requirements for Game Intelligence System. Each maps to roadmap phases.

### Game Inventory

- [x] **INV-01**: Agent scans Steam library (libraryfolders.vdf parsing) + configured non-Steam paths at boot and reports all installed games to server via GameInventoryUpdate WS message
- [x] **INV-02**: Server persists per-pod game inventory in `pod_game_inventory` table, updated on each agent connect/heartbeat
- [ ] **INV-03**: Kiosk game picker only shows games installed on the current pod (filtered by server-side pod_game_inventory)
- [x] **INV-04**: Agent re-scans game inventory every 5 minutes (periodic re-fetch pattern per boot resilience standing rule)

### Combo Validation

- [x] **COMBO-01**: Agent validates all enabled AC presets against local filesystem at boot — car folder exists, track folder exists, track config subfolder exists, AI line files present
- [x] **COMBO-02**: Agent sends `ComboValidationResult` per preset to server after boot-time validation completes (async-decoupled from WS connect)
- [x] **COMBO-03**: Server aggregates combo validation across fleet — marks presets as valid (all pods), partial (some pods), or invalid (no pods) with per-pod availability list
- [x] **COMBO-04**: Presets invalid on ALL pods are auto-disabled with reason logged and staff notification
- [ ] **COMBO-05**: Kiosk only shows AC car+track combos that are valid for the current pod

### Launch Intelligence

- [x] **LAUNCH-01**: Launch timeout watchdog — if no GameStateUpdate ACK within 90s default (or dynamic per-combo from historical data), auto-transition GameTracker to Error state and trigger DiagnosticTrigger::GameLaunchTimeout
- [ ] **LAUNCH-02**: New `DiagnosticTrigger::GameLaunchTimeout` variant wired into tier_engine Tier 1 Game Doctor diagnostic, with `#[serde(other)]` added to enum BEFORE new variant (backward compat)
- [x] **LAUNCH-03**: Crash loop detection — 3+ agent restarts in 5min triggers ERROR log + WhatsApp alert + `crash_loop: true` flag in fleet health response
- [x] **LAUNCH-04**: Chain failure detection — 3+ consecutive game launch failures on same pod/combo triggers `MeshSystemicAlert` with severity=Critical via EscalationRequest WS path to WhatsApp
- [ ] **LAUNCH-05**: Launch timeline events (ws_sent, agent_received, process_spawned, playable_signal) stored in `launch_timeline_spans` table with incident-level timestamps

### Reliability Dashboard

- [ ] **DASH-01**: Admin dashboard page shows fleet game matrix — which pods have which games installed, with install status badges
- [ ] **DASH-02**: Admin dashboard page shows per-combo reliability scores with flagged unreliable combos highlighted in red, sortable by success rate
- [ ] **DASH-03**: Admin dashboard page shows launch timeline for debugging specific incidents — expandable per-launch event view with checkpoint timestamps

## Future Requirements

### Enhanced Intelligence (v42.0+)

- **INTEL-01**: Dynamic timeout displayed in kiosk UI so staff knows if "still launching" is normal
- **INTEL-02**: Per-combo crash spiral detection — combos with >90% failure rate auto-escalate max_auto_relaunch
- **INTEL-03**: Chaos testing for WS drop during launch (automated test suite)
- **INTEL-04**: Post-relaunch billing audit test (relaunch must NOT create new BillingTimer)
- **INTEL-05**: race.ini content verification — parse and verify config matches user selection post-generation

## Out of Scope

| Feature | Reason |
|---------|--------|
| Real-time filesystem watcher for game installs | Unreliable on Windows/Steam — polling at 5min intervals is safer and simpler |
| Automated combo disable without staff review | Hides transient failures — only auto-disable when invalid on ALL pods |
| Cross-pod combo sync (pod A installs → pod B shows) | Each pod reports its own state independently; no cross-pod push needed |
| Dynamic timeout display in kiosk UI | Infrastructure exists (LAUNCH-08 from v24.0), UI deferred to future |
| Non-AC combo validation (F1, iRacing, Forza) | Non-AC games are binary (installed or not) — no car+track combos to validate |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| INV-01 | Phase 316 | Complete |
| INV-02 | Phase 317 | Complete |
| INV-03 | Phase 320 | Pending |
| INV-04 | Phase 316 | Complete |
| COMBO-01 | Phase 316 | Complete |
| COMBO-02 | Phase 316 | Complete |
| COMBO-03 | Phase 317 | Complete |
| COMBO-04 | Phase 317 | Complete |
| COMBO-05 | Phase 320 | Pending |
| LAUNCH-01 | Phase 318 | Complete |
| LAUNCH-02 | Phase 315 | Pending |
| LAUNCH-03 | Phase 317 | Complete |
| LAUNCH-04 | Phase 317 | Complete |
| LAUNCH-05 | Phase 318 | Pending |
| DASH-01 | Phase 319 | Pending |
| DASH-02 | Phase 319 | Pending |
| DASH-03 | Phase 319 | Pending |

**Coverage:**
- v41.0 requirements: 17 total
- Mapped to phases: 17
- Unmapped: 0 ✓

---
*Requirements defined: 2026-04-03*
*Last updated: 2026-04-03 — traceability filled after roadmap creation*
