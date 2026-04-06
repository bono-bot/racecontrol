# Racing Point — Data Flow Diagrams

Key system flows documented for debugging. Follow these traces to find where data breaks.

---

## 1. Customer Walk-In Flow

```
Customer arrives
    |
    v
Staff assigns customer to pod
  POST /api/v1/auth/assign { driver_id, pod_id }
    |
    v
Server creates auth_token → WS: ShowPinLockScreen { token_id, driver_name }
    |
    v
Pod rc-agent → lock_screen shows PIN entry on Edge :18923
    |
    v
Customer enters PIN on kiosk
  POST /api/v1/auth/validate-pin { pin }
    |
    v
Server validates → creates billing session
  POST /api/v1/billing/start { pod_id, driver_id, pricing_tier_id }
    |
    v
BillingFSM: Pending → WaitingForGame → Active
    |
    v
Server → WS: BillingStarted { allocated_seconds, session_token }
    |
    v
rc-agent → lock_screen shows timer + game selection
    |
    v
Staff/Customer selects game
  POST /api/v1/games/launch { pod_id, sim_type, launch_args }
    |
    v
Server → WS: LaunchGame { sim_type, launch_args }
    |
    v
rc-agent → ac_launcher → Content Manager → generates ini files → spawns game
    |
    v
rc-agent → WS: GameStateUpdate(Launching → Running)
    |
    v
Telemetry flows: Shared Memory / UDP → rc-agent → WS: Telemetry/LapCompleted
    |
    v
Session ends (timer, manual stop, or inactivity 10min)
  POST /api/v1/billing/{id}/stop
    |
    v
BillingFSM: Active → Completed
Server calculates charges, updates wallet
    |
    v
rc-agent → lock_screen shows session summary
    → then returns to PIN entry (BetweenSessions)
```

### Where Data Can Break

| Step | Failure Mode | Debug |
|------|-------------|-------|
| Auth assign | Token not reaching pod | Check WS connected in fleet/health |
| PIN validation | 401 / rate limited | Check auth rate limiter (5/min per IP) |
| Billing start | FSM rejects transition | Check billing_fsm logs for invalid state |
| Game launch | WS message lost | GameTracker timeout (120s AC) catches this |
| Telemetry | Shared memory stale | `verify_shm_alive()` guard in sim adapters |
| Session end | Double-end race condition | CAS protection in `authoritative_end_session()` |
| Refund calc | Overwritten DB value | F-05 fix (5d1ea000): read before UPDATE |

---

## 2. Game Launch Flow (Detailed)

```
API: POST /games/launch { pod_id, sim_type, launch_args }
    |
    v
game_launcher.rs:252 — validate args, check billing gate
    |
    v
Create GameTracker (state: Launching)
Register ACK waiter BEFORE send (WSCMD-01)
    |
    v
WS: CoreToAgentMessage::LaunchGame → agent_senders[pod_id]
    |
    v
[Pod rc-agent receives]
    |
    v
ac_launcher::launch_via_content_manager()
  → Generate race.ini (AI_LEVEL, CARS, track, weather)
  → Generate assists.ini (ABS, TC, auto_shifter)
  → Spawn Content Manager process
    |
    v
game_process::GameProcess monitors PID + shared memory
    |
    v
rc-agent → WS: GameStateUpdate(Launching)    [ACK]
    |
    v
launch_verifier checks window exists (process alive + responding)
    |
    v
rc-agent → WS: GameStateUpdate(Running)
    |
    v
[Server: GameTracker Launching → Running]
    |
    v
check_game_health() polls every ~5s:
  - Timeout: 120s (AC), 90s (others), dynamic from history, 180s hard cap
  - On timeout: LaunchTimedOut → agent, GameTracker → Error
    |
    v
Dashboard: DashboardEvent::GameStateChanged broadcast
```

### Timeout Values

| Game | Default Timeout | Dynamic Adjusted | Hard Cap |
|------|----------------|-----------------|----------|
| Assetto Corsa | 120s | From historical data | 180s |
| AC Evo | 120s | From historical data | 180s |
| AC Rally | 120s | From historical data | 180s |
| F1 25 | 90s | From historical data | 180s |
| iRacing | 90s | From historical data | 180s |
| Le Mans Ultimate | 90s | From historical data | 180s |

---

## 3. Billing FSM State Machine

```
                    ┌──────────┐
                    │ Pending  │
                    └────┬─────┘
                         │ start_billing()
                    ┌────▼─────────────┐
                    │ WaitingForGame   │
                    └────┬─────────────┘
                         │ game_launched()
                    ┌────▼─────┐
            ┌───────│  Active  │───────┐
            │       └──┬───┬───┘       │
            │          │   │           │
    ┌───────▼───┐  ┌───▼───▼──┐  ┌────▼──────────┐
    │PausedGame │  │PausedDis │  │PausedManual    │
    │Pause      │  │connect   │  │                │
    └───────┬───┘  └───┬──────┘  └────┬───────────┘
            │          │              │
            │  ┌───────▼────────┐     │
            │  │PausedCrash     │     │
            │  │Recovery        │     │
            │  └───────┬────────┘     │
            │          │              │
            ▼          ▼              ▼
    ┌───────────────────────────────────────┐
    │  Completed | EndedEarly | Cancelled   │
    │  CancelledNoPlayable                  │
    └───────────────────────────────────────┘
```

**Transitions (24 total):**
- Active → any Paused state (game pause, disconnect, manual, crash)
- Any Paused → Active (resume)
- Active or any Paused → Completed/EndedEarly/Cancelled
- WaitingForGame → CancelledNoPlayable (no pods available)
- **CAS protection:** `authoritative_end_session()` uses Compare-And-Swap to prevent double-end

---

## 4. Cloud Sync Cycle

```
                    VENUE (Server .23)                          CLOUD (Bono VPS)
                    ═══════════════                          ══════════════════

                    racecontrol.exe                          racecontrol.exe
                    SQLite (WAL)                             SQLite (WAL)
                    LOCAL AUTHORITY:                          CLOUD AUTHORITY:
                    billing, laps, game state                drivers, pricing, catalog

                         │                                        │
                         │    ┌─────────────────────┐             │
                         ├───►│ Relay Mode (2s)     │◄────────────┤
                         │    │ comms-link WS tunnel │             │
                         │    │ Push only (each side)│             │
                         │    └─────────────────────┘             │
                         │                                        │
                         │    ┌─────────────────────┐             │
                         ├───►│ HTTP Fallback (30s) │◄────────────┤
                         │    │ Direct when relay    │             │
                         │    │ down. Circuit breaker│             │
                         │    │ (5 fails → 60s open) │             │
                         │    └─────────────────────┘             │
                         │                                        │
                    ┌────▼──────┐                          ┌──────▼────┐
                    │ Push:     │                          │ Push:     │
                    │ billing   │ ─────────────────────►   │ drivers   │
                    │ laps      │                          │ pricing   │
                    │ sessions  │   ◄─────────────────────  │ wallets   │
                    │ games     │                          │ catalog   │
                    └───────────┘                          └───────────┘

Anti-loop: _push timestamp prevents re-pushing received data
Hysteresis: 3 failures → down, 2 successes → up
Backoff: 5s → 10s → 20s → ... → 300s cap
```

### 15 Synced Tables

| Table | Authority | Direction |
|-------|-----------|-----------|
| drivers | Cloud | Cloud → Venue |
| wallets | Cloud | Cloud → Venue |
| pricing_tiers | Cloud | Cloud → Venue |
| pricing_rules | Cloud | Cloud → Venue |
| billing_rates | Cloud | Cloud → Venue |
| kiosk_experiences | Cloud | Cloud → Venue |
| kiosk_settings | Cloud | Cloud → Venue |
| auth_tokens | Venue | Venue → Cloud |
| reservations | Bidirectional | Both → Both |
| debit_intents | Venue | Venue → Cloud |
| staff_members | Cloud | Cloud → Venue |
| driver_ratings | Cloud | Cloud → Venue |
| fleet_solutions | Venue | Venue → Cloud |
| model_evaluations | Venue | Venue → Cloud |
| metrics_rollups | Venue | Venue → Cloud |

---

## 5. WebSocket Message Flow

```
                    Pod rc-agent                Server                  Dashboard
                    ════════════                ══════                  ═════════

                         │                         │                       │
    [Boot]               │                         │                       │
                         │──Register(PodInfo)──►   │                       │
                         │                         │──PodUpdate────────►   │
                         │   ◄──Registered────     │                       │
                         │   ◄──FlagSync──────     │                       │
                         │   ◄──BillingStarted     │  (if session active)  │
                         │                         │                       │
    [Game Launch]        │                         │                       │
                         │   ◄──LaunchGame────     │                       │
                         │                         │                       │
                         │──GameStateUpdate───►    │                       │
                         │  (Launching)            │──GameStateChanged──►  │
                         │                         │                       │
                         │──GameStateUpdate───►    │                       │
                         │  (Running)              │──GameStateChanged──►  │
                         │                         │                       │
    [During Game]        │                         │                       │
                         │──Telemetry─────────►    │──Telemetry────────►  │
                         │──LapCompleted──────►    │──LeaderboardUpdate─► │
                         │──AssistState───────►    │                       │
                         │                         │                       │
    [Billing Tick]       │                         │                       │
                         │   ◄──BillingTick───     │──BillingTick──────►  │
                         │  (remaining_secs,       │                       │
                         │   cost, rate, paused)   │                       │
                         │                         │                       │
    [Session End]        │                         │                       │
                         │   ◄──BillingStopped     │──BillingChanged───►  │
                         │   ◄──SessionEnded──     │                       │
                         │                         │                       │
    [Crash]              │                         │                       │
                         │──GameCrashed───────►    │──GameStateChanged──►  │
                         │  (billing_active flag)  │                       │
```

---

## 6. Self-Healing Decision Tree (5-Tier)

```
    [Problem Detected]
         │
    ┌────▼─────────────────────┐
    │ Tier 0: Audit KB         │  Check audit_known_issues table
    │ (Server, instant)        │  POST /mesh/audit-seed populates
    └────┬──────────┬──────────┘
         │ Found    │ Not found
         │          │
    ┌────▼────┐     │
    │Escalate │     │
    │with msg │     │
    └─────────┘     │
                    │
    ┌───────────────▼──────────┐
    │ Tier 1: Deterministic    │  Kill orphans, clear temp, fix config
    │ (Pod-local, no AI)       │  Port checks, file existence, process count
    │ rc-agent/self_heal.rs    │  Always safe, always fast
    └────┬──────────┬──────────┘
         │ Fixed    │ Not fixed
         │          │
    ┌────▼────┐     │
    │  Done   │     │
    └─────────┘     │
                    │
    ┌───────────────▼──────────┐
    │ Tier 2: Knowledge Base   │  SQLite KB: past solutions by symptom
    │ (Pod-local SQLite)       │  Reputation scoring, confidence threshold
    │ rc-agent/knowledge_base  │  Promotes working fixes, demotes failed
    └────┬──────────┬──────────┘
         │ Found    │ Not found
         │          │
    ┌────▼────┐     │
    │ Apply   │     │
    │ & Log   │     │
    └─────────┘     │
                    │
    ┌───────────────▼──────────┐
    │ Tier 3: Local Ollama     │  qwen2.5:3b on James .27:11434
    │ (James workstation)      │  Crash diagnostics, CLOSE_WAIT, network
    │ rc-agent/ai_debugger     │  10s timeout → fall to Tier 4
    └────┬──────────┬──────────┘
         │ Fixed    │ Not fixed
         │          │
    ┌────▼────┐     │
    │  Done   │     │
    └─────────┘     │
                    │
    ┌───────────────▼──────────┐
    │ Tier 4: Cloud AI         │  OpenRouter MMA (budget: $0.05/day)
    │ (OpenRouter API)         │  Status: MOSTLY STUB in rc-agent
    │ rc-agent/openrouter      │  rc-watchdog has working integration
    └────┬──────────┬──────────┘
         │ Fixed    │ Not fixed
         │          │
    ┌────▼────┐     │
    │  Done   │     │
    └─────────┘     │
                    │
    ┌───────────────▼──────────┐
    │ Tier 5: Escalation       │  WhatsApp + Bono relay + email
    │ (Human intervention)     │  Status: MOSTLY STUB in rc-agent
    │ Staff notification       │  rc-watchdog has working alerts
    └──────────────────────────┘
```

### Mesh Intelligence Data Flow

```
    Pod discovers problem
         │
    ┌────▼─────────────────────┐
    │ Tier 1-3 local fix       │
    └────┬─────────────────────┘
         │ Solution found
         │
    ┌────▼─────────────────────┐
    │ mesh_gossip → Server     │  AgentMessage containing solution
    │ (WS: /ws/ai-channel)    │  Gossip protocol for distributed learning
    └────┬─────────────────────┘
         │
    ┌────▼─────────────────────┐
    │ Server aggregates        │  mesh_handler.rs
    │ fleet_solutions table    │  Consensus from multiple pods
    └────┬─────────────────────┘
         │
    ┌────▼─────────────────────┐
    │ Cloud sync → Bono VPS    │  fleet_solutions synced venue → cloud
    └────┬─────────────────────┘
         │
    ┌────▼─────────────────────┐
    │ Other pods learn         │  Server broadcasts verified solutions
    │ (Tier 2+ only, gated)   │  Tier 1 deterministic = pod-local only
    │ confidence >= 0.8        │  Staff fixes = Tier 2+ gated broadcast
    └──────────────────────────┘
```

### Mesh Intelligence API Endpoints

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/mesh/solutions` | All ML-discovered solutions |
| GET | `/mesh/solutions/search` | Search by symptom |
| GET | `/mesh/solutions/{id}` | Solution detail + evidence |
| GET | `/mesh/incidents` | All incidents discovered |
| GET | `/mesh/stats` | MI statistics (confidence, accuracy) |
| POST | `/mesh/solutions/{id}/promote` | Make solution auto-apply |
| POST | `/mesh/solutions/{id}/retire` | Retire solution |
| POST | `/mesh/audit-seed` | Seed audit findings into KB |

### Mesh Types (rc-common/mesh_types.rs)

- `MeshSolution` — Solution discovered via diagnosis
- `DiagnosisTier` — Which tier found the solution (1-5)
- `SolutionStatus` — Active / Retired / Promoted
- `FleetEvent` — Event kind for fleet-wide incidents

---

## 7. Lock Screen State Machine

```
    [rc-agent Boot]
         │
    ┌────▼─────────┐
    │   Hidden      │  (No Edge browser, no lock screen)
    └────┬──────────┘
         │ WS: BlankScreen
         │
    ┌────▼─────────┐
    │ScreenBlanked │  Edge launches fullscreen black
    └────┬──────────┘
         │ WS: ShowPinLockScreen
         │
    ┌────▼─────────┐
    │  PinEntry    │  PIN pad displayed on :18923
    └────┬──────────┘
         │ PIN validated
         │
    ┌────▼─────────┐
    │ActiveSession │  Timer + game selection
    │              │  BillingTick updates remaining_secs
    └────┬──────────┘
         │ Session ends
         │
    ┌────▼──────────────┐
    │ SessionSummary    │  Score, lap time, feedback
    └────┬──────────────┘
         │ Timeout / dismiss
         │
    ┌────▼──────────────┐
    │ BetweenSessions   │  Returns to PIN entry
    └───────────────────┘
```

**Edge Browser Management:**
- Launched in kiosk mode: `--app=http://127.0.0.1:18923 --kiosk --start-fullscreen`
- Session data cleaned (SNSS files) to prevent window persistence
- Virtual screen bounds via GetSystemMetrics (handles NVIDIA Surround)

---

## 8. Pod Recovery Flow

```
    [Pod rc-agent crashes]
         │
    ┌────▼──────────────────┐
    │ rc-sentry detects     │  Health poll fails 3x (5s interval)
    │ :8091 on pod          │  Extracts: panic msg, exit code, last phase
    └────┬──────────────────┘
         │
    ┌────▼──────────────────┐
    │ Session 1 restart     │  WTSQueryUserToken + CreateProcessAsUser
    │ (NOT schtasks!)       │  start-rcagent.bat in interactive desktop
    └────┬──────────────────┘
         │ Verify: poll health 3x @ 5s
         │
         ├──── Success ──────► Done (recovery-pod.jsonl logged)
         │
         ├──── Fail (2nd attempt) ──► WhatsApp alert to Bono
         │
         ├──── Fail (3rd) ──────────► MAINTENANCE_MODE sentinel written
         │                            (auto-clears after 30min via rc-watchdog)
         │
    ┌────▼──────────────────┐
    │ Server pod_monitor    │  Detects ws_connected: false
    │ fleet_health.rs       │  Crash loop: >3 startups in 5min with uptime <30s
    └────┬──────────────────┘
         │
         ├──── crash_loop: true flag in fleet health
         ├──── WhatsApp alert sent (ws/mod.rs)
         └──── Graduated recovery: WoL → AI Escalation → Alert Staff
```

---

## 9. Deploy Flow

```
    [Developer commits]
         │
    ┌────▼──────────────────┐
    │ touch build.rs        │  Force GIT_HASH refresh
    │ cargo build --release │
    └────┬──────────────────┘
         │
    ┌────▼──────────────────┐
    │ Security gate         │  gate-check.sh (Suite 0, 4)
    │ Manifest check        │  release-manifest.toml (SHA256, git_commit)
    └────┬──────────────────┘
         │
    ┌────▼──────────────────┐
    │ Pod 8 canary          │  deploy-pod.sh → Pod 8 first
    │ Verify build_id       │  curl :8090/health → build_id matches
    │ Test specific fix     │
    └────┬──────────────────┘
         │ Pass
         │
    ┌────▼──────────────────┐
    │ Fleet deploy          │  Parallel Pods 1-7
    │ (via rc-sentry :8091) │  Hash-based naming: rc-agent-<hash>.exe
    │                       │  Previous preserved: rc-agent-prev.exe (72hr)
    └────┬──────────────────┘
         │
    ┌────▼──────────────────┐
    │ Server deploy         │  deploy-server.sh v3.0 (8-step, auto-rollback)
    │ (deploy-staging/)     │  Disable watchdog → download → kill → swap → start → verify
    └────┬──────────────────┘
         │
    ┌────▼──────────────────┐
    │ Frontend rebuild      │  MANDATORY after server deploy
    │ kiosk + web + admin   │  Stale JS = WS churn (connect/disconnect loop)
    └────┬──────────────────┘
         │
    ┌────▼──────────────────┐
    │ Cloud deploy          │  git push → Bono relay git_pull → rebuild
    │ (DEPLOY PARITY)       │  Verify health on BOTH venue + cloud
    └──────────────────────┘
```
