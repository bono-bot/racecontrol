# Architecture Research

**Domain:** Pod fleet self-healing — Windows Service, WebSocket exec, firewall auto-config, fleet dashboard
**Researched:** 2026-03-15
**Confidence:** HIGH — based on direct codebase inspection of rc-agent/main.rs, rc-core/ws/mod.rs, rc-core/deploy.rs, rc-common/protocol.rs, rc-core/state.rs, remote_ops.rs, pod_monitor.rs, and all kiosk/web TSX pages

---

## Existing System Map

Understanding what already exists is prerequisite to placing new code correctly.

### Current Communication Paths

```
rc-core (:8080)
  |
  |-- /ws/agent   (WebSocket) <──────────────── rc-agent (pods, outbound connect)
  |                                              sends: AgentMessage enum
  |                                              receives: CoreToAgentMessage enum
  |
  |-- /ws/kiosk   (WebSocket) <──────────────── kiosk Next.js (browser, outbound)
  |-- /ws/dashboard (WebSocket) <──────────────── web Next.js (browser, outbound)
  |
  |-- HTTP REST   <──────────────── kiosk Next.js, web Next.js, PWA
  |
rc-core also connects OUT to:
  |-- :8090/exec  (HTTP POST) ──────────────── rc-agent remote_ops.rs (inbound)
  |-- :8090/write (HTTP POST) ──────────────── rc-agent remote_ops.rs
```

### Key AppState Fields (rc-core/src/state.rs)

```
agent_senders: RwLock<HashMap<String, mpsc::Sender<CoreToAgentMessage>>>
  — the WS send channel per pod. If pod disconnects, sender.is_closed() == true.

pod_deploy_states: RwLock<HashMap<String, DeployState>>
  — per-pod deploy lifecycle: Idle/Downloading/SizeCheck/Starting/VerifyingHealth/Complete/Failed/WaitingSession

pending_deploys: RwLock<HashMap<String, String>>
  — binary URL queued for pods with active billing sessions

pod_watchdog_states: RwLock<HashMap<String, WatchdogState>>
  — FSM: Healthy/Restarting/Verifying/RecoveryFailed
```

### Current CoreToAgentMessage Variants (rc-common/src/protocol.rs)

Registered, StartSession, StopSession, Configure, BillingStarted, BillingStopped,
SessionEnded, SubSessionEnded, LaunchGame, StopGame, ShowPinLockScreen,
ShowQrLockScreen, ClearLockScreen, BlankScreen, BillingTick, ShowAssistanceScreen,
EnterDebugMode, SetTransmission, SetFfb, PinFailed, SettingsUpdated,
ShowPauseOverlay, HidePauseOverlay, Ping, SetAssist, SetFfbGain, QueryAssistState

### Current AgentMessage Variants (rc-common/src/protocol.rs)

Register, Heartbeat, Telemetry, LapCompleted, SessionUpdate, DrivingStateUpdate,
Disconnect, GameStateUpdate, AiDebugResult, PinEntered, Pong, GameStatusUpdate,
FfbZeroed, GameCrashed, ContentManifest, AssistChanged, FfbGainChanged, AssistState

### Current deploy.rs exec path

`exec_on_pod()` — HTTP POST to http://{pod_ip}:8090/exec — uses rc-core's reqwest client.
Every deploy step (download, size-check, self-swap, health-verify) goes through this function.

---

## Integration Architecture: 5 Specific Questions Answered

### Q1: Windows Service Registration — NSSM wrapper vs ServiceMain in main.rs

**Decision: NSSM wrapper. Do not touch main.rs for service registration.**

**Rationale:**

The `windows-service` crate requires restructuring `main()` into a `ServiceMain` callback and threading startup through `service_dispatcher::start()`. This is a substantial, high-risk rewrite of rc-agent's main.rs (470+ lines of carefully sequenced startup: single-instance mutex, logging, early lock screen, config load, FFB zero, etc.). Getting Session handling wrong corrupts the startup sequence.

NSSM (Non-Sucking Service Manager) wraps the existing binary without any code change. rc-agent starts as-is, Session 0 isolation is handled by NSSM's `AppEnvironmentExtra` or the existing HKLM Run key hybrid approach.

**Session 0 vs Session 1 implications:**

This is the critical constraint. Windows Services run in Session 0 by default. Session 0 has no desktop — GUI calls (Edge browser for lock screen, game launch via Steam, overlay) all fail silently. The existing HKLM Run key approach solves this by starting rc-agent at user login (Session 1) but provides no crash restart.

The correct hybrid for v4.0 is:

```
NSSM service (Session 0 aware):
  - Installed as Windows Service with NSSM
  - Configured with "Interact with Desktop" or type=own start=auto
  - On pod login: HKLM Run key start-rcagent.bat already runs rc-agent in Session 1
  - NSSM monitors the process started by start-rcagent.bat via process name matching
  - NSSM restarts on exit code != 0 with backoff (3s, 10s, 30s)
```

However, NSSM monitoring a process started by another mechanism is unreliable. The cleaner v4.0 approach:

**Recommended: NSSM as crash-restart watchdog for start-rcagent.bat itself**

```
NSSM service wraps start-rcagent.bat (not rc-agent.exe directly)
  - NSSM starts C:\RacingPoint\start-rcagent.bat as a service
  - start-rcagent.bat uses `start /wait` to launch rc-agent in a new session
  - NSSM detects bat exit and restarts it (restart window: 3s delay)
  - GUI processes still run via the bat's session inheritance
```

This requires zero changes to rc-agent main.rs and is installable via pod-agent exec. New file: `crates/rc-agent/scripts/install-service.bat` — called by deploy.rs after binary deploy.

**Where the code goes:**
- New file: `deploy-staging/install-service.bat` — NSSM install commands
- Modified: `deploy.rs` — add post-deploy service registration step
- No changes to main.rs

---

### Q2: Adding Exec Variant to CoreToAgentMessage — Backward Compatibility

**Decision: Add `Exec` and `ExecResult` as new enum variants with `#[serde(other)]` unknown-variant handling on the agent side.**

**Existing serde config:**

```rust
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "snake_case")]
pub enum CoreToAgentMessage { ... }
```

With `serde(tag = "type")`, unknown variants cause a deserialization error by default. Old agents (pre-v4.0) receiving an `Exec` message would crash the deserialization, causing the WebSocket loop to log an error and discard the message. This is acceptable since deploy happens before the exec path is used.

However, for maximum safety during rolling deploy (when some pods are on old binary):

**Pattern: Add `UnknownCommand` catch-all variant**

```rust
/// Catch-all for unknown commands — allows old agents to ignore new variants gracefully.
#[serde(other)]
UnknownCommand,
```

`#[serde(other)]` on an enum variant requires the variant to be a unit variant (no data). With `serde(tag = "type", content = "data")`, this works for the type discriminant. The old agent receives `{"type": "exec", "data": {...}}`, maps to `UnknownCommand`, and the main.rs match arm ignores it.

**New protocol additions (rc-common/src/protocol.rs):**

```rust
// In CoreToAgentMessage:
/// Remote shell exec via WebSocket — fallback when HTTP :8090 is blocked by firewall.
/// Core sends this; agent runs cmd /C and sends back ExecResult.
Exec {
    request_id: String,  // UUID — matches response to request
    cmd: String,
    timeout_ms: u64,
},

// In AgentMessage:
/// Response to CoreToAgentMessage::Exec
ExecResult {
    request_id: String,  // matches the Exec request_id
    success: bool,
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
},
```

**Where the handling code goes:**

Agent side (rc-agent/src/main.rs — the WS receive select arm that already handles `CoreToAgentMessage`):
- Add arm for `CoreToAgentMessage::Exec` — spawn a blocking task, run `cmd /C`, send `AgentMessage::ExecResult` back over `ws_tx`.
- The existing `exec_command()` logic in remote_ops.rs is reusable as a pure function — extract the command execution core into a shared helper in rc-agent.

Core side (rc-core/src/ws/mod.rs — the `handle_agent` function that receives `AgentMessage`):
- Add arm for `AgentMessage::ExecResult` — forward result to a `pending_exec_requests` map (oneshot channel per request_id).

**New AppState field needed:**

```rust
// rc-core/src/state.rs
pub pending_ws_execs: RwLock<HashMap<String, tokio::sync::oneshot::Sender<ExecResult>>>
```

**New helper function in deploy.rs:**

```rust
/// Execute command on pod via WebSocket (fallback when HTTP :8090 blocked).
/// Falls back to HTTP exec if WS channel not available.
async fn exec_on_pod_ws(
    state: &Arc<AppState>,
    pod_id: &str,
    cmd: &str,
    timeout_ms: u64,
) -> Result<(bool, String, String), String>
```

Deploy.rs already has `exec_on_pod()` for HTTP. The WS variant sends `CoreToAgentMessage::Exec` to `agent_senders`, registers a oneshot receiver in `pending_ws_execs`, and awaits with timeout.

---

### Q3: Firewall Auto-Configuration — Startup in main.rs vs Separate Module

**Decision: New module `rc-agent/src/firewall.rs`, called from main.rs startup before the remote_ops server binds.**

**Rationale:**

Firewall configuration must succeed before remote_ops starts listening on :8090, or the port is usable but the firewall still blocks inbound. It's a distinct responsibility from main startup (which handles config, sim, billing). Separating it keeps main.rs readable.

**Module placement and interface:**

```rust
// rc-agent/src/firewall.rs
/// Ensure Windows Firewall rules exist for RaceControl ports.
/// Creates rules if missing. Safe to call repeatedly (idempotent).
/// Returns a list of actions taken for reporting to rc-core.
pub fn ensure_firewall_rules() -> Vec<FirewallAction>

pub enum FirewallAction {
    AlreadyPresent(String),  // rule name
    Created(String),         // rule name
    Failed { rule: String, error: String },
}
```

**Windows Firewall via Rust (no batch file dependency):**

Uses `std::process::Command` to run `netsh advfirewall firewall` with `add rule` and `show rule` subcommands. This eliminates the CRLF-damaged batch file failure mode. The existing `CREATE_NO_WINDOW` flag from remote_ops.rs applies here too.

```rust
// Check if rule exists:
netsh advfirewall firewall show rule name="RaceControl-RemoteOps"
// If not found (exit code != 0 or "No rules match"):
netsh advfirewall firewall add rule name="RaceControl-RemoteOps" protocol=TCP dir=in localport=8090 action=allow
// Also add ICMP rule:
netsh advfirewall firewall add rule name="RaceControl-ICMP" protocol=icmpv4 dir=in action=allow
```

**Call site in main.rs:**

```rust
// After logging init, before remote_ops::start()
let firewall_actions = firewall::ensure_firewall_rules();
for action in &firewall_actions {
    match action {
        FirewallAction::Created(rule) => tracing::info!("Firewall rule created: {}", rule),
        FirewallAction::AlreadyPresent(rule) => tracing::debug!("Firewall rule OK: {}", rule),
        FirewallAction::Failed { rule, error } => tracing::warn!("Firewall rule failed: {} — {}", rule, error),
    }
}
// Optionally report to rc-core via startup AgentMessage (see startup error reporting feature)
```

**Important: This runs in main() synchronously before the async runtime**

`ensure_firewall_rules()` is a synchronous function using `std::process::Command::output()` (blocking). It runs before `#[tokio::main]` enters the async executor — or in a `spawn_blocking` immediately after. Given the netsh calls are fast (< 1s), calling them synchronously in `main()` before `#[tokio::main]` is the simplest approach.

**No new dependencies required** — `std::process::Command` is in std. The `CREATE_NO_WINDOW` flag already exists in remote_ops.rs as a pattern to copy.

---

### Q4: Deploy.rs Changes — WebSocket Exec Instead of HTTP

**Decision: Make deploy.rs exec dual-path: try HTTP first (existing `exec_on_pod()`), fall back to WebSocket exec if HTTP returns connection refused.**

**Rationale:**

Full migration to WebSocket-only breaks the download step — `curl.exe` downloads a binary to pod disk, and that 100MB download over WebSocket (serialized as text/JSON) is impractical. HTTP exec for file download remains correct. WebSocket exec is the fallback for when firewall blocks :8090 post-deploy-restart.

**Modified exec_on_pod() signature:**

```rust
// Current (HTTP only):
async fn exec_on_pod(state, pod_ip, cmd, timeout_ms) -> Result<(bool, String, String), String>

// New (dual-path):
async fn exec_on_pod(state, pod_id, pod_ip, cmd, timeout_ms) -> Result<(bool, String, String), String>
```

The new signature adds `pod_id` (needed to look up WS sender). Implementation tries HTTP first (unchanged), and if HTTP fails with a connection error (not a command error), retries via `exec_on_pod_ws()`.

**Deploy sequence changes for WS exec:**

The self-swap trigger is the only step where WS fallback matters in practice. Download, size-check, and config-write all require HTTP (binary download, file read, file write). The swap trigger sends a detached batch that kills and replaces rc-agent — after the swap, :8090 may briefly be unreachable. Health verification already uses WS (`is_ws_connected()`) so deploy.rs handles this correctly.

**New DeployState variant:**

```rust
// rc-common/src/types.rs
pub enum DeployState {
    // ... existing variants ...
    Rollback { reason: String },  // NEW: rolling back to previous binary
}
```

**Rollback logic in deploy_pod():**

After `VERIFY_DELAYS` exhausted and no full health, if `rc-agent-prev.exe` exists on disk, deploy.rs sends a rollback exec via WS:

```
cmd: "move /Y C:\RacingPoint\rc-agent-prev.exe C:\RacingPoint\rc-agent.exe && start /D C:\RacingPoint rc-agent.exe"
```

The self-swap script should be modified to save the old binary as `rc-agent-prev.exe` before overwriting. This is a one-line addition to the swap bat string in `deploy_pod()`.

**Modified do-swap.bat contents:**

```batch
@echo off
timeout /t 3 /nobreak
taskkill /F /IM rc-agent.exe
timeout /t 2 /nobreak
copy /Y rc-agent.exe rc-agent-prev.exe   <- NEW: save old binary
del /Q rc-agent.exe
move rc-agent-new.exe rc-agent.exe
start "" /D C:\RacingPoint rc-agent.exe
```

No structural changes to `deploy_rolling()` or `check_and_trigger_pending_deploy()`.

---

### Q5: Fleet Dashboard — Extend Existing Kiosk vs New Page

**Decision: Add a new `/fleet` page to the existing kiosk Next.js app (not a separate app).**

**Rationale:**

The kiosk already has real-time pod state via `useKioskSocket()` hook which consumes the `/ws/kiosk` WebSocket. All pod status, billing timers, deploy states, game states, and watchdog states are already available or can be added to `DashboardEvent`. The kiosk serves on :3300 which is accessible from Uday's phone (192.168.31.23:3300).

The `web` Next.js app (staff admin) also has pod state via `/ws/dashboard`, but the kiosk is the mobile-optimized surface and has the simpler auth model (staff PIN, already implemented).

**Existing kiosk pages for reference:**

| Page | Route | Purpose |
|------|-------|---------|
| Control | `/control` | Per-pod billing/game controls — 8 pod cards |
| Settings | `/settings` | Kiosk settings |
| Spectator | `/spectator` | Customer-facing live display |
| Debug | `/debug` | Diagnostic info |

**New page: `/fleet`**

Separate from `/control` because `/control` is interaction-heavy (billing, game launch). `/fleet` is monitoring-only — a read-only health grid optimized for phone viewing.

**Fleet page data needs:**

| Data | Current Source | Status |
|------|---------------|--------|
| Pod online/offline | `pods` map from `useKioskSocket` | Exists |
| WS connected | `pods[].status` | Exists (inferred) |
| Billing active | `billingTimers` from `useKioskSocket` | Exists |
| Game state | `gameStates` from `useKioskSocket` | Exists |
| Deploy state | Not in kiosk WS | MISSING |
| Watchdog state | Not in kiosk WS | MISSING |
| Service (NSSM) status | Not exposed | MISSING |
| Firewall rule status | Not exposed | MISSING |
| Last restart timestamp | Not exposed | MISSING |

**New DashboardEvent variants needed (rc-common/src/protocol.rs):**

```rust
// In DashboardEvent:
/// Fleet health snapshot for all 8 pods
FleetHealth(Vec<PodHealthSnapshot>),

/// Single pod health update
PodHealthUpdate(PodHealthSnapshot),
```

```rust
// In rc-common/src/types.rs:
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodHealthSnapshot {
    pub pod_id: String,
    pub pod_number: u32,
    pub ws_connected: bool,
    pub deploy_state: DeployState,
    pub watchdog_state_label: String,   // "Healthy" / "Restarting(2)" / "RecoveryFailed"
    pub service_running: bool,           // NSSM service status (polled via HTTP or WS exec)
    pub firewall_ok: Option<bool>,       // None until first startup report
    pub last_restart_at: Option<String>, // ISO8601, None if never restarted by watchdog
    pub last_error: Option<String>,      // Last known error string
}
```

**Where rc-core publishes fleet health:**

New module `rc-core/src/fleet_health.rs` — background task that reads from AppState (watchdog states, deploy states, WS liveness) and broadcasts `DashboardEvent::FleetHealth` every 5 seconds. The kiosk WS handler (ws/mod.rs `handle_dashboard`) already broadcasts all `DashboardEvent` variants to dashboard subscribers.

**Fleet page component structure:**

```
kiosk/src/app/fleet/page.tsx           <- NEW route
kiosk/src/components/FleetGrid.tsx     <- NEW 8-pod health grid
kiosk/src/components/PodHealthCard.tsx <- NEW per-pod health card (compact)
```

`PodHealthCard` shows: pod number, WS dot (green/red), deploy state badge, watchdog state, last error. No billing controls. Phone-optimized (single column on mobile, 2 columns on tablet).

---

## System Overview After v4.0

```
┌─────────────────────────────────────────────────────────────────────┐
│                     rc-core :8080 (Racing-Point-Server .23)          │
│                                                                       │
│  ws/mod.rs                state.rs                deploy.rs           │
│  ┌──────────┐    ┌─────────────────────┐    ┌──────────────────┐    │
│  │ agent_ws │    │  agent_senders      │    │ exec_on_pod()    │    │
│  │ /ws/agent│    │  pod_deploy_states  │    │   ├── HTTP :8090  │    │
│  └────┬─────┘    │  pod_watchdog_states│    │   └── WS fallback│    │
│       │          │  pending_ws_execs   │←── └──────────────────┘    │
│       │          │  (NEW)              │                             │
│       │          └─────────────────────┘                             │
│                                                                       │
│  fleet_health.rs (NEW)    pod_monitor.rs     pod_healer.rs            │
│  ┌───────────────┐   ┌───────────────┐   ┌───────────────┐          │
│  │ FleetHealth   │   │ heartbeat     │   │ restart       │          │
│  │ broadcast/5s  │   │ timeout check │   │ via :8090/exec│          │
│  └───────────────┘   └───────────────┘   └───────────────┘          │
└──────────────┬────────────────────────────────────────────┬──────────┘
               │ WebSocket /ws/agent                         │ /ws/kiosk
               │                                             │
  ┌────────────▼─────────────────┐          ┌───────────────▼──────────┐
  │  rc-agent (each pod, :8090)  │          │ kiosk Next.js :3300      │
  │                              │          │                          │
  │  main.rs                     │          │ /control  (existing)     │
  │  ├── firewall.rs (NEW)       │          │ /fleet    (NEW)          │
  │  ├── remote_ops.rs (:8090)   │          │  FleetGrid.tsx           │
  │  ├── lock_screen.rs (:18923) │          │  PodHealthCard.tsx       │
  │  ├── overlay.rs (:18925)     │          └──────────────────────────┘
  │  └── ws_connect_loop         │
  │      handles Exec variant    │
  │      (NEW in main.rs)        │
  │                              │
  │  NSSM service (NEW install)  │
  │  wraps start-rcagent.bat     │
  └──────────────────────────────┘
```

---

## Component Map: New vs Modified

### New Components

| Component | Location | Purpose | Touches Existing? |
|-----------|----------|---------|------------------|
| `firewall.rs` | `rc-agent/src/firewall.rs` | Rust netsh firewall rules | No — called from main.rs |
| `fleet_health.rs` | `rc-core/src/fleet_health.rs` | Fleet health broadcast task | No — spawned from main.rs |
| `FleetGrid.tsx` | `kiosk/src/components/` | 8-pod health grid | No |
| `PodHealthCard.tsx` | `kiosk/src/components/` | Per-pod compact card | No |
| `fleet/page.tsx` | `kiosk/src/app/fleet/` | Fleet route | No |
| `PodHealthSnapshot` | `rc-common/src/types.rs` | Health data type | Additive |
| NSSM install bat | `deploy-staging/` | Service registration | No |
| `self_healing.rs` | `rc-agent/src/self_healing.rs` | Config/registry repair | No |

### Modified Components

| Component | Location | What Changes | Risk |
|-----------|----------|-------------|------|
| `CoreToAgentMessage` | `rc-common/src/protocol.rs` | Add `Exec`, `UnknownCommand` variants | LOW — additive, backward safe |
| `AgentMessage` | `rc-common/src/protocol.rs` | Add `ExecResult` variant | LOW — additive |
| `DashboardEvent` | `rc-common/src/protocol.rs` | Add `FleetHealth`, `PodHealthUpdate` | LOW — additive |
| `DeployState` | `rc-common/src/types.rs` | Add `Rollback` variant | LOW — additive |
| `AppState` | `rc-core/src/state.rs` | Add `pending_ws_execs` field | LOW — additive |
| `deploy.rs` | `rc-core/src/deploy.rs` | Add pod_id param to exec_on_pod, WS fallback, rollback logic | MEDIUM — touches core deploy path |
| `main.rs` (agent) | `rc-agent/src/main.rs` | Add `Exec` handling in WS receive loop, call `firewall::ensure_rules()` | MEDIUM — touches main event loop |
| `ws/mod.rs` | `rc-core/src/ws/mod.rs` | Handle `AgentMessage::ExecResult`, route to pending_ws_execs | LOW — additive arm |

---

## Data Flow Changes

### WebSocket Exec Flow (NEW)

```
rc-core deploy.rs
  1. HTTP exec fails (connection refused on :8090)
  2. Look up agent_senders[pod_id] — is WS open?
  3. Generate request_id = UUID
  4. Create oneshot channel, store in pending_ws_execs[request_id]
  5. Send CoreToAgentMessage::Exec { request_id, cmd, timeout_ms }
  6. Await oneshot receiver with timeout_ms + 5s buffer

rc-agent main.rs (WS receive loop)
  7. Receive CoreToAgentMessage::Exec
  8. spawn_blocking: cmd /C <cmd>
  9. Send AgentMessage::ExecResult { request_id, success, exit_code, stdout, stderr }

rc-core ws/mod.rs (handle_agent)
  10. Receive AgentMessage::ExecResult
  11. Look up pending_ws_execs[request_id]
  12. Send result over oneshot → deploy.rs await resolves
```

### Firewall Auto-Config Flow (NEW)

```
rc-agent main.rs startup (before remote_ops::start)
  1. firewall::ensure_firewall_rules() — synchronous
  2. netsh show rule → check if exists
  3. netsh add rule → create if missing
  4. Returns Vec<FirewallAction>
  5. Log results; after WS connected, optionally include in startup AgentMessage
```

### Fleet Health Broadcast Flow (NEW)

```
rc-core fleet_health.rs (every 5s)
  1. Read agent_senders — determine ws_connected per pod
  2. Read pod_deploy_states — get DeployState per pod
  3. Read pod_watchdog_states — get WatchdogState per pod
  4. Build Vec<PodHealthSnapshot>
  5. Broadcast DashboardEvent::FleetHealth to dashboard_tx
  → kiosk /ws/kiosk subscribers receive and update FleetGrid
```

---

## Recommended Build Order

Build order matters because protocol.rs is the contract between rc-agent and rc-core. Compile breaks propagate upward.

### Phase 1: rc-common protocol additions (foundation)

Add to `rc-common/src/protocol.rs` and `rc-common/src/types.rs`:
- `CoreToAgentMessage::Exec`, `UnknownCommand`
- `AgentMessage::ExecResult`
- `DashboardEvent::FleetHealth`, `DashboardEvent::PodHealthUpdate`
- `DeployState::Rollback`
- `PodHealthSnapshot` struct

Write characterization tests first — verify existing enum variants still serialize/deserialize identically after additions. Run `cargo test -p rc-common` green before proceeding.

**Why first:** All other crates depend on rc-common. Additions here break the build for rc-core and rc-agent until they handle new variants. Do it once, do it right.

### Phase 2: rc-agent firewall module

Add `crates/rc-agent/src/firewall.rs`. Call from main.rs. Run `cargo test -p rc-agent`.

**Why second:** Isolated, no dependencies on rc-core changes. Low risk. Can be verified on Pod 8 immediately. Fixes the immediate post-Mar-15 pain.

### Phase 3: rc-agent Exec handling in main.rs

Add `CoreToAgentMessage::Exec` arm to the WS receive select loop. Extract exec logic from `remote_ops.rs` into a shared helper. Run `cargo test -p rc-agent`.

**Why third:** Depends on Phase 1 (new protocol variant). rc-core's WS exec path is not needed yet — rc-agent just needs to handle the message and respond.

### Phase 4: rc-agent self-healing config check

Add `crates/rc-agent/src/self_healing.rs`. Check toml, bat, registry keys on startup. Add call from main.rs after config load.

**Why fourth:** Standalone, no cross-crate dependencies. Adds important self-repair before service restarts amplify any damage.

### Phase 5: rc-core WS exec path + deploy.rs changes

Modify `ws/mod.rs` to handle `AgentMessage::ExecResult`. Add `pending_ws_execs` to `AppState`. Add WS fallback to `exec_on_pod()` in deploy.rs. Add rollback logic.

**Why fifth:** Depends on Phase 1 (ExecResult variant) and Phase 3 (agent actually responds). Can now be tested end-to-end.

### Phase 6: NSSM service install

New bat scripts for NSSM installation. Add step to deploy.rs post-deploy flow. Deploy via pod-agent HTTP to all 8 pods sequentially.

**Why sixth:** Depends on all agent changes being live (Phase 2-4). Service restarts bring up a fresh agent — that agent needs firewall auto-config (Phase 2) and self-healing (Phase 4) to work correctly on first restart.

### Phase 7: Fleet health dashboard

Add `fleet_health.rs` to rc-core. Add fleet route + components to kiosk Next.js. Wire `DashboardEvent::FleetHealth` through kiosk WS hook.

**Why last:** Observability. Depends on all health data being available in AppState (populated by Phases 5-6). Can be built and deployed independently as it's read-only.

---

## Anti-Patterns to Avoid

### Anti-Pattern 1: Implementing ServiceMain in main.rs

**What people do:** Add `windows-service` crate, restructure main() into `ServiceMain` callback, handle `ServiceControl` events.

**Why wrong:** The existing startup sequence in main.rs has carefully ordered steps (single-instance mutex, early lock screen, config validation, FFB zero). The ServiceMain callback pattern requires moving all of this into an async context inside the service, handling `SERVICE_CONTROL_STOP` with a shutdown channel, and testing Session 0 GUI rendering. One wrong step makes the agent start in Session 0 and show a blank screen to customers.

**Do instead:** NSSM wrapper. Zero code change to main.rs. NSSM handles restart-on-crash. Session 1 startup preserved via existing HKLM Run key.

### Anti-Pattern 2: WebSocket-only exec for binary download

**What people do:** Route all deploy exec through WS to avoid HTTP firewall issues.

**Why wrong:** A 15MB binary as base64-encoded JSON over WebSocket is ~20MB of text. The WS message buffer in axum defaults to 64MB but the encoding round-trip adds ~33% overhead. More importantly, the download cmd runs for 60-120 seconds — blocking the WS receive loop on both sides for that duration. The firewall issue only affects inbound connections to :8090; outbound from rc-agent to rc-core :8080 is never blocked.

**Do instead:** Keep download via HTTP exec. Use WS exec only for short commands (self-swap trigger, health checks, registry edits) where :8090 may be temporarily blocked post-restart.

### Anti-Pattern 3: Enum-matching in deploy.rs on WatchdogState

**What people do:** Add WatchdogState-aware logic directly to deploy.rs.

**Why wrong:** deploy.rs and pod_monitor.rs both write watchdog states. Adding WatchdogState reads to deploy.rs creates a third writer/reader that can be out of sync. The watchdog FSM lives in pod_monitor.rs and pod_healer.rs.

**Do instead:** deploy.rs checks `pod_deploy_states` (its own field) and `agent_senders` (WS liveness). When a deploy is active, `pod_monitor.rs` already skips watchdog restart for that pod (deploy guard). This separation is already correct in the codebase — keep it.

### Anti-Pattern 4: Firewall rules from a batch file

**What people do:** Ship `setup-firewall.bat` and call it from deploy.

**Why wrong:** This is exactly what caused the Mar 15 incident. CRLF corruption silently breaks batch files. The batch runs once at setup and is forgotten. Future Windows Updates reset firewall rules.

**Do instead:** `firewall::ensure_firewall_rules()` runs on every rc-agent startup. Idempotent. CRLF-safe (Rust strings). Self-healing: if firewall resets, next restart restores the rules.

### Anti-Pattern 5: Fleet dashboard as a separate Next.js app

**What people do:** Create a new `/fleet-monitor` Next.js project with its own port.

**Why wrong:** The kiosk already has the WS connection, pod state, deploy state, and auth model. A new app means duplicating the WS hook, auth, and API client. Uday needs to remember a new URL. The kiosk already runs on :3300 which is in his bookmarks.

**Do instead:** Add `/fleet` route to the existing kiosk Next.js app. Reuse `useKioskSocket()`. Add new `DashboardEvent` variants for fleet-specific data. One codebase, one URL.

---

## Integration Boundaries Summary

| Boundary | Communication | Contract |
|----------|---------------|---------|
| rc-agent WS receive → Exec handling | New match arm in main.rs select loop | `CoreToAgentMessage::Exec` → `AgentMessage::ExecResult` |
| rc-core deploy.rs → WS exec | New `exec_on_pod_ws()` in deploy.rs | Uses `agent_senders` + `pending_ws_execs` |
| rc-core ws/mod.rs → ExecResult | New arm in `handle_agent()` | Routes to `pending_ws_execs` oneshot |
| rc-agent startup → firewall | `firewall::ensure_firewall_rules()` in main.rs | Called before `remote_ops::start()` |
| rc-core → kiosk fleet page | `DashboardEvent::FleetHealth` over `/ws/kiosk` | Existing broadcast channel |
| NSSM service → start-rcagent.bat | NSSM wraps existing bat | No code change to rc-agent |

---

## Sources

- Direct codebase inspection: `crates/rc-agent/src/main.rs` (474+ lines startup sequence)
- Direct codebase inspection: `crates/rc-core/src/deploy.rs` (exec_on_pod, deploy_pod, deploy_rolling)
- Direct codebase inspection: `crates/rc-core/src/ws/mod.rs` (handle_agent, agent_senders pattern)
- Direct codebase inspection: `crates/rc-common/src/protocol.rs` (CoreToAgentMessage, AgentMessage, DashboardEvent)
- Direct codebase inspection: `crates/rc-core/src/state.rs` (AppState field map)
- Direct codebase inspection: `crates/rc-agent/src/remote_ops.rs` (exec implementation, semaphore pattern)
- Direct codebase inspection: `kiosk/src/app/control/page.tsx` (useKioskSocket pattern)
- Project context: `.planning/PROJECT.md` (v4.0 requirements, Mar 15 incident motivation)
- Memory: Session 0 vs Session 1 HKLM Run key history (deployed all 8 pods)
- Confidence: HIGH — all architectural claims are derived from reading actual source files

---

*Architecture research for: Pod Fleet Self-Healing (v4.0)*
*Researched: 2026-03-15*
