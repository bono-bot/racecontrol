# Requirements: RaceControl

**Last updated:** 2026-03-17
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
- [x] **BILL-04**: Bot-triggered session end fences cloud sync — waits for sync acknowledgment before completing teardown to prevent wallet CRDT race

### Server Bot Coordinator

- [x] **BOT-01**: `bot_coordinator.rs` on racecontrol server handles billing recovery message routing and server-side bot responses

### Lap Quality

- [x] **LAP-01**: `is_valid` flag wired from AC and F1 25 sim adapters into `persist_lap` (currently unwired in both adapters)
- [x] **LAP-02**: Per-track minimum lap time configurable in track catalog (Monza, Silverstone, Spa as initial set)
- [x] **LAP-03**: Laps classified as hotlap vs practice based on session type reported by sim adapter

### PIN Security

- [x] **PIN-01**: Customer and staff PIN failure counters tracked separately (not shared counter)
- [x] **PIN-02**: Staff PIN is never locked out by customer PIN failure accumulation

### Telemetry & Multiplayer

- [x] **TELEM-01**: Bot detects UDP silence >60s during active billing session and alerts staff via email — game-state-aware (no alert during menu or idle state)
- [x] **MULTI-01**: Bot detects AC multiplayer server disconnect mid-race and triggers safe session teardown (lock screen → end billing → log event)

## v5.5 Requirements — Billing Credits

**Defined:** 2026-03-17
**Core Value:** Staff can adjust pricing instantly from the admin panel without a code deploy, and every screen shows credits instead of rupees.

### Billing Engine (BILL)

- [x] **BILLC-01**: User session cost displays in credits (1 cr = ₹1 = 100 paise) in overlay, kiosk, and admin — not rupees
- [x] **BILLC-02**: `compute_session_cost()` uses non-retroactive additive algorithm: 45 min = (30 × 25) + (15 × 20) = 1050 cr, not 45 × 20
- [x] **BILLC-03**: BillingManager holds in-memory rate cache (`RwLock<Vec<BillingRateTier>>`) with hardcoded defaults matching seed data
- [x] **BILLC-04**: Rate cache refreshes from DB at startup and every 60s — never blocks the per-second billing tick
- [x] **BILLC-05**: Final session cost saved to `wallet_debit_paise` column on session end

### Rate Configuration (RATE)

- [x] **RATE-01**: `billing_rates` table with columns: id, tier_order, tier_name, threshold_minutes, rate_per_min_paise, is_active
- [x] **RATE-02**: Three default seed rows: Standard (0–30 min, 2500 p/min = 25 cr/min), Extended (31–60 min, 2000 p/min = 20 cr/min), Marathon (60+ min, 1500 p/min = 15 cr/min)
- [x] **RATE-03**: `billing_rates` added to cloud_sync SYNC_TABLES for cloud replication

### Admin API (ADMIN)

- [x] **ADMIN-01**: Staff can GET all billing rates via `/billing/rates`
- [x] **ADMIN-02**: Staff can create a rate tier via POST `/billing/rates`
- [x] **ADMIN-03**: Staff can update a rate tier via PUT `/billing/rates/{id}` — cache invalidates immediately
- [x] **ADMIN-04**: Staff can delete a rate tier via DELETE `/billing/rates/{id}` — cache invalidates immediately

### UI — Credits Display (UIC)

- [x] **UIC-01**: Overlay `format_cost()` shows "X cr" instead of "Rs. X" (rc-agent overlay.rs)
- [x] **UIC-02**: Admin billing history page shows credits (replaces formatINR)
- [x] **UIC-03**: Admin pricing page includes Per-Minute Rates section with inline editing (replaces formatINR)
- [x] **UIC-04**: BillingStartModal shows credits (replaces formatINR)

### Protocol (PROTOC)

- [x] **PROTOC-01**: `minutes_to_value_tier` renamed to `minutes_to_next_tier` in rc-common protocol.rs with `#[serde(alias)]` backward compat
- [x] **PROTOC-02**: `tier_name` field added to BillingTick as `Option<String>` (previously `&'static str`)

## v6.0 Requirements — Salt Fleet Management

**Defined:** 2026-03-17
**Core Value:** Fleet management via SaltStack replaces custom pod-agent/remote_ops — standard tooling, no custom HTTP endpoints, one command manages all pods.

### Infrastructure (INFRA)

- [ ] **INFRA-01**: WSL2 Ubuntu 24.04 with mirrored networking mode configured on James (.27), reachable from pods at 192.168.31.27
- [ ] **INFRA-02**: salt-master 3008 LTS installed in WSL2, listening on TCP 4505/4506
- [ ] **INFRA-03**: Both firewall layers opened — Windows Defender + Hyper-V firewall for inbound 4505/4506 on James's machine
- [ ] **INFRA-04**: salt-api (rest_cherrypy) running in WSL2 with token auth, accessible from racecontrol server (.23)
- [ ] **INFRA-05**: WSL2 + salt-master + salt-api auto-start on James's machine boot via Windows Task Scheduler

### Minion Bootstrap (MINION)

- [ ] **MINION-01**: Salt minion 3008 LTS silently installed on Pod 8 (canary) with minion ID `pod8` pointing to master 192.168.31.27
- [ ] **MINION-02**: `salt 'pod8' test.ping` returns True from James's WSL2 terminal
- [ ] **MINION-03**: install.bat rewritten — Defender exclusions + rc-agent binary + salt-minion MSI bootstrap only (all pod-agent portions removed)
- [ ] **MINION-04**: Windows `sc failure` recovery configured for salt-minion service on every pod
- [ ] **MINION-05**: Salt minion installed on all 8 pods + server (.23), all keys accepted, `salt '*' test.ping` all True

### Rust Integration (SALT)

- [ ] **SALT-01**: `salt_exec.rs` module in racecontrol crate wrapping salt-api REST calls (cmd.run, cp.get_file, service management) via existing reqwest
- [ ] **SALT-02**: `deploy.rs` migrated from pod-agent HTTP to salt_exec for binary distribution and service restart
- [ ] **SALT-03**: `fleet_health.rs` migrated from pod-agent health checks to Salt test.ping / grains
- [ ] **SALT-04**: `pod_monitor.rs` migrated from pod-agent status checks to salt_exec
- [ ] **SALT-05**: `pod_healer.rs` migrated from pod-agent exec to salt_exec for healing commands

### Code Removal (PURGE)

- [ ] **PURGE-01**: `remote_ops.rs` deleted from rc-agent, port 8090 HTTP listener removed
- [ ] **PURGE-02**: All pod-agent references removed from Rust source (firewall.rs 8090 rules, constants, imports)
- [ ] **PURGE-03**: Pod-agent references removed from deploy scripts, training data, and operational docs
- [ ] **PURGE-04**: Port 8090 firewall rules removed from install.bat and pod netsh configs
- [ ] **PURGE-05**: rc-agent compiles and all existing tests pass without remote_ops module

### Fleet Rollout (FLEET)

- [ ] **FLEET-01**: Pod 8 canary — rc-agent without remote_ops deployed, billing lifecycle verified end-to-end
- [ ] **FLEET-02**: All 8 pods running rc-agent without remote_ops, Salt connectivity verified
- [ ] **FLEET-03**: Deploy workflow fully migrated — staff can deploy new rc-agent.exe to any pod via Salt from James's machine

## Future Requirements (Deferred)

### Advanced Bot Intelligence

- **DBG-01**: `DebugMemory` pattern keys include billing context (billing_active, session_duration) — prevents destructive mid-session fix replay
- **DBG-02**: Bot action log visible in staff dashboard (/kiosk/bot-log) — shows what was fixed, when, on which pod

### Differentiators (Deferred)

- **CRASH-D1**: Multi-crash threshold detection — 3 crashes in 30 min triggers "unhealthy pod" alert, suppresses auto-restart
- **USB-D1**: USB hub power-cycle via Windows DevCon API when reconnect polling fails 3 times
- **BILL-D1**: Auto-refund partial credit when bot terminates session due to hardware failure (with staff approval gate)

### Salt Advanced (Deferred)

- **SALT-D1**: Salt state files for idempotent pod configuration enforcement (Defender exclusions, power settings, registry)
- **SALT-D2**: Custom Salt grains for pod metadata (pod number, MAC, wheelbase model, game installs)
- **SALT-D3**: Salt beacon for proactive monitoring (disk space, process health)

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
| BILL-04 | Phase 25 | Complete |
| BOT-01 | Phase 25 | Complete |
| LAP-01 | Phase 26 | Complete |
| LAP-02 | Phase 26 | Complete |
| LAP-03 | Phase 26 | Complete |
| PIN-01 | Phase 26 | Complete |
| PIN-02 | Phase 26 | Complete |
| TELEM-01 | Phase 26 | Complete |
| MULTI-01 | Phase 26 | Complete |

| BILLC-01 | Phase 35 | Complete |
| BILLC-02 | Phase 33 | Complete |
| BILLC-03 | Phase 33 | Complete |
| BILLC-04 | Phase 33 | Complete |
| BILLC-05 | Phase 33 | Complete |
| RATE-01 | Phase 33 | Complete |
| RATE-02 | Phase 33 | Complete |
| RATE-03 | Phase 33 | Complete |
| ADMIN-01 | Phase 34 | Complete |
| ADMIN-02 | Phase 34 | Complete |
| ADMIN-03 | Phase 34 | Complete |
| ADMIN-04 | Phase 34 | Complete |
| UIC-01 | Phase 35 | Complete |
| UIC-02 | Phase 35 | Complete |
| UIC-03 | Phase 35 | Complete |
| UIC-04 | Phase 35 | Complete |
| PROTOC-01 | Phase 33 | Complete |
| PROTOC-02 | Phase 33 | Complete |

**Coverage:**
- v5.0 requirements: 19 total
- Mapped to phases: 19
- Unmapped: 0
- v5.5 requirements: 18 total
- Mapped to phases: 18
- Unmapped: 0

---
*Requirements defined: 2026-03-16*
*Last updated: 2026-03-17 — v6.0 Salt Fleet Management requirements added (20 reqs: 5 INFRA, 5 MINION, 5 SALT, 5 PURGE, 3 FLEET)*
