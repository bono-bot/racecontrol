# Architecture Research

**Domain:** SaltStack Fleet Management integration with existing RaceControl Rust/Axum stack
**Researched:** 2026-03-17
**Confidence:** HIGH (existing codebase read directly; Salt/WSL2 networking verified against official Microsoft and Salt docs)

---

## Standard Architecture

### System Overview — Before Salt (current state)

```
James (.27)                     Server (.23)                   Pods (.89/.33/.28 etc.)
+-----------------------+       +---------------------------+  +----------------------+
|  Claude Code / bash   |       |  racecontrol :8080        |  |  rc-agent (service)  |
|                       |       |  +- deploy.rs             |  |  +- remote_ops.rs    |
|  deploy-staging/      |       |  |   POST :8090/exec -----+--+--  :8090 (HTTP)      |
|  HTTP server :9998    +-------+  +- fleet_health.rs       |  |  +- lock_screen.rs   |
|                       |       |  |   GET :8090/health ----+--+  +- billing.rs        |
|                       |       |  +- pod_monitor.rs        |  |  +- game_process      |
|                       |       |  |   POST :8090/exec -----+--+                       |
|                       |       |  +- pod_healer.rs         |  |  WebSocket -> :8080   |
+-----------------------+       |     POST :8090/exec -------+--+----------------------+
                                +---------------------------+
```

Problems: custom HTTP server in every rc-agent binary, deploy pipeline is bespoke curl+HTTP,
pod_monitor restarts via HTTP exec, pod_healer diagnoses via HTTP exec, fleet_health probes
HTTP every 15s. Port 8090 open on all pods = unauthenticated attack surface.

### System Overview — After Salt (target state)

```
James (.27) - WSL2 Ubuntu              Server (.23)                  Pods
+--------------------------------+     +--------------------+        +----------------+
|  salt-master (Ubuntu,          |     |  racecontrol :8080 |        |  salt-minion   |
|    mirrored NIC)               |     |  +- salt_exec.rs   |  ZMQ   |  (Windows svc) |
|  :4505 (PUB) :4506 (RET)       |<----+  |   HTTP->salt-api|<-------+  :4505/:4506   |
|  :8000 (salt-api REST)         |     |  +- fleet_health   |        |                |
|                                |     |  |   (rewritten)   |        |  rc-agent      |
|  /srv/salt/                    |     |  +- pod_monitor    |        |  (service)     |
|  +- top.sls                    |     |  |   (rewritten)   |        |                |
|  +- files/rc-agent.exe         |     |  +- pod_healer     |        |  WS -> :8080   |
|  +- grains/pod_number.py       |     |     (rewritten)    |        +----------------+
|                                |     +--------------------+
|  192.168.31.27 (mirrored NIC)  |
+--------------------------------+
```

Salt minions connect outbound to master on .27 (ports 4505/4506). Master pushes commands.
racecontrol drives Salt via `salt_exec.rs` which calls the salt-api REST endpoint on :8000.
No HTTP on port 8090 anywhere.

---

## Component Boundaries After Migration

| Component | Before | After | Action |
|-----------|--------|-------|--------|
| `remote_ops.rs` (rc-agent) | HTTP server :8090, exec/file/screenshot/input | Removed entirely | DELETE |
| `deploy.rs` (racecontrol) | curl to :8090/exec + :8090/write + do-swap.bat | `salt cp.get_file` + `cmd.run do-swap.bat` | REWRITE |
| `fleet_health.rs` (racecontrol) | HTTP probe :8090/health every 15s | `salt '*' test.ping` + WS for version | REWRITE |
| `pod_monitor.rs` (racecontrol) | POST :8090/exec to restart rc-agent service | `salt 'pod{N}' service.restart rc-agent` | MODIFY |
| `pod_healer.rs` (racecontrol) | POST :8090/exec for diagnostic cmds | `salt 'pod{N}' cmd.run 'netstat...'` | MODIFY |
| `salt_exec.rs` (racecontrol) | Does not exist | NEW: wraps salt-api REST client | CREATE |
| `salt-minion` (all pods + .23) | Does not exist | Windows service, auto-start, connects to .27 | INSTALL |
| `salt-master` + `salt-api` (WSL2) | Does not exist | Ubuntu, mirrored networking, :4505/:4506/:8000 | INSTALL |
| `firewall.rs` (rc-agent) | Opens :8090 at startup | Remove :8090 rule; keep :18923 lock screen | MODIFY |
| `install.bat` (deploy kit) | Kill rc-agent, Defender, binary, HKLM, :8090 fw | Kill rc-agent, Defender, binary, HKLM, salt-minion bootstrap | SLIM |

---

## WSL2 Networking: Why Mirrored Mode

**Decision: Use mirrored networking mode. Not NAT. Not bridged.**

### Three Options Evaluated

**NAT (default WSL2 mode):**
WSL2 gets a private 172.x IP. Minions on 192.168.31.x cannot reach the salt-master
without `netsh portproxy` rules on Windows. Those rules reference the WSL2 IP, which
changes on every WSL2 restart. Requires a startup script to re-apply rules. If the script
fails to run before the first pod tries to connect, all 8 minions lose their master.
Venue-stopping failure risk at 09:00 with customers arriving. Rejected.

**Bridged mode:**
Complex Hyper-V Virtual Switch setup. Requires creating an external switch in Hyper-V
Manager, modifying WSL's network adapter. This is a fragile config that breaks on
Windows updates. WSL documentation does not officially support it. Rejected.

**Mirrored mode (recommended):**
WSL2 inherits James's Windows NIC at 192.168.31.27. Salt-master binds to 0.0.0.0
inside Ubuntu, which resolves to 192.168.31.27 from the pod's perspective. Minions
point to `master: 192.168.31.27` and connect directly. No portproxy, no startup scripts,
no IP drift.

### Known Mirrored Mode Limitation (LOW risk for Salt)

GitHub issue microsoft/WSL#10535: multicast/broadcast packets from remote LAN devices
are not always received in mirrored mode. This affects Docker, some VPN clients, and
multicast-based service discovery.

Salt uses ZeroMQ unicast TCP on :4505 and :4506. Not multicast. This bug does not affect
Salt. The minion-to-master handshake is standard unicast TCP. Confirmed safe.

### Required Configuration

**`.wslconfig` on James's machine (`C:\Users\bono\.wslconfig`):**
```ini
[wsl2]
networkingMode=mirrored
```

**Hyper-V firewall rule (one-time, PowerShell as admin):**
```powershell
Set-NetFirewallHyperVVMSetting -Name '{40E0AC32-46A5-438A-A0B2-2B479E8F2E90}' -DefaultInboundAction Allow
```
Without this, packets destined for the WSL2 NIC are dropped by the Hyper-V firewall
layer before reaching the salt-master process, even though the IP matches.

**Windows Defender Firewall (inbound, TCP):**
```powershell
New-NetFirewallRule -DisplayName "Salt Master ZMQ" -Direction Inbound -Protocol TCP -LocalPort 4505-4506 -Action Allow
New-NetFirewallRule -DisplayName "Salt API" -Direction Inbound -Protocol TCP -LocalPort 8000 -Action Allow
```

**Salt master config (`/etc/salt/master` in WSL2):**
```yaml
interface: 0.0.0.0
publish_port: 4505
ret_port: 4506
```

---

## Salt Connectivity Model

ZeroMQ communication is **minion-initiates-only**. This is the critical architectural fact:

```
Minion (pod, .89)              Master (WSL2 via .27)
    |                               |
    |-- connect to .27:4505 ------->|  (minion subscribes to PUB socket)
    |<-- command broadcast ---------|  (master pushes cmd to all or targeted)
    |-- return data to .27:4506 --->|  (minion reports result)
```

Pods only need outbound TCP to 192.168.31.27:4505 and :4506. No inbound firewall rules
on pods for Salt. This is strictly better security than the current :8090 inbound HTTP.

**Minion config on each pod (`C:\salt\conf\minion`):**
```yaml
master: 192.168.31.27
id: pod1    # pod2...pod8, server
```

Use static `id` (not hostname). Minion IDs are predictable for targeting. Pod hostnames
are not guaranteed consistent across reinstalls.

---

## Custom Grains Strategy

Salt grains are per-minion static metadata. Define `pod_number` as a grain to enable
targeting by pod number in addition to minion ID.

**Grain file on each pod:**

Location note: In newer Salt versions on Windows, grains go in
`C:\ProgramData\Salt Project\Salt\conf\grains` (not `C:\salt\grains` — that is the
legacy path). GitHub issue saltstack/salt#63024 documents this ambiguity. Verify after
install with `salt 'pod1' grains.get pod_number`.

```yaml
# C:\ProgramData\Salt Project\Salt\conf\grains
pod_number: 3
venue: racingpoint
role: sim_pod
```

**Targeting patterns (from WSL2 salt-master CLI):**
```bash
salt 'pod*' cmd.run 'tasklist /NH | findstr rc-agent'   # all pods by minion ID glob
salt -G 'pod_number:3' cmd.run 'whoami'                  # specific pod by grain
salt 'server' service.status racecontrol                 # server by exact minion ID
salt '*' test.ping                                        # all 9 minions at once
```

**Grains are NOT for real-time state.** Do not put `billing_active`, `game_running`, or
any live pod state in grains. Grains are cached on the master and only refreshed on minion
restart or explicit `saltutil.refresh_grains`. Real-time pod state stays in racecontrol
`AppState.pods` updated via WebSocket.

---

## New Component: `salt_exec.rs`

racecontrol cannot run the `salt` CLI directly (it's a Windows binary; salt CLI is in
WSL2 Ubuntu). The integration seam is the **salt-api HTTP REST interface** (`rest_cherrypy`
module), which exposes Salt commands as HTTP endpoints.

racecontrol uses its existing `reqwest` HTTP client (already in `Cargo.toml`) to call
salt-api. Same pattern as existing `cloud_sync.rs` and `email_alerts.rs` integrations.
No new Rust dependencies.

**Integration architecture:**
```
racecontrol (.23)
  salt_exec.rs
  reqwest::Client
    |
    | HTTP POST http://192.168.31.27:8000/run
    | Header: X-Auth-Token: <token>
    | Body: { client: "local", tgt: "pod8", fun: "cmd.run", arg: ["whoami"] }
    |
    v
  salt-api (WSL2 on .27, port 8000)
  rest_cherrypy module
    |
    | ZeroMQ
    v
  salt-master -> salt-minion (pod8)
```

**`salt_exec.rs` public interface:**

```rust
pub struct SaltClient {
    base_url: String,       // http://192.168.31.27:8000
    token: String,          // from racecontrol.toml [salt] section
    http: reqwest::Client,  // shared with AppState
}

impl SaltClient {
    // Execute a shell command on a single minion, return stdout
    pub async fn cmd_run(&self, target: &str, cmd: &str) -> anyhow::Result<String>

    // Copy a file from master file_roots to minion path
    pub async fn cp_get_file(&self, target: &str, src: &str, dst: &str) -> anyhow::Result<()>

    // Check minion reachability (returns true if minion responds to ping)
    pub async fn ping(&self, target: &str) -> anyhow::Result<bool>

    // Ping all minions matching glob, return (minion_id, reachable) pairs
    pub async fn ping_all(&self, target_glob: &str) -> anyhow::Result<Vec<(String, bool)>>

    // Restart a Windows service on the minion
    pub async fn service_restart(&self, target: &str, service: &str) -> anyhow::Result<()>
}
```

**No new crates.** `reqwest 0.12` is already in `racecontrol/Cargo.toml`. `serde_json`
already present. Salt API token goes in `racecontrol.toml` under `[salt]`.

**Binary file transfer warning:** The salt-users list documents a ZeroMQ error with
`cp.get_file` and `cp.get_dir` when minions are on a different VLAN from the master
after a few runs. Use `file.managed` state (via `state.apply`) for binary file
distribution instead of `cp.get_file` ad-hoc calls. Alternatively, host rc-agent.exe
on the existing HTTP server at :9998 and use `cmd.run` to download with curl — this is
what deploy.rs already does and it is proven reliable.

**Recommendation for binary deploy:** Keep the curl-from-HTTP-server pattern for the
initial binary download step. Use Salt only for the trigger step (`cmd.run do-swap.bat`).
Salt file distribution (`file.managed`) for config files (small TOML files) is reliable.

---

## Data Flow Changes

### Deploy Flow (current vs new)

**Current (HTTP-based):**
```
1. James starts HTTP server :9998 on .27
2. racecontrol deploy.rs: POST :8090/exec "curl http://.27:9998/rc-agent.exe -o rc-agent-new.exe"
3. POST :8090/exec "cmd /C do-swap.bat" (detached)
4. Poll POST :8090/health to verify new version running
```

**New (Salt-based):**
```
1. James copies rc-agent.exe to /srv/salt/files/ on WSL2 master (or keeps HTTP server)
2. racecontrol salt_exec.rs: cmd_run('pod8', 'curl http://192.168.31.27:9998/rc-agent.exe ...')
3. salt_exec.rs: cmd_run('pod8', 'C:\\RacingPoint\\do-swap.bat') [detached]
4. salt_exec.rs: ping('pod8') + WebSocket StartupReport to verify new version
```

The self-swap pattern (`do-swap.bat`) is preserved. Salt delivers the trigger; the bat
handles the 3s wait + binary rename + service restart sequence. Salt cannot replace a
running binary on Windows directly — this is an OS constraint, not a Salt constraint.

### Health Check Flow (current vs new)

**Current:**
```
fleet_health.rs start_probe_loop(): every 15s
  HTTP GET http://{ip}:8090/health for each pod
  -> http_reachable: bool, build_id: String, uptime_secs: u64
```

**New:**
```
fleet_health.rs start_probe_loop(): every 60s (less frequent; Salt is slower than direct HTTP)
  salt_exec.rs.ping_all('pod*')
  -> minion_reachable: bool per pod

build_id: moved to WebSocket StartupReport message (rc-common protocol change)
uptime_secs: remains in StartupReport (already there)
ws_connected: unchanged (WebSocket liveness)
```

The `http_reachable` field in `FleetHealthStore` is renamed `minion_reachable`. The API
response field is renamed accordingly. Fleet health dashboard shows Salt minion reachability
instead of HTTP port reachability.

### Pod Monitor Restart Flow (current vs new)

**Current:**
```
pod_monitor.rs:
  POST http://{ip}:8090/exec {"cmd": "sc stop rc-agent && sc start rc-agent"}
```

**New:**
```
pod_monitor.rs:
  salt_exec.service_restart('pod{N}', 'rc-agent')
  // OR: salt_exec.cmd_run('pod{N}', 'sc stop rc-agent') then cmd_run('sc start rc-agent')
```

`service.restart` is the preferred form — it is a first-class Salt module that handles
Windows service stop/start natively without `sc.exe`. The WatchdogState FSM and
EscalatingBackoff logic in `pod_monitor.rs` are unchanged; only the transport for the
restart command changes.

### Pod Healer Diagnostic Flow (current vs new)

**Current:**
```
pod_healer.rs exec_on_pod():
  POST http://{ip}:8090/exec {"cmd": "netstat -ano | findstr CLOSE_WAIT"}
  POST http://{ip}:8090/exec {"cmd": "tasklist /FO CSV /NH"}
  POST http://{ip}:8090/exec {"cmd": "wmic logicaldisk ..."}
```

**New:**
```
pod_healer.rs exec_on_pod() -> uses salt_exec.cmd_run():
  salt_exec.cmd_run('pod{N}', 'netstat -ano | findstr CLOSE_WAIT')
  salt_exec.cmd_run('pod{N}', 'tasklist /FO CSV /NH')
  salt_exec.cmd_run('pod{N}', 'wmic logicaldisk ...')
```

All diagnostic command strings are identical. Output parsing logic in
`collect_diagnostics()` does not change. Only the `exec_on_pod()` helper is replaced.

---

## Modules: Remove vs Modify vs Create vs Keep

### Remove (delete entirely)

| File | Rationale |
|------|-----------|
| `crates/rc-agent/src/remote_ops.rs` | Entire module removed. HTTP exec, file ops, screenshot, input simulation — all replaced by Salt. Port :8090 eliminated. |

Remove the call to `remote_ops::start(8090)` in `crates/rc-agent/src/main.rs`.

### Create (new files)

| File | What it does |
|------|-------------|
| `crates/racecontrol/src/salt_exec.rs` | Salt REST API client. `cmd_run`, `cp_get_file`, `ping`, `ping_all`, `service_restart`. Uses existing `reqwest`. |

### Modify (targeted changes, not rewrites)

| File | What changes |
|------|-------------|
| `crates/racecontrol/src/deploy.rs` | Replace HTTP steps with `salt_exec` calls. `do-swap.bat` pattern preserved. Remove `POD_AGENT_PORT` constant. |
| `crates/racecontrol/src/fleet_health.rs` | Replace `start_probe_loop` HTTP probes with `salt_exec.ping_all()`. Rename `http_reachable` -> `minion_reachable`. |
| `crates/racecontrol/src/pod_monitor.rs` | Replace `exec_on_pod_via_http()` with `salt_exec.cmd_run()` or `salt_exec.service_restart()`. Remove `POD_AGENT_PORT`. |
| `crates/racecontrol/src/pod_healer.rs` | Replace `exec_on_pod()` helper with `salt_exec.cmd_run()`. Remove `POD_AGENT_PORT`. All parse logic unchanged. |
| `crates/racecontrol/src/state.rs` | Add `salt_client: Arc<SaltClient>` to `AppState`. |
| `crates/racecontrol/src/config.rs` | Add `[salt]` section: `api_url: String`, `api_token: String`. |
| `crates/racecontrol/src/lib.rs` | Export `salt_exec` module. |
| `crates/rc-agent/src/main.rs` | Remove `remote_ops::start(8090)` call. |
| `crates/rc-agent/src/firewall.rs` | Remove the :8090 firewall open call. Keep :18923 lock screen rule. |
| `crates/rc-common/src/protocol.rs` | Add `build_id: Option<String>` to `StartupReport` (moved from :8090/health endpoint). |
| `install.bat` | Remove pod-agent kill, remove :8090 firewall rule, add salt-minion MSI bootstrap. |

### Keep (unchanged)

| What | Rationale |
|------|-----------|
| WebSocket protocol (all AgentMessage variants) | Salt does not touch the real-time game management channel. |
| `pod_monitor.rs` WatchdogState FSM | Logic and thresholds unchanged. Only transport changes. |
| `pod_healer.rs` diagnostic parsing | All netstat/tasklist/wmic parse logic unchanged. |
| `fleet_health_handler` API endpoint | Response shape preserved; field rename only. |
| `billing.rs`, `lock_screen.rs`, `game_launcher.rs` | Unaffected by Salt migration. |
| `deploy.rs` DeployState FSM | State machine preserved. Steps change, tracking stays. |
| `do-swap.bat` self-swap mechanism | Still required. Windows OS constraint: cannot replace running binary. |
| `billing_guard.rs`, `failure_monitor.rs`, `bot_coordinator.rs` | v5.0 bot logic is orthogonal to fleet management transport. |

---

## Build Order (Phase Dependencies)

Each phase depends on the previous completing and being verified on Pod 8.

### Phase 1: WSL2 Salt Master + salt-api (no code changes)

Must exist before any Rust code is written. All subsequent phases require a running master.

```
1a. Verify WSL2 is installed on .27 (Ubuntu 22.04 LTS recommended)
1b. Add networkingMode=mirrored to C:\Users\bono\.wslconfig, restart WSL2
1c. Run Hyper-V firewall allow rule (PowerShell admin):
      Set-NetFirewallHyperVVMSetting -Name '{40E0AC32-46A5-438A-A0B2-2B479E8F2E90}' -DefaultInboundAction Allow
1d. Add Windows Defender Firewall rules for :4505, :4506, :8000 inbound TCP
1e. In WSL2: apt install salt-master salt-api
1f. Configure /etc/salt/master (interface: 0.0.0.0)
1g. Configure /etc/salt/master (rest_cherrypy section, local eauth, port 8000)
1h. Start salt-master and salt-api, verify: salt --version, curl localhost:8000/login
```

**Verification gate:** From Windows PowerShell: `Test-NetConnection -ComputerName 192.168.31.27 -Port 4505` returns TcpTestSucceeded: True.

### Phase 2: Salt Minion on Pod 8 (canary)

Install one minion to validate WSL2 mirrored networking actually routes ZeroMQ before touching any Rust code.

```
2a. Download Salt Minion MSI (3007.x, Py3, AMD64) to D:\pod-deploy\ pendrive
2b. On Pod 8 (via existing remote_ops :8090 or manual):
      msiexec /i salt-minion.msi /quiet /norestart MASTER=192.168.31.27 MINION_ID=pod8
2c. Write grains file: C:\ProgramData\Salt Project\Salt\conf\grains
      pod_number: 8 / venue: racingpoint / role: sim_pod
2d. On WSL2 master: salt-key -a pod8 (accept the minion key)
2e. Verify: salt 'pod8' test.ping -> pod8: True
2f. Verify: salt 'pod8' cmd.run 'whoami' -> returns hostname
```

**Do NOT proceed to Phase 3 until Phase 2 verification passes.** This is the only WSL2 networking validation gate.

### Phase 3: `salt_exec.rs` (new Rust module on server)

The API client that all other modules will import.

```
3a. Create crates/racecontrol/src/salt_exec.rs with SaltClient struct
3b. Add [salt] section to racecontrol.toml (api_url, api_token)
3c. Add SaltClient to AppState (Arc<SaltClient>)
3d. Export salt_exec from lib.rs
3e. Write unit tests with mocked reqwest (no live Salt required)
3f. Write integration test: salt_exec.ping('pod8') against live Pod 8
```

**Why before touching existing modules:** deploy.rs, fleet_health.rs, pod_monitor.rs, pod_healer.rs all import from salt_exec. If it does not compile, none of those edits compile.

### Phase 4: Rewrite `fleet_health.rs` probe loop

Read-only. Wrong implementation is observable but not destructive. Safest migration.

```
4a. Add build_id: Option<String> to StartupReport in rc-common/protocol.rs
4b. Update rc-agent to populate build_id in StartupReport at connection time
4c. Deploy updated rc-agent to Pod 8 (via existing :8090 -- do not remove it yet)
4d. Rewrite start_probe_loop: replace HTTP probes with salt_exec.ping_all('pod*')
4e. Rename FleetHealthStore.http_reachable -> minion_reachable
4f. Update fleet_health_handler API response field name
4g. Verify: GET /api/v1/fleet/health shows pod8 minion_reachable: true, build_id populated
```

### Phase 5: Rewrite `pod_healer.rs` exec calls

Diagnostic only (no restarts). Low-risk migration.

```
5a. Replace exec_on_pod() helper: swap HTTP reqwest call for salt_exec.cmd_run()
5b. Adjust timeout: Salt ZMQ round-trip is ~200-500ms vs HTTP ~50ms. Use 15s timeout.
5c. Remove POD_AGENT_PORT constant from pod_healer.rs
5d. Verify: trigger a healer cycle on Pod 8, confirm diagnostic output is correct
```

### Phase 6: Rewrite `pod_monitor.rs` restart calls

Restart path is highest-risk (affects live sessions). Migrate last among server modules.

```
6a. Replace HTTP exec restart with salt_exec.service_restart('pod{N}', 'rc-agent')
    OR: salt_exec.cmd_run('pod{N}', 'sc stop rc-agent') + cmd_run('sc start rc-agent')
6b. Remove POD_AGENT_PORT constant from pod_monitor.rs
6c. Keep WatchdogState FSM, EscalatingBackoff, post-restart verification unchanged
6d. Verify: force rc-agent crash on Pod 8, confirm pod_monitor restarts it via Salt
```

### Phase 7: Rewrite `deploy.rs`

Most complex module (multi-step, rollback). Last to migrate.

```
7a. Replace binary download step: keep curl from HTTP :9998 server (proven reliable)
    Trigger download via: salt_exec.cmd_run('pod8', 'curl http://192.168.31.27:9998/rc-agent.exe ...')
7b. Keep do-swap.bat: trigger via salt_exec.cmd_run('pod8', 'C:\\RacingPoint\\do-swap.bat')
7c. Replace config write: salt_exec.cp_get_file or cmd.run curl for .toml config
7d. Replace health verify: salt_exec.ping('pod8') + WebSocket StartupReport build_id check
7e. Remove POD_AGENT_PORT constant from deploy.rs
7f. Verify: end-to-end deploy to Pod 8 works, rollback works
```

### Phase 8: Remove `remote_ops.rs` from rc-agent

Only after ALL server-side callers have been migrated to Salt in Phases 4-7.

```
8a. Delete crates/rc-agent/src/remote_ops.rs
8b. Remove remote_ops::start(8090) from main.rs
8c. Remove :8090 firewall open from firewall.rs
8d. cargo build -- verify compiles without remote_ops
8e. Deploy updated rc-agent to Pod 8
8f. Verify: no regression in game launch, lock screen, billing on Pod 8
```

### Phase 9: Roll out to all pods + server (.23)

```
9a. Update install.bat: remove pod-agent kill, add salt-minion MSI bootstrap
9b. Install salt-minion on all remaining pods (1-7) + server (.23) via updated install.bat
9c. Accept all minion keys: salt-key -A (or per-minion: salt-key -a podN)
9d. Deploy updated rc-agent (without :8090) to all 8 pods
9e. Verify: salt '*' test.ping returns 9 True responses (8 pods + server)
9f. Verify: GET /api/v1/fleet/health shows all 8 pods minion_reachable: true
```

---

## Architectural Patterns

### Pattern 1: Salt REST API as the Integration Seam

**What:** racecontrol calls salt-api HTTP endpoints (`rest_cherrypy`) rather than shelling out to `salt` CLI or making direct ZeroMQ connections.

**When to use:** Any time racecontrol needs to run a command on a pod or check minion status.

**Trade-offs:**
- Pro: No subprocess, no PATH dependency, no exec across WSL2/Windows boundary
- Pro: Full async via reqwest (non-blocking)
- Pro: salt-api returns structured JSON, easier to parse than stdout
- Pro: One additional service (salt-api) vs managing SSH credentials or subprocess shells
- Con: salt-api must be running in WSL2 (add to WSL2 startup mechanism)
- Con: API token must be managed (stored in racecontrol.toml, excluded from git)

**Request shape:**
```rust
let body = serde_json::json!({
    "client": "local",
    "tgt": target,
    "fun": "cmd.run",
    "arg": [cmd],
});
let resp = self.http
    .post(format!("{}/run", self.base_url))
    .header("X-Auth-Token", &self.token)
    .json(&body)
    .timeout(Duration::from_secs(30))
    .send()
    .await?;
```

### Pattern 2: Preserve do-swap.bat for Self-Update

**What:** Salt delivers `rc-agent-new.exe` to the pod, then triggers `do-swap.bat` via `cmd_run`. The bat waits 3s, stops the rc-agent service, renames the binary, starts the service.

**Why it must stay:** Salt minion runs as a separate Windows service. It survives rc-agent being killed and can trigger the restart. But Salt cannot atomically replace a file that is open/locked by a running process. The 3s wait + rename is the unlock window. This is a Windows OS constraint.

**When to use:** Every rc-agent binary deploy.

### Pattern 3: WebSocket as the Real-Time Channel, Salt as the Fleet Ops Channel

**What:** Two parallel communication stacks with distinct responsibilities.

| What | Use |
|------|-----|
| Game state, billing events, lap data, lock screen auth | WebSocket (unchanged) |
| Deploy, restart, health check, diagnostic exec | Salt (new) |

**Trade-offs:**
- Pro: Salt failure does not break game management. WebSocket is independent.
- Pro: Clear responsibility split — no ambiguity about which channel to use.
- Con: "Pod offline" diagnosis requires checking both WS status and minion reachability.
- Con: Two communication stacks to understand and maintain.

---

## Anti-Patterns

### Anti-Pattern 1: Using NAT Networking for WSL2 Salt Master

**What people do:** Use default WSL2 NAT, add `netsh portproxy` rules to forward :4505/:4506 from Windows to WSL2 IP (172.x.x.x).

**Why it's wrong:** The WSL2 IP changes on every WSL2 restart (it is DHCP-assigned from Hyper-V virtual switch). The portproxy rules must be re-applied after every WSL2 restart. If the startup script fails or WSL2 starts before the script runs, all 8 pod minions lose their master simultaneously. This is a reliability anti-pattern for a venue that opens at 09:00.

**Do this instead:** Enable mirrored networking. WSL2 gets 192.168.31.27 permanently. One-time configuration change. No startup scripts, no IP drift.

### Anti-Pattern 2: Keeping Port :8090 Alongside Salt

**What people do:** Leave `remote_ops.rs` running "for fallback" while adding Salt.

**Why it's wrong:** Creates two code paths for the same operations. Any bug in either path is twice as hard to diagnose. Keeps the unauthenticated HTTP exec surface open on all pods. The migration has no value if the old attack surface remains.

**Do this instead:** Phases 1-7 migrate all callers. Phase 8 removes remote_ops.rs. The deadline is the same release. No partial state.

### Anti-Pattern 3: Using Salt Grains for Real-Time Pod State

**What people do:** Store `billing_active: true` or `game_running: acs.exe` as grains to enable targeting billed pods with `salt -G 'billing_active:true' cmd.run`.

**Why it's wrong:** Grains are cached on the master. They are only refreshed at minion startup or on explicit `saltutil.refresh_grains`. A grains-based `billing_active` query could be hours stale. Targeting a pod that "appears" billed for a kill command based on stale grains is a billing data integrity failure.

**Do this instead:** Real-time pod state lives in racecontrol `AppState.pods` (updated via WebSocket every few seconds). Salt grains carry static metadata only: `pod_number`, `venue`, `role`.

### Anti-Pattern 4: Using Salt for Real-Time Game Events

**What people do:** Route lock screen auth responses, billing ticks, or lap completions through Salt `cmd.run` instead of WebSocket.

**Why it's wrong:** Salt ZeroMQ round-trip latency is 200-500ms minimum. Lock screen PIN auth must respond in <500ms or customers perceive it as broken. Billing tick acknowledgment needs sub-second. Salt is a fleet management tool, not a real-time event bus.

**Do this instead:** WebSocket handles all real-time events. Salt handles batch fleet operations (deploy once per deployment event, restart on crash, diagnostic scan every 2 minutes).

### Anti-Pattern 5: Separate salt-api Token Per Pod

**What people do:** Issue a different salt-api credential for each pod's management scope.

**Why it's wrong:** salt-api targeting is handled by Salt itself (minion ID, grains, glob). racecontrol always talks to ONE master. One token is sufficient. Multiple tokens = multiple credentials to rotate, no security benefit.

**Do this instead:** One salt-api token in `racecontrol.toml`. The token has permission to run commands on `pod*` and `server` targets. Rotate it when racecontrol.toml is redeployed.

---

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| salt-master (WSL2 .27) | HTTP REST via salt-api, reqwest client in salt_exec.rs | Token auth. One token in racecontrol.toml. |
| salt-minion (each pod, server) | Managed by salt-master. racecontrol never talks to minions directly. | ZMQ transport hidden behind salt-api. |
| Tailscale mesh | Existing. Does not affect Salt which runs on LAN 192.168.31.x only. | No change needed. |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| racecontrol -> salt-master | HTTP REST (reqwest, port 8000) | Async. Timeout: 30s for cmd.run, 5s for ping. |
| racecontrol <-> rc-agent | WebSocket :8080 (unchanged) | All game events, billing, lock screen. |
| salt-master -> salt-minion | ZeroMQ :4505/:4506 | Minion-initiates only. Master never connects to pods. |
| rc-agent -> racecontrol | WebSocket + StartupReport (add build_id field) | build_id moved here from :8090/health JSON. |

---

## Confidence Assessment

| Area | Confidence | Evidence |
|------|------------|----------|
| WSL2 mirrored networking for Salt ZeroMQ | MEDIUM-HIGH | MS docs confirm mirrored mode. Known unicast-works/multicast-bugs split. Salt is unicast TCP. Hyper-V firewall requirement documented. WSL2 mirrored mode LAN bug confirmed as multicast-specific. |
| Salt ports 4505/4506, minion-initiates model | HIGH | Official Salt docs and Broadcom KB. Consistent across multiple sources. |
| Salt Windows minion silent install (MSI) | HIGH | Official Salt install guide. MSI `/quiet MASTER= MINION_ID=` flags confirmed. Service name `salt-minion`. |
| Custom grains path on Windows minions | MEDIUM | Known ambiguity between `C:\salt\grains` and `C:\ProgramData\Salt Project\Salt\conf\grains`. GitHub issue #63024. Must verify on Pod 8 after install. |
| salt-api rest_cherrypy integration via reqwest | MEDIUM | Official Salt docs show rest_cherrypy. cp.get_file ZMQ bug documented for cross-VLAN scenarios. Workaround (curl HTTP server) is existing proven pattern. |
| remote_ops.rs callers inventory | HIGH | Read all four caller files directly from source. Only `POD_AGENT_PORT = 8090` references in deploy.rs, fleet_health.rs, pod_monitor.rs, pod_healer.rs. No other callers. |
| do-swap.bat preservation requirement | HIGH | Read deploy.rs self-swap pattern directly. Windows locks running binary. Salt minion cannot solve this OS constraint. |

---

## Sources

- [Microsoft WSL Networking docs](https://learn.microsoft.com/en-us/windows/wsl/networking) — NAT vs mirrored, portproxy, Hyper-V firewall (updated 2025-12)
- [WSL mirrored mode LAN packets bug](https://github.com/microsoft/WSL/issues/10535) — confirms multicast-specific, unicast TCP unaffected
- [WSL mirrored mode practical guide](https://hy2k.dev/en/blog/2025/10-31-wsl2-mirrored-networking-dev-server/) — 2025-10
- [Salt firewall docs](https://docs.saltproject.io/en/3007/topics/tutorials/firewall.html) — ports 4505/4506, minion-initiates model
- [Salt Windows install guide](https://docs.saltproject.io/salt/install-guide/en/latest/topics/install-by-operating-system/windows.html) — MSI silent install
- [salt.modules.cp](https://docs.saltproject.io/en/latest/ref/modules/all/salt.modules.cp.html) — file distribution
- [Salt cp.get_file ZMQ issue](https://groups.google.com/g/salt-users/c/rtjniGu1UPM) — cross-VLAN failure; use file.managed or cmd.run curl instead
- [rest_cherrypy docs](https://docs.saltproject.io/en/latest/ref/netapi/all/salt.netapi.rest_cherrypy.html) — salt-api HTTP REST
- [Salt custom grains Windows bug](https://github.com/saltstack/salt/issues/63024) — grains path issue on newer Windows minions
- [Salt ports Broadcom KB](https://knowledge.broadcom.com/external/article/403589/port-requirements-for-saltminionsaltmast.html) — port requirements confirmed
- Direct source reads (2026-03-17): `remote_ops.rs`, `deploy.rs`, `fleet_health.rs`, `pod_monitor.rs`, `pod_healer.rs`, `state.rs`, `config.rs`, `main.rs` (rc-agent), `.planning/PROJECT.md`

---

*Architecture research for: SaltStack Fleet Management integration (v6.0)*
*Researched: 2026-03-17*
