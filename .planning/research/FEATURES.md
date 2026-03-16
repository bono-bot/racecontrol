# Feature Research

**Domain:** Pod Fleet Self-Healing — Windows gaming PC fleet management (8-pod venue)
**Researched:** 2026-03-15
**Confidence:** HIGH (official Windows docs + existing codebase cross-referenced + incident post-mortem from Mar 15)

---

## Context: What Already Exists (Do Not Re-Plan)

The following capabilities are ALREADY BUILT and must not be re-researched or re-planned:

- WebSocket connection with keepalive ping/pong — CONN-01 through CONN-03
- HTTP remote ops on port 8090: exec, file read/write, health, screenshot — `remote_ops.rs`
- Pod monitoring with escalating backoff 30s→2m→10m→30m — `pod_monitor.rs`
- Post-restart verification at 5s/15s/30s/60s intervals — `deploy.rs`
- Deploy via self-swap pattern: download as `rc-agent-new.exe`, bat script kills→renames→restarts
- HKLM Run key for auto-start at Windows login (`start-rcagent.bat` per pod)
- Email alerts with rate limiting — `email_alerts.rs`
- Config validation at startup — DEPLOY-01, DEPLOY-04
- `CoreToAgentMessage` enum: rich typed protocol, Ping/Pong, all lifecycle messages
- Rolling deploy with Pod 8 canary-first established pattern

The milestone adds NEW self-healing on top of this foundation. Every feature below is additive.

**Incident motivation (Mar 15, 2026):** 4-hour debugging session. Pods 1/3/4 offline due to:
1. exec slot exhaustion (no visibility into which commands held slots)
2. Missing firewall rules (CRLF-damaged batch file silently failed to apply them)
3. CRLF-damaged batch files from Windows-style line endings in the write endpoint
4. rc-agent crash with no auto-restart (HKLM Run key does not restart on crash)
5. No remote diagnostics once HTTP port 8090 was blocked by the broken firewall

Every P1 feature below eliminates one or more of these five root causes.

---

## Feature Landscape

### Table Stakes (Ops Teams Expect These)

Features any ops team managing a Windows device fleet takes for granted. Missing these means
the system is not operationally viable — any fleet manager who sees these absent would
say the product is not production-ready.

| Feature | Why Expected | Complexity | Dependency on Existing | Notes |
|---------|--------------|------------|----------------------|-------|
| **Service auto-restart on crash** | Windows service failure actions have existed since Win2000. Any fleet agent that dies and stays dead is not a fleet agent. The SCM restart action is the first thing any ops engineer checks when setting up a Windows service. | LOW | Replaces HKLM Run key which has no crash restart capability. Requires Windows Service registration. Does not change rc-agent code. | Use a service wrapper (shawl, WinSW, or NSSM). NSSM is abandoned (last release 2017). shawl (Rust, MIT, mtkennerly/shawl) is the best fit for a Rust shop — wraps any exe as a service, handles ctrl-C. Configure failure actions via `sc.exe failure RCAgent reset=3600 actions=restart/5000/restart/30000/restart/60000`. |
| **Startup self-check before connecting to racecontrol** | Before announcing presence, verify own prerequisites. This is the "health check before accepting traffic" pattern from AWS Builders Library. Any distributed system that connects to a coordinator without verifying its own state causes cascading confusion. | LOW | Extends existing `config.rs` startup validation. New checks: registry key present, firewall rule present, bat file not CRLF-corrupted. | Pattern: startup phase returns `StartupHealth { ok: bool, anomalies: Vec<String> }`. If anomalies found, attempt repair. If repair fails, report to racecontrol before proceeding. This prevents a pod from appearing "registered" while actually broken. |
| **Remote exec over existing WebSocket** | When firewall blocks HTTP port 8090, management fails. WebSocket connection is already established and authenticated — routing commands over it is the universal pattern (Kubernetes kubectl exec, AWS Systems Manager, Ansible over SSH). No ops team expects management to stop working because a firewall rule is missing. | MEDIUM | Uses existing `CoreToAgentMessage` enum. Add `Exec { request_id: String, cmd: String, timeout_ms: u64 }` variant. Add `ExecResult { request_id: String, success: bool, exit_code: Option<i32>, stdout: String, stderr: String }` to `AgentMessage`. racecontrol routes by pod_id (already does this for all CoreToAgentMessage variants). | Both crates must be rebuilt and redeployed. Command execution logic copies existing `remote_ops.rs` exec handler — same semaphore-gated, `CREATE_NO_WINDOW`, timeout-wrapped pattern. Correlation by `request_id` UUID matches responses to requests without message ordering assumptions. |
| **Deploy verification with automatic rollback gate** | The existing deploy sequence checks health at 5/15/30/60s. That is correct. What is missing is the response to failure: currently, failure sends an email and stops. Every deployment system (Kubernetes, CodeDeploy, Octopus Deploy) uses health check failure as an automatic rollback trigger on canary deployments. | MEDIUM | Extends existing `deploy.rs` VERIFY_DELAYS logic. Requires: keeping `rc-agent-prev.exe` alongside `rc-agent.exe` on the pod (modified `do-swap.bat`). If 60s check fails, send rollback Exec command over WebSocket (since HTTP may be blocked at this point). | Gate on Pod 8 canary: Pod 8 must pass 60s check before racecontrol proceeds to pods 1-7. If Pod 8 rolls back, fleet deploy aborts and email fires. This converts the existing canary pattern from convention into a safety mechanism. |
| **Firewall rules applied in Rust, not batch files** | CRLF-damaged batch files silently failing to apply firewall rules was a direct root cause of the Mar 15 incident. Batch files are fragile: they can be CRLF-corrupted, they can fail silently, they depend on working netsh which can itself be blocked. Moving to Rust `Command::new("netsh")` at startup eliminates the entire failure mode. This is what any ops engineer would recommend after a CRLF-caused outage. | LOW | New module in rc-agent, runs at startup before WebSocket connect attempt. Uses `std::process::Command` to invoke `netsh advfirewall firewall add rule`. Does NOT depend on Windows Service (netsh can be called from any elevated context). | Two rules: ICMP echo (ping) + TCP 8090 (remote ops inbound). Idempotent: check with `netsh advfirewall firewall show rule name="..."` first, add only if missing. Running as SYSTEM (via service wrapper) satisfies the elevation requirement. |
| **Startup error reporting before crash** | If rc-agent panics during init, the failure is invisible to racecontrol. The pod appears offline with no diagnostic information. Distributed systems fail loudly: report last known error before exiting. This is standard practice for any agent that manages critical infrastructure. | MEDIUM | New: structured startup phase with named stages. If a stage fails, POST to racecontrol `/api/agent/startup-error` (new endpoint) with `{ pod_id, phase, error, timestamp }` before exiting. Uses existing `reqwest` client. | If WebSocket not yet established (startup failure happens before connect), use HTTP POST directly to racecontrol. If racecontrol unreachable, write structured JSON to `C:\RacingPoint\startup-error.json` for manual retrieval via remote_ops `/file` endpoint. |

### Differentiators (Competitive Advantage for Racing Point Operations)

Features that go beyond the minimum and match the specific operational reality: 8-pod venue,
one ops person (Uday), mobile-first management, no on-site IT staff available at all times.

| Feature | Value Proposition | Complexity | Dependency on Existing | Notes |
|---------|-------------------|------------|----------------------|-------|
| **Config self-heal: detect and repair missing files** | Pods 1/3/4 went offline on Mar 15 partly due to missing/corrupted files and registry keys. Auto-repair means a freshly imaged pod returns to operational without physical intervention. This is desired-state enforcement (Chef/Puppet/Ansible philosophy) applied to Windows endpoints. | MEDIUM | Extends startup self-check. Check list: `rc-agent-podN.toml` (present, parses), `start-rcagent.bat` (present, LF not CRLF), HKLM Run key (exists with correct path), `C:\RacingPoint\` directory structure. Repair using embedded templates via `include_str!()`. | Embed default toml + bat templates as string literals in rc-agent binary. On startup, if file missing or CRLF-corrupted, write correct version from embedded template and log the anomaly. Report via `AgentMessage::StartupReport`. |
| **Fleet health dashboard for Uday's phone** | Uday manages from his phone. Without a visual overview, every pod problem requires either checking racecontrol logs (not phone-friendly) or physical inspection. A mobile-friendly dashboard with pod status, uptime, and last crash time is the minimum viable ops view. Fleet management tools (Geotab, Verizon Connect, Lytx) all show this as their primary screen. | MEDIUM | Uses existing `DashboardEvent::PodUpdate` and `PodList`. Gap: no dedicated mobile-friendly view. Requires: add `agent_version`, `last_restart_time`, `last_crash_time` fields to `PodInfo`. Render as status cards in `/fleet` Next.js route. | Color coding: green = healthy WS + heartbeat within 6s, yellow = restarted in last 15 min or version mismatch, red = offline >5 min or RecoveryFailed state. 10s auto-refresh. No new backend protocol needed — just field additions to PodInfo. |
| **Agent version visible in heartbeat and dashboard** | Without per-pod version display, deploying to 8 pods has no verification that all pods actually updated. Version drift (some pods on old binary, some on new) is silent. This is the "did it actually work on all 8 pods?" check. | LOW | Extend `PodInfo` struct in rc-common with `agent_version: Option<String>`. Populate in rc-agent Register and Heartbeat messages using `env!("CARGO_PKG_VERSION")`. Display in fleet dashboard pod card. | One field addition. Low risk. Required to make the fleet dashboard useful. If version after deploy does not match expected, show yellow alert on that pod. |
| **Exec slot visibility in health endpoint** | The Mar 15 incident included exec slot exhaustion. The semaphore exists (4 slots) but there is no visibility into which commands are holding slots or for how long. Without this, diagnosing a frozen pod requires guessing. | LOW | Extends existing `/health` endpoint response in `remote_ops.rs`. Add `exec_queue: Vec<{ cmd_preview: String, elapsed_ms: u64 }>` to health JSON. Track start time per acquired permit. | A command held >30s (expected max for any command) is a diagnostic signal. Surface in dashboard. Adding per-command timing to semaphore acquisition is ~10 lines of Rust. |
| **WebSocket exec with request_id correlation** | The HTTP exec endpoint blocks for the full command duration. Long-running commands (curl binary download) hold exec slots and have a hard 10s timeout. WebSocket exec with request_id decouples command dispatch from response handling — racecontrol can fire commands and process responses asynchronously without blocking. | MEDIUM | This is the async pattern for WS exec. `request_id` UUID generated by racecontrol. Agent matches response to pending request map. Timeout handled client-side: if no ExecResult within `timeout_ms + 5000ms`, racecontrol marks it timed out. | This is the preferred implementation of WS exec — not just adding exec to WS but using the correlation pattern that allows multiple outstanding commands to a single pod. More robust than fire-and-forget. |

### Anti-Features (Commonly Requested, Often Problematic)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| **Automatic reboot on any crash** | "If nothing else works, reboot." Maximum self-healing. | On a gaming PC mid-session, a reboot destroys a customer's game and billing. Windows Service failure action "Restart Computer" (the third action) is the nuclear option. It should never fire automatically in a customer-facing system. | Use service restart (process-level) for up to 3 attempts. Only human-triggered reboot via kiosk power controls or WoL. Existing power controls in kiosk already cover this. |
| **Continuous config file monitoring (inotify-style)** | "Watch all config files and repair on any change." | Creates a feedback loop: repair writes trigger change events, which trigger more repairs. Also, polling file system every second wastes I/O on a gaming PC running a sim at 60fps. | Startup-time check only. Config is only missing after OS reinstall or human error — both require a restart to manifest. Startup check is sufficient. |
| **Full Windows Service implementation in rc-agent** | "Native service is cleaner — write the SCM dispatcher in rc-agent itself." | rc-agent is a GUI process with a lock screen window. Windows Services run in Session 0 and cannot show UI. Writing a full service dispatcher in rc-agent forces a Session 0/Session 1 split that defeats the purpose of the HKLM Run key + service wrapper hybrid. | Use a lightweight service wrapper (shawl, ~2MB binary) that handles SCM communication while rc-agent stays as a normal Session 1 process. The wrapper and the agent are separate executables. |
| **WebSocket exec output streaming (real-time stdout)** | "I want to see curl download progress live." Better DX. | Streaming output over WebSocket requires: partial line buffering, ordering guarantees, backpressure on slow dashboard connections, and cleanup when dashboard disconnects mid-stream. This is 3x the complexity of request/response exec. Not needed for the stability goal of v4.0. | Use request/response (non-streaming) WS exec for v4.0. For download progress, use a two-step: trigger download (returns immediately), then poll `/health` exec_queue field for slot occupancy. Streaming exec is a v5.0 feature. |
| **Centralized push-config management** | "Push configuration updates to all pods from racecontrol." | The existing `CoreToAgentMessage::Configure { config_json }` already provides this protocol primitive. Building a separate config management system duplicates the protocol and adds a management layer that is not needed at 8-pod scale. | Extend the existing Configure message and config.rs validation. The gap is not the protocol — it is that the agent does not verify and repair config at startup. Fix the startup check, not the protocol. |
| **Crash dump collection and minidump analysis** | "When rc-agent crashes, capture a minidump for diagnosis." | Windows minidump collection requires WER registration or DbgHelp API. Parsing minidumps requires a symbol server and a debugger. This is debugger tooling, not ops tooling. The setup overhead exceeds the diagnostic value for a 8-pod venue. | Use structured startup error reporting: name each startup phase, catch panics at the phase level, report last known phase + error to racecontrol before exiting. This delivers 90% of the diagnostic value at 5% of the complexity. |
| **Agent self-update (pull new binary on version mismatch)** | "Agent should update itself when a new version is deployed." | The self-swap deploy pattern already does this from racecontrol side. An agent that spontaneously self-updates outside of the controlled deploy sequence creates race conditions with billing sessions and skips the Pod 8 canary gate. | Use the existing racecontrol-orchestrated deploy with self-swap. The deploy is initiated by staff from kiosk — this is intentional. Autonomous self-update skips the human review gate. |

---

## Feature Dependencies

```
[Windows Service wrapper (SVC-01)]
    └──enables──> [Auto-restart on crash (SVC-02)]
    └──enables──> [Runs as SYSTEM → firewall rules apply without UAC (FWALL-01)]
    └──enables──> [Registry writes succeed without UAC (CFGHEAL-01)]

[Firewall auto-configuration in Rust (FWALL-01)]
    └──unblocks──> [HTTP remote ops on port 8090 (existing)]
    └──unblocks──> [WebSocket exec usable as fallback (WS-EXEC-01)]
    └──requires──> [SYSTEM context or pre-existing elevation (satisfied by SVC-01)]

[Startup self-check (STARTUP-01)]
    └──enables──> [Config self-heal (CFGHEAL-01)]
    └──enables──> [Startup error reporting (ERR-01)]
    └──runs before──> [WebSocket connect to racecontrol]

[Config self-heal (CFGHEAL-01)]
    └──requires──> [Startup self-check (STARTUP-01)]
    └──requires──> [SYSTEM context for registry writes (SVC-01)]
    └──requires──> [Embedded templates in binary (include_str!)]
    └──reports via──> [AgentMessage::StartupReport (PROTO-03)]

[Startup error reporting (ERR-01)]
    └──requires──> [HTTP POST to racecontrol OR local file fallback]
    └──requires──> [New racecontrol endpoint: POST /api/agent/startup-error]
    └──runs on──> [startup phase failure, before process exit]

[WebSocket exec (WS-EXEC-01)]
    └──requires──> [CoreToAgentMessage::Exec { request_id, cmd, timeout_ms } (PROTO-01)]
    └──requires──> [AgentMessage::ExecResult { request_id, success, exit_code, stdout, stderr } (PROTO-02)]
    └──requires──> [racecontrol: pending-request map keyed by request_id]
    └──enables──> [Deploy rollback when HTTP blocked (ROLL-01)]
    └──enables──> [Management when firewall has not yet been repaired]

[Deploy rollback (ROLL-01)]
    └──requires──> [Modified do-swap.bat: rename current→prev before swap (ROLL-02)]
    └──requires──> [WebSocket exec (WS-EXEC-01) — HTTP may be blocked during failed deploy]
    └──requires──> [Existing VERIFY_DELAYS 60s gate in deploy.rs]
    └──enhances──> [Pod 8 canary pattern — converts from convention to safety gate]

[Agent version in PodInfo (VER-01)]
    └──requires──> [PodInfo struct field: agent_version: Option<String>]
    └──enhances──> [Fleet health dashboard (DASH-01)]

[Fleet health dashboard (DASH-01)]
    └──requires──> [agent_version in PodInfo (VER-01)]
    └──requires──> [last_restart_time, last_crash_time in PodInfo (new fields)]
    └──uses──> [Existing DashboardEvent::PodUpdate push (no new protocol)]
    └──renders in──> [New /fleet Next.js route in kiosk]

[AgentMessage::StartupReport (PROTO-03)]
    └──requires──> [New racecontrol handler: persist anomalies, surface in dashboard]
    └──displayed in──> [Fleet health dashboard pod card (DASH-01)]
```

### Dependency Notes

- **Windows Service is the prerequisite chain opener:** It enables SYSTEM context, which enables reliable firewall rules and registry writes. Everything in self-healing config depends on having SYSTEM-level privileges. The service wrapper is the first thing to implement.
- **WebSocket exec requires protocol changes in rc-common:** Both racecontrol and rc-agent must be rebuilt and redeployed simultaneously. This is a two-crate atomic change — ship it as a single commit.
- **Deploy rollback depends on prev-binary existing:** The modified `do-swap.bat` must be deployed to all pods BEFORE rollback can function. Rollback without a prev binary is a no-op. The bat file update must be part of the same deploy that enables rollback in racecontrol.
- **Fleet dashboard needs two new PodInfo fields:** `last_restart_time` and `last_crash_time` are captured in pod_monitor.rs state transitions but not currently persisted in PodInfo. These must be added to rc-common types (PodInfo struct) and populated by pod_monitor.rs.

---

## MVP Definition

This is v4.0 on an already-shipped product. "MVP" means: minimum set that eliminates
the five root causes of the Mar 15 4-hour outage.

### Launch With (v4.0 Core — eliminates Mar 15 root causes)

- [ ] **Windows Service wrapper** — Eliminates root cause #4 (no crash restart). HKLM Run key survives reboots, not crashes. Service failure actions restart the process. LOW complexity, HIGH urgency.
- [ ] **Firewall auto-configuration in Rust** — Eliminates root cause #2 and #3 (CRLF-damaged batch file silently broke firewall rules). LOW complexity, HIGH urgency.
- [ ] **Config self-heal at startup** — Eliminates root cause #3 broadly (missing/corrupted files). Detect and repair on every startup. MEDIUM complexity, HIGH urgency.
- [ ] **Startup error reporting** — Eliminates root cause #5 partially (no remote diagnostics on crash). Report phase + error before exit. MEDIUM complexity.
- [ ] **WebSocket exec (request/response with request_id)** — Eliminates root cause #5 fully (management fails when HTTP blocked). MEDIUM complexity. Requires protocol changes in rc-common.

### Add After Core (v4.0 Phase 2 — operational visibility)

- [ ] **Deploy rollback on 60s health gate failure** — Prevents bad deploys from leaving pods offline. Requires prev-binary pattern + rollback trigger. MEDIUM complexity.
- [ ] **Agent version in heartbeat and dashboard** — Verifies all 8 pods actually updated after deploy. LOW complexity. Required for next item to be useful.
- [ ] **Fleet health dashboard for Uday's phone** — Single-screen ops view. Real-time pod status, uptime, last crash, version. MEDIUM complexity for new Next.js route.

### Future Consideration (v5.0)

- [ ] **WebSocket exec output streaming** — Improves DX for long-running commands. HIGH complexity. Defer until request/response exec is stable.
- [ ] **Pod uptime trend (7-day heatmap)** — Nice to have history. New DB table + UI. Defer until fleet is stable.
- [ ] **Exec slot visibility in dashboard** — Diagnostic tool for slot exhaustion debugging. LOW complexity but LOW urgency post-v4.0.

---

## Feature Prioritization Matrix

| Feature | Ops Value | Implementation Cost | Priority |
|---------|-----------|---------------------|----------|
| Windows Service wrapper | HIGH — eliminates no-crash-restart | LOW — shawl/sc.exe, no rc-agent code change | P1 |
| Firewall auto-config in Rust | HIGH — eliminates CRLF failure mode | LOW — 20 lines Rust, idempotent netsh calls | P1 |
| Config self-heal at startup | HIGH — eliminates silent missing-file failures | MEDIUM — file checks, template embedding, winreg | P1 |
| Startup error reporting | HIGH — silent crash becomes visible alert | MEDIUM — pre-exit HTTP POST + file fallback | P1 |
| WebSocket exec (request/response) | HIGH — management works when HTTP blocked | MEDIUM — protocol changes, handler in both crates | P1 |
| Deploy rollback | HIGH — bad deploy auto-reverses on canary | MEDIUM — prev-binary + rollback trigger | P2 |
| Agent version in heartbeat | MEDIUM — verifies deploy success fleet-wide | LOW — 1 field in PodInfo struct | P2 |
| Fleet health dashboard | HIGH — Uday sees status from phone | MEDIUM — new Next.js route, 2 new PodInfo fields | P2 |
| Exec slot visibility | MEDIUM — diagnose slot exhaustion | LOW — extend /health endpoint | P2 |
| WebSocket exec streaming | LOW — DX improvement | HIGH — buffering, ordering, backpressure | P3 |
| Pod uptime trend | LOW — historical ops data | MEDIUM — new DB table + chart | P3 |

**Priority key:**
- P1: Must have for v4.0 — directly eliminates Mar 15 root causes
- P2: Should ship in v4.0 — adds operational visibility and resilience
- P3: Defer to v5.0 — nice to have, not stability-critical

---

## Capability Deep-Dive

### 1. Service Crash Recovery on Windows

**What ops teams expect:** Windows Service failure actions are standard. The SCM supports up to three failure actions: first failure, second failure, and subsequent failures. Each action can be: restart service (most common), run a program, or restart computer. The reset period determines how long before the failure count resets to 0.

**Industry standard configuration for a critical service:**
```
sc.exe failure RCAgent reset=3600 actions=restart/5000/restart/30000/restart/60000
```
Translation: restart after 5s on first failure, 30s on second, 60s on third. Reset failure count after 3600s (1 hour of stability).

**Options ranked by fit for this project:**
1. **shawl** (Rust, MIT, mtkennerly/shawl, v1.7.0 Jan 2025) — wraps any exe as a Windows service, handles ctrl-C/SIGTERM. Best fit: no rc-agent code changes, actively maintained, Rust-native. ~2MB binary added to pod-deploy kit.
2. **WinSW** (Java-based, XML config, v2.12 2023) — actively maintained but requires JRE on pods. Pods may not have Java. Worse fit.
3. **NSSM** (C, last release 2017, abandoned) — stable but unmaintained. No future security patches. Worse fit.
4. **windows-service crate in rc-agent** — write SCM dispatcher natively. Conflicts with Session 1 GUI requirement (services run Session 0). Highest complexity, worst fit.

**Session 0/Session 1 note:** Windows Services run in Session 0 (no GUI). rc-agent has a lock screen window (GUI, Session 1). The hybrid works: shawl is the service (Session 0, handles SCM), the existing HKLM Run key starts rc-agent in Session 1 at first user login. On crash, shawl restarts rc-agent as... a console process in Session 0 (no window). This is a known limitation of the hybrid approach — the restart gets GUI only after the next user logout/login cycle. **Mitigation:** shawl can be configured to spawn the process into the active user session using `--pass-start-args`. This requires evaluation. Alternative: if the service wrapper cannot reliably restart in Session 1, a separate watchdog that monitors the Session 1 process is the fallback.

**Complexity:** LOW for service install and failure actions. MEDIUM if Session 1 restart is required (needs testing with shawl's session flags).

### 2. Remote Exec over WebSocket

**What ops teams expect:** Management channels stay operational even when managed nodes have network issues. Kubernetes `kubectl exec`, AWS SSM Session Manager, and Ansible over SSH all route commands through existing authenticated channels rather than opening new ports.

**Protocol design:**
```rust
// Add to CoreToAgentMessage:
Exec {
    request_id: String,   // UUID, generated by racecontrol
    cmd: String,          // Command string (same as HTTP /exec)
    timeout_ms: u64,      // Default 10000, overridable
}

// Add to AgentMessage:
ExecResult {
    request_id: String,   // Echoed back for correlation
    success: bool,
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
}
```

**racecontrol side:** Maintain a `HashMap<String, oneshot::Sender<ExecResult>>` per pod in AppState. When Exec is sent, insert a pending entry. When ExecResult arrives from the agent, look up and resolve the sender. If timeout elapses without ExecResult, remove from map and return timeout error to caller.

**rc-agent side:** Receive Exec in the CoreToAgentMessage handler. Execute using the same logic as `remote_ops.rs::exec_command` (same semaphore, same CREATE_NO_WINDOW, same timeout). Send ExecResult back via the WebSocket sender.

**Timeout handling:** Agent-side timeout is `timeout_ms`. Client-side timeout in racecontrol is `timeout_ms + 5000ms` (buffer for network RTT). If client timeout fires, ExecResult may still arrive — discard it (no pending entry in map).

**Streaming deferral rationale:** Streaming stdout line-by-line requires: partial line buffering in the agent, ordered delivery guarantees (WebSocket does NOT guarantee message ordering under reconnection), backpressure if the dashboard consumer is slow, and cleanup if the dashboard disconnects mid-stream. This is a separate feature with 3x the complexity of request/response. Start with request/response for v4.0.

**Complexity:** MEDIUM. Protocol changes require both crates rebuilt. Execution logic is a copy of existing code. Main risk: reconnection during a pending Exec (the response arrives on a new WebSocket connection after reconnect). Mitigation: pending requests time out on the racecontrol side; the agent retries nothing (fire and forget on the agent's end).

### 3. Self-Healing Configuration

**What ops teams expect:** Configuration management tools (Chef, Puppet, Ansible) run on every startup and converge the system to desired state. The expectation is not "config is correct" but "if config is wrong, it will be corrected." This is idempotent desired-state enforcement.

**What to check on every rc-agent startup (before WebSocket connect):**

| Check | Method | Repair |
|-------|--------|--------|
| `C:\RacingPoint\rc-agent-podN.toml` exists | `fs::metadata()` | Write from embedded template, then exit with error (need pod number to generate correct config — this is a one-time setup) |
| TOML parses without error | `toml::from_str()` | If parse fails, rename to `.bak`, write from embedded template, log anomaly |
| `start-rcagent.bat` exists and is LF-terminated | `fs::read()` + scan for 0x0D 0x0A | If CRLF detected, rewrite with LF. This was the Mar 15 root cause. |
| HKLM Run key `RCAgent` exists with correct path | `winreg::RegKey::open_subkey()` | Write via `RegKey::set_value()`. Requires SYSTEM context (satisfied by service wrapper). |
| Firewall rule TCP 8090 exists | `netsh advfirewall firewall show rule name="RCAgent-RemoteOps"` | `netsh advfirewall firewall add rule ...` |
| Firewall rule ICMP echo exists | Same pattern | Same repair |

**Embedded templates:** Use `include_str!()` for bat file template. TOML template cannot embed pod number — if toml is missing, report anomaly and continue with defaults (do not exit). The bat file template is universal across all pods.

**Anomaly reporting:** Send `AgentMessage::StartupReport { pod_id, anomalies: Vec<String>, repairs: Vec<String> }` once WebSocket connected. racecontrol persists this and displays in fleet dashboard as a warning badge on the pod card.

**Complexity:** MEDIUM. Requires: `winreg` crate (check if already in dependency tree — it may be via registry-based pod lockdown code), embedded templates, structured startup phase enum. No new async complexity.

### 4. Deployment Rollback

**What ops teams expect:** Canary deployments are only meaningful if failure stops the rollout and reverses the canary. Kubernetes stops a rolling deployment when readiness probes fail. AWS CodeDeploy rolls back on alarm. The pattern is universal: health gate → pass/fail → proceed or revert.

**Two-part implementation:**

**Part A — Modified do-swap.bat** (runs on the pod):
```batch
:: Before killing current binary, preserve it as prev
if exist C:\RacingPoint\rc-agent.exe (
    copy /Y C:\RacingPoint\rc-agent.exe C:\RacingPoint\rc-agent-prev.exe
)
:: Existing swap logic follows...
```

**Part B — racecontrol rollback trigger** (in deploy.rs, at 60s health gate):
If the 60s check fails:
1. Log rollback trigger with reason
2. Send `CoreToAgentMessage::Exec` with rollback command: `copy /Y C:\RacingPoint\rc-agent-prev.exe C:\RacingPoint\rc-agent.exe && taskkill /F /IM rc-agent.exe && start "" C:\RacingPoint\start-rcagent.bat`
3. Wait 30s, re-run health check
4. If rollback succeeds: update DeployState to `RolledBack`, send email alert
5. If rollback fails: update DeployState to `RollbackFailed`, send critical alert

**Fleet rollback gate:** If Pod 8 (canary) rolls back, abort the fleet deploy. Do not proceed to pods 1-7. This requires pod 8 to be explicitly tracked as canary in the deploy sequence (it already is by convention — make it explicit in code).

**Complexity:** MEDIUM. Two independent pieces: bat file modification (simple, LOW) and racecontrol rollback branch (MEDIUM — depends on WebSocket exec being available when HTTP is down).

### 5. Fleet Health Dashboard

**What ops teams expect:** Any ops dashboard for a device fleet shows: device identifier, online/offline status, uptime, software version, last-seen timestamp, and recent error count. This is the minimum for an ops person to triage "what is broken and why" without SSH access. Fleet management tools (Geotab, Lytx, Verizon Connect) all show this as their primary view.

**For Uday's phone specifically:**
- Mobile-first card layout (not a table — tables scroll awkwardly on phones)
- 8 pod cards in a 2-column grid
- Per-card: pod number, status color, game state, uptime, agent version, last restart time
- Summary row: "X/8 pods online" at top
- 10-second auto-refresh (or WebSocket push via existing DashboardEvent)
- Tap a pod card to see: recent anomalies, startup reports, exec slot status

**Alert thresholds (based on fleet management norms):**
| Threshold | Indicator | Rationale |
|-----------|-----------|-----------|
| Pod offline > 5 minutes | RED | A pod offline during business hours needs immediate attention |
| Pod restarted in last 15 min | YELLOW | Recent crash is a warning signal even if now healthy |
| Agent version mismatch after deploy | YELLOW | Deploy succeeded partially — some pods still on old version |
| Exec slot held > 30s | YELLOW | A command is stuck; no new commands can run once all 4 slots fill |
| Pod in RecoveryFailed state | RED | watchdog gave up trying to restart — needs human |
| Startup anomalies reported | YELLOW | Config was corrupted and repaired — investigate root cause |

**New fields needed in PodInfo (rc-common types.rs):**
- `agent_version: Option<String>` — populated from `CARGO_PKG_VERSION` in rc-agent
- `last_restart_time: Option<DateTime<Utc>>` — set by pod_monitor.rs when watchdog restarts
- `last_crash_time: Option<DateTime<Utc>>` — set by pod_monitor.rs on crash detection
- `startup_anomalies: Vec<String>` — last startup report

**Complexity:** MEDIUM for the Next.js dashboard route. LOW for the PodInfo field additions. The main work is design: making 8 pod cards usable on a 390px wide phone screen.

---

## Sources

- Windows Service failure actions: [sc failure — Microsoft Learn](https://learn.microsoft.com/en-us/previous-versions/windows/it-pro/windows-server-2012-r2-and-2012/cc742019(v=ws.11))
- Windows Service recovery configuration: [Understanding Windows Services Recovery — ServerBrain](https://www.serverbrain.org/system-administration/understanding-windows-services-recovery-features.html)
- shawl (Rust service wrapper, v1.7.0): [mtkennerly/shawl — GitHub](https://github.com/mtkennerly/shawl)
- NSSM vs WinSW vs shawl comparison: [Servy vs NSSM vs WinSW — DEV Community](https://dev.to/aelassas/servy-vs-nssm-vs-winsw-2k46)
- windows-service crate (mullvad): [docs.rs/windows-service](https://docs.rs/windows-service/latest/windows_service/)
- WebSocket exec pattern (Kubernetes): [WebSocket exec in Kubernetes pods — Jason Stitt](https://jasonstitt.com/websocket-kubernetes-exec)
- Health check implementation patterns: [Implementing health checks — AWS Builders Library](https://aws.amazon.com/builders-library/implementing-health-checks/)
- Self-healing architecture: [Azure Well-Architected — Self-preservation strategies](https://learn.microsoft.com/en-us/azure/well-architected/reliability/self-preservation)
- Automated rollback on health check failure: [Automated Rollback Procedures — OneUptime (2026)](https://oneuptime.com/blog/post/2026-02-09-automated-rollback-health-failures/view)
- Canary deployment patterns: [Canary Deployments — Netdata](https://www.netdata.cloud/academy/canary-deployment/)
- Fleet health dashboard table stakes: [Fleet Management Dashboard — PCS Software](https://pcssoft.com/blog/fleet-management-dashboard/)
- Self-healing device fleet patterns: [Self-Healing Devices — Radix International](https://radix-int.com/self-healing-devices-future-intelligent-fleet-management/)
- Existing codebase analyzed: `crates/rc-agent/src/remote_ops.rs`, `crates/racecontrol/src/deploy.rs`, `crates/racecontrol/src/pod_monitor.rs`, `crates/rc-common/src/protocol.rs`, `.planning/PROJECT.md`

---

*Feature research for: RaceControl v4.0 Pod Fleet Self-Healing*
*Researched: 2026-03-15*
