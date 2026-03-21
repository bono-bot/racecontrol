# Architecture Research

**Domain:** Process Guard integration into racecontrol/rc-agent fleet
**Researched:** 2026-03-21
**Confidence:** HIGH (based on direct codebase inspection)

## Existing Architecture Context

Before describing process guard integration, the current system must be understood precisely:

**Deployment topology:**
- `racecontrol` (server .23, port 8080) — central Axum server, fleet manager, dashboard host
- `rc-agent` (all 8 pods, port 8090) — per-pod agent, WebSocket client to racecontrol
- `rc-common` — shared lib: protocol types, exec primitives, watchdog backoff
- `rc-sentry` — hardened fallback ops tool on pods (6 endpoints, no WS dependency)
- James workstation (.27) — runs comms-link (Node.js), Ollama, deploy tooling — no rc-agent

**Existing rc-agent module decomposition (post v11.0):**

```
rc-agent/src/
├── config.rs          AgentConfig (TOML deserialization)
├── app_state.rs       AppState (long-lived state across WS reconnects)
├── ws_handler.rs      handle_ws_message() + handle_ws_exec()
├── event_loop.rs      ConnectionState + select! loop
├── self_monitor.rs    Background daemon: CLOSE_WAIT + WS-dead detection
├── kiosk.rs           Process allowlist enforcement (existing, pods only)
├── pre_flight.rs      Pre-session checks (v11.1)
└── ...
```

**Existing protocol extension points:**
- `AgentMessage::ProcessApprovalRequest` — pod asks server to approve an unknown process
- `AgentMessage::KioskLockdown` — pod reports kiosk locked due to rejected process
- `CoreToAgentMessage::ApproveProcess` / `RejectProcess` — server approves/rejects
- Server-side: `/api/v1/pods/allowlist` poll endpoint used by pods every 5 minutes

**Key insight:** `kiosk.rs` already does a form of process monitoring on pods. Process guard extends this pattern to a fleet-wide, continuously-running, whitelist-enforced daemon covering processes + ports + auto-start entries + binary placement.

---

## System Overview: Process Guard Integration

```
+-----------------------------------------------------------------------------+
|                     James Workstation (.27)                                  |
|  +----------------------------------------------------------------------+   |
|  |  rc-process-guard (standalone binary -- NEW)                         |   |
|  |  +------------------+  +------------------+  +------------------+   |   |
|  |  |  ProcessMonitor  |  | AutoStartAuditor |  |  PortMonitor     |   |   |
|  |  |  (sysinfo scan)  |  | (registry+tasks) |  |  (netstat parse) |   |   |
|  |  +--------+---------+  +--------+---------+  +--------+---------+   |   |
|  |           +---------------------+-----------------------+            |   |
|  |                    ViolationReporter                                  |   |
|  |                 (HTTP POST to racecontrol)                            |   |
|  +----------------------------------------------------------------------+   |
+-----------------------------------------------------------------------------+
          | HTTP (Tailscale 100.71.226.83:8080 or LAN .23:8080)
          v
+-----------------------------------------------------------------------------+
|                    racecontrol (Server .23, port 8080)                       |
|  +----------------------------------------------------------------------+   |
|  |  process_guard.rs (NEW MODULE in racecontrol crate)                  |   |
|  |  +----------------------+  +---------------------------+             |   |
|  |  |  ProcessGuardStore   |  |  WhitelistConfig          |             |   |
|  |  |  (violation log,     |  |  (central whitelist       |             |   |
|  |  |   audit trail)       |  |   + per-machine overrides)|             |   |
|  |  +----------------------+  +---------------------------+             |   |
|  |  +------------------------------------------------------------------+|   |
|  |  |  HTTP endpoints:                                                 ||   |
|  |  |  GET  /api/v1/guard/whitelist/{machine_id}                       ||   |
|  |  |  POST /api/v1/guard/violations                                   ||   |
|  |  |  GET  /api/v1/guard/audit                                        ||   |
|  |  +------------------------------------------------------------------+|   |
|  +----------------------------------------------------------------------+   |
|                                                                               |
|  Existing WS broadcast to staff kiosk (violation alerts)                    |
+------------------------------+----------------------------------------------+
                               | WebSocket (persistent, bidirectional)
               +---------------+---------------+
               v               v               v
     +--------------+ +--------------+ +--------------+
     |  Pod 1-8     | |  Pod 1-8     | |  Pod 1-8     |
     |  rc-agent    | |  rc-agent    | |  rc-agent    |
     |  +---------+ | |  +---------+ | |  +---------+ |
     |  | process | | |  | process | | |  | process | |
     |  | guard   | | |  | guard   | | |  | guard   | |
     |  | (NEW)   | | |  | (NEW)   | | |  | (NEW)   | |
     |  +---------+ | |  +---------+ | |  +---------+ |
     +--------------+ +--------------+ +--------------+
```

---

## Component Responsibilities

| Component | Location | Responsibility | Communication |
|-----------|----------|---------------|---------------|
| `process_guard.rs` | `rc-agent/src/` (NEW) | Continuous monitoring on pods: processes, ports, auto-start, binary placement | WS AgentMessage::ProcessViolation to racecontrol |
| `process_guard.rs` | `racecontrol/src/` (NEW) | Server-side: whitelist store, violation log, HTTP endpoints, alert dispatch | Receives WS violations + HTTP from rc-process-guard |
| `rc-process-guard` | Standalone binary (NEW) | Same logic as rc-agent module, packaged for James (.27) with HTTP reporter | HTTP POST to racecontrol |
| Whitelist config | `racecontrol.toml` (MODIFIED) | Central `[process_guard]` section + `[process_guard.overrides.james]` etc. | Read at startup by racecontrol |
| `rc-agent/config.rs` | MODIFIED | Add `ProcessGuardConfig` section for agent TOML | Deserialized at agent startup |
| `rc-common/protocol.rs` | MODIFIED | New message variants: ProcessViolation, ProcessGuardUpdate | Used by both sides |

---

## Recommended Project Structure

New files to create:

```
crates/
├── rc-agent/src/
│   └── process_guard.rs       NEW -- pod guard module
│       pub fn spawn(config, ws_tx) -> JoinHandle
│       fn run_scan(whitelist) -> Vec<Violation>
│       fn enforce(violation) -> EnforcementResult
│       fn audit_autostart() -> Vec<AutoStartEntry>
│       fn audit_ports() -> Vec<PortViolation>
│
├── racecontrol/src/
│   └── process_guard.rs       NEW -- server guard module
│       pub struct ProcessGuardStore
│       pub struct WhitelistConfig
│       pub async fn get_whitelist_handler(machine_id)
│       pub async fn post_violation_handler(report)
│       pub async fn get_audit_handler()
│
├── rc-common/src/
│   types.rs                   MODIFIED -- MachineWhitelist, ProcessViolation types
│   protocol.rs                MODIFIED -- ProcessViolation, ProcessGuardUpdate messages
│
└── rc-process-guard/          NEW CRATE (standalone binary for James .27)
    ├── Cargo.toml
    └── src/
        └── main.rs            Loop: fetch whitelist -> scan -> report via HTTP -> sleep
```

Modified files:

```
crates/rc-agent/src/
├── config.rs                  + ProcessGuardConfig struct
├── app_state.rs               + guard_whitelist: Arc<RwLock<MachineWhitelist>>
└── main.rs                    + spawn guard background task after AppState init
                               + fetch whitelist on WS connect

crates/racecontrol/src/
├── config.rs                  + ProcessGuardConfig in Config struct
├── state.rs                   + guard_store: Arc<RwLock<ProcessGuardStore>>
└── main.rs                    + register guard routes

C:\RacingPoint\racecontrol.toml    + [process_guard] section
C:\RacingPoint\rc-agent.toml       + [process_guard] section (per-pod)
```

---

## Architectural Patterns

### Pattern 1: Background Daemon in rc-agent (tokio::spawn + interval)

**What:** A `tokio::spawn` background task that loops on an interval, independent of the WS event loop. This is the same pattern used by `self_monitor.rs` — spawn once in `main.rs`, runs for the binary lifetime.

**When to use:** Preferred for process guard. Monitoring must survive WS disconnects. Violations during WS disconnect are held in a bounded in-memory queue and flushed on reconnect.

**Trade-offs:**
- Pro: Simple. No coordination overhead. Survives WS disconnect.
- Con: Sysinfo scans are blocking — must wrap in `tokio::task::spawn_blocking` to avoid blocking the async runtime.
- Con: Cannot receive real-time commands mid-scan (acceptable — whitelist updates arrive via WS message handler).

**Implementation note:** The guard task receives a clone of `mpsc::Sender<AgentMessage>` passed from `main.rs`. It calls `ws_tx.try_send()` for each violation. The main WS send loop drains this channel and forwards over the socket. This matches the existing `ws_exec_result_tx` pattern in `AppState`.

### Pattern 2: Whitelist Fetch on WS Connect

**What:** On each WS reconnect, `main.rs` performs `GET /api/v1/guard/whitelist/{pod_id}` and writes the result into `state.guard_whitelist: Arc<RwLock<MachineWhitelist>>`. The background guard daemon reads this shared whitelist on each scan cycle.

**When to use:** Ensures whitelist is always current after every reconnect (which includes startup, failover, and network recovery). No separate poll timer needed.

**Trade-offs:** One HTTP round-trip on connect is negligible. The whitelist is valid for the connection lifetime. For mid-session whitelist changes, the server pushes an update (Pattern 3).

### Pattern 3: Server-Push Whitelist Update

**What:** When admin edits the central whitelist, racecontrol broadcasts `CoreToAgentMessage::UpdateProcessWhitelist { whitelist }` to all connected pods. The agent's `ws_handler.rs` handles this in a new match arm: acquire write lock on `state.guard_whitelist`, replace contents, release.

**When to use:** When an admin adds or removes a process from the whitelist and wants immediate propagation without waiting for pod reconnect.

**Trade-offs:** Requires the new `CoreToAgentMessage::UpdateProcessWhitelist` variant in rc-common. Server must broadcast to all connected pods (existing broadcast infrastructure handles this). Backward compatible — old agents ignore unknown message types.

### Pattern 4: rc-process-guard as HTTP Reporter (James .27)

**What:** Standalone binary `rc-process-guard.exe` on James runs the same scan logic as the rc-agent module but reports via `HTTP POST http://192.168.31.23:8080/api/v1/guard/violations`. No WebSocket, no billing, no session state.

**When to use:** James has no rc-agent (standing rule #2: never run pod binaries on James). The standalone binary uses Tailscale (`100.71.226.83:8080`) when available, falls back to LAN (`.23:8080`).

**Trade-offs:**
- Pro: No standing rule violation. Clean separation. Reports even during LAN instability via Tailscale.
- Con: Separate binary to build and deploy. Deploy path: `deploy-staging` HTTP server on James, download to `C:\Users\bono\racingpoint\rc-process-guard\`.
- Shared types from `rc-common` eliminate duplication of `MachineWhitelist`, `ProcessViolation`.

---

## Data Flow

### Whitelist Config Flow (Central to Per-Machine)

```
racecontrol.toml
  [process_guard]
    global_whitelist = ["rc-agent.exe", "racecontrol.exe", ...]
  [process_guard.overrides.james]
    allow_extra = ["ollama.exe", "claude.exe"]
    deny = ["steam.exe"]
  [process_guard.overrides.pod]
    deny = ["steam.exe", "kiosk.exe"]
        |
        v
racecontrol startup -> loads into ProcessGuardStore.whitelist_config
        |
        +-- GET /api/v1/guard/whitelist/pod1
        |       returns merged: global - overrides.deny + overrides.allow_extra
        |
        +-- GET /api/v1/guard/whitelist/james
                returns merged james whitelist
```

### Violation Report Flow (Pod to Server to Staff)

```
rc-agent process_guard.rs background task
  run_scan() -> finds steam.exe (not in whitelist)
  enforce() -> kill steam.exe via Windows API (TerminateProcess)
  AgentMessage::ProcessViolation { pod_id, process, action, timestamp }
        |
        | WebSocket (existing connection, same socket as heartbeats)
        v
racecontrol ws handler (existing handler, new match arm)
  -> ProcessGuardStore::record_violation()
  -> DashboardEvent::ProcessViolation broadcast to staff kiosk
  -> email_alert if severity = Critical
  -> audit log append (SQLite or append-only log file)
```

### James Violation Flow (James to Server to Staff)

```
rc-process-guard.exe (standalone, James .27)
  run_scan() -> finds kiosk.exe (wrong machine, not in james whitelist)
  enforce() -> kill process
  HTTP POST http://192.168.31.23:8080/api/v1/guard/violations
    body: ProcessViolationReport { machine_id: "james", ... }
        |
        v
racecontrol violation endpoint
  -> same ProcessGuardStore::record_violation()
  -> same alert path (WS broadcast to staff kiosk + optional email)
```

### Auto-Start Audit Flow

```
rc-agent process_guard.rs OR rc-process-guard (James)
  audit_autostart():
    1. HKCU\Software\Microsoft\Windows\CurrentVersion\Run (winreg crate)
    2. HKLM\Software\Microsoft\Windows\CurrentVersion\Run (winreg crate)
    3. %APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup\ (std::fs::read_dir)
    4. Scheduled Tasks -- parse schtasks /query /fo CSV output
  compare each entry name against whitelist.approved_autostart_keys
  for each violation:
    - remove registry value via winreg
    - delete startup shortcut via std::fs::remove_file
    - disable task via schtasks /change /tn <name> /disable
    - report as AutoStartViolation
```

---

## Integration Points

### New Protocol Messages in rc-common

Additions to `rc-common/src/protocol.rs`:

| Message | Direction | Purpose |
|---------|-----------|---------|
| `AgentMessage::ProcessViolation` | Pod to Server | Report process/port/autostart violation with action taken |
| `AgentMessage::ProcessGuardStatus` | Pod to Server | Periodic summary: scan count, violation count, last scan time |
| `CoreToAgentMessage::UpdateProcessWhitelist` | Server to Pod | Push whitelist update without WS reconnect |

New types in `rc-common/src/types.rs`:
- `struct MachineWhitelist { processes: Vec<String>, ports: Vec<u16>, autostart_keys: Vec<String> }`
- `enum ViolationType { Process, Port, AutoStart, WrongMachineBinary }`
- `struct ProcessViolation { machine_id, violation_type, name, exe_path, action_taken, timestamp }`

### New HTTP Endpoints in racecontrol

| Endpoint | Method | Auth | Purpose |
|----------|--------|------|---------|
| `/api/v1/guard/whitelist/{machine_id}` | GET | Internal (LAN, HMAC optional) | Fetch merged whitelist for a machine |
| `/api/v1/guard/violations` | POST | Shared secret header | Receive violation report from rc-process-guard (James) |
| `/api/v1/guard/audit` | GET | Staff JWT | Audit log for dashboard (paginated) |

### Integration with Existing event_loop.rs

The guard daemon is spawned as a background task (not inline in `select!`). The `mpsc::Sender<AgentMessage>` from the existing `ws_exec_result_tx` pattern in `AppState` demonstrates this approach. A new `guard_violation_tx: mpsc::Sender<AgentMessage>` follows the same pattern:

1. `main.rs` creates `(guard_tx, guard_rx)` channel pair
2. Guard spawn receives `guard_tx`, sends violations
3. `event_loop.rs` adds `guard_rx` to `AppState`, drains it in the `select!` loop alongside other outbound messages
4. Violations forward over the WS socket like any other `AgentMessage`

### Integration with Existing kiosk.rs

`kiosk.rs` already monitors processes for the kiosk allowlist. `process_guard.rs` is a parallel but distinct module:

| Dimension | kiosk.rs | process_guard.rs |
|-----------|----------|-----------------|
| Trigger | Active billing session only | Always (continuous daemon) |
| Scope | Pod processes only | Processes + ports + auto-start + binary placement |
| Whitelist source | Server HTTP poll every 5 min | TOML + server fetch on WS connect |
| Action | LLM classify -> temp allow -> server approve | Warn first scan, kill on second |
| Machine coverage | Pods only | Pods + Server (.23) + James (.27) |
| Module lifecycle | Session-scoped intervals | Binary lifetime background task |

Both modules coexist. Do not merge them.

---

## Config Structure

### racecontrol.toml additions

```toml
[process_guard]
enabled = true
scan_interval_secs = 30
# "kill_and_report" for production; "report_only" for initial rollout
violation_action = "kill_and_report"
# Warn-only on first sighting, kill on second consecutive scan
warn_before_kill = true

# Processes allowed on ALL machines
global_whitelist = [
  "racecontrol.exe", "rc-agent.exe",
  "System", "svchost.exe", "csrss.exe", "wininit.exe",
  "explorer.exe", "taskhostw.exe", "RuntimeBroker.exe",
]

global_allowed_ports = [8080, 8090, 3300, 3200, 443, 80]

global_allowed_autostart = [
  "RCAgent",
  "RaceControl",
]

[process_guard.overrides.james]
allow_extra_processes = ["ollama.exe", "node.exe", "python.exe", "chrome.exe", "Code.exe"]
allow_extra_ports = [11434, 9999, 3000]
allow_extra_autostart = []
deny_processes = ["kiosk.exe"]

[process_guard.overrides.server]
allow_extra_processes = ["node.exe"]
allow_extra_ports = [3300, 3200]
allow_extra_autostart = ["Kiosk", "WebDashboard"]

[process_guard.overrides.pod]
deny_processes = ["steam.exe", "EpicGamesLauncher.exe"]
allow_extra_ports = [9996, 20777, 5300, 8090, 18923, 6789, 5555]
```

### rc-agent.toml additions

```toml
[process_guard]
enabled = true
# Agent fetches full whitelist from racecontrol on WS connect.
# This section only provides scan timing; whitelist content comes from server.
scan_interval_secs = 30
```

---

## Build Order

Dependencies determine order. rc-common is a shared dependency of everything — changes to it must compile before either racecontrol or rc-agent can build.

### Phase 1: Protocol Foundation (rc-common)

Add to `rc-common/src/protocol.rs` and `rc-common/src/types.rs`:
- `MachineWhitelist` struct
- `ViolationType` enum
- `ProcessViolation` struct
- `AgentMessage::ProcessViolation` variant
- `AgentMessage::ProcessGuardStatus` variant
- `CoreToAgentMessage::UpdateProcessWhitelist` variant

**Reason first:** Both racecontrol and rc-agent depend on rc-common. Protocol changes must compile cleanly before either binary can import the new types. This phase has no runtime deployment — library only.

### Phase 2: Server Side (racecontrol)

1. Add `ProcessGuardConfig` to `racecontrol/src/config.rs`
2. Add `guard_store: Arc<RwLock<ProcessGuardStore>>` to `racecontrol/src/state.rs`
3. Create `racecontrol/src/process_guard.rs` with store, whitelist merge logic, and HTTP handlers
4. Add WS handler arm for `AgentMessage::ProcessViolation` in the existing WS handler file
5. Register routes in `racecontrol/src/main.rs`
6. Build and deploy to server .23

**Reason second:** HTTP endpoints must be live before pods or James try to fetch the whitelist. Deploy server first, then build agents. Smoke-test the whitelist endpoint independently with curl before deploying agents.

### Phase 3: Pod Agent Module (rc-agent)

1. Add `ProcessGuardConfig` to `rc-agent/src/config.rs`
2. Add `guard_whitelist: Arc<RwLock<MachineWhitelist>>` to `AppState`
3. Create `rc-agent/src/process_guard.rs` with spawn, scan, enforce, audit_autostart, audit_ports
4. Add `CoreToAgentMessage::UpdateProcessWhitelist` handler in `ws_handler.rs`
5. On WS connect in `main.rs`: fetch whitelist via HTTP, store in `state.guard_whitelist`
6. Spawn guard background task in `main.rs` after AppState init
7. Build, canary deploy to Pod 8, validate, then roll to all pods

**Reason third:** rc-agent needs the server endpoints live for the whitelist fetch during testing. Canary on Pod 8 first is the standing deploy rule.

### Phase 4: Standalone Binary (rc-process-guard)

1. Create `crates/rc-process-guard/Cargo.toml` with deps: `rc-common`, `sysinfo`, `reqwest`, `tokio`, `serde`, `tracing`, `winreg`
2. Create `crates/rc-process-guard/src/main.rs`:
   - Startup: fetch whitelist from `GET /api/v1/guard/whitelist/james`
   - Loop: `spawn_blocking` scan -> enforce -> `POST /api/v1/guard/violations` -> sleep
   - Reads config from `C:\Users\bono\racingpoint\rc-process-guard.toml`
3. Install on James via HKLM Run key (not HKCU — survives session changes and reboots)

**Reason last:** Standalone binary shares type definitions from rc-common but has no interdependency with rc-agent. Build after the agent module is validated on pods, so scan and enforce patterns are proven before applying to James.

---

## Anti-Patterns

### Anti-Pattern 1: Merging guard into kiosk.rs

**What people do:** Extend the existing `kiosk.rs` to also check ports and auto-start entries.

**Why it's wrong:** `kiosk.rs` is session-scoped (billing active), uses LLM classification, and targets customer session security. Process guard is always-on, covers non-session time, and covers machines with no kiosk at all (server, James). Merging creates tangled lifecycle logic and a god module.

**Do this instead:** New `process_guard.rs` module. Calls `sysinfo` independently. Imports nothing from `kiosk.rs`. Both modules coexist.

### Anti-Pattern 2: Running rc-agent on James for process monitoring

**What people do:** Install rc-agent on James (.27) to get process guard without a new binary.

**Why it's wrong:** Standing rule #2 — NEVER run pod binaries on James. rc-agent registers as a pod, participates in billing lifecycle, and changes fleet routing behavior. The original incident context (STATE-v12.1.md) confirms this is a recurring source of problems.

**Do this instead:** `rc-process-guard` standalone binary. Same guard logic, no pod identity, no billing, HTTP-only reporter.

### Anti-Pattern 3: Polling whitelist on a separate timer

**What people do:** Add a `whitelist_poll_interval` in `ConnectionState` that fetches the whitelist every N minutes via HTTP, separate from WS lifecycle.

**Why it's wrong:** Whitelist is already fetched on WS connect (which includes startup, failover, network recovery). An extra poll adds timer complexity and potential races with `UpdateProcessWhitelist` server pushes. The 30-second scan interval means a one-reconnect latency window for whitelist updates is acceptable at this venue scale.

**Do this instead:** Fetch on WS connect. Accept server push for immediate changes. No separate poll timer.

### Anti-Pattern 4: Killing processes without logging first

**What people do:** Kill a process immediately on first scan detection without recording why.

**Why it's wrong:** A kill without a log leaves no audit trail. Legitimate processes temporarily outside the whitelist (Windows Update, driver installer during maintenance) get silently killed, causing support confusion with no record.

**Do this instead:** Warn-only on first sighting (log + report, no kill). Kill on second consecutive detection. Always send the violation report before or concurrently with enforcement.

### Anti-Pattern 5: Storing full process snapshots in violation records

**What people do:** Serialize the entire running process list into each violation report.

**Why it's wrong:** Each sysinfo snapshot on Windows contains 200-400 processes. Sending the full list on every scan floods the audit log and creates a storage and privacy concern.

**Do this instead:** Report only the violating processes: name, exe_path, violation_type, action_taken. Server stores violation records, not snapshots.

---

## Scaling Considerations

This system runs on a fixed 10-machine fleet (8 pods, 1 server, 1 James workstation). Fleet growth is not a concern. The relevant operational considerations are:

| Concern | Current (10 machines) | Notes |
|---------|----------------------|-------|
| Violation log storage | SQLite append in racecontrol | Add TTL purge (keep 30 days) from day 1 |
| Whitelist endpoint load | 10 HTTP fetches at startup | Not concurrent, fully cached in racecontrol memory |
| WS violation messages | Low volume (violations rare post-cleanup) | No concern |
| Audit query performance | Full table scan fine at fewer than 10k rows | Add index on (machine_id, timestamp) |

---

## Sources

- Direct inspection: `crates/rc-agent/src/` — self_monitor.rs, kiosk.rs, event_loop.rs, app_state.rs, config.rs, pre_flight.rs, ws_handler.rs
- Direct inspection: `crates/racecontrol/src/` — fleet_health.rs, state.rs, config.rs
- Direct inspection: `crates/rc-common/src/protocol.rs` — all existing AgentMessage and CoreToAgentMessage variants
- Project context: `.planning/PROJECT.md` — v12.1 feature requirements and target features
- Project context: `.planning/STATE-v12.1.md` — incident origin (Steam, kiosk, voice watchdog on James), standing rules
- Confidence: HIGH — based on reading actual source files, not training data inference

---

*Architecture research for: v12.1 E2E Process Guard integration into racecontrol/rc-agent*
*Researched: 2026-03-21 IST*
