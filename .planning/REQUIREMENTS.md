# Requirements: RaceControl v5.0 RC Bot Expansion

**Defined:** 2026-03-16
**Core Value:** The auto-fix bot handles every common failure class autonomously — staff only intervene for hardware replacement and physical reboots.

## v5.0 Requirements

### Protocol Foundation

- [ ] **PROTO-01**: rc-common `PodFailureReason` enum covers all 9 bot failure classes (crash, hang, launch, USB, billing, telemetry, multiplayer, PIN, lap)
- [ ] **PROTO-02**: 5 new `AgentMessage` variants (HardwareFailure, TelemetryGap, BillingAnomaly, LapFlagged, MultiplayerFailure) for pod→server reporting
- [ ] **PROTO-03**: `is_pod_in_recovery()` shared utility in rc-common prevents concurrent fix races across all bot tasks

### Crash, Hang & Launch Bot

- [ ] **CRASH-01**: Bot detects game freeze (UDP silent 30s + IsHungAppWindow) and kills/restarts game without staff intervention
- [ ] **CRASH-02**: Bot detects launch timeout (game not running 90s after launch command) and kills Content Manager + retries launch
- [ ] **CRASH-03**: Bot zeros FFB torque before any game kill in teardown sequence (safety ordering — FFB zero must precede game kill)
- [ ] **UI-01**: Bot suppresses Windows error dialogs (WER, crash reporters) before any process kill — customer never sees system internals during recovery

### USB Hardware Bot

- [ ] **USB-01**: Bot polls for wheelbase USB reconnect (hidapi 5s scan, VID:0x1209 PID:0xFFB0) and restarts FFB controller when device re-appears

### Billing Guard

- [ ] **BILL-01**: `billing.rs` characterization test suite written before any billing bot code — covers start_session, end_session, idle detection, sync paths
- [ ] **BILL-02**: Bot detects stuck session (billing active >60s after game process exits) and triggers safe `end_session()` via correct StopSession → SessionUpdate::Finished order
- [ ] **BILL-03**: Bot detects idle billing drift (billing active + DrivingState inactive > 5 minutes) and alerts staff rather than auto-ending
- [ ] **BILL-04**: Bot-triggered session end fences cloud sync — waits for sync acknowledgment before completing teardown to prevent wallet CRDT race

### Server Bot Coordinator

- [ ] **BOT-01**: `bot_coordinator.rs` on racecontrol server handles billing recovery message routing and server-side bot responses

### Lap Quality

- [ ] **LAP-01**: `is_valid` flag wired from AC and F1 25 sim adapters into `persist_lap` (currently unwired in both adapters)
- [ ] **LAP-02**: Per-track minimum lap time configurable in track catalog (Monza, Silverstone, Spa as initial set)
- [ ] **LAP-03**: Laps classified as hotlap vs practice based on session type reported by sim adapter

### PIN Security

- [ ] **PIN-01**: Customer and staff PIN failure counters tracked separately (not shared counter)
- [ ] **PIN-02**: Staff PIN is never locked out by customer PIN failure accumulation

### Telemetry & Multiplayer

- [ ] **TELEM-01**: Bot detects UDP silence >60s during active billing session and alerts staff via email — game-state-aware (no alert during menu or idle state)
- [ ] **MULTI-01**: Bot detects AC multiplayer server disconnect mid-race and triggers safe session teardown (lock screen → end billing → log event)

## v6.0 Requirements (Deferred)

### Advanced Bot Intelligence

- **DBG-01**: `DebugMemory` pattern keys include billing context (billing_active, session_duration) — prevents destructive mid-session fix replay
- **DBG-02**: Bot action log visible in staff dashboard (/kiosk/bot-log) — shows what was fixed, when, on which pod

### Differentiators (Deferred)

- **CRASH-D1**: Multi-crash threshold detection — 3 crashes in 30 min triggers "unhealthy pod" alert, suppresses auto-restart
- **USB-D1**: USB hub power-cycle via Windows DevCon API when reconnect polling fails 3 times
- **BILL-D1**: Auto-refund partial credit when bot terminates session due to hardware failure (with staff approval gate)

## Out of Scope

| Feature | Reason |
|---------|--------|
| LLM-based bot reasoning | Deterministic rules are faster, more reliable, and don't require Ollama active. Ollama use remains manual/diagnostic. |
| Auto-refund on billing anomaly | Too risky without human review — BILL-03 alerts staff instead |
| Multiplayer auto-rejoin | AC session token path does not exist in current architecture. Safe teardown only. |
| Retroactive lap invalidation | Never hard-delete historical laps. `review_required` flag only — FEATURES.md anti-feature. |
| Staff PIN lockout | Staff must always be able to unlock — PIN-02 makes this explicit. |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| PROTO-01 | Phase 23 | Pending |
| PROTO-02 | Phase 23 | Pending |
| PROTO-03 | Phase 23 | Pending |
| CRASH-01 | Phase 24 | Pending |
| CRASH-02 | Phase 24 | Pending |
| CRASH-03 | Phase 24 | Pending |
| UI-01 | Phase 24 | Pending |
| USB-01 | Phase 24 | Pending |
| BILL-01 | Phase 25 | Pending |
| BILL-02 | Phase 25 | Pending |
| BILL-03 | Phase 25 | Pending |
| BILL-04 | Phase 25 | Pending |
| BOT-01 | Phase 25 | Pending |
| LAP-01 | Phase 26 | Pending |
| LAP-02 | Phase 26 | Pending |
| LAP-03 | Phase 26 | Pending |
| PIN-01 | Phase 26 | Pending |
| PIN-02 | Phase 26 | Pending |
| TELEM-01 | Phase 26 | Pending |
| MULTI-01 | Phase 26 | Pending |

**Coverage:**
- v5.0 requirements: 19 total
- Mapped to phases: 19
- Unmapped: 0 ✓

---
*Requirements defined: 2026-03-16*
*Last updated: 2026-03-16 after initial definition*
