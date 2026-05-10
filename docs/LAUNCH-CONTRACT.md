# Game Launch Contract

> **Status:** Draft v2 — 2026-04-28. Authored by Claude/Bono on `/root/racecontrol/` (cloud checkout) at user request after observing 7+ launch-related PACTs filed in 48 hours (091, 092, 093, 095, 097, 103, 104) and HALO R.4 firing (4/4 crashes had no `launch_timeline_spans` entry). v2 adds §10 (multiplayer); v1 was lost to a cron sync between turns and is being re-written here at `/tmp/` to survive sync.
>
> This document is **descriptive first, prescriptive second**. §1–§4 describe what the code currently does. §5–§7 surface contradictions and recommend changes. §10 covers multiplayer with the same shape. The goal is to make the launch path's behavior auditable so future PACTs target named gaps instead of patching symptoms.
>
> **Source-of-truth precedence** when this document and code disagree: the code wins, this document gets updated. When this document and ARCHITECTURE.md §17 disagree, this document wins (§17 is stale — see §0.2 below).
>
> **See also (prior art on the same path):**
> - `docs/GAME-LAUNCH-E2E-MAP.md` — single-page debugging trace, file:line refs, scope-limited to SP/AC. Different artifact: this document is a *specification*; that document is a *trace for debugging*. Both should exist.
> - `docs/ARCHITECTURE.md` §17 — stale; should be reduced to a one-paragraph pointer to this contract.
> - `crates/rc-common/src/launch_contract.rs` — the message-level contract types; this document is the lifecycle wrapper around them.

---

## §0. Naming Conflicts and Drift to Resolve First

The launch path has accumulated three forms of doc/code drift that must be acknowledged before the contract is readable.

### §0.1 Two enums named `LaunchState`

| Where | Values | Purpose | Owner |
|-------|--------|---------|-------|
| `rc-common/protocol.rs:93` | `LaunchStarted, AiAnalysisRequested, IssueBeingFixed, IssueFixed, NeedsManualIntervention` | UI status-card lifecycle (Phase 368) | Server, dashboard |
| `ARCHITECTURE.md §17.2` (documented; agent-internal) | `Idle, WaitingForLive { launched_at, attempt }, Live` | rc-agent's per-launch state during AC SP launch | Pod agent (rc-agent) |

This document refers to them as **`LaunchState_UI`** and **`LaunchState_Agent`**. **Action item:** rename one. Tracked as gap **G-NAMING**.

### §0.2 ARCHITECTURE.md §17 is stale relative to current code

§17 was written before Phase 368 introduced `LaunchState_UI` and `LaunchStatusCard`. It does not mention `launch_timeline_spans`, `launch_events`, `recovery_events`, `LaunchStateMachine`, the `LaunchOrigin` enum, or the 5-state UI card lifecycle. **Action item:** replace §17 with a pointer to this document. Tracked as **G-DOCS**.

### §0.3 Four overlapping timeout regimes

| Source | Scope | Value | What fires |
|--------|-------|-------|-----------|
| ARCHITECTURE.md §17.2 | AC launch | 60s | "Cancel after timeout" |
| ARCHITECTURE.md §17.2 | Generic sim | 180s | "Cancel after timeout" |
| `game_launcher_support.rs` (STATE-01) | Server tracker | 30s | Auto-transitions GameState to Error if no Loading/Error received |
| `game_launcher_support.rs` (LAUNCH-08) | Server tracker | dynamic, default AC=120s, others=90s | Combo-reliability-derived timeout |

**These coexist.** First to fire wins; others become no-ops. Tracked as **G-TIMEOUT**.

---

## §1. The Five State Machines That Participate in a Launch (SP Layer)

A single SP launch attempt traverses all five. Multiplayer adds two more — see §10.

| # | Name (this doc) | Code identity | Process | File |
|---|-----------------|---------------|---------|------|
| SM1 | **GameState** | `rc_common::types::GameState` | Server racecontrol | `rc-common/src/types.rs:496` |
| SM2 | **LaunchCardState** | `rc_common::protocol::LaunchState` | Server racecontrol | `rc-common/src/protocol.rs:93` |
| SM3 | **AgentLaunchPhase** | `LaunchState` (agent-internal) | Pod rc-agent | per ARCHITECTURE.md §17.2 |
| SM4 | **CrashRecoveryState** | `CrashRecoveryState` | Server racecontrol | per ARCHITECTURE.md §17.3 |
| SM5 | **LaunchRequestStatus** | `game_launch_requests.status` (TEXT) | Server DB | `migrate_game.rs:347` |

**Contract:** for any single launch attempt, all five must remain coherent — no two in mutually-exclusive states. Code today does not enforce coherence; gaps in §5.

### SM1 — GameState (server-side per-pod tracker)

```text
Idle ─→ Launching ─→ Loading ─→ Running ─→ Stopping ─→ Idle
                          │            │
                          └────→ Error ←────────────────┘
[InLobby is multiplayer's SM1 value — see §10]
```

| State | Entered when | Owner of write | Persisted? | Exit conditions |
|-------|-------------|---------------|------------|-----------------|
| `Idle` | Pod boot, after `Stopping` ACK, or after `Error` resolution | `GameTracker` removed (`game_launcher_state.rs:36`) | **In-memory only** | Customer launch → `Launching` |
| `Launching` | Server queues `LaunchGame` WS message | `game_launcher_ops.rs:29-290` (LIFE-04) | **In-memory only** | Agent reports process spawned → `Loading`; STATE-01 30s timeout → `Error` |
| `Loading` | Agent sends `GameStateUpdate(Loading)` | `game_launcher_state.rs:45-46` | **In-memory only** | PlayableSignal → `Running`; LAUNCH-08 timeout → `Error` |
| `Running` | Agent sends PlayableSignal | `game_launcher_state.rs:97-116` (also writes `IssueFixed` to SM2) | **In-memory + `launch_events.duration_to_playable_ms`** | Stop → `Stopping`; crash → `Error` |
| `Stopping` | Staff calls `/games/stop` | `game_launcher_ops_stop.rs:17-82` | **In-memory only** | Agent ACK → `Idle`; ACK timeout 5s → tracker stays in `Stopping` (**G-STOP-LIMBO**) |
| `Error` | Agent sends `GameStateUpdate(Error)` OR STATE-01 timeout | `game_launcher_state.rs:156-219` | **In-memory + `launch_events` + `launch_timeline_spans`** | RECOVER-02 spawns recovery |

**Contract invariant:** at most one tracker per pod_id at any time. Per-pod uniqueness enforced by map; rapid-fire calls gated only by 4-pod-wide semaphore (RESIL-06). **Gap G-CONCURRENT**.

### SM2 — LaunchCardState (server-side UI status card)

```text
LaunchStarted ─→ AiAnalysisRequested ─→ IssueBeingFixed ─→ IssueFixed [terminal]
        │                       │                  │
        ├─→ IssueFixed [happy path skip — line 134]│
        │                       │                  │
        └─→ NeedsManualIntervention [terminal] ←───┴─→ NeedsManualIntervention [terminal]
```

Validated transitions explicit at `launch_state.rs:124-144`. Backward, same-state, and out-of-order rejected.

**Contract invariant:** one card per `launch_id`. Identity owned by server-minted UUID at `game_launcher_ops.rs:88`. Auto-dismiss: `IssueFixed` after 5 min; `NeedsManualIntervention` never (staff must dismiss).

### SM3 — AgentLaunchPhase (pod-side internal)

Per ARCHITECTURE.md §17.2 only — needs re-verification against current rc-agent code.

```text
Idle ─→ WaitingForLive { launched_at, attempt: 1 } ─→ Live
                            │
                            └─→ (timeout AC=60s, generic=180s) → cancel/retry once
```

Mapping SM3↔SM1 not enforced anywhere. **Gap G-AGENT-MAPPING**.

### SM4 — CrashRecoveryState (auto-relaunch FSM)

```text
Idle ─→ PausedWaitingRelaunch { attempt: 1, timer: 30s, last_sim_type, last_launch_args } ─→ Idle (relaunch fired)
                                       │
                                       └─→ AutoEndPending (attempt 2 failed) ─→ session ends, no charge
```

Per-pod, runs in parallel with SM1. Relaunch creates new `GameTracker` (new SM1 instance), shares billing.

### SM5 — LaunchRequestStatus (DB-only, customer PWA flow)

`game_launch_requests.status`. Values `'pending'` and `'expired'`. 10-minute server-side TTL. Not synced to cloud. **Logically disconnected from SM1–SM4** — no foreign-key from `launch_id` to `game_launch_requests.id`. **Gap G-REQUEST-LINK**.

### §1.6 Multiplayer (`InLobby`)

`InLobby` is the SM1 value held while a pod is in an MP lobby. The full MP contract — lobby state machine (SM6: `MpLobbyState`), AC dedicated server lifecycle (SM7: `AcServerStatus`), multi-pod billing rules — is in **§10 below**.

---

## §2. Canonical SP Launch Lifecycle (Layered View)

Eight phases. Notation: `[component]` = process. `[SM:state]` = SM and value.

```text
P1. Request               [PWA or Kiosk]    [SM5:pending]
P2. Validate              [Server]          [SM5:pending → resolved]
                                            (Pre-flight check — see §3.)
P3. Dispatch              [Server → Agent]  [SM1:Idle → Launching] [SM2:LaunchStarted]
                                            (Server mints launch_id UUID v4.)
P4. Spawn                 [Agent]           [SM3:Idle → WaitingForLive]
                                            (pre-launch checks, write race.ini, spawn acs.exe.)
P5. Wait for live         [Agent + Server]  [SM1:Loading] [SM3:WaitingForLive]
                                            (STATE-01 30s + LAUNCH-08 90-120s in parallel.)
P6. Live                  [Agent + Server]  [SM1:Running] [SM3:Live] [SM2:IssueFixed]
                                            (PlayableSignal → billing Active.)
P7. End                   [Customer/Staff/Game] [SM1:Running → Stopping → Idle]
                                            (write launch_timeline_spans success outcome.)
P8. Recover (if crashed)  [Server SM4 + Agent retry]
                                            (classify error, write events, attempt relaunch.)
```

**Contract:** every phase has a defined trigger, owner, write target, exit condition, timeout. Implementation has all of these per phase, but they're scattered across files with no central enforcement.

---

## §3. Pre-Flight Check Specification (Proposed)

Prescriptive. Today most don't exist or fire too late. Pre-flight runs at P2 (Validate) before SM1 enters `Launching`. Fail → SM2:NeedsManualIntervention; never a black screen mid-load.

| Check | Source of truth | Fail → action | Currently exists? |
|-------|-----------------|---------------|-------------------|
| **PF-1** Pod WebSocket connected | `agent_senders` map | Reject — pod offline | Implicit (silent fail on WS send) |
| **PF-2** Pod build_id within 2 phases of server | Heartbeat metadata | Warn (don't block) | **No** |
| **PF-3** Pod free disk space ≥ 5 GB | New endpoint on agent | Reject — pod-disk-low | **No** |
| **PF-4** Pod RAM headroom ≥ 4 GB | New endpoint on agent | Reject — pod-ram-low | **No** |
| **PF-5** No active game on pod | `active_games` map | 409 Conflict | **Yes** (Phase 366) |
| **PF-6** Game binary present on pod | New endpoint on agent | Reject — game-not-installed | **No** (silently fails at spawn time) |
| **PF-7** Telemetry UDP listener alive | New endpoint on agent | Warn — telemetry-degraded | **No** |
| **PF-8** No `MAINTENANCE_MODE` sentinel | Pod's `pre_launch_checks()` | Reject — maintenance | **Yes** |
| **PF-9** No `OTA_DEPLOYING` sentinel | Pod's `pre_launch_checks()` | Reject — deploying | **Yes** |
| **PF-10** Pod not in crash-loop | Recent launch failure rate | Reject if ≥3 launches in 5 min with uptime <30s | **No** |
| **PF-11** Combo success rate ≥ 50% in last 24h | `combo_reliability` (existing) | Warn — degraded combo | **No** (data exists, unused) |
| **PF-12** AI debugger not in destructive auto-fix loop | `ai_debugger.rs` recent-action count | Reject — pod-self-healing | **No** (PACT-104) |
| **PF-13** Active billing session exists for driver | `billing_sessions` | Reject — no-session | **Yes** |
| **PF-14** Customer wallet balance ≥ tier minimum | `wallets` | Reject — insufficient-funds | **Yes** |
| **PF-15** Feature flag `game_launch` enabled | `feature_flags` | Reject — feature-disabled | **Yes** |

**6 of 15 exist (40%). Highest-impact additions: PF-1, PF-3, PF-6, PF-7, PF-12.** All can be served by a single new pod endpoint `GET /v1/agent/launch-readiness` returning a JSON readiness manifest.

---

## §4. Persistence Contract (Tables That Carry Launch State)

Five tables. Each has a defined writer, reader, and authority.

| Table | Authoritative on | Writer (file:line) | Reader (file:line) | Cloud-synced? |
|-------|------------------|--------------------|--------------------|---------------|
| `launch_timeline_spans` | Crashed/stopped/successful launches (one row per launch_id) | Agent: `agent_sync_misc.rs:84` (INSERT OR REPLACE, full timeline). Server: `game_launcher_state.rs:208` (INSERT OR IGNORE, stub on crash) + `game_launcher_ops_stop.rs:155` (stub on stop) | `api/game_state.rs:72`, `api/debug_launches.rs` | Yes |
| `launch_events` | Per-attempt audit (success or crash) | `game_launcher_state.rs:180` (crash row), `:237` (UPDATE on Running), `:432+` (RE relaunch row) | Combo reliability, dashboards | Yes |
| `recovery_events` | Crash recovery attempts | `game_launcher_state.rs:330-375` (async spawn), `metrics.rs::record_recovery_event` | `:303` (RECOVER-03 historical lookup) | Yes |
| `fleet_incidents` | Cross-pod patterns (incidents, not per-launch) | `app_health_monitor.rs`, `pod_monitor_check.rs`, `fleet_anomaly_detection.rs` | Dashboard anomaly views | Yes |
| `game_launch_requests` | PWA-side request TTL only | `api/pwa_game_request.rs` (INSERT), `billing_jobs.rs` (UPDATE expired) | `billing_jobs.rs` (TTL sweep) | **No** (local-only) |

### Contract invariants

**INV-P1**: Every `launch_id` that reaches `Running` MUST have a corresponding row in `launch_timeline_spans`. **Currently violated by HALO R.4** — 4/4 crashes had no row within ±120s.

**INV-P2**: Every `launch_id` in `launch_timeline_spans` with `outcome != 'success'` MUST have matching `launch_events` row with error_taxonomy. *Likely satisfied; not verified.*

**INV-P3**: Every Race-Engineer-triggered relaunch MUST write `launch_events.created_by_agent = 'RE'` AND `recovery_events.created_by_agent = 'RE'`. **Partial** — `launch_events` correct; `recovery_events` not (PACT-091).

**INV-P4**: Every `game_launch_requests` row resolved into a launch MUST set `resolved_at` + `resolved_by`, AND should reference the resulting `launch_id`. **Violated by PACT-097**.

**INV-P5**: For any time T, union over all pods of `(active_games[pod_id].launch_id)` ⊆ {launch_ids in launch_timeline_spans where created_at < T - 60s} ∪ {launch_ids in active_games}. *Has at least 30s window during early-crash where no row exists.*

---

## §5. Gaps / Contradictions Found (SP Layer)

| Gap ID | Description | Affected SM | Severity |
|--------|-------------|-------------|----------|
| **G-NAMING** | Two enums named `LaunchState` (UI vs agent-internal) | SM2/SM3 | Low |
| **G-DOCS** | ARCHITECTURE.md §17 stale | All | Low |
| **G-TIMEOUT** | Four overlapping timeout regimes (60s/180s/30s/90-120s) | SM1/SM3 | **Medium** |
| **G-CONCURRENT** | Per-pod concurrent launch not gated; semaphore is fleet-wide (4) | SM1 | **High** — PACT-092 |
| **G-LAUNCH-ID** | Server mints launch_id; agent also mints internally; they don't share | SM1/SM3 | Medium |
| **G-AGENT-MAPPING** | SM3 (`WaitingForLive/Live`) ↔ SM1 (`Loading/Running`) mapping not enforced | SM1/SM3 | Medium |
| **G-ERROR-RACE** | If rc-agent crashes during `Launching` (before sending `Error`), server doesn't learn until 30s STATE-01 timeout. 0–30s of every launch invisible to server. | SM1 | **High** — HALO R.4 |
| **G-TIMEOUT-EVENT** | LAUNCH-08 / STATE-01 timeouts auto-transition to `Error`, but the *fact* that it was a timeout (vs. crash) not recorded | `launch_events` | Medium |
| **G-RELAUNCH-COUNTER** | `auto_relaunch_count` per-`GameTracker`, resets when pod switches game. Cascade across games not prevented. | SM4 | **High** |
| **G-RECOVERY-ATTRIBUTION** | `recovery_events.created_by_agent` not set by RE path | INV-P3 | Low (PACT-091) |
| **G-STOP-LIMBO** | `Stopping` ACK timeout has no fallback; tracker can stay in `Stopping` indefinitely | SM1 | **High** |
| **G-CLOUD-AUTHORITY** | `game_launch_requests` is local-only; cloud has no view of pending PWA requests | SM5 | Low |
| **G-REQUEST-LINK** | `launch_id` does not back-link to `game_launch_requests.id` | SM5 | Medium (PACT-097) |
| **G-AI-DESTRUCTIVE** | AI Debugger's `kill_stale_game` / `kill_error_dialogs` operate without consulting SM1; can kill game in `Running`, can kill kiosk Edge | All | **Critical** (PACT-104) |
| **G-PRE-FLIGHT** | 9 of 15 useful pre-flight checks don't exist | All | **High** |
| **G-SPAWN-SUCCESS-PRINCIPLE** | `launch_contract.rs:7-12` declares "Launch is SIMPLE: validate → spawn → done. Launcher EXITS." Reality: Race Engineer watches ~30s, relaunches, persists timelines. Stated principle and code disagree. | All | Medium |

**Three gaps cause most customer-visible damage:** G-CONCURRENT (PACT-092), G-ERROR-RACE (HALO R.4), G-AI-DESTRUCTIVE (PACT-104).

---

## §6. Existing Open PACTs Mapped to Contract Gaps

| PACT-ID | Topic | Subsumed by gap(s) | Independent? |
|---------|-------|-------------------|--------------|
| **PACT-091** | MI watermark for launch events | G-RECOVERY-ATTRIBUTION (partial), G-LAUNCH-ID (correlation) | Partial |
| **PACT-092** | dual-crash on concurrent launch | **G-CONCURRENT** | Subsumed |
| **PACT-093** | recovery_events 33h write-silence | G-ERROR-RACE (async spawn loses on crash), INV-P3 | Subsumed |
| **PACT-095** | F1 25 EAC overlay compat | None — needs new pre-flight PF-12-extended | Independent |
| **PACT-097** | game_launch_requests empty | G-REQUEST-LINK + missing INSERT path | Subsumed |
| **PACT-103** | loading→running event-loss | G-ERROR-RACE, INV-P1 | Subsumed |
| **PACT-104** | ai_debugger destructive race | **G-AI-DESTRUCTIVE** | Subsumed |
| **HALO R.4** | 4/4 crashes have no launch_timeline_spans | **G-ERROR-RACE**, INV-P1 | Subsumed |
| **HALO V.2/V.10** | 8/8 pods rc-agent dead, 0/8 ws_connected | None — fleet-state issue upstream of any launch | Independent (blocks all of §3 PF checks) |

**Of 9 active items, 6 subsumed by named gaps, 2 partial, 1 truly independent.** Closing G-CONCURRENT, G-ERROR-RACE, G-AI-DESTRUCTIVE + adding pre-flight (G-PRE-FLIGHT) converts "scattered firefighting" into "three named pieces of work."

---

## §7. Recommended Next Concrete Steps (SP)

### Step 1 — Pick gold-standard combination (1-2 days; scope decision)

Currently launch supports F1 25 / AC / iRacing / LMU × SP/MP × 8 pods × difficulty/assists/AI/EAC ≈ ~96 combinations. We firefight all 96.

**Designate F1 25 SP on any pod** as gold-standard. ~80% of customers. Goal: zero PACT-class bugs on this combination for two weeks. Then expand. **This is a scope call**, not engineering work.

### Step 2 — Fix G-ERROR-RACE (~1 day)

rc-agent emits `LaunchTimelineReport` with full event trace **at start of launch** with empty events array, then updates it. Server uses partial reports as the timeline_span row even if launch crashes before completion. Closes HALO R.4, PACT-103, contributes to PACT-093.

### Step 3 — Add `/v1/agent/launch-readiness` + 5 pre-flight checks (~1 day)

PF-1 (already exists), PF-3 (disk), PF-6 (binary present), PF-7 (telemetry alive), PF-10 (game-launch crash-loop). Single new pod endpoint, single new server pre-flight middleware. Reject before customer pays.

### Step 4 — Fix G-CONCURRENT (~half day)

Replace fleet-wide semaphore with per-pod exclusive lock. Single-line change in `game_launcher_ops.rs:39`. Closes PACT-092.

### Step 5 — Fix G-AI-DESTRUCTIVE (~half day to 1 day)

Gate `ai_debugger.rs::kill_stale_game` and `kill_error_dialogs` on SM1 state. Don't kill `Running`. Don't kill kiosk Edge without process verification. Closes PACT-104.

### Step 6 — Update ARCHITECTURE.md §17 (~1 hour)

Replace §17.2-17.5 with paragraph linking to this document. Closes G-DOCS.

### Step 7 — Rename one of the two `LaunchState` enums (~half day)

`LaunchState_UI` → `LaunchCardState`. Closes G-NAMING.

**Total Steps 1–7: ~5 days.** Replaces ~7 in-flight PACTs.

---

## §8. Maintenance of This Document

- **Update on:** any new state added to SM1–SM7, any new persistence table, any new pre-flight check added, any §5 / §10.5 gap closed.
- **Verify on:** every milestone ship (run §4 / §10.4 invariant queries against production data; if any INV-* fails, file PACT linking to gap ID).
- **Re-derive on:** schedule yearly re-read of `game_launcher_*.rs`, `lobby.rs`, `ac_server*.rs` against this document.
- **Pair with:** ARCHITECTURE.md §17 (after Step 6), `GAME-LAUNCH-E2E-MAP.md`, `rc-common/launch_contract.rs`, `racecontrol/launch_state.rs`.

---

## §9. Out of Scope for v2

- **Cloud-side launch behavior** — venue-side only. If Bono VPS ever runs launches (single-pod arcade mode, cloud-test fixture), this contract needs a Linux-side variant.
- **rc-agent internal state machine in detail** — SM3 from ARCHITECTURE.md only; needs grep through `ac_launcher.rs` + `game_process.rs` + `event_loop.rs`.
- **POS PC role** — POS runs same binaries but doesn't launch games. Contract assumes pod_id ∈ {1..8}.
- **Telemetry stream contract** — boundary (when does launch hand off to telemetry?) not specified.
- **F1 25 multiplayer** — §10 covers AC multiplayer (only MP path with venue-managed dedicated server). F1 25 MP uses EA matchmaking, outside racecontrol's coordination layer.
- **Cross-venue multiplayer** — single-venue LAN only (REGISTER_TO_LOBBY=0 enforced).

---

## §10. Multiplayer Launch Extension

### §10.0 What v2 adds

Multiplayer in this codebase means **AC LAN multiplayer with a venue-managed dedicated server**, plus the championship/weekend orchestration on top. F1 25 / iRacing / LMU multiplayer is not coordinated by racecontrol.

### §10.1 Two More State Machines: SM6 and SM7

Total state machine count: **seven**.

| # | Name (this doc) | Code identity | Process | File |
|---|-----------------|---------------|---------|------|
| SM6 | **MpLobbyState** | `rc_common::types::LobbyPhase` | Server racecontrol (`LobbyManager`) | `lobby.rs:42-144`, `rc-common/types.rs:518` |
| SM7 | **AcServerStatus** | `rc_common::types::AcServerStatus` | Server racecontrol (`acServer.exe` lifecycle wrapper) | `ac_server.rs`, `rc-common/types.rs:616-622` |

#### §10.1.1 SM6 — MpLobbyState

```text
                          (timeout 120s while in Forming)
                           ┌──────────────────────────────┐
                           │                              ▼
[create_lobby] → Forming → AllReady → Starting → ?? → Cancelled
                           
                  Active   ← enum value, never written by LobbyManager (gap G-LOBBY-NEVER-ACTIVE)
```

| State | Entered when | Owner of write | Persisted? | Exit |
|-------|-------------|---------------|------------|------|
| `Forming` | `LobbyManager::create_lobby()` | `lobby.rs:70` | **In-memory only** (`RwLock<HashMap>`) | All pods report ready → `AllReady`; 120s elapsed → `get_timed_out_lobbies()` returns it |
| `AllReady` | `mark_pod_ready()` when `all_ready()` true | `lobby.rs:106` | **In-memory only** | `mark_starting()` → `Starting` |
| `Starting` | `mark_starting()` (caller about to send `LobbyGo`) | `lobby.rs:116` | **In-memory only** | **No transition path defined** — see G-LOBBY-NEVER-ACTIVE |
| `Active` | **Never set in `lobby.rs`.** Enum at `rc-common/types.rs:522`; no `LobbyManager` method writes it. | — | — | — |
| `Cancelled` | Caller transition after timeout (no method does this in `lobby.rs` itself) | (caller code, not yet identified) | **In-memory only** | `remove_lobby()` deletes |

**Contract invariant:** at most one lobby per `group_session_id`. Carries `assigned_pods`, `ready_pods`, `phase`, `created_at`. **No DB persistence** — server restart loses all lobby state. **G-LOBBY-NO-PERSIST**.

**Concurrency:** singleton `LobbyManager` per server, all access via `RwLock<HashMap<group_session_id, LobbyInstance>>`. Keyed by `group_session_id`, NOT per-pod.

#### §10.1.2 SM7 — AcServerStatus

```text
[start_ac_server] → Starting → Running → Stopping → Stopped
                       │                                ▲
                       └──── (init failure) → Error ────┘

Singleton — only ONE acServer instance can be in
{Starting | Running} globally (ac_server.rs:77-85 bails if another exists)
```

| State | Entered when | Persistence | Exit |
|-------|-------------|-------------|------|
| `Starting` | `start_ac_server()` spawns `acServer.exe`, allocates ports, writes config | `ac_sessions` table row | After 500ms hardcoded sleep → `Running` (**G-AC-SERVER-FAKE-READY**) |
| `Running` | 500ms after spawn (`ac_server.rs:230-239`) | `ac_sessions.status='running'` | `stop_ac_server()` → `Stopping` |
| `Stopping` | `stop_ac_server()` initiates shutdown | DB updated | `child.kill()` + `wait()` complete + results collected → `Stopped` |
| `Stopped` | Process dead, ports moved to 4-min cooldown | `ac_sessions.status='stopped'` | (terminal) |
| `Error` | Spawn or initialization failure | `ac_sessions.status='error'` | (terminal) |

**Contract invariant:** at most ONE instance in `{Starting, Running}` globally. **Implication:** if a 30-min race is live, a second group must wait ≥30 min before their race starts. **G-AC-SINGLETON**.

### §10.2 Canonical Multiplayer Launch Lifecycle

```text
M1. Book                  [Staff Kiosk OR Self-Service Kiosk]
    └─ /api/v1/multiplayer/book (staff, shared PIN) OR
       /api/v1/kiosk/multiplayer/book (self-service, unique PIN per pod)
    └─ Inserts: group_sessions, group_session_members. Wallet debit at this phase.
    └─ Sends ShowPinLockScreen to each pod.

M2. Members validate      [Pods × N — customers enter PIN]
    └─ Each pod: PIN entry → group_session_members.status='validated'

M3. Group ready trigger   [Server racecontrol]
    └─ When all members validated → start_ac_lan_for_group()
    └─ LobbyManager.create_lobby (SM6:Forming)

M4. AC server boot        [Server racecontrol → acServer.exe]
    └─ port_allocator allocates {udp_port, tcp_port, http_port}
    └─ write server_cfg.ini + entry_list.ini + extra_cfg.yml
    └─ spawn acServer.exe → SM7:Starting → SM7:Running (500ms later)

M5. Pod launch (per pod)  [Server → Agents × N]
    └─ Send LaunchGame to each pod with JSON: {game_mode:"multi", server_ip, ...}
    └─ Each pod's SM1 enters Launching → Loading → Running
    └─ Each pod's SM2 (LaunchCardState) goes through normal SP path

M6. In-lobby coordination [Pods × N + LobbyManager]
    └─ Each pod's GameState reaches InLobby (SM1)
    └─ Pod sends "ready" → mark_pod_ready() → LobbyManager
    └─ Lobby SM6: Forming → AllReady when last pod ready
    └─ Server calls mark_starting() → SM6:Starting
    └─ Server sends LobbyGo to all pods (synchronized race start)

M7. Race active           [Pods × N + acServer]
    └─ Pods in GameState::Running, sending telemetry to AC server
    └─ monitor_lobby_sync() polls acServer GET /INFO every 3s
    └─ Per-pod billing accrues independently

M8. Race end              [acServer → server orchestrates teardown]
    └─ acServer reports race complete via /INFO
    └─ collect_results() scrapes results/race_result.json before kill
    └─ stop_ac_server() → SM7: Running → Stopping → Stopped
    └─ Ports moved to 4-min cooldown
    └─ Per-pod billing ends, group_sessions.status='completed'
    └─ multiplayer_results rows inserted

M9. Recover (any phase)   [Server + billing_multiplayer]
    └─ If any pod crashes: pause_multiplayer_group() — ALL pods paused
    └─ Recovery: resume_multiplayer_group() resumes all
    └─ If lobby times out (120s in Forming): cancel, send StopGame to pods
```

### §10.3 Multiplayer Pre-Flight Checks (Additions to §3)

| Check | Source of truth | Fail → action | Currently exists? |
|-------|-----------------|---------------|-------------------|
| **MPF-1** No active acServer instance | `ac_server.instances` map | Reject — already-active OR queue | **Yes** (`ac_server.rs:77-85`) — bails with error, no queue |
| **MPF-2** Free port slot in allocator | `port_allocator` | Reject — capacity-full | **Yes** — silent fail UX |
| **MPF-3** All N pods PF-1 to PF-15 pass | per-pod readiness | Reject — at least one pod not ready | **No** (each pod's pre-flight runs at LaunchGame time, AFTER AC server is already booting) |
| **MPF-4** AssettoServer config sane (REGISTER_TO_LOBBY=0) | config generator | Reject — security violation | **Yes** (test enforces) |
| **MPF-5** Wallet debit successful for ALL pods | atomic transaction | Reject — partial debit must roll back | **Partial** — kiosk path atomic; staff path not verified |
| **MPF-6** No port-cooldown blocking re-booking | port allocator state | Warn — same experience re-booked < 4 min ago | **No** (silent fail "no ports available") |
| **MPF-7** No prior unfinished group_session for these pods | DB query | Reject — pods still locked | **Unknown** — needs verification |

**MPF-3 is highest-impact addition.** Today: AC server boots before per-pod readiness check → missing game binary on Pod 5 → AC server already running → entire group cancelled with refund logic. Checking pod readiness BEFORE booting AC server eliminates this whole class.

### §10.4 Multiplayer Persistence Tables

| Table | Authoritative on | Writer | Reader | Cloud-synced? |
|-------|------------------|--------|--------|---------------|
| `group_sessions` | Booking + lifecycle | `book_multiplayer*`, `start_ac_lan_for_group()`, `check_and_stop_multiplayer_server()` | dashboard, staff endpoints | **Unknown — verify** |
| `group_session_members` | Per-pod participation | `book_multiplayer*`, member validate endpoint | `pause_multiplayer_group()`, `check_and_stop_multiplayer_server()` | **Unknown** |
| `ac_sessions` | acServer process lifecycle | `start_ac_server()` insert, `stop_ac_server()` update | orphan recovery (`ac_server.rs:340-383`) | Local-only |
| `multiplayer_results` *(inferred)* | Per-driver race outcome | `collect_results()` (`ac_server_results.rs`) | leaderboards, race summaries | **Likely yes** |

#### Contract invariants

**INV-MP1**: Every `group_session` row at phase=`active` MUST have non-null `ac_session_id` referring to a `Running` or `Stopped` row in `ac_sessions`. Probably satisfied; not verified.

**INV-MP2**: Every `ac_sessions` row with `status='running'` MUST correspond to exactly one `LobbyManager` instance in `{Forming, AllReady, Starting}`. **Currently violated by design** — `LobbyManager` in-memory only; server restart loses lobby while DB persists. **G-LOBBY-NO-PERSIST**.

**INV-MP3**: At any moment, count of `ac_sessions.status IN ('starting', 'running')` ≤ 1. Enforced at write-time. *Satisfied.*

**INV-MP4**: For every `group_sessions.status='completed'`, every `group_session_members` row MUST have per-pod billing reconciled (no orphan billing_session). *Believed satisfied; not query-verified.*

**INV-MP5**: Lobby timeout ≤ minimum-pod-launch-timeout in the group. Today: lobby = 120s, pod SP launch = 30s/60s/90-120s. **Violation possible** — lobby can time out while pod still in P5 (Spawn). Interaction not specified. **G-MP-TIMEOUT-INTERACTION**.

### §10.5 Multiplayer Gaps

| Gap ID | Description | Affected SM | Severity |
|--------|-------------|-------------|----------|
| **G-LOBBY-NEVER-ACTIVE** | `LobbyPhase::Active` defined in `rc-common/types.rs:522` but NO `LobbyManager` method writes it. `mark_starting()` sets `Starting`; that's the last state before `remove_lobby()`. Enum value unreachable. | SM6 | **Medium** — silent dead code. Either remove or add the transition. |
| **G-LOBBY-NO-PERSIST** | `LobbyManager` in-memory only. Server restart loses lobby state. `ac_sessions.status='running'` persists in DB but coordinator disappears. | SM6 | **High** — server crash mid-race leaves customers in-game with no coordination |
| **G-AC-SINGLETON** | Only ONE `acServer.exe` can be `{Starting, Running}` globally. Two group bookings cannot run concurrently. | SM7 | **High** — capacity ceiling is 1 concurrent MP race regardless of pod count |
| **G-AC-SERVER-FAKE-READY** | `start_ac_server()` transitions `Starting → Running` after a hardcoded **500ms sleep** (`ac_server.rs:230-239`). Not based on probing acServer HTTP. If acServer takes >500ms to bind ports / load content, server transitions to `Running` while still initializing. Pods receive `LaunchGame` and connect to a not-yet-ready AC server. | SM7 | **High** — race condition between "server ready" claim and actual readiness |
| **G-MP-TIMEOUT-INTERACTION** | Lobby timeout (120s in `Forming`) and pod-launch timeouts (30s/60s/90-120s/180s) coexist. Lobby can time out while pods still launching. Cancellation while a pod is mid-launch is undefined behavior. | SM1 + SM6 | **Medium** |
| **G-MP-PIN-MODEL** | Staff booking (`book_multiplayer`) → SHARED PIN. Kiosk booking (`book_multiplayer_kiosk`) → UNIQUE PIN per pod. Two different auth models for same product. | M1/M2 | Low — UX inconsistency |
| **G-MP-PORT-COOLDOWN** | 4-minute cooldown on ports after race ends (Windows TCP TIME_WAIT). Re-booking same experience within 4 min: port allocator skips slots. If all 16 in cooldown: "no ports available" customer-visible failure. | port_allocator | **Medium** — surfaces during high-cadence venue operation |
| **G-MP-PAUSE-SCOPE** | `pause_multiplayer_group()` pauses ALL pods when ANY pod crashes. UX unclear: do staff know why their non-crashed pods are paused? | billing | Low — design decision; needs documentation |
| **G-AC-SERVER-DEATH-SILENT** | If `acServer.exe` crashes, `monitor_lobby_sync()` will fail to connect to `/INFO`. NO fallback signal detects the crash. Pods stay in-game indefinitely thinking server is alive. Results never collected. | SM7 + monitor | **High** — customer-visible: "the race never ends" |
| **G-MP-CLOUD-AUTHORITY** | Lobby state, `ac_sessions`, `group_sessions` cloud-sync authority **not documented**. Cloud may have stale or absent visibility into in-flight MP races. | All MP tables | Low |
| **G-MP-WEEKEND-COMPLEXITY** | `weekend.rs` orchestration adds Practice→Quali→Race phase transitions on top of SM6+SM7. Phase transitions detected by polling acServer `/INFO` every 3s. Polling failure = transition missed. | SM6 + SM7 + weekend | Low — championships are venue-managed events, low frequency |
| **G-MP-NO-MPF-3** | Per-pod pre-flight readiness checked AT `LaunchGame` time, AFTER AC server is already booting. A missing game binary on Pod 5 → entire group cancellation with refund logic. | M3/M4 | **High** — most common partial-MP-failure mode |

**Three gaps drive most customer-visible MP failures:** G-AC-SERVER-DEATH-SILENT (race never ends), G-MP-NO-MPF-3 (one bad pod kills the group), G-LOBBY-NO-PERSIST (server restart abandons in-flight race).

### §10.6 Open MP-Related PACTs Mapped to Gaps

The Explore agent's grep found no explicit MP-tagged PACTs in the codebase scan. Memory references "Multiplayer system acknowledged" (msg #89 this session) but the underlying proposal/feature was not traced. **Evidence gap**, not necessarily a reality gap.

**Action item:** When James is online, ask him whether a multiplayer-tagged PACT exists in `comms-link/proposals/` that this contract should reference. Absence of MP PACTs may itself be interesting — suggests MP is treated as "shipped and stable" while §10.5 gaps are real.

| Source | MP PACT | Subsumed by gap | Status |
|--------|---------|-----------------|--------|
| (none identified in code scan) | — | — | — |

### §10.7 Recommended Next Concrete Steps for MP

#### Step M1 — Decide on AC server concurrency model (1-day decision, not engineering)

Singleton design (one acServer at a time): intentional or accidental?
- **Intentional** → document in §10.1.2 + add capacity planning note
- **Accidental** → design and prototype N-instance pattern (port allocator already supports 16 slots)

Product/business question, not engineering. Affects pricing model and venue throughput.

#### Step M2 — Fix G-AC-SERVER-FAKE-READY (~half day)

Replace 500ms sleep with `GET http://localhost:{http_port}/INFO` poll — wait for HTTP 200 with ≤30s timeout. Single-file change in `ac_server.rs:230-239`.

#### Step M3 — Add MPF-3 (all-pods-ready) pre-flight (~1 day)

Before `start_ac_server()`, query each pod's `/v1/agent/launch-readiness` endpoint (proposed in §3 for SP). If any pod fails: reject booking with refund, never spin up acServer. Eliminates "AC server booted, then Pod 5 missing game" failure class.

#### Step M4 — Fix G-AC-SERVER-DEATH-SILENT (~half day)

`monitor_lobby_sync()` already polls `/INFO`. On 3 consecutive failures, transition SM7 to `Error` and trigger group teardown with refund. Closes "race never ends."

#### Step M5 — Persist lobby state to DB (~1 day)

Mirror in-memory `LobbyManager` state to a `lobby_states` table on every transition. Server restart restores from DB. Closes G-LOBBY-NO-PERSIST.

#### Step M6 — Resolve G-LOBBY-NEVER-ACTIVE (~1 hour)

Either remove `LobbyPhase::Active` from the enum (if dead) or add the missing transition (if intended). Code-archaeology question.

#### Step M7 — Document acServer concurrency capacity in PROJECT-DOCS / customer-facing materials (~half day)

If Step M1 lands on "1 concurrent race", that's a business fact venue ops needs to know — affects scheduling, pricing tiers, peak-hour staffing.

**Total Steps M1–M7: ~4–5 days.** Most are small. Large ones (M1 design, M3 pre-flight) overlap heavily with SP §7 — implementing `/v1/agent/launch-readiness` once gives both PF-1..PF-15 (SP) and MPF-3 (MP).

### §10.8 What's Still Out of Scope (v3+)

- **Championship/weekend orchestration** (`weekend.rs`) — Practice/Quali/Race phase transitions. Captured as G-MP-WEEKEND-COMPLEXITY for visibility.
- **Bot coordinator deep-trace** — naming is misleading (it's not AI-opponents-for-MP, it's anomaly detection). Worth a v3 §10.9 to clarify.
- **F1 25 / iRacing / LMU MP** — different architecture (no venue-managed dedicated server). Per §9, out of scope for racecontrol's coordination layer today.

---

*End of v2.*
