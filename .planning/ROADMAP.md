# Roadmap: RaceControl

## Completed Milestones

<details>
<summary>v1.0 RaceControl HUD & Safety — 5 phases, 15 plans (Shipped 2026-03-13)</summary>

See [milestones/v1.0-ROADMAP.md](milestones/v1.0-ROADMAP.md) for full phase details and plan breakdown.

Phases: State Wiring & Config Hardening → Watchdog Hardening → WebSocket Resilience → Deployment Pipeline Hardening → Blanking Screen Protocol

</details>

<details>
<summary>v2.0 Kiosk URL Reliability — 6 phases, 12 plans (Shipped 2026-03-14)</summary>

Phases: Diagnosis → Server-Side Pinning → Pod Lock Screen Hardening → Edge Browser Hardening → Staff Dashboard Controls → Customer Experience Polish

</details>

<details>
<summary>v3.0 Leaderboards, Telemetry & Competitive — Phases 12–13.1 complete, 14–15 paused (2026-03-15)</summary>

Phases complete: Data Foundation → Leaderboard Core → Pod Fleet Reliability (inserted)
Phases paused: Events and Championships (Phase 14), Telemetry and Driver Rating (Phase 15) — deferred until v4.0 completes.

</details>

<details>
<summary>v4.0 Pod Fleet Self-Healing — Phases 16–22 (Shipped 2026-03-16)</summary>

Phases: Firewall Auto-Config → WebSocket Exec → Startup Self-Healing → Watchdog Service → Deploy Resilience → Fleet Health Dashboard → Pod 6/7/8 Recovery and Remote Restart Reliability

</details>

<details>
<summary>v4.5 AC Launch Reliability — Phases 28–32 (Shipped 2026-03-16)</summary>

Phases: Billing-Game Lifecycle → Game Crash Recovery → Launch Resilience → Multiplayer Server Lifecycle → Synchronized Group Play

Key: billing↔game lifecycle wired end-to-end; CM fallback diagnostics; acServer.exe auto-start/stop on booking/billing; kiosk self-serve multiplayer with per-pod PINs; coordinated group launch + continuous race mode + join failure recovery.

</details>

## Current Milestone

### v5.0 RC Bot Expansion (Phases 23–26)

**Milestone Goal:** Expand the AI auto-fix bot with deterministic pattern-match rules for every failure class — pod crashes, billing edges, USB hardware, game launch failures, telemetry gaps, multiplayer issues, kiosk PIN problems, and lap time filtering. Staff only intervene for hardware replacement and physical reboots.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 23: Protocol Contract + Concurrency Safety** - Shared failure taxonomy and concurrency guard land in rc-common before any bot detection code exists — cross-crate compile dependency makes this non-negotiable first (completed 2026-03-16)
- [ ] **Phase 24: Crash, Hang, Launch + USB Bot Patterns** - failure_monitor.rs detects game freeze, launch timeout, and USB disconnect on the pod; ai_debugger.rs gains 6 new fix arms including FFB zero-force on crash
- [ ] **Phase 25: Billing Guard + Server Bot Coordinator** - billing_guard.rs detects stuck sessions and idle drift on the pod; bot_coordinator.rs on racecontrol routes anomalies to recovery and fences the cloud sync wallet race
- [ ] **Phase 26: Lap Filter, PIN Security, Telemetry + Multiplayer** - lap_filter.rs wires game-reported validity into persist_lap; PIN counters separate customer from staff; telemetry gap and multiplayer desync alert via bot_coordinator

## Phase Details

### Phase 23: Protocol Contract + Concurrency Safety
**Goal**: The shared failure taxonomy and concurrency guard exist in rc-common before any bot detection code is written — PodFailureReason enum, 5 new AgentMessage variants, and is_pod_in_recovery() utility compile cleanly in both consuming crates
**Depends on**: Phase 22
**Requirements**: PROTO-01, PROTO-02, PROTO-03
**Success Criteria** (what must be TRUE):
  1. rc-common compiles with PodFailureReason enum covering all 9 bot failure classes (crash, hang, launch, USB, billing, telemetry, multiplayer, PIN, lap) — verified by `cargo test -p rc-common` passing
  2. Both rc-agent and racecontrol compile after handling the 5 new AgentMessage variants (HardwareFailure, TelemetryGap, BillingAnomaly, LapFlagged, MultiplayerFailure) — no unhandled variant warnings in match arms
  3. Calling is_pod_in_recovery() in a test with an active recovery state returns true and blocks a second bot task from acting — confirmed by unit test in racecontrol
  4. All 47 existing tests remain green after the rc-common additions — no regressions from enum extension
**Plans**: 2 plans

Plans:
- [ ] 23-01-PLAN.md — PodFailureReason enum (PROTO-01) + 5 AgentMessage variants + ws stub arms (PROTO-02)
- [ ] 23-02-PLAN.md — is_pod_in_recovery() predicate + 4 unit tests in pod_healer.rs (PROTO-03)

### Phase 24: Crash, Hang, Launch + USB Bot Patterns
**Goal**: The bot autonomously handles game freeze, launch timeout, and USB wheelbase disconnect on any pod — staff no longer walk to the pod when a game hangs or a wheelbase drops mid-session
**Depends on**: Phase 23
**Requirements**: CRASH-01, CRASH-02, CRASH-03, UI-01, USB-01
**Success Criteria** (what must be TRUE):
  1. When a game process produces no UDP packets for 30 seconds AND IsHungAppWindow returns true, the bot kills the game and relaunches it without any staff action — verified on Pod 8 by blocking UDP and checking rc-agent logs
  2. When Content Manager is still running 90 seconds after a launch command, the bot kills Content Manager and retries the launch — the pod returns to a playable state without staff intervention
  3. When the Conspit Ares wheelbase is physically unplugged and replugged during a session, the bot detects the VID:0x1209 PID:0xFFB0 device reappearing within 10 seconds and restarts the FFB controller
  4. Any game kill triggered by the bot sends CMD_ESTOP (FFB zero-force) to the wheelbase before the process kill executes — confirmed by log ordering showing FFB zero before game kill in every test run
  5. Windows error dialogs (WerFault, crash reporters) are suppressed by the bot before any process kill — customers never see a system error dialog during recovery
**Plans**: 4 plans

Plans:
- [ ] 24-01-PLAN.md — Wave 0: PodStateSnapshot Default derive + 3 new fields + 10 RED test stubs (all requirements)
- [ ] 24-02-PLAN.md — Wave 1a: fix_frozen_game, fix_launch_timeout, fix_usb_reconnect + 2 new try_auto_fix arms (CRASH-01, CRASH-02, CRASH-03, UI-01)
- [ ] 24-03-PLAN.md — Wave 1b: failure_monitor.rs new file with FailureMonitorState, spawn(), detection logic (CRASH-01, CRASH-02, USB-01)
- [ ] 24-04-PLAN.md — Wave 2: main.rs wiring — mod declaration, watch channel, 8 state update sites, PodStateSnapshot new fields (all requirements)

### Phase 25: Billing Guard + Server Bot Coordinator
**Goal**: The bot detects and recovers from stuck billing sessions and idle drift without risking wallet corruption — bot_coordinator.rs on racecontrol routes anomalies through the correct StopSession sequence and fences the cloud sync race
**Depends on**: Phase 24
**Requirements**: BILL-01, BILL-02, BILL-03, BILL-04, BOT-01
**Success Criteria** (what must be TRUE):
  1. billing.rs has a characterization test suite covering start_session, end_session, idle detection, and sync paths — all tests pass before any billing bot code is written (BILL-01 prerequisite gate)
  2. When billing is active and the game process has exited for more than 60 seconds, the bot triggers end_session() via the correct StopSession → SessionUpdate::Finished sequence — the billing timer stops and the lock screen appears within 5 seconds
  3. When billing is active and DrivingState is inactive for more than 5 minutes, the bot sends a staff alert rather than auto-ending the session — staff receive the alert and can choose to act
  4. A bot-triggered session end waits for cloud sync acknowledgment before completing teardown — verified by confirming no wallet balance discrepancy after an artificially induced stuck session recovery
  5. bot_coordinator.rs on racecontrol receives BillingAnomaly, TelemetryGap, and HardwareFailure messages and routes each to the correct handler — confirmed by integration test sending each variant and asserting the handler fires
**Plans**: TBD

### Phase 26: Lap Filter, PIN Security, Telemetry + Multiplayer
**Goal**: Invalid laps are caught at capture time and never reach the leaderboard, PIN failures cannot lock out staff, and telemetry gaps and multiplayer disconnects trigger staff alerts through the coordinator
**Depends on**: Phase 25
**Requirements**: LAP-01, LAP-02, LAP-03, PIN-01, PIN-02, TELEM-01, MULTI-01
**Success Criteria** (what must be TRUE):
  1. When AC or F1 25 marks a lap as invalid (track cut, collision), the isValidLap flag from the sim adapter is wired into persist_lap and the lap is stored with valid=false — it does not appear on the public leaderboard
  2. A per-track minimum lap time is configurable in the track catalog (verified with Monza, Silverstone, Spa) — a lap below the minimum floor is flagged with review_required=true regardless of game validity signal
  3. Customer PIN failure attempts and staff PIN failure attempts are tracked in separate counters — exhausting customer PIN attempts does not lock out the staff PIN path
  4. When UDP telemetry is silent for more than 60 seconds during an active billing session (game state is Live), staff receive an email alert — no alert fires during menu navigation or idle state
  5. When an AC multiplayer server disconnect is detected mid-race, the bot triggers lock screen → end billing → log event in that order — the pod ends up in a clean idle state, not a stuck billing limbo
**Plans**: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 23 → 24 → 25 → 26

Note: Phase 23 (Protocol) is non-negotiable first — rc-common compiles before both consuming crates and any new enum variant breaks them until handled. Phase 24 (Crash/USB patterns) can be validated on Pod 8 canary before the server coordinator exists. Phase 25 (Billing Guard) requires the concurrency guard from Phase 23 and the detection foundation from Phase 24; billing.rs characterization tests must pass before any billing bot code is written. Phase 26 (Lap/PIN/Telemetry/Multiplayer) depends on bot_coordinator.rs from Phase 25 being in place for alert routing.

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. State Wiring & Config Hardening | v1.0 | 3/3 | Complete | 2026-03-13 |
| 2. Watchdog Hardening | v1.0 | 3/3 | Complete | 2026-03-13 |
| 3. WebSocket Resilience | v1.0 | 3/3 | Complete | 2026-03-13 |
| 4. Deployment Pipeline Hardening | v1.0 | 3/3 | Complete | 2026-03-13 |
| 5. Blanking Screen Protocol | v1.0 | 3/3 | Complete | 2026-03-13 |
| 6. Diagnosis | v2.0 | 2/2 | Complete | 2026-03-13 |
| 7. Server-Side Pinning | v2.0 | 2/2 | Complete | 2026-03-14 |
| 8. Pod Lock Screen Hardening | v2.0 | 3/3 | Complete | 2026-03-14 |
| 9. Edge Browser Hardening | v2.0 | 1/1 | Complete | 2026-03-14 |
| 10. Staff Dashboard Controls | v2.0 | 2/2 | Complete | 2026-03-14 |
| 11. Customer Experience Polish | v2.0 | 2/2 | Complete | 2026-03-14 |
| 12. Data Foundation | v3.0 | 2/2 | Complete | 2026-03-14 |
| 13. Leaderboard Core | v3.0 | 5/5 | Complete | 2026-03-15 |
| 13.1. Pod Fleet Reliability | v3.0 | 3/3 | Complete | 2026-03-15 |
| 14. Events and Championships | v3.0 | 0/? | Deferred | - |
| 15. Telemetry and Driver Rating | v3.0 | 0/? | Deferred | - |
| 16. Firewall Auto-Config | v4.0 | 1/1 | Complete | 2026-03-15 |
| 17. WebSocket Exec | v4.0 | 3/3 | Complete | 2026-03-15 |
| 18. Startup Self-Healing | v4.0 | 2/2 | Complete | 2026-03-15 |
| 19. Watchdog Service | v4.0 | 2/2 | Complete | 2026-03-15 |
| 20. Deploy Resilience | v4.0 | 2/2 | Complete | 2026-03-15 |
| 21. Fleet Health Dashboard | v4.0 | 2/2 | Complete | 2026-03-15 |
| 22. Pod 6/7/8 Recovery + Remote Restart Reliability | v4.0 | 2/2 | Complete | 2026-03-16 |
| 28. Billing-Game Lifecycle | v4.5 | 2/2 | Complete | 2026-03-16 |
| 29. Game Crash Recovery | v4.5 | 2/2 | Complete | 2026-03-16 |
| 30. Launch Resilience | v4.5 | 2/2 | Complete | 2026-03-16 |
| 31. Multiplayer Server Lifecycle | v4.5 | 2/2 | Complete | 2026-03-16 |
| 32. Synchronized Group Play | v4.5 | 2/2 | Complete | 2026-03-16 |
| 23. Protocol Contract + Concurrency Safety | v5.0 | 2/2 | Complete | 2026-03-16 |
| 24. Crash, Hang, Launch + USB Bot Patterns | v5.0 | 3/4 | In Progress | - |
| 25. Billing Guard + Server Bot Coordinator | v5.0 | 0/? | Not started | - |
| 26. Lap Filter, PIN Security, Telemetry + Multiplayer | v5.0 | 0/? | Not started | - |

### Phase 27: Tailscale Mesh + Internet Fallback

**Goal:** All 8 pods, server, and Bono's VPS join a Tailscale mesh network — installed as a Windows Service via WinRM, cloud_sync routes through Tailscale IP, and the server pushes telemetry/game state/pod health events to Bono in real time with a bidirectional command relay for PWA-triggered game launches
**Requirements**: TS-01, TS-02, TS-03, TS-04, TS-05, TS-06, TS-DEPLOY
**Depends on:** Phase 26
**Plans:** 1/5 plans executed

Plans:
- [ ] 27-01-PLAN.md — Wave 1 (TDD): BonoConfig in config.rs + bono_relay.rs skeleton with 3 RED test stubs (TS-01, TS-02, TS-03, TS-04)
- [ ] 27-02-PLAN.md — Wave 2: Full bono_relay.rs implementation — spawn loop, push_event, handle_command, build_relay_router; AppState bono_event_tx channel (TS-02, TS-03, TS-04)
- [ ] 27-03-PLAN.md — Wave 3: main.rs wiring — bono_relay::spawn() + second Axum listener on Tailscale IP:8099 (TS-02, TS-03, TS-06)
- [ ] 27-04-PLAN.md — Wave 2 (parallel): scripts/deploy-tailscale.ps1 — WinRM fleet deploy script, canary Pod 8 first (TS-DEPLOY)
- [ ] 27-05-PLAN.md — Wave 4: racecontrol.toml [bono] section + build + deploy + human verify Pod 8 Tailscale IP + relay 401 auth (TS-05, TS-06, TS-DEPLOY)
