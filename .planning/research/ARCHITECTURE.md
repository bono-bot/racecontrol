# Architecture Research

**Domain:** Connectivity & Redundancy — health monitoring, config sync, auto-failover, and failback for a Rust/Axum + rc-agent sim racing venue management system
**Researched:** 2026-03-20 IST
**Confidence:** HIGH (existing codebase read directly: bono_relay.rs, cloud_sync.rs, self_monitor.rs, config.rs, rc-agent/main.rs, rc-sentry/main.rs, state.rs)

---

> **Milestone scope:** v10.0 Connectivity & Redundancy ONLY.
> Existing stack (Rust/Axum, SQLite, rc-agent, WebSocket, rc-sentry, Tailscale, cloud_sync, bono_relay) is not re-researched.
> Focus: integration points for health monitoring, config sync, failover trigger, and failback. What is new, what is modified, and the exact build order.

---

## Existing System Topology (Current State — Pre v10.0)

```
James Workstation (.27)       Server (.23)               Pods (8x, 192.168.31.x)
+---------------------+       +----------------------+   +----------------------+
| Claude Code         |  HTTP | racecontrol :8080    |WS |  rc-agent :8090      |
| deploy-staging :9998+------>| kiosk :3300          +-->|  rc-sentry :8091     |
| webterm :9999       |       | admin :3200          |   |  rc-watchdog svc     |
|                     |       | SQLite racecontrol.db|   |  Ollama qwen3:0.6b   |
|                     |       | cloud_sync.rs        |   |                      |
|                     |       |   HTTP push/pull 30s |   |  cfg: core.url =     |
|                     |       | bono_relay.rs :8099  |   |  ws://192.168.31.23: |
|                     |       |   event push + cmds  |   |  8080/ws/agent       |
+---------------------+       +----------------------+   +----------------------+
                                       |
                               Tailscale mesh (pods only — Phase 27)
                               100.x.x.x/Tailscale IPs
                                       |
                              Bono VPS (72.60.101.58)
                              app.racingpoint.cloud :8080
                              cloud racecontrol instance
```

**Key structural facts from code:**
- `rc-agent/src/main.rs`: `core.url` in TOML hardcoded to `ws://127.0.0.1:8080/ws/agent` by default; can be overridden per-pod. This is the ONLY place rc-agent knows which racecontrol to connect to.
- `cloud_sync.rs`: Dual-mode — 2s relay (comms-link) or 30s HTTP fallback. Syncs tables: drivers, wallets, pricing_tiers, pricing_rules, billing_rates, kiosk_experiences, kiosk_settings, auth_tokens.
- `bono_relay.rs`: One-way event push (POSTs to webhook_url) + inbound command endpoint at `/relay/command`. Auth via `X-Relay-Secret` header.
- `rc-agent/self_monitor.rs`: WS dead 5+ min triggers `relaunch_self()`. Runs every 60s. No awareness of failover — just restarts rc-agent.
- `config.rs BonoConfig`: `enabled`, `webhook_url`, `relay_secret` — bono relay is opt-in.
- `rc-sentry/main.rs`: Port 8091, `GET /ping` + `POST /exec`. No auth. Deployed on server AND pods. Independent of racecontrol/rc-agent lifecycle.

---

## Proposed v10.0 Architecture

### System Overview — After v10.0

```
James Workstation (.27)
+-----------------------------------------------+
| server_monitor (NEW — Rust binary or tokio    |
|   task in racecontrol? See §Component Decision)|
|   - polls .23:8080/health every 10s           |
|   - tracks consecutive failures               |
|   - triggers failover when threshold met      |
|   - triggers failback when .23 recovers       |
|   - notifies Uday on state transitions        |
+--------------------+--------------------------+
                     | HTTP
                     v
+--------------------+--------------------------+
|  Server (.23)                                  |
|  racecontrol :8080 (PRIMARY)                  |
|   + config_export endpoint (MODIFIED)         |
|     GET /api/v1/config/export → racecontrol.toml snapshot |
|   + health endpoint (EXISTING /health or /api/v1/pods)    |
|                                                |
|  Fixed IP via DHCP reservation (PRECONDITION) |
+--------------------+--------------------------+
         |                          |
         | WebSocket                | Tailscale mesh
         |                          |
+--------+---------+      +---------+--------+
| Pods (8x)        |      | Bono VPS          |
| rc-agent         |      | racecontrol :8080 |
|                  |      | (FAILOVER target) |
| core.url in TOML |      |                   |
| = ws://.23:8080  |      | config_sync recv  |
|   (PRIMARY)      |      | (MODIFIED)        |
|                  |      |                   |
| failover handled |      | relay command:    |
| by UPDATE cmd    |      | SwitchBackToLocal |
| (MODIFIED)       |      |   (NEW)           |
+------------------+      +-------------------+
```

---

## Component Decision: Where Does Health Monitor Live?

**Question:** Should the health monitor run on James (.27), inside racecontrol (server .23), or as a separate binary?

**Analysis:**

| Location | Pros | Cons | Verdict |
|----------|------|------|---------|
| James .27 (separate process) | Survives server .23 crash. Independent. Low blast radius. | Requires James's workstation to be on. Not always reliable at night. | ACCEPTABLE for Phase 1 |
| Server .23 (inside racecontrol) | Self-monitoring. Always-on when server is running. | Can't detect its own crash. Circular dependency — if racecontrol is down, the monitor is down. WRONG for failover. | REJECT |
| Server .23 (separate sidecar) | Always-on on server. Can detect racecontrol process death. | Still on .23 — if machine goes down entirely, sidecar is gone too. Doesn't help with server power loss. | WEAK |
| Bono VPS (cloud) | Survives .23 going down entirely. Always-on. | Cloud is the failover target — can't be both judge and destination. Latency from India to VPS. | PARTIAL ROLE ONLY |

**Recommendation:**

Use a **two-tier monitoring approach**:

1. **James .27 — `server-monitor` background process** (NEW, Rust/tokio binary): Polls racecontrol :8080 every 10s. Detects outages. Drives failover commands to pods. This is the PRIMARY arbiter because it is outside the failure domain of server .23. Limitation: only active when James's workstation is on. Acceptable for a venue with staff hours.

2. **Bono VPS — secondary watchdog** (MODIFIED `bono_relay.rs` on cloud side): If racecontrol stops pushing events to the webhook for N minutes, Bono's cloud instance considers the local server dead. Can independently update pods via Tailscale to connect to cloud. This is the SECONDARY arbiter — handles night/weekend failures when James's machine is off.

**Build order implication:** Phase 1 builds the James .27 monitor (simpler, staff-hours coverage). Phase 2 adds Bono cloud watchdog (full 24/7 coverage).

---

## Component Boundaries

### New Components (v10.0)

| Component | Location | Responsibility | Communicates With |
|-----------|----------|----------------|-------------------|
| `server-monitor` | James .27 | Poll server .23 health, detect outages, drive failover/failback, notify Uday | racecontrol :8080, rc-agent :8090 (via failover command), Bono VPS relay |
| `config-exporter` endpoint | racecontrol on .23 | Expose current racecontrol.toml as a sanitized JSON snapshot | Polled by server-monitor, pushed to Bono VPS |
| `failover_controller.rs` | NEW module in racecontrol | Receive failover commands, push new `core.url` to all connected pods via WebSocket | rc-agent WS channel (existing), rc-sentry :8091 (fallback) |
| Bono VPS watchdog | Bono cloud racecontrol | Detect heartbeat gap from venue, optionally drive failover independently | bono_relay webhook, pod Tailscale IPs |

### Modified Components (v10.0)

| Component | File | Current Behavior | New Behavior |
|-----------|------|-----------------|--------------|
| `bono_relay.rs` | `crates/racecontrol/src/bono_relay.rs` | Push events to Bono webhook. Accept LaunchGame/StopGame/GetStatus commands. | ADD: `ConfigSync` command (push full config snapshot to Bono). ADD: heartbeat timestamp in event push. |
| `rc-agent/main.rs` | `crates/rc-agent/src/main.rs` | `core.url` read from TOML at startup, never changes at runtime. | ADD: `SwitchController` AgentMessage that updates the in-memory WS target URL and triggers reconnect to new URL. |
| `rc-common/src/protocol.rs` | `crates/rc-common/src/protocol.rs` | `CoreToAgentMessage` enum with existing variants. | ADD: `SwitchController { new_url: String, reason: String }` variant. |
| `cloud_sync.rs` | `crates/racecontrol/src/cloud_sync.rs` | Sync SYNC_TABLES bidirectionally. | ADD: config snapshot push on sync cycle. Cloud receives and stores venue config for failover. |
| `rc-agent/self_monitor.rs` | `crates/rc-agent/src/self_monitor.rs` | Relaunch on WS dead 5+ min. | MODIFY: add backoff before relaunch when actively switching controllers. Prevent restart-loop during intentional failover. |
| `config.rs BonoConfig` | `crates/racecontrol/src/config.rs` | `enabled`, `webhook_url`, `relay_secret`. | ADD: `failover_enabled: bool`, `cloud_api_url: String` (the Bono VPS racecontrol WS endpoint). |

---

## Data Flow Changes

### 1. DHCP Reservation (Precondition)

No code change. Pure infrastructure.

```
Router DHCP table:
  MAC: 10-FF-E0-80-B1-A7  →  192.168.31.23  (permanent)

Before:  .23 randomly reassigned every night ~01:05
After:   .23 permanent, no drift, no stale IPs
```

This is a gate for everything else. Server IP drift is the root cause of the current weekly ops burden. Fix this first.

### 2. Tailscale SSH from James to Server

No new Rust code. Infrastructure + config.

```
James (.27)
  tailscale ssh ADMIN@racing-point-server
    ↓
  Server (.23) — Tailscale SSH service
    ↓
  cmd.exe as ADMIN
```

**What this enables:** James can remotely restart racecontrol.exe, pull new binaries, edit configs — without physical access. Required before any automated failover is useful (otherwise failback requires walking to the server).

### 3. Config Sync Flow (Local → Cloud)

**Current state:** `cloud_sync.rs` syncs database tables. `racecontrol.toml` is never sent to cloud.

**New flow:**

```
racecontrol (.23) on startup or config change:
  ↓
config_exporter (GET /api/v1/config/export)
  → sanitized JSON: venue name, pod definitions, billing rates
  → EXCLUDES: database path, credentials, secrets
  ↓
bono_relay.rs → POST to Bono VPS webhook
  → RelayCommand::ConfigSync { snapshot: ConfigSnapshot }
  ↓
Bono VPS stores snapshot in its local DB
  → used when pods connect during failover
  → ensures cloud racecontrol knows pod count, pod numbers, game config
```

**Trigger for config push:**
- On racecontrol startup (always push current config)
- On any admin config change (e.g., billing rate update)
- On 24h schedule (ensure freshness even if cloud missed a startup push)

**What ConfigSnapshot contains** (sanitized — no secrets):
```rust
pub struct ConfigSnapshot {
    pub venue_name: String,
    pub pod_count: u32,
    pub pods: Vec<PodConfigSnapshot>,  // number, name, sim_type
    pub billing_rates: Vec<BillingRate>,  // from DB (already in SYNC_TABLES)
    pub updated_at: DateTime<Utc>,
}
```

The `billing_rates` are already in `SYNC_TABLES` — config sync is primarily about pod definitions which are TOML-only, not in the DB.

### 4. Failover Flow (Local Server Down → Cloud)

```
Server .23 goes down (power cut, crash, DHCP drift, anything)

↓ (within 30-60s)

server-monitor on James .27 detects:
  - N consecutive poll failures to http://192.168.31.23:8080/health
  - N = 3 (30s × 3 = 90s — avoid false positives from racecontrol restart)
  ↓
server-monitor triggers failover:
  ↓
  Option A (rc-agent WS exec — preferred, fastest):
    POST http://192.168.31.{pod-ip}:8090/exec
    cmd: "switch-controller ws://100.x.bono.ts/ws/agent"
    [Note: rc-agent :8090 is local LAN, doesn't need .23]
  ↓
  Option B (Tailscale to pod, rc-sentry exec — fallback):
    POST http://100.x.pod-ts-ip:8091/exec
    cmd: "curl -X POST localhost:8090/switch ..."
  ↓
  rc-agent receives SwitchController AgentMessage:
    - stores new_url in AtomicPtr or RwLock<String>
    - closes existing WS connection
    - reconnect loop picks up new_url on next iteration
    - connects to Bono VPS Tailscale IP
  ↓
  Bono VPS racecontrol receives rc-agent connections:
    - pods authenticate using same pod_id + WebSocket protocol
    - billing continues (cloud racecontrol has synced billing_rates)
    - sessions tracked against cloud DB
  ↓
  Uday notified (email via existing send_email.js):
    "Venue server offline. Pods running on cloud backup. Lap times still recording."
```

**Critical constraint from code:** rc-agent's `core.url` in TOML is read at startup in `main.rs` line ~213: `fn default_core_url() -> String { "ws://127.0.0.1:8080/ws/agent".to_string() }`. The reconnect loop reconnects to the same URL forever. To support runtime switching, the reconnect loop must read from a shared `Arc<RwLock<String>>` instead of the startup config value.

**self_monitor.rs interaction:** The WS-dead-5min relaunch in `self_monitor.rs` will trigger if failover takes too long. This is acceptable — after relaunch, rc-agent will read the (now updated?) config. But if the TOML is not updated, rc-agent will keep trying to connect to dead .23 again. The `SwitchController` approach avoids TOML writes; it only updates in-memory URL. On rc-agent relaunch, it re-reads TOML → re-connects to .23 → detects .23 still down → hits 5min WS dead → relaunches again. **Loop risk.**

**Resolution:** server-monitor must also write the failover URL to each pod's TOML via rc-sentry exec during failover. This ensures relaunches reconnect to cloud, not dead .23.

### 5. Failback Flow (Local Server Recovers → Switch Back to .23)

```
Server .23 comes back online

↓ (within 30-60s)

server-monitor detects recovery:
  - N consecutive successful polls to .23:8080/health
  - N = 2 (20s — recover quickly, don't leave pods on cloud longer than needed)
  ↓
server-monitor triggers failback:
  - Posts SwitchController { new_url: "ws://192.168.31.23:8080/ws/agent", reason: "local_recovery" } to all pods
  - Also writes new_url back to each pod's TOML (same as failover, but restores original)
  - Notifies Uday: "Server recovered. Pods reconnecting to local server."
  ↓
cloud racecontrol:
  - Pushes pending billing/session data to local racecontrol via cloud_sync
  - Sessions that started during failover sync back to local DB
  ↓
server-monitor confirms all pods reconnected to .23:
  - Polls /api/v1/pods on .23 and checks connected count
  - Alert if any pod fails to reconnect after 5 minutes
```

**Failback data integrity:** Sessions that ran on cloud during failover must sync back. `cloud_sync.rs` already handles bidirectional sync of `billing_rates` etc. Billing events (sessions) are currently local-authoritative. This needs a new sync direction: cloud → local for sessions created during failover. This is the most complex data flow change.

**Simplification option:** Mark cloud sessions as "failover mode" in the DB. On failback, sync engine detects these and merges them into the local session history. Lap times recorded during failover sync normally via `cloud_sync` SYNC_TABLES.

---

## Build Order

Dependencies drive this sequence. Each phase is a hard prerequisite for the next.

```
Phase 1: Infrastructure Foundation (no Rust changes)
  1a. Server DHCP reservation → MAC 10-FF-E0-80-B1-A7 → fixed IP 192.168.31.23
      [Precondition for everything. Do first. Verify IP holds after nightly DHCP cycle.]
  1b. Tailscale SSH on server .23
      [Enables James remote access. Precondition for automated failback without physical access.]

Phase 2: Config Sync (Rust changes: racecontrol + cloud)
  2a. Add ConfigSnapshot type to rc-common
  2b. Add GET /api/v1/config/export endpoint to racecontrol
  2c. Modify bono_relay.rs to push ConfigSnapshot on startup + schedule
  2d. Bono VPS racecontrol: receive and store ConfigSnapshot
  2e. Verify: config snapshot visible in cloud DB after venue racecontrol restart
  [No failover risk — purely additive push. If push fails, cloud just has stale/no config.]

Phase 3: Pod SwitchController (Rust changes: rc-common + rc-agent)
  3a. Add SwitchController variant to CoreToAgentMessage in rc-common
  3b. Modify rc-agent reconnect loop to use Arc<RwLock<String>> for target URL
  3c. Add SwitchController handler in rc-agent WS receive loop
  3d. Modify self_monitor.rs: skip relaunch if SwitchController received in last 5 min
  3e. Verify on Pod 8: send SwitchController via /api/v1/pods/exec → pod reconnects
  [Build and test before any failover automation. Pod 8 canary pattern.]

Phase 4: Failover Controller (Rust changes: racecontrol server)
  4a. Add failover_controller.rs module to racecontrol
  4b. Implement: broadcast SwitchController to all connected pods via WS
  4c. Implement: fallback path via rc-sentry :8091 for pods not WS-connected
  4d. Add POST /api/v1/admin/failover endpoint (trigger failover manually)
  4e. Verify: manual failover command moves all pods to cloud WS
  [Manual trigger first — no automated detection yet. Allows testing the pod-switching path.]

Phase 5: Health Monitor on James .27 (new binary: server-monitor)
  5a. New Rust binary (crates/server-monitor): polls racecontrol :8080 every 10s
  5b. Tracks consecutive failure count; threshold = 3 (90s to declare outage)
  5c. On outage: calls POST /api/v1/admin/failover on Bono VPS relay
  5d. On recovery: calls POST /api/v1/admin/failback via relay
  5e. Email notification via send_email.js shell-out (existing pattern)
  5f. Verify: kill racecontrol on .23 → server-monitor triggers failover within 90s
  [First automated failover end-to-end test.]

Phase 6: Failback + Data Sync (Rust changes: cloud_sync + billing)
  6a. Add session sync direction: cloud → local for failover-mode sessions
  6b. Implement failback confirmation: server-monitor verifies pod count on .23 after switch
  6c. Test full cycle: outage → failover → recovery → failback → sessions synced
  6d. Uday notification on failback (email: "server recovered")
```

---

## Architectural Patterns

### Pattern 1: In-Memory URL Switch with TOML Durability

**What:** rc-agent holds its target WebSocket URL in `Arc<RwLock<String>>` instead of reading from config at compile time. On `SwitchController` message, updates the shared URL and triggers reconnect. Server-monitor also writes the new URL to the pod's TOML via rc-sentry exec, ensuring the URL survives rc-agent restarts.

**Why:** Runtime URL switch handles the fast path (pod is connected via WS). TOML write handles the durable path (rc-agent restarts don't undo the switch). Both are needed to handle the loop risk identified in §4.

**Trade-off:** TOML write via rc-sentry is a shell command — fragile if pod filesystem is readonly or rc-sentry is unreachable. Mitigation: rc-agent should write the current `active_url` to a sidecar file `C:\RacingPoint\rc-agent-active-url.txt` that it reads at startup, giving preference over TOML.

```
rc-agent startup:
  1. Read TOML core.url (base config)
  2. Check C:\RacingPoint\rc-agent-active-url.txt (override)
  3. Use override if present and newer than TOML
  4. Connect to resolved URL
  5. On SwitchController: update in-memory URL + write override file
```

### Pattern 2: Failover State Machine in server-monitor

**What:** server-monitor tracks a simple FSM: `Healthy | Degraded(count) | Failover | Recovering(count) | Healthy`. State transitions trigger actions. No action-on-every-tick.

**When to use:** Prevents duplicate failover triggers, ensures failback doesn't happen on a single successful poll (flap protection).

```
Healthy
  ↓ 3 consecutive failures
Failover  ← trigger failover command, send Uday alert
  ↓ server back up (2 consecutive successes)
Recovering(0..n)  ← wait for pod reconnect confirmation
  ↓ all pods confirmed on .23
Healthy  ← send Uday "server recovered" alert
```

**Trade-off:** State lives in memory. If server-monitor crashes during failover, it restarts in `Healthy` state and may not complete failback. Mitigation: persist state to a small file `~/.local/server-monitor.state.json`.

### Pattern 3: Bono Relay as Command Bus for Failover

**What:** Use existing `bono_relay.rs` command relay (already auth'd with `X-Relay-Secret`) to send failover/failback commands from server-monitor on James .27 to racecontrol on .23. server-monitor doesn't call racecontrol directly; it posts to the relay on Bono VPS, which forwards to .23.

**Wait — this is backwards for failover:** If .23 is down, the relay can't forward to it. The relay runs on .23.

**Correction:** During failover (when .23 is down), server-monitor calls the Bono VPS racecontrol directly (`POST https://app.racingpoint.cloud/relay/command`) to drive pod switching from there. When .23 is up, server-monitor can reach it directly.

**Revised pattern:**
```
server-monitor → .23 down
  → POST https://app.racingpoint.cloud/api/v1/admin/failover
  → Bono VPS racecontrol sends SwitchController to pods via Tailscale

server-monitor → .23 up (recovery)
  → POST http://192.168.31.23:8080/api/v1/admin/failback-complete
  → .23 racecontrol sends SwitchController back to local to all pods
```

### Pattern 4: Incremental Failover (One Pod at a Time)

**What:** Rather than switching all 8 pods atomically, server-monitor switches Pod 8 first, waits 30s to verify it connects to cloud, then proceeds with remaining pods.

**When to use:** During testing and initial rollout. Prevents all-pods-broken scenario if cloud WS endpoint is misconfigured.

**Production note:** Once tested, can switch to parallel broadcast for faster failover (minimizes revenue impact from idle pods).

---

## Integration Points with Existing Architecture

### bono_relay.rs — Modifications Required

**Current:** Pushes 6 event types + handles 3 command types (LaunchGame, StopGame, GetStatus).

**Add:**
1. `BonoEvent::Heartbeat { timestamp: DateTime<Utc>, pod_count: u32, active_sessions: u32 }` — sent every 30s to Bono VPS. Enables cloud-side watchdog to detect venue outage.
2. `RelayCommand::SwitchPodController { new_url: String, pod_numbers: Vec<u32> }` — received from Bono VPS or James monitor. Triggers failover_controller.rs.
3. `RelayCommand::ConfigSync { snapshot: ConfigSnapshot }` — received from self (racecontrol pushing its own config). Stored on Bono VPS.

### rc-common/protocol.rs — Modifications Required

**Current:** `CoreToAgentMessage` enum has variants for game control, session management, PIN auth etc.

**Add:**
```rust
CoreToAgentMessage::SwitchController {
    new_url: String,   // ws://100.x.x.x:8080/ws/agent (Tailscale URL) or original
    reason: String,    // "failover" | "failback" | "manual"
    write_override: bool,  // whether to write C:\RacingPoint\rc-agent-active-url.txt
}
```

### rc-agent/main.rs — Modifications Required

1. Change `config.core.url: String` (read once at startup) to `active_url: Arc<RwLock<String>>` initialized from config.
2. Reconnect loop reads from `active_url.read().await.clone()` on each retry, not cached startup value.
3. Add `SwitchController` match arm in incoming WS message handler: updates `active_url`, closes current connection, logs event.
4. On switch: write to `C:\RacingPoint\rc-agent-active-url.txt` if `write_override = true`.
5. In `self_monitor.rs`: suppress WS-dead relaunch for 5 min after `SwitchController` received. Add `last_switch_time: Option<Instant>` to monitor state.

**Risk assessment:** This touches rc-agent's core reconnect loop. Must be built with Pod 8 canary testing before fleet deployment.

### racecontrol/src — New Module Required

`crates/racecontrol/src/failover_controller.rs`:
- `pub fn trigger_failover(state: Arc<AppState>, cloud_url: String)` — broadcasts `SwitchController` to all WS-connected pods; for disconnected pods, tries rc-sentry :8091.
- `pub fn trigger_failback(state: Arc<AppState>)` — broadcasts `SwitchController` with `new_url = local WS URL`.
- Registered as axum route: `POST /api/v1/admin/failover`, `POST /api/v1/admin/failback`.
- Auth: requires `X-Admin-Secret` header (reuse existing terminal_secret from config, or add dedicated `failover_secret`).

---

## Failure Mode Analysis

| Failure | Detection | Response | Recovery |
|---------|-----------|----------|---------|
| Server .23 powers off | server-monitor 90s timeout | Failover to cloud | server-monitor detects recovery, failback |
| Server .23 racecontrol.exe crashes (machine stays up) | server-monitor 90s timeout | Failover (or watchdog restarts rc) | rc-watchdog on .23 restarts racecontrol; server-monitor detects recovery |
| Server .23 DHCP drifts (pre-reservation fix) | server-monitor poll fails | Failover; DHCP fix is Phase 1 precondition | ELIMINATED by DHCP reservation |
| James .27 is off (night/weekend) | No server-monitor running | Bono VPS secondary watchdog (Phase 2) | Bono VPS watchdog detects heartbeat gap → triggers failover |
| Tailscale down on pods | Pods can't reach cloud via Tailscale | Pods stuck on dead .23; no failover path | Failover impossible without Tailscale. Mitigation: verify Tailscale health in server-monitor before triggering. |
| Pod loses WS to .23 (transient blip) | self_monitor.rs 5-min WS-dead detection | Pod relaunches rc-agent → reconnects to .23 (if still up) | self_monitor already handles this |
| Failover triggered but cloud unreachable | server-monitor pre-checks cloud health before triggering | Abort failover, alert Uday | Pods stay on .23, even if .23 is degraded |
| Failback triggers but sessions still on cloud | cloud_sync.rs session sync | cloud_sync pulls failover-mode sessions before failback complete signal | Sequence: sync → confirm → failback |

---

## Scalability Considerations

This is an 8-pod venue. Scalability is not a concern at this scale. The constraints are:

| Concern | At 8 pods | Mitigation Needed? |
|---------|-----------|-------------------|
| Failover broadcast time | ~8 WebSocket sends, <100ms total | No |
| Cloud WS capacity during failover | 8 additional WS connections | No (cloud handles far more) |
| Config snapshot size | <10KB for 8 pods | No |
| Cloud billing sync during failover | 8 pods × up to 2 sessions = 16 rows/min | No |

---

## Anti-Patterns

### Anti-Pattern 1: Self-Monitoring Server Declares Its Own Death

**What people do:** Run the failover health check inside racecontrol on .23 (e.g., a self-check task that monitors its own health and posts to the relay if it detects problems).

**Why it's wrong:** If the process is hung, the self-check task is also hung. If the machine loses power, both die together. A health monitor must run outside the failure domain of what it monitors.

**Do this instead:** server-monitor runs on James .27, which is a different machine. It polls .23 from the outside. This is the only correct topology for detecting total machine failure.

### Anti-Pattern 2: Relying on rc-agent WS Timeout Alone for Failover

**What people do:** Modify `self_monitor.rs` to switch `core.url` to the cloud URL when WS is dead 5+ minutes.

**Why it's wrong:** rc-agent doesn't know if .23 is down or if .23 just restarted and will be back in 60s (e.g., binary update). The 5-min WS-dead threshold was calibrated for relaunch, not failover. rc-agent switching to cloud on every WS blip would cause repeated unnecessary failovers during normal deployments.

**Do this instead:** Keep rc-agent passive. Failover is driven by the external monitor (server-monitor or Bono VPS watchdog) which has full context (is .23 responding at all? or just WS reset?). rc-agent only switches when explicitly commanded via `SwitchController`.

### Anti-Pattern 3: Writing Failover URL to Pod TOML via SSH/SCP

**What people do:** SSH into each pod and overwrite rc-agent-podN.toml with a new `core.url` pointing to cloud.

**Why it's wrong:** rc-agent is already running. Changing the TOML file on disk doesn't affect the running process — it only takes effect on next restart. A restart during an active billing session will end the session, lose the game state, and potentially lose revenue.

**Do this instead:** Send `SwitchController` via the live WS connection (zero session disruption). Write the override file (`rc-agent-active-url.txt`) for durability. rc-agent reads the override on next startup. TOML remains unchanged (the "default" URL still points to .23 for when failover is over).

### Anti-Pattern 4: Syncing racecontrol.toml Secrets to Cloud

**What people do:** Push the full `racecontrol.toml` to Bono's VPS via config sync, including `terminal_pin`, `auth.jwt_secret`, `database.path`, `gmail.refresh_token`.

**Why it's wrong:** Bono's VPS is a different security domain. Credentials for the local server have no purpose on the cloud and create a leak vector.

**Do this instead:** `ConfigSnapshot` is a purpose-built struct that extracts only venue name, pod definitions, and billing rates — no credentials, no paths. See the ConfigSnapshot definition in §Data Flow Changes §3.

---

## File Change Manifest

### New Files

| File | Purpose |
|------|---------|
| `crates/server-monitor/src/main.rs` | NEW CRATE: health poll loop, failover FSM, Uday notification |
| `crates/server-monitor/Cargo.toml` | Deps: tokio, reqwest, serde, chrono, tracing |
| `crates/racecontrol/src/failover_controller.rs` | Broadcast SwitchController to pods, HTTP endpoints |
| `crates/rc-common/src/config_snapshot.rs` | ConfigSnapshot struct (shared between venue and cloud) |

### Modified Files

| File | Change |
|------|--------|
| `crates/rc-common/src/protocol.rs` | Add `SwitchController` to `CoreToAgentMessage` |
| `crates/rc-agent/src/main.rs` | `active_url: Arc<RwLock<String>>`, reconnect loop reads from it, SwitchController handler |
| `crates/rc-agent/src/self_monitor.rs` | Add `last_switch_time` guard — suppress relaunch after SwitchController |
| `crates/racecontrol/src/bono_relay.rs` | Add `Heartbeat` event type, `SwitchPodController` and `ConfigSync` command types |
| `crates/racecontrol/src/cloud_sync.rs` | Push ConfigSnapshot on startup + 24h schedule |
| `crates/racecontrol/src/config.rs` | Add `failover_enabled: bool`, `cloud_ws_url: String` to `BonoConfig` |
| `crates/racecontrol/src/main.rs` | Register failover_controller routes; spawn heartbeat push task |
| `Cargo.toml` (workspace) | Add `server-monitor` to members |

---

## Sources

- Existing codebase (read directly 2026-03-20 IST):
  - `crates/racecontrol/src/bono_relay.rs` — relay command/event types, auth pattern
  - `crates/racecontrol/src/cloud_sync.rs` — SYNC_TABLES, relay mode, HTTP fallback
  - `crates/racecontrol/src/config.rs` — BonoConfig, CloudConfig, full config structure
  - `crates/racecontrol/src/state.rs` — AppState, WatchdogState, broadcast channels
  - `crates/racecontrol/src/pod_monitor.rs` — heartbeat timeout, WatchdogState FSM
  - `crates/racecontrol/src/main.rs` — router setup, proxy patterns, startup sequence
  - `crates/rc-agent/src/main.rs` — default_core_url(), startup sequence, config loading
  - `crates/rc-agent/src/self_monitor.rs` — WS_DEAD_SECS=300, relaunch_self(), CHECK_INTERVAL_SECS=60
  - `crates/rc-common/src/protocol.rs` — CoreToAgentMessage, AgentMessage enums
  - `crates/rc-sentry/src/main.rs` — :8091 exec endpoint, no auth
- PROJECT.md (v10.0 goals: DHCP fix, Tailscale SSH, health monitor, config sync, auto-failover, Uday notification, failback)
- MEMORY.md (server MAC 10-FF-E0-80-B1-A7, DHCP lease nightly ~01:05, Tailscale installed Phase 27, Bono VPS 72.60.101.58, pod subnet 192.168.31.x)

---
*Architecture research for: v10.0 Connectivity & Redundancy — health monitoring, config sync, auto-failover, failback*
*Researched: 2026-03-20 IST*
