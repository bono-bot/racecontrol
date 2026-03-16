# Requirements: RaceControl

**Last updated:** 2026-03-16
**Core Value (v5.0):** The auto-fix bot handles every common failure class autonomously — staff only intervene for hardware replacement and physical reboots.

## v4.5 Requirements — AC Launch Reliability (Completed 2026-03-16)

**Core Value:** No customer ever plays for free and no customer ever pays for downtime — billing and game process always in sync.

### Billing-Game Lifecycle (LIFE) — Phases 28

- [x] **LIFE-01**: When billing session expires or is manually stopped, the running game is force-closed within 10 seconds
- [x] **LIFE-02**: Staff cannot launch a game on a pod that has no active billing session
- [x] **LIFE-03**: After session ends, pod shows a brief session summary (15s) then returns to the idle lock screen automatically
- [x] **LIFE-04**: Rapid "launch game" requests are deduplicated — only one game launch per active billing session

### Game Crash Recovery (GCR) — Phase 29

> Note: GCR prefix used to avoid collision with v5.0 bot CRASH- requirements (different layer: lifecycle vs auto-fix)

- [x] **GCR-01**: rc-agent detects game process exit within 5 seconds of the process ending
- [x] **GCR-02**: Billing timer auto-pauses when the game process crashes or closes unexpectedly
- [x] **GCR-03**: Staff sees "Game Crashed" status on kiosk dashboard for the affected pod
- [x] **GCR-04**: Staff can re-launch the game from kiosk after a crash without starting a new billing session

### Launch Resilience (LAUNCH) — Phase 30

- [x] **LAUNCH-01**: When Content Manager hangs or fails, AC falls back to direct acs.exe launch within 15 seconds
- [x] **LAUNCH-02**: Game launch failure details (exit code, CM log errors) are reported to racecontrol and visible on the dashboard
- [x] **LAUNCH-03**: When game launch fails entirely, billing is auto-paused until staff takes action

### AC Multiplayer Lifecycle (AML) — Phase 31

> Note: AML prefix used to avoid collision with v5.0 bot MULTI- requirements (different layer: server lifecycle vs bot recovery)

- [x] **AML-01**: When a multiplayer booking is confirmed, acServer.exe auto-starts with the selected track/car/session config
- [x] **AML-02**: When billing ends for all pods in a multiplayer session, acServer.exe auto-stops within 10 seconds
- [x] **AML-03**: Customer can select "Play with Friends" on kiosk booking wizard to start a multiplayer session without staff
- [x] **AML-04**: Each friend in a kiosk multiplayer booking gets a unique PIN and assigned pod number

### Synchronized Group Play (GROUP) — Phase 32

- [x] **GROUP-01**: All pods in a multiplayer group launch AC and join the server simultaneously (coordinated start)
- [x] **GROUP-02**: Staff can enable "continuous" mode — when a race ends, a new session auto-starts while billing is active
- [x] **GROUP-03**: If any pod fails to join the AC server, staff sees which pod failed and can retry from kiosk
- [x] **GROUP-04**: Staff can change track/car between races in continuous mode without stopping the full AC server

---

## v5.0 Requirements

### Protocol Foundation

- [x] **PROTO-01**: rc-common `PodFailureReason` enum covers all 9 bot failure classes (crash, hang, launch, USB, billing, telemetry, multiplayer, PIN, lap)
- [x] **PROTO-02**: 5 new `AgentMessage` variants (HardwareFailure, TelemetryGap, BillingAnomaly, LapFlagged, MultiplayerFailure) for pod→server reporting
- [x] **PROTO-03**: `is_pod_in_recovery()` shared utility in rc-common prevents concurrent fix races across all bot tasks

### Crash, Hang & Launch Bot

- [x] **CRASH-01**: Bot detects game freeze (UDP silent 30s + IsHungAppWindow) and kills/restarts game without staff intervention
- [x] **CRASH-02**: Bot detects launch timeout (game not running 90s after launch command) and kills Content Manager + retries launch
- [x] **CRASH-03**: Bot zeros FFB torque before any game kill in teardown sequence (safety ordering — FFB zero must precede game kill)
- [x] **UI-01**: Bot suppresses Windows error dialogs (WER, crash reporters) before any process kill — customer never sees system internals during recovery

### USB Hardware Bot

- [x] **USB-01**: Bot polls for wheelbase USB reconnect (hidapi 5s scan, VID:0x1209 PID:0xFFB0) and restarts FFB controller when device re-appears

### Billing Guard

- [x] **BILL-01**: `billing.rs` characterization test suite written before any billing bot code — covers start_session, end_session, idle detection, sync paths
- [x] **BILL-02**: Bot detects stuck session (billing active >60s after game process exits) and triggers safe `end_session()` via correct StopSession → SessionUpdate::Finished order
- [x] **BILL-03**: Bot detects idle billing drift (billing active + DrivingState inactive > 5 minutes) and alerts staff rather than auto-ending
- [ ] **BILL-04**: Bot-triggered session end fences cloud sync — waits for sync acknowledgment before completing teardown to prevent wallet CRDT race

### Server Bot Coordinator

- [x] **BOT-01**: `bot_coordinator.rs` on racecontrol server handles billing recovery message routing and server-side bot responses

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
| LIFE-01 | Phase 28 | Complete |
| LIFE-02 | Phase 28 | Complete |
| LIFE-03 | Phase 28 | Complete |
| LIFE-04 | Phase 28 | Complete |
| GCR-01 | Phase 29 | Complete |
| GCR-02 | Phase 29 | Complete |
| GCR-03 | Phase 29 | Complete |
| GCR-04 | Phase 29 | Complete |
| LAUNCH-01 | Phase 30 | Complete |
| LAUNCH-02 | Phase 30 | Complete |
| LAUNCH-03 | Phase 30 | Complete |
| AML-01 | Phase 31 | Complete |
| AML-02 | Phase 31 | Complete |
| AML-03 | Phase 31 | Complete |
| AML-04 | Phase 31 | Complete |
| GROUP-01 | Phase 32 | Complete |
| GROUP-02 | Phase 32 | Complete |
| GROUP-03 | Phase 32 | Complete |
| GROUP-04 | Phase 32 | Complete |
| PROTO-01 | Phase 23 | Complete |
| PROTO-02 | Phase 23 | Complete |
| PROTO-03 | Phase 23 | Complete |
| CRASH-01 | Phase 24 | Complete |
| CRASH-02 | Phase 24 | Complete |
| CRASH-03 | Phase 24 | Complete |
| UI-01 | Phase 24 | Complete |
| USB-01 | Phase 24 | Complete |
| BILL-01 | Phase 25 | Complete |
| BILL-02 | Phase 25 | Complete |
| BILL-03 | Phase 25 | Complete |
| BILL-04 | Phase 25 | Pending |
| BOT-01 | Phase 25 | Complete |
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
- Unmapped: 0

---
*Requirements defined: 2026-03-16*
*Last updated: 2026-03-16 — roadmap created, phase assignments confirmed*
