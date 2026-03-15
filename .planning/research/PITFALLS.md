# Pitfalls Research

**Domain:** Pod Fleet Self-Healing — Windows Service, WebSocket Exec, Firewall Auto-Config, Self-Update, Small-Scale Fleet Management (8 pods, Rust/Axum/Windows)
**Researched:** 2026-03-15
**Confidence:** HIGH — all critical pitfalls directly observed during the Mar 15, 2026 4-hour outage (Pods 1/3/4 offline), verified against the actual codebase (remote_ops.rs, deploy.rs, lock_screen.rs), and cross-referenced with official Microsoft documentation, tokio process docs, and Windows Defender/AV reports.

---

## Critical Pitfalls

### Pitfall 1: Windows Service Runs in Session 0 — GUI Is Invisible

**What goes wrong:**
Installing rc-agent as a Windows Service (via NSSM or the native Service API) causes the process to run in Session 0 — the isolated, non-interactive session reserved for services since Windows Vista. Session 0 has no user desktop. Edge.exe launched from Session 0 renders into an invisible WinStation. The lock screen (fullscreen Edge kiosk + local HTTP on port 18923) simply does not appear on the pod monitor. The process is running and healthy by every metric, but the customer sees a black screen.

**Why it happens:**
The instinct when adding crash-restart is "install as a Windows Service." Services give SCM recovery actions (restart on failure) and run before login. The trap: rc-agent is not a pure background daemon — it owns the user-facing lock screen. "Allow service to interact with desktop" was a historical workaround that was permanently disabled in Windows 10 version 1803. The UI0Detect service (Interactive Services Detection, the bridge that used to surface Session 0 UI) was removed in Windows 10 1803 / Server 2019. Enabling the checkbox in Services.msc on Windows 11 has zero effect.

**How to avoid:**
Use a two-layer architecture:
1. A **minimal watchdog service** (NSSM, native SC, or built-in service) runs in Session 0. Its sole job: detect rc-agent death and restart it in the user session.
2. rc-agent itself **always runs in Session 1** (the logged-in user's session).

The simplest Session-1-restart mechanism for this codebase is Task Scheduler:
```bat
schtasks /create /tn "RCAgent" /tr "C:\RacingPoint\rc-agent.exe" /sc ONLOGON /ru %USERNAME% /rl HIGHEST /f
```
The watchdog service, on detecting rc-agent absent, runs:
```bat
schtasks /run /tn "RCAgent"
```
This avoids all Win32 session token API complexity (`WTSGetActiveConsoleSessionId`, `WTSQueryUserToken`, `CreateProcessAsUser`) and correctly targets Session 1. If no user is logged in, the task refuses to start — which is correct behaviour (no GUI needed when no user is present).

**Warning signs:**
- rc-agent.exe is visible in Task Manager, uptime increasing, but lock screen never appears on pod display.
- `tasklist /v /fi "IMAGENAME eq rc-agent.exe"` shows Session# = 0.
- Port 18923 `/health` returns connection refused despite process running.
- `tasklist /v` shows `Console` in the Session Name column — this is Session 0, not the user's session.

**Phase to address:**
Phase implementing Windows Service / auto-restart. The watchdog-service + Task-Scheduler-restart pattern must be explicitly designed and tested before any pods are migrated. Simulate a crash on Pod 8 (canary) and verify rc-agent restarts in Session 1 with the lock screen visible.

---

### Pitfall 2: CRLF Line Endings Silently Break All Batch File Commands

**What goes wrong:**
Batch files (.bat) created with LF-only line endings (written by the Claude Write tool, Linux editors, or git with `core.autocrlf=false`) fail silently on Windows cmd.exe. Commands like `set`, `netsh`, `taskkill`, and `start` appear to execute but either do nothing or produce garbage output. No error is raised — the shell tokenises the entire file as a single line because cmd.exe splits on `\r\n`, not `\n`.

This was the root cause of the Mar 15 outage: `do-swap.bat`, firewall scripts, and startup scripts all contained LF-only endings. `netsh` received a single line of text with embedded `\n` characters instead of a sequence of commands. Firewall rules were never applied. Ports were never opened. Pod 3 became unreachable.

**Why it happens:**
All modern editors and developer tools default to LF. The Write tool writes LF. Git on cross-platform repos may strip CRLFs. The failure is entirely silent — cmd.exe does not error on LF-only files; it simply misparses them.

**How to avoid:**
Any Rust code that writes a .bat file must join lines with `\r\n`, not `\n`:
```rust
let bat_lines = vec![
    "@echo off",
    "timeout /t 3 /nobreak",
    "taskkill /F /IM rc-agent.exe",
    // ...
];
let bat_content = bat_lines.join("\r\n") + "\r\n";
fs::write("C:\\RacingPoint\\do-swap.bat", bat_content)?;
```

The current `do-swap.bat` generation in deploy.rs uses a single inline `echo ... > do-swap.bat` chain — cmd.exe generates this file with correct CRLF endings because cmd.exe is writing it. This pattern is safe. The danger is **any path where Rust writes bytes directly to a .bat file** via `fs::write()`, the `/write` HTTP endpoint, or the planned firewall auto-config Rust code.

Add a unit test in the firewall module:
```rust
#[test]
fn bat_file_uses_crlf() {
    let content = generate_firewall_bat();
    assert!(content.contains("\r\n"), "bat file must use CRLF line endings");
    assert!(!content.contains("\r\n\r\n"), "no double CRLF");
}
```

**Warning signs:**
- `netsh advfirewall firewall show rule name="RCAgent_8090"` returns "No rules match" immediately after a "successful" install.
- `set VARNAME=value` in a bat file results in an env var named `VARNAME=value\n` (literal newline in name).
- Batch script produces no output and exits with code 0 (appears to succeed).
- Firewall rules missing after reboot even though the install bat reported no errors.
- `certutil -dump C:\RacingPoint\do-swap.bat | findstr "0a"` shows `0a` without preceding `0d` — confirming LF-only.

**Phase to address:**
Firewall auto-config phase and any startup self-healing that generates .bat files. Must include a unit test that asserts CRLF in all generated bat content.

---

### Pitfall 3: Exec Slot Exhaustion Leaves the Pod Unmanageable

**What goes wrong:**
The `/exec` endpoint in remote_ops.rs uses a 4-slot semaphore with a default 10-second timeout. Deploy operations consume slots sequentially: download (up to 120s), size check, self-swap trigger. If rc-core sends concurrent requests to the same pod — which happens when a rolling deploy overlaps with scheduled health checks — all 4 slots can be consumed simultaneously. The semaphore uses `try_acquire()` (non-blocking), so the 5th request immediately returns HTTP 429. The pod becomes unmanageable via HTTP for the duration of the hung operations.

The Mar 15 incident (pre-fix): rc-agent processes without `CREATE_NO_WINDOW` opened cmd.exe console windows that waited for user input — never exiting. Slots were permanently consumed. The timeout was too short for downloads. Multiple overlapping deploy-retry attempts from rc-core exhausted all 4 slots within minutes.

**Why it happens:**
The acute bug (`CREATE_NO_WINDOW` missing) is already fixed in the current codebase. The remaining risk is simultaneous requests from rc-core to the same pod: rolling deploy download + concurrent health check + concurrent session status poll = 3 slots consumed in parallel, leaving only 1 for any further management.

**How to avoid:**
Three mitigations, all needed:
1. rc-core must **serialize exec requests per pod** — never send concurrent HTTP exec calls to the same pod. Use a per-pod `Mutex<()>` in rc-core's deploy coordinator.
2. Before sending any management exec, rc-core should check `exec_slots_available` in the `/health` response and back off if `< 2`.
3. The planned WebSocket exec path must use a **separate semaphore** (or no semaphore limit) since it is the recovery path when HTTP exec is exhausted. If WebSocket exec shares the HTTP semaphore, both management paths fail together.

**Warning signs:**
- `/health` on port 8090 returns `exec_slots_available: 0`.
- Deploy log shows HTTP 429 "Too many concurrent commands" for a single pod.
- Pod continues normal game session operation (rc-agent is alive) but all management commands fail.
- `exec_slots_available` does not recover to 4 after 10+ seconds — indicates a slot has leaked (process that did not die within the timeout still holding a reference).

**Phase to address:**
WebSocket exec phase. The HTTP semaphore monitoring should be added to the fleet health dashboard. WebSocket exec semaphore must be independent.

---

### Pitfall 4: Antivirus Holds File Handle During Self-Swap — Rename Fails

**What goes wrong:**
The self-swap deploy sequence: download `rc-agent-new.exe` → wait 3s → `taskkill /F /IM rc-agent.exe` → `move rc-agent-new.exe rc-agent.exe` → start. The `move` step fails with `ERROR_SHARING_VIOLATION` when Windows Defender holds a file handle on `rc-agent-new.exe` while scanning the newly downloaded binary. The scanner opens the file the moment it appears on disk.

The result is catastrophic: `rc-agent.exe` has been killed and deleted, `rc-agent-new.exe` cannot be renamed, and the pod is left with no running rc-agent and no way to recover remotely (port 8090 is down because rc-agent is gone).

**Why it happens:**
Windows Defender exclusions are configured for `C:\RacingPoint\rc-agent.exe` by path and name. The temporary staging file `rc-agent-new.exe` is a different name — it is not covered by the exclusion. Freshly compiled Rust binaries frequently trigger heuristic (ML-based) AV detection because they are unsigned, novel executables that match behavioural patterns (network connections, process spawning, registry access) common in malware.

**How to avoid:**
Three layers of protection:
1. Add `C:\RacingPoint\rc-agent-new.exe` explicitly to Windows Defender exclusions (in addition to `rc-agent.exe` and the whole `C:\RacingPoint\` directory path).
2. Extend `do-swap.bat` to retry the rename up to 5 times with a 2-second sleep between attempts:
```bat
:RETRY
move rc-agent-new.exe rc-agent.exe
if %ERRORLEVEL% NEQ 0 (
    timeout /t 2 /nobreak >nul
    set /a RETRIES+=1
    if %RETRIES% LSS 5 goto RETRY
    echo SWAP FAILED after 5 retries >> C:\RacingPoint\deploy-error.log
    exit /b 1
)
```
3. Startup self-healing must verify AV exclusions on every boot: check `HKLM\SOFTWARE\Microsoft\Windows Defender\Exclusions\Paths` and re-apply via `powershell Add-MpPreference -ExclusionPath "C:\RacingPoint"` if missing.

**Warning signs:**
- Self-swap bat exits 1 specifically on the `move` command (not `taskkill`).
- `dir C:\RacingPoint\rc-agent*.exe` shows `rc-agent-new.exe` present with correct size, `rc-agent.exe` absent.
- Event Viewer → Windows Logs → Application shows Windows Defender quarantine event timestamp matching deploy time.
- Port 8090 unreachable after a deploy that appeared to reach the "Starting" step.

**Phase to address:**
Deploy resilience phase (self-swap hardening). The retry loop is required before any production deployment of the new binary. AV exclusion verification belongs in startup self-healing phase.

---

### Pitfall 5: Firewall Rules Applied to Wrong Profile — Blocked Inbound Despite Rule Existing

**What goes wrong:**
`netsh advfirewall firewall add rule` without an explicit `profile=` parameter applies the rule to the **current active profile** of the NIC at the time the command runs. On pod PCs where the NIC is categorised as "Unidentified network" or "Public network" (common when DHCP lease is still being negotiated or when the domain controller is unreachable), the active profile is Public, not Private. Later, when Windows reclassifies the NIC to Private, the inbound allow rule is absent from the Private profile — traffic is blocked.

The Mar 15 outage: Pod 3's firewall rule was never applied (CRLF bug, Pitfall 2). When the rule was subsequently added by a manual bat file, it was applied to the wrong profile because the NIC was temporarily in Public profile during the command execution.

**How to avoid:**
Always specify `profile=any` when adding firewall rules programmatically:
```
netsh advfirewall firewall add rule name="RCAgent_8090" dir=in action=allow protocol=TCP localport=8090 remoteip=192.168.31.0/24 profile=any
```

Note the `remoteip=192.168.31.0/24` scope — restrict to the LAN subnet for security (Pitfall 10 covers this).

When implementing firewall management in Rust, always:
1. Check for rule existence first (idempotent — see Pitfall 8).
2. Apply with `profile=any`.
3. Verify after applying by re-running the show command AND by checking the active profile: `netsh advfirewall show currentprofile`.

**Warning signs:**
- `netsh advfirewall firewall show rule name="RCAgent_8090"` shows the rule exists, but port 8090 is unreachable inbound from rc-core.
- `netsh advfirewall show currentprofile` shows "Public" or "Domain" while the rule was added to "Private" or vice versa.
- After a Windows Update reboot, one or more pods become unreachable despite firewall rules nominally present.
- rc-core marks pod offline after successful deploy because post-deploy HTTP health check to port 8090 times out.

**Phase to address:**
Firewall auto-config phase. The Rust startup code must use `profile=any`, verify rule application by testing actual connectivity (not just exit code), and log the current active profile name for diagnostics.

---

### Pitfall 6: HKLM Run Key Provides No Crash Restart

**What goes wrong:**
The current mechanism — HKLM `Run` key triggers `start-rcagent.bat` at Windows login — correctly starts rc-agent in Session 1. But it is a one-shot logon trigger. If rc-agent crashes between logins, it stays dead until the next reboot. The Run key has no "restart on failure" semantics. This is exactly what happened with Pod 3 on Mar 15: rc-agent crashed at an unknown time, nobody knew, the pod appeared online on the network (remote_ops HTTP server was part of rc-agent, so even port 8090 was down), and there was no way to restart remotely.

**Why it happens:**
The HKLM Run key is the simplest one-shot startup mechanism. It was chosen as a quick fix for the Session 0 isolation problem. It solves "start in Session 1 at boot" but not "restart on crash."

**How to avoid:**
Replace the HKLM Run key with a Task Scheduler task that has restart-on-failure configured:
```bat
schtasks /create /tn "RCAgent" ^
  /tr "C:\RacingPoint\rc-agent.exe" ^
  /sc ONLOGON ^
  /ru %USERNAME% ^
  /rl HIGHEST ^
  /f
```
Then set retry policy via XML or PowerShell:
```powershell
$task = Get-ScheduledTask -TaskName "RCAgent"
$task.Settings.RestartCount = 10
$task.Settings.RestartInterval = "PT1M"  # 1-minute retry
$task | Set-ScheduledTask
```

Startup self-healing must verify the Task Scheduler task exists and has the correct restart policy on every boot, re-creating it if absent:
```bat
schtasks /query /tn "RCAgent" >nul 2>&1 || (
    schtasks /create /tn "RCAgent" /tr "C:\RacingPoint\rc-agent.exe" /sc ONLOGON /ru %USERNAME% /rl HIGHEST /f
)
```

**Warning signs:**
- rc-agent process absent from `tasklist` on a pod that last rebooted hours ago.
- Pod has no WebSocket connection to rc-core, and port 8090 is also unreachable (both are in the same binary after the pod-agent merge).
- rc-agent.log (if it exists at `C:\RacingPoint\rc-agent.log`) has no entries since the crash time.
- `schtasks /query /tn "RCAgent"` returns "ERROR: The system cannot find the path specified."

**Phase to address:**
Windows Service / auto-restart phase. Replace HKLM Run key with Task Scheduler task on all 8 pods. Test crash-restart by killing rc-agent manually on Pod 8 (canary) and verifying it restarts within 2 minutes.

---

### Pitfall 7: WebSocket Exec Output Buffering and Zombie Processes

**What goes wrong:**
When adding `CoreToAgentMessage::Exec` over the existing WebSocket channel, the natural implementation sends a command and waits for a single reply message. Long-running commands (curl downloads up to 120s, game installs, taskkill with process wait) block the response. If the WebSocket drops mid-command, two problems occur simultaneously:
1. rc-core never receives the result and does not know if the command succeeded.
2. The child process continues running on the pod (zombie from rc-core's perspective) — the pod's exec semaphore slot may remain held, and the subsequent retry command causes a second instance of the same operation.

A secondary issue: very large stdout/stderr from commands like `dir /s C:\` can produce multi-MB output. A single WebSocket frame containing multi-MB data may hit tungstenite's default 64MB limit — but more practically, it causes multi-second pauses while the frame is assembled, during which no other WebSocket messages are processed.

**Why it happens:**
The existing HTTP `/exec` endpoint collects full stdout/stderr via `cmd.output()` and returns them at once — safe for short commands. Replicating this pattern over WebSocket is correct for typical management commands but creates the issues above for deploy-class operations. The `kill_on_drop(true)` flag on `tokio::process::Command` handles the zombie issue when the Rust process holding the child drops — but if the child outlives the timeout (returns an error) and the WebSocket reconnects, the orphaned child is not tracked.

**How to avoid:**
For initial WebSocket exec implementation:
- Assign a `correlation_id: Uuid` to each exec request in `CoreToAgentMessage::Exec`. Include the same ID in `AgentMessage::ExecResult`.
- Cap stdout/stderr at 1MB in the response; include a `truncated: bool` flag.
- Set a generous per-command timeout (180 seconds for download-class commands).
- Maintain a per-pod in-flight exec map on the agent side: `HashMap<Uuid, AbortHandle>`. On WebSocket reconnect, abort all in-flight execs and report them as cancelled (not zombie).
- Exec semaphore for WebSocket exec path must be independent from the HTTP exec semaphore (separate static or per-connection state).

**Warning signs:**
- WebSocket exec for a curl download has no response for 60+ seconds — expected and normal, not a hang.
- After WebSocket reconnect, rc-core retries a command that already ran — verify with a follow-up `tasklist` or `dir` check before retrying destructive commands.
- `exec_slots_available` on HTTP `/health` drops and does not recover after a WS exec — semaphore shared between paths.
- Truncation: stdout in exec result ends mid-line followed by `[truncated]` marker.

**Phase to address:**
WebSocket exec implementation phase. Correlation IDs and abort-on-reconnect must be in the initial implementation, not added later.

---

### Pitfall 8: Duplicate Firewall Rules Accumulate on Every Startup

**What goes wrong:**
`netsh advfirewall firewall add rule` does not check for existing rules with the same name. Running firewall setup on every rc-agent startup (for self-healing purposes) without checking first creates a new duplicate rule each time. Windows Firewall allows multiple inbound rules with the same name. After a week of daily reboots, each pod has 7+ identical `RCAgent_8090` rules. This is cosmetically confusing, creates noise in security audits, and in edge cases could interfere with more restrictive rules added later (the "most permissive wins" evaluation means the duplicates are harmless but invisible complexity accumulates).

**Why it happens:**
The natural self-healing pattern is "always apply on startup." Without an existence check, each startup adds another rule. The fact that the firewall continues to work masks the accumulation — there is no visible error.

**How to avoid:**
Implement an idempotent `ensure_firewall_rule()` function in the planned Rust firewall module:
```rust
pub async fn ensure_firewall_rule(name: &str, port: u16) -> Result<()> {
    let check = Command::new("netsh")
        .args(["advfirewall","firewall","show","rule",
               &format!("name={}", name)])
        .creation_flags(CREATE_NO_WINDOW)
        .output().await?;
    let out = String::from_utf8_lossy(&check.stdout);
    if out.contains("No rules match") {
        Command::new("netsh")
            .args(["advfirewall","firewall","add","rule",
                   &format!("name={}", name),
                   "dir=in","action=allow","protocol=TCP",
                   &format!("localport={}", port),
                   "remoteip=192.168.31.0/24",
                   "profile=any"])
            .creation_flags(CREATE_NO_WINDOW)
            .output().await?;
    }
    Ok(())
}
```
All firewall functions in the module must be idempotent from day one.

**Warning signs:**
- `netsh advfirewall firewall show rule name="RCAgent_8090"` returns the same rule listed 3, 5, or 10 times.
- Windows Firewall → Inbound Rules in the GUI shows multiple identically-named entries.
- `netsh advfirewall firewall show rule name="RCAgent_8090" | find /c "Rule Name:"` returns > 1.

**Phase to address:**
Firewall auto-config phase. The idempotency check must be in the initial implementation.

---

### Pitfall 9: Self-Swap Process Death Race — Old Binary Locked at Rename Time

**What goes wrong:**
The current `do-swap.bat` uses:
```bat
taskkill /F /IM rc-agent.exe
timeout /t 2 /nobreak
del /Q rc-agent.exe
move rc-agent-new.exe rc-agent.exe
```
`taskkill /F` sends a terminate signal but **does not wait for the process to fully exit**. On a loaded system (game running, CPU busy), the OS may still be cleaning up the process's memory and file handles when the `del` or `move` command runs 2 seconds later. The rename fails with `ERROR_ACCESS_DENIED`. The new binary is stranded as `rc-agent-new.exe` with the correct name occupied by a dead-but-not-released file handle. Both files may end up in an indeterminate state.

**Why it happens:**
The 2-second `timeout` is a timing assumption that holds on idle systems but not under load. Windows process cleanup, especially for processes that held network sockets and USB device handles (rc-agent holds the wheelbase HID connection), can take longer than 2 seconds to complete.

**How to avoid:**
Extend do-swap.bat generation in deploy.rs to poll for process death before renaming:
```bat
@echo off
timeout /t 3 /nobreak >nul
taskkill /F /IM rc-agent.exe >nul 2>&1
:WAIT_DEAD
tasklist /NH /FI "IMAGENAME eq rc-agent.exe" 2>nul | findstr rc-agent.exe >nul
if %ERRORLEVEL%==0 (
    timeout /t 1 /nobreak >nul
    goto WAIT_DEAD
)
del /Q rc-agent.exe >nul 2>&1
move rc-agent-new.exe rc-agent.exe
if %ERRORLEVEL% NEQ 0 (
    echo SWAP FAILED: rename error >> C:\RacingPoint\deploy-error.log
    exit /b 1
)
start "" /D C:\RacingPoint rc-agent.exe
```
Add a cap: if the `WAIT_DEAD` loop runs for more than 15 iterations (15 seconds), abort the swap and leave `rc-agent-new.exe` in place for manual recovery.

This bat content must be generated with `\r\n` line endings (see Pitfall 2).

**Warning signs:**
- deploy.rs log shows "Starting" state but rc-agent never reconnects via WebSocket.
- `dir C:\RacingPoint\rc-agent*.exe` shows `rc-agent-new.exe` present, `rc-agent.exe` absent.
- Port 8090 unreachable after deploy.
- `deploy-error.log` in `C:\RacingPoint\` contains "SWAP FAILED: rename error".

**Phase to address:**
Deploy resilience phase. Upgrade do-swap.bat generation in deploy.rs to use the poll-until-dead pattern.

---

### Pitfall 10: One-Way Connectivity — WebSocket Up, HTTP Blocked, Falsely Healthy

**What goes wrong:**
After a Windows Update, a firewall group policy push, or a profile change, the inbound TCP 8090 rule is silently removed. rc-core's HTTP management calls to the pod all fail. However, the pod's outbound WebSocket connection to rc-core port 8080 continues to work — outbound connections are allowed by default. rc-core continues to receive heartbeats, marks the pod as "connected," and the dashboard shows all pods green. But every deploy attempt, health check, and exec command to port 8090 fails silently or with a timeout.

This is a false-healthy state: the system reports no problem, but the pod is unmanageable.

**Why it happens:**
The WebSocket connection is outbound from the pod. The HTTP management port (8090) requires an inbound allow rule. These are independent. rc-core's pod health assessment currently treats WebSocket heartbeat as the primary liveness signal. The HTTP reachability is only tested when a management operation is attempted.

**How to avoid:**
Two changes needed:
1. rc-agent startup self-healing must verify and re-apply its own firewall rule on every boot (idempotent, see Pitfall 8). This prevents the state from occurring.
2. rc-core's pod monitor should distinguish `ws_connected` from `http_reachable` and surface both states on the fleet dashboard. A pod that is WS-connected but HTTP-unreachable should show a distinct "managed-offline" status — not the same green dot as a fully healthy pod.
3. The WebSocket exec path (new feature) provides a recovery mechanism: rc-core can send `CoreToAgentMessage::Exec` carrying a `netsh` command to re-add the inbound rule from inside the pod, without needing inbound HTTP.

**Warning signs:**
- Pod shows WebSocket connected in rc-core logs (heartbeats arriving) but all HTTP calls to port 8090 timeout.
- Deploy fails at step 0 (binary URL validation) or step 2 (download exec): "Failed to reach pod-agent."
- After a Windows Update, one or more pods lose HTTP reachability but remain WS-connected.
- The 30-second rolling deploy health check shows process alive but WebSocket-only — lock screen check fails because it uses HTTP exec.

**Phase to address:**
Firewall auto-config phase (self-apply on startup as prevention). WebSocket exec phase (recovery path). Fleet health dashboard should show WS and HTTP status as separate indicators.

---

### Pitfall 11: Fleet Monitoring Overhead Exceeds Management Benefit at 8 Pods

**What goes wrong:**
With only 8 pods, over-engineered monitoring creates more problems than it solves. Common over-engineering patterns that fail specifically at this scale:
- **Per-pod telemetry polling every 5 seconds** from rc-core: 8 pods × 12 polls/min = 96 HTTP calls/min to the management port. This consumes exec slots that should be reserved for actual management commands.
- **Centralised log aggregation** (ELK, Grafana Loki): operational overhead to maintain the aggregator exceeds the value of having it. At 8 pods, the existing `tracing` logs written to disk and surfaced via the dashboard are sufficient.
- **Health check cascade**: rc-core checks pod health, pod reports to rc-core, rc-core checks the report, and schedules a secondary verify. This creates circular state updates that produce false-positive "recovery" events.
- **Prometheus/metrics endpoint**: Adds a server-side scrape target. At 8 pods, the existing `/health` JSON endpoint is sufficient. A full metrics endpoint adds code complexity without actionable benefit until scale increases by 10x.

**Why it happens:**
Fleet management tooling at larger scale (100+ nodes) justifies dedicated monitoring infrastructure. Developers apply the same patterns to small fleets by analogy. The existing WebSocket heartbeat already provides the liveness signal needed for 8 pods.

**How to avoid:**
For 8 pods, the correct monitoring strategy is:
1. **Liveness**: WebSocket heartbeat (already implemented). If heartbeat stops, pod is offline.
2. **Manageability**: HTTP `/health` endpoint polled on-demand (not scheduled), called before any management operation.
3. **Status**: Dashboard shows the last-known pod state from AppState — no additional polling loop needed.
4. **Alerts**: Email on state changes (already implemented in email_alerts.rs). Do not add alert channels for a fleet of 8.
5. **Logs**: Each pod writes its own `rc-agent.log`. Centralised log review is done by fetching the log file via `/file` endpoint when debugging — not streamed continuously.

The fleet health dashboard for Uday should be a read-only view of AppState, not a separate polling system.

**Warning signs:**
- rc-core's pod_monitor.rs spawns more than 2 background tasks per pod.
- The monitoring loop makes HTTP calls to pods more than once per 30 seconds during idle operation.
- A pod that is playing a game session shows degraded WebSocket response times due to management traffic competing with game telemetry traffic on the same port.

**Phase to address:**
Fleet health dashboard phase. Design the dashboard as a view of existing AppState, not a new polling system.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| HKLM Run key for startup | Simple, works at login | No crash-restart; pod stays dead after any crash until reboot | Never — replace with Task Scheduler before v4.0 ships |
| Inline echo-chain bat generation in deploy.rs | No file write, no CRLF risk (cmd.exe generates CRLF) | Hard to read, fragile quoting, cannot add retry loops | Acceptable only for 1-2 line scripts; use Rust file write for multi-step scripts |
| Single 4-slot semaphore for all exec | Simple code | Deploy blocks health checks, health blocks deploy | Replace with two semaphores: deploy-class (4) and monitoring-class (2) |
| Hardcoded 2s sleep in do-swap.bat | Simple timing | Races on loaded systems; partial swap leaves pod dead | Replace with poll-until-dead loop in Pitfall 9 |
| HTTP-only management transport | Simple request-response | Single point of failure when firewall blocks inbound | Add WebSocket exec as secondary path — required for v4.0 |
| No firewall verification after apply | Faster startup | Silent failures cascade (CRLF bug pattern) | Never — always verify rules after applying |
| `netsh` in bat files for firewall | Works without Rust dependencies | CRLF-sensitive, hard to test in unit tests | Replace with Rust `Command::new("netsh")` with CRLF-safe argument passing |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| netsh in Rust | Trust exit code 0 = success | Parse stdout for "No rules match" / "1 rule(s) found" — exit codes are unreliable for show commands |
| Task Scheduler from Rust | `schtasks /create` and assume success | Use `/f` flag (force overwrite), verify with `schtasks /query /tn RCAgent`, check "Status: Ready" |
| do-swap.bat content from Rust | `lines.join("\n")` | `lines.join("\r\n") + "\r\n"` — unit test asserts `\r\n` present |
| WebSocket exec correlation | Ignore message ordering, use sequence numbers | Assign `Uuid` correlation ID per request; match by ID not position |
| Windows Defender exclusions | Add `rc-agent.exe` path only | Add full directory `C:\RacingPoint\`, `rc-agent-new.exe`, and `do-swap.bat`; verify at startup via `Get-MpPreference` |
| Session detection for Task Scheduler restart | Assume user is always logged in | `WTSGetActiveConsoleSessionId()` returns `0xFFFFFFFF` (-1) when no user is logged in; Task Scheduler `/sc ONLOGON` handles this correctly |
| exec timeout for download | Use default 10s timeout | Deploy download uses 120_000ms timeout — already correct in deploy.rs; ensure WebSocket exec also supports per-command timeout override |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Scheduled HTTP health polling to all 8 pods | 96+ HTTP calls/min, exec slots consumed by monitoring | Use WebSocket heartbeat for liveness; HTTP health only on-demand before management ops | As soon as any management operation runs concurrently |
| Centralised log aggregation | Aggregator becomes single point of failure; maintenance overhead exceeds value | Serve logs on-demand via `/file` endpoint | Unnecessary complexity at 8 pods; reconsider at 50+ pods |
| WebSocket exec collecting unlimited stdout | 2-second pauses assembling large frames; risk of OOM | Cap stdout/stderr at 1MB; add truncation marker | Any command producing >1MB output (e.g., `dir /s C:\`) |
| Rolling deploy without per-pod serialisation | Concurrent exec calls exhaust semaphore slots on target pod | deploy.rs already has 5s delay between pods — keep it; add per-pod Mutex in rc-core | Any concurrent deploy + health scenario on same pod |
| Firewall rule check on every heartbeat | ~360 netsh invocations/hour per pod, each spawning a child process | Check only on startup and on WebSocket reconnect (implies reboot/restart) | Fine at 8 pods; avoid entirely with rc-agent startup self-healing |
| Dashboard polling AppState faster than 1s | Spurious updates, dashboard flicker, websocket message storm | Dashboard event bus already exists; push events only on state change | Any refresh rate faster than the actual state change rate |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| `/exec` endpoint with no authentication | Any LAN device (customer laptop on pod network) can execute arbitrary commands on any pod | Shared secret header `X-RC-Token` in all pod management requests; validate in remote_ops.rs before processing |
| WebSocket exec without message authentication | A rogue WebSocket client sending `CoreToAgentMessage::Exec` could execute commands on pods | rc-core must validate that Exec messages originate from its own internal coordinator, not from external WebSocket connections |
| Firewall rule allows `remoteip=any` | Port 8090 accessible from any device, including customer machines and internet if port-forwarded | Always add `remoteip=192.168.31.0/24` to restrict management port to server subnet only |
| do-swap.bat left on disk after deploy | Script can be re-triggered by any user who finds it; contains hardcoded paths and timing logic | rc-agent startup self-healing must delete any stale `do-swap.bat` files as first action |
| AV exclusions applied globally | Unnecessarily broad; `C:\RacingPoint\` exclusion also covers customer-writable subpaths if any are created | Scope exclusions to specific file names (`rc-agent.exe`, `rc-agent-new.exe`) rather than the full directory if possible |

---

## "Looks Done But Isn't" Checklist

- [ ] **Session 0 test:** After installing watchdog service, verify with `tasklist /v /fi "IMAGENAME eq rc-agent.exe"` that Session# = 1 (not 0). Lock screen must be visible on physical pod display.
- [ ] **Crash-restart test:** On Pod 8, kill rc-agent with Task Manager. Verify it restarts within 90 seconds without a reboot. Verify lock screen reappears.
- [ ] **Firewall rule applied:** `netsh advfirewall firewall show rule name="RCAgent_8090"` shows exactly 1 rule. Test actual connectivity from rc-core: `curl http://192.168.31.X:8090/ping` returns "pong".
- [ ] **Firewall profile verified:** `netsh advfirewall show currentprofile` on pod — confirm the rule was applied with `profile=any` and not to a mismatched profile.
- [ ] **CRLF in bat files:** Any bat file written by Rust: `certutil -dump path\to\file.bat | findstr "0d 0a"` must show `0d 0a` sequences. LF-only files show `0a` without preceding `0d`.
- [ ] **Idempotent firewall:** Restart rc-agent 5 times on a test pod. Run `netsh advfirewall firewall show rule name="RCAgent_8090" | find /c "Rule Name:"` — must return 1, not 5.
- [ ] **AV exclusions:** `powershell -c "Get-MpPreference | Select-Object -ExpandProperty ExclusionPath"` on pod — must include `C:\RacingPoint\` or both `rc-agent.exe` and `rc-agent-new.exe`.
- [ ] **Self-swap under load:** Trigger a deploy while a game session is running on the pod. Verify the swap completes (new binary starts) or aborts cleanly (old binary preserved, `deploy-error.log` written). Never a state where both binaries are absent.
- [ ] **WebSocket exec independence:** Block inbound TCP 8090 via firewall. Verify HTTP exec returns connection refused. Verify WebSocket exec (`CoreToAgentMessage::Exec`) still executes commands. These must be independent.
- [ ] **Exec slot recovery:** Send 4 concurrent exec requests with 5-second timeouts. After 15 seconds, verify `/health` shows `exec_slots_available: 4` (all slots recovered). Confirm no leak.
- [ ] **Task Scheduler task persists:** Delete `start-rcagent.bat` (simulate corrupted startup). Verify rc-agent startup self-healing recreates the Task Scheduler task and bat file.

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Session 0 lock screen invisible | HIGH — physical access or RDP | RDP to pod, kill service-installed rc-agent, run `schtasks /run /tn "RCAgent"` to start in Session 1; then replace service with Task Scheduler pattern |
| CRLF-broken firewall bat | MEDIUM — WebSocket exec available | Use WebSocket exec (`CoreToAgentMessage::Exec`) to run `netsh advfirewall firewall add rule name=RCAgent_8090 dir=in action=allow protocol=TCP localport=8090 profile=any` directly; no bat file needed |
| Exec slot exhaustion | LOW — self-resolves | Wait for timeout (10s default for standard commands, 120s for downloads); or restart pod via WoL + power cycle; slots release on process restart |
| AV quarantined rc-agent-new.exe | HIGH | Via pod-agent `/exec` (if old binary still alive): `powershell Add-MpPreference -ExclusionPath C:\RacingPoint`; then retry deploy. If port 8090 is down, physical USB deploy from pod-deploy kit |
| Self-swap left pod dead (both binaries absent) | HIGH | If do-swap.bat retry log shows partial swap: rename `rc-agent-new.exe` to `rc-agent.exe` via pod-agent `/exec`. If port 8090 is also down (impossible since binary is rc-agent itself): physical USB deploy |
| Firewall reset after Windows Update | LOW — WebSocket exec available | WebSocket exec: `netsh advfirewall firewall add rule name=RCAgent_8090 dir=in action=allow protocol=TCP localport=8090 profile=any`; startup self-healing prevents recurrence |
| Task Scheduler task deleted | MEDIUM — requires working rc-agent | Via pod-agent `/exec`: run `schtasks /create /tn "RCAgent" ...` to recreate; then verify with `schtasks /query /tn "RCAgent"` |
| HKLM Run key deleted (pre-migration) | MEDIUM — pod stays dead until reboot | Via pod-agent `/exec`: `reg add "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run" /v "RCAgent" /t REG_SZ /d "C:\RacingPoint\start-rcagent.bat" /f`; then trigger reboot via WoL |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Session 0 GUI invisible | Windows Service / auto-restart phase | `tasklist /v` shows Session# = 1; lock screen visible on pod display after simulated crash |
| CRLF-broken bat files | Firewall auto-config phase (Rust bat writer) | Unit test: generated bat content contains `\r\n`; hex-verify any written bat file |
| Exec slot exhaustion | WebSocket exec phase | 5 concurrent exec requests: 5th returns 429; slots recover after timeout; WS exec unaffected |
| AV quarantine of new binary | Deploy resilience phase | Deploy with AV active; binary not quarantined; retry loop in do-swap.bat handles handle hold |
| Firewall profile mismatch | Firewall auto-config phase | Block inbound 8090, restart rc-agent; port accessible again within 10s; correct profile in netsh show |
| HKLM Run key no crash restart | Windows Service / auto-restart phase | Kill rc-agent; restarts within 90s without reboot; verified on Pod 8 canary first |
| WebSocket exec output truncation | WebSocket exec phase | Command producing >1MB output: `truncated: true` in response; no crash; slot recovered |
| Duplicate firewall rules | Firewall auto-config phase | 5 consecutive restarts; exactly 1 rule named `RCAgent_8090` exists |
| Self-swap process death race | Deploy resilience phase | Deploy while game running (loaded CPU); swap completes or aborts cleanly; no both-absent state |
| One-way connectivity | Firewall auto-config + WS exec phases | Remove inbound rule; pod shows "managed-offline" in dashboard; WS exec re-adds rule; HTTP recovers |
| Fleet monitoring overhead | Fleet health dashboard phase | Dashboard shows correct status using AppState view only; no new polling loops added to pod_monitor.rs |

---

## Sources

- **Direct observation — HIGH:** 4-hour debugging session, Racing Point pods, Mar 15 2026. Pods 1/3/4 offline. Root causes confirmed: CRLF in do-swap.bat, exec slot exhaustion (missing CREATE_NO_WINDOW pre-fix), missing firewall rules (CRLF cascade), rc-agent crash with no restart (HKLM Run key limitation), Pod 3 one-way connectivity.
- **Codebase — HIGH:** `crates/rc-agent/src/remote_ops.rs` (semaphore, CREATE_NO_WINDOW, EXEC_SEMAPHORE, try_acquire), `crates/rc-core/src/deploy.rs` (self-swap pattern, do-swap.bat generation, VERIFY_DELAYS), `crates/rc-agent/src/lock_screen.rs` (port 18923 HTTP server, Session 1 requirement), `crates/rc-agent/src/main.rs` (startup flow).
- **Microsoft — HIGH:** [Application Compatibility - Session 0 Isolation](https://techcommunity.microsoft.com/blog/askperf/application-compatibility---session-0-isolation/372361) — confirms Session 0 non-interactive since Vista; UI0Detect removed Windows 10 1803+.
- **Microsoft — HIGH:** [Launching an interactive process from Windows Service in Windows Vista and later](https://learn.microsoft.com/en-us/archive/blogs/winsdk/launching-an-interactive-process-from-windows-service-in-windows-vista-and-later) — WTSGetActiveConsoleSessionId + WTSQueryUserToken + CreateProcessAsUser pattern.
- **Microsoft — HIGH:** [Use netsh advfirewall firewall context](https://learn.microsoft.com/en-us/troubleshoot/windows-server/networking/netsh-advfirewall-firewall-control-firewall-behavior) — profile parameter behaviour, `profile=any` requirement.
- **CoreTechnologies — HIGH:** [Why doesn't "Allow service to interact with desktop" work?](https://www.coretechnologies.com/blog/windows-services/interact-with-desktop/) — confirms checkbox is permanently non-functional on Windows 10 1803+ / Windows 11.
- **Rust community — HIGH:** [Anti-virus deleting my executables](https://users.rust-lang.org/t/anti-virus-deleting-my-executables/80776), [rustup Windows rename issues](https://github.com/rust-lang/rustup/issues/3636) — AV scanner file handle hold on newly downloaded Rust binaries; false-positive detection of unsigned binaries.
- **tokio docs — HIGH:** [tokio::process::Command](https://docs.rs/tokio/latest/tokio/process/struct.Command.html) — `kill_on_drop(true)` semantics; zombie process behaviour on Windows; `CREATE_NO_WINDOW` flag necessity.
- **NSSM — MEDIUM:** [nssm.cc/usage](https://nssm.cc/usage) — GUI applications do not respond to WM_CLOSE from Session 0; console window creation limitations.
- **Windows forum — MEDIUM:** [Task Scheduler "at logon" for GUI apps](https://windowsforum.com/threads/show-app-ui-at-logon-with-windows-11-task-scheduler.392418/) — confirms "Run only when user is logged on" is the correct Session 1 trigger; restart-on-failure settings available.

---
*Pitfalls research for: v4.0 Pod Fleet Self-Healing (Windows Service, WebSocket Exec, Firewall Auto-Config, Self-Update)*
*Researched: 2026-03-15*
