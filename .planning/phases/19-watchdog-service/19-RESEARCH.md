# Phase 19: Watchdog Service - Research

**Researched:** 2026-03-15
**Domain:** Windows SYSTEM Service + Session 1 Process Spawning + Rust (windows-service crate)
**Confidence:** HIGH

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| SVC-01 | rc-watchdog.exe runs as a Windows Service (SYSTEM) and auto-starts on boot | windows-service 0.8 crate provides ServiceMain + SCM registration; `sc create` with `start= auto` |
| SVC-02 | Watchdog detects rc-agent crash within 10 seconds and restarts it in Session 1 | sysinfo or tasklist polling loop ≤10s; WTSGetActiveConsoleSessionId + WTSQueryUserToken + CreateProcessAsUser via winapi to spawn in Session 1 |
| SVC-03 | Watchdog reports crash events to racecontrol via HTTP (startup count, crash time, exit code) | reqwest blocking client; POST to racecontrol `/api/watchdog/crash-report`; new `WatchdogCrashReport` type in rc-common |
| SVC-04 | Install script registers watchdog service with SCM failure actions (restart on failure) | `sc.exe create` + `sc.exe failure` with `actions= restart/5000` in install.bat; covered by existing deploy pipeline |
</phase_requirements>

---

## Summary

Phase 19 builds `rc-watchdog.exe` — a new binary in the workspace (`crates/rc-watchdog`) that runs as a Windows SYSTEM service and keeps rc-agent alive. The key architectural decision from STATE.md is already settled: the watchdog wraps `start-rcagent.bat` (preserving Session 1 startup), does NOT use NSSM (external dep), and does NOT implement a native ServiceMain inside rc-agent itself (Session 0 GUI boundary).

The implementation requires two distinct capabilities: (1) running as a proper Windows service via the `windows-service` crate, and (2) launching `start-rcagent.bat` in the active interactive session (Session 1) from a SYSTEM context. The second capability requires WinAPI calls — `WTSGetActiveConsoleSessionId`, `WTSQueryUserToken`, and `CreateProcessAsUser` — because `std::process::Command` from SYSTEM always lands in Session 0 and cannot show a GUI.

After each rc-agent crash, the watchdog must report to racecontrol (SVC-03) via a fire-and-forget HTTP POST within 30 seconds. racecontrol receives the report at a new endpoint and logs it as pod activity. The `EscalatingBackoff` struct in rc-common already exists and can be reused for crash loop detection.

**Primary recommendation:** New workspace crate `crates/rc-watchdog` using `windows-service 0.8`, `winapi` with `wtsapi32` + `processthreadsapi` features, and `reqwest` blocking for crash reports. Install via updated `install.bat` using `sc create` + `sc failure`.

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| windows-service | 0.8.0 | ServiceMain, service_control_handler, service_manager | The only maintained Rust crate for Windows SCM integration; used by Mullvad VPN |
| winapi | 0.3 | WTSQueryUserToken, CreateProcessAsUser, WTSGetActiveConsoleSessionId | Already in rc-agent Cargo.toml; provides wtsapi32 + processthreadsapi features |
| reqwest | 0.12 | HTTP POST crash reports to racecontrol | Already in rc-agent; use blocking feature in watchdog (no tokio runtime needed in watchdog) |
| serde / serde_json | 1 | Serialize WatchdogCrashReport payload | Already workspace dep |
| chrono | 0.4 | Timestamp crash events | Already workspace dep |
| tracing / tracing-appender | 0.1 / 0.2 | Log to file (C:\RacingPoint\watchdog.log) | Already workspace dep |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| rc-common | workspace | WatchdogCrashReport type, EscalatingBackoff reuse | Always — shared types |
| anyhow | 1 | Error handling | Already workspace dep |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| windows-service 0.8 | Native ServiceMain via winapi directly | Much more boilerplate; no benefit |
| winapi wtsapi32 | windows crate (microsoft/windows-rs) | windows crate has complex feature matrix; winapi already in workspace |
| reqwest blocking | tokio + reqwest async | Watchdog is simple loop — no need for async runtime |
| New watchdog crate | Embed watchdog in rc-agent | rc-agent is the monitored process; can't be both watchdog and watched |

**Installation:**
```toml
# crates/rc-watchdog/Cargo.toml additions
[dependencies]
windows-service = "0.8"
reqwest = { version = "0.12", features = ["json", "blocking"] }

[target.'cfg(windows)'.dependencies]
winapi = { version = "0.3", features = ["wtsapi32", "processthreadsapi", "winbase", "handleapi", "securitybaseapi", "userenv", "winnt"] }
```

---

## Architecture Patterns

### New Crate: crates/rc-watchdog

```
crates/rc-watchdog/
├── Cargo.toml
└── src/
    ├── main.rs          # Service entry point: define_windows_service! + SCM dispatch
    ├── service.rs       # Service main loop: poll + restart logic
    ├── session.rs       # Session 1 process spawn (WTSQueryUserToken + CreateProcessAsUser)
    └── reporter.rs      # HTTP crash report to racecontrol (blocking reqwest)
```

### Pattern 1: Windows Service Entry Point

**What:** `define_windows_service!` macro generates the FFI entry point; `service_dispatcher::start` hands control to SCM.

**When to use:** Always — required for any Windows service in Rust.

```rust
// Source: docs.rs/windows-service/0.8/windows_service/macro.define_windows_service.html
use std::ffi::OsString;
use windows_service::{define_windows_service, service_dispatcher};

define_windows_service!(ffi_service_main, service_main);

fn main() -> Result<(), windows_service::Error> {
    service_dispatcher::start("RCWatchdog", ffi_service_main)?;
    Ok(())
}

fn service_main(arguments: Vec<OsString>) {
    if let Err(e) = service::run(arguments) {
        tracing::error!("Service error: {}", e);
    }
}
```

### Pattern 2: Service Control Handler + Stop Signal

**What:** Register a handler for SCM stop/shutdown signals; use a channel to signal the main loop.

**When to use:** Required — SCM must be able to stop the service cleanly.

```rust
// Source: mullvad/windows-service-rs/examples/ping_service.rs pattern
use std::sync::mpsc;
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::service::{ServiceControl, ServiceStatus, ServiceState, ServiceType,
    ServiceControlAccept, ServiceExitCode};

fn run(_arguments: Vec<OsString>) -> anyhow::Result<()> {
    let (shutdown_tx, shutdown_rx) = mpsc::channel();

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                shutdown_tx.send(()).ok();
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register("RCWatchdog", event_handler)?;

    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: std::time::Duration::default(),
        process_id: None,
    })?;

    // Main poll loop
    loop {
        if shutdown_rx.try_recv().is_ok() {
            break;
        }
        // ... check rc-agent, restart if needed ...
        std::thread::sleep(std::time::Duration::from_secs(5));
    }

    // Report stopped
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: std::time::Duration::default(),
        process_id: None,
    })?;

    Ok(())
}
```

### Pattern 3: Session 1 Process Spawn from SYSTEM Service

**What:** SYSTEM service must use WTSGetActiveConsoleSessionId + WTSQueryUserToken + CreateProcessAsUser to spawn rc-agent (via start-rcagent.bat) in the active interactive session. `std::process::Command` from SYSTEM always targets Session 0 — cannot show GUI.

**When to use:** Whenever the watchdog needs to restart rc-agent. Must handle "no user logged in yet" gracefully (return early, retry on next poll).

**Critical sequence:**
1. `WTSGetActiveConsoleSessionId()` — get active session ID (returns 0xFFFFFFFF if none)
2. `WTSQueryUserToken(session_id, &mut token)` — requires SE_TCB_NAME privilege (held by LocalSystem)
3. `DuplicateTokenEx(token, ...)` — create primary token from impersonation token
4. `CreateEnvironmentBlock(dup_token, ...)` — build user environment
5. `CreateProcessAsUserW(dup_token, ..., "cmd.exe /c start-rcagent.bat", ...)` — launch in Session 1

```rust
// Source: Verified pattern from petemoore gist + murrayju/CreateProcessAsUser + winapi docs
#[cfg(windows)]
use winapi::um::{
    wtsapi32::{WTSGetActiveConsoleSessionId, WTSQueryUserToken},
    processthreadsapi::{CreateProcessAsUserW, PROCESS_INFORMATION, STARTUPINFOW},
    securitybaseapi::DuplicateTokenEx,
    userenv::CreateEnvironmentBlock,
    winnt::{SecurityImpersonation, TokenPrimary, TOKEN_ALL_ACCESS},
    handleapi::CloseHandle,
};

pub fn spawn_in_session1(exe_dir: &Path) -> anyhow::Result<()> {
    unsafe {
        let session_id = WTSGetActiveConsoleSessionId();
        if session_id == 0xFFFF_FFFF {
            anyhow::bail!("No active console session — deferring restart");
        }

        let mut user_token = std::ptr::null_mut();
        if WTSQueryUserToken(session_id, &mut user_token) == 0 {
            anyhow::bail!("WTSQueryUserToken failed: {}", GetLastError());
        }

        let mut dup_token = std::ptr::null_mut();
        DuplicateTokenEx(user_token, TOKEN_ALL_ACCESS, std::ptr::null_mut(),
            SecurityImpersonation, TokenPrimary, &mut dup_token);
        CloseHandle(user_token);

        // Build process command: cmd /c start-rcagent.bat
        // start-rcagent.bat already handles the "start /b rc-agent.exe" launch
        let bat_path = exe_dir.join("start-rcagent.bat");
        let cmd = format!("cmd.exe /c \"{}\"", bat_path.display());
        // ... CreateProcessAsUserW call ...
        CloseHandle(dup_token);
    }
    Ok(())
}
```

**IMPORTANT:** If `WTSGetActiveConsoleSessionId()` returns `0xFFFFFFFF` (no session), defer restart and retry on next poll. The watchdog must not crash on this case — it is the normal state before user login.

### Pattern 4: Process Detection

**What:** Check if rc-agent.exe is running before deciding to restart. Use `sysinfo` crate (already in rc-agent) OR `tasklist /NH /FI "IMAGENAME eq rc-agent.exe"` via `std::process::Command`.

**Recommended:** `std::process::Command("tasklist")` — simpler, no extra dep for the watchdog crate. Checks exit code.

```rust
fn is_rc_agent_running() -> bool {
    let mut cmd = std::process::Command::new("tasklist");
    cmd.args(["/NH", "/FI", "IMAGENAME eq rc-agent.exe"]);
    #[cfg(windows)]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    match cmd.output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout).contains("rc-agent"),
        Err(_) => false, // Conservative: assume running if can't check
    }
}
```

### Pattern 5: SCM Service Registration (install.bat)

**What:** `sc create` registers the service; `sc failure` sets SCM-level restart actions (SVC-04).

**Critical detail:** SCM failure actions only fire if the SERVICE PROCESS exits with nonzero. Since rc-watchdog is a Rust binary that properly reports its own lifecycle, this is the backstop for if rc-watchdog itself crashes. Primary crash recovery is in the watchdog loop.

```batch
REM Register service
sc create RCWatchdog binPath= "C:\RacingPoint\rc-watchdog.exe" start= auto DisplayName= "RaceControl Watchdog"
sc description RCWatchdog "Monitors rc-agent and restarts it in Session 1 after crashes"

REM Set failure actions: restart after 5s on 1st/2nd/3rd failure; reset counter after 1hr
sc failure RCWatchdog reset= 3600 actions= restart/5000/restart/10000/restart/30000
```

### Pattern 6: WatchdogCrashReport Protocol (SVC-03)

**What:** Add `WatchdogCrashReport` to `rc-common::protocol::AgentMessage` OR as a standalone HTTP POST body to a new racecontrol endpoint. HTTP POST is preferred — watchdog has no WebSocket connection.

**Decision:** New HTTP POST endpoint in racecontrol (not WebSocket — watchdog runs as SYSTEM and has no agent identity). The report is fire-and-forget; failure to deliver is non-fatal.

```rust
// rc-common/src/types.rs — new type
#[derive(Debug, Serialize, Deserialize)]
pub struct WatchdogCrashReport {
    pub pod_id: String,        // from config (same as rc-agent pod name)
    pub exit_code: Option<i32>, // None if process disappeared without exit code
    pub crash_time: String,    // ISO 8601 UTC
    pub restart_count: u32,    // since watchdog started
    pub watchdog_version: String,
}

// racecontrol — new handler in api/ or ws/
// POST /api/pods/{pod_id}/watchdog-report
```

**racecontrol handler** logs as pod activity (same pattern as StartupReport) and updates pod state.

### Anti-Patterns to Avoid

- **`std::process::Command` from SYSTEM without WTSQueryUserToken:** Process always spawns in Session 0. GUI never appears.
- **Polling faster than 5s:** CPU waste; rc-agent takes 2-3s to exit cleanly. 5-second poll is sufficient.
- **Panicking on WTSGetActiveConsoleSessionId == 0xFFFFFFFF:** This is the normal boot state before login. Return early, retry next cycle.
- **Blocking indefinitely on HTTP report:** Use a short timeout (5s). Never let crash reporting block the restart loop.
- **Running rc-watchdog.exe on James's machine (.27):** Same rule as rc-agent — pod-only binary.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Windows SCM registration | Custom ServiceMain via raw winapi | windows-service 0.8 | Boilerplate, lifecycle edge cases, SCM protocol |
| Session 1 spawn | Custom session enumeration | WTSGetActiveConsoleSessionId + WTSQueryUserToken pattern | Established Win32 API; manual session enumeration is brittle |
| HTTP crash report client | Custom HTTP library | reqwest blocking 0.12 | Already in workspace; handles TLS, retries, etc. |
| Process existence check | Full sysinfo integration | `tasklist /FI` via Command | sysinfo is a heavyweight dep; tasklist is the same approach used by pod_monitor.rs |
| Backoff logic | Custom retry counter | rc-common::watchdog::EscalatingBackoff | Already exists, tested (100 rc-common tests pass) |

---

## Common Pitfalls

### Pitfall 1: Session 0 Isolation

**What goes wrong:** Developer calls `std::process::Command::new("rc-agent.exe").spawn()` from the SYSTEM service — the process starts but shows no GUI, the lock screen never appears, and the test appears to fail even though rc-agent.exe is running.

**Why it happens:** Windows Vista+ enforces Session 0 isolation — services run in session 0 which has no interactive desktop. User sessions are session 1+.

**How to avoid:** Always use `WTSGetActiveConsoleSessionId` + `WTSQueryUserToken` + `CreateProcessAsUserW`. Spawn `cmd.exe /c start-rcagent.bat` — the batch file then does `start "" /D C:\RacingPoint rc-agent.exe` which launches rc-agent on the user desktop.

**Warning signs:** `tasklist /v` shows rc-agent in Session# = 0. Lock screen never appears. Pod does not connect to racecontrol.

### Pitfall 2: No Active Session at Boot Time

**What goes wrong:** Watchdog starts before any user logs in (expected on reboot with auto-login or before auto-login fires). `WTSGetActiveConsoleSessionId` returns `0xFFFFFFFF`. If the watchdog panics or exits here, it fails SVC-02.

**Why it happens:** Windows auto-login may take 15-30 seconds after the service starts. The watchdog loop runs before the user desktop is ready.

**How to avoid:** On `0xFFFFFFFF`, log "no active session, deferring" and `continue` to the next poll iteration. The pod-level requirement (Session 1 lock screen within 60s of boot) means the watchdog needs to wait for login, not force login.

**Warning signs:** Watchdog exits immediately on reboot with no user logged in.

### Pitfall 3: Double-Restart Race

**What goes wrong:** Watchdog polls at T=0 (rc-agent dead), spawns bat, polls again at T=5 (rc-agent still starting), spawns bat AGAIN — two instances running. Second instance may fail to bind port 8090.

**Why it happens:** rc-agent takes 2-5 seconds to start fully. A 5s poll interval with no state tracking causes double-spawn.

**How to avoid:** Track a `restart_in_progress` bool and a `last_restart_at` timestamp. After spawning, skip the next 1-2 poll cycles (10s grace window) before checking again. Only restart if process was confirmed absent AND grace window has elapsed.

### Pitfall 4: winapi Feature Flags

**What goes wrong:** Compiler error `cannot find function WTSQueryUserToken in scope` even though `winapi` is a dependency.

**Why it happens:** The `winapi` crate requires explicit feature flags per module. `wtsapi32` must be listed, AND the type imports require additional feature flags (`winnt` for TOKEN_ALL_ACCESS, `processthreadsapi` for CreateProcessAsUserW, etc.).

**How to avoid:** In Cargo.toml:
```toml
[target.'cfg(windows)'.dependencies]
winapi = { version = "0.3", features = [
    "wtsapi32", "processthreadsapi", "winbase",
    "handleapi", "securitybaseapi", "userenv", "winnt"
] }
```

### Pitfall 5: sc.exe vs sc PowerShell Alias

**What goes wrong:** `sc failure ...` in PowerShell does nothing or errors because PowerShell aliases `sc` to `Set-Content`.

**Why it happens:** PowerShell's `sc` is `Set-Content`. Must use `sc.exe` explicitly.

**How to avoid:** In any PowerShell or bat scripts: use `sc.exe create ...` and `sc.exe failure ...`. In pure batch (.bat) files, `sc` works correctly.

### Pitfall 6: start-rcagent.bat Already Kills rc-agent

**What goes wrong:** The existing `start-rcagent.bat` has `taskkill /F /IM rc-agent.exe` as its first line (from self_heal.rs). If the watchdog calls it when rc-agent is still starting, it kills the new instance.

**Why it happens:** The bat file was designed for clean restarts, not for watchdog-called restarts.

**How to avoid:** The watchdog should ONLY call `start-rcagent.bat` when it has confirmed rc-agent is NOT running. The `is_rc_agent_running()` check before calling the bat is mandatory. Do not call the bat if rc-agent is running (even if it's the watchdog doing a scheduled restart).

---

## Code Examples

### Workspace Cargo.toml Addition

```toml
# Root Cargo.toml — add to workspace members
members = [
    "crates/rc-common",
    "crates/racecontrol",
    "crates/rc-agent",
    "crates/rc-watchdog",   # NEW
]
```

### rc-watchdog/Cargo.toml

```toml
[package]
name = "rc-watchdog"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true

[[bin]]
name = "rc-watchdog"
path = "src/main.rs"

[dependencies]
rc-common = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
chrono = { workspace = true }
anyhow = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
tracing-appender = { workspace = true }

# Windows service lifecycle
windows-service = "0.8"

# HTTP crash reports — blocking (no tokio needed)
reqwest = { version = "0.12", features = ["json", "blocking"] }

[target.'cfg(windows)'.dependencies]
winapi = { version = "0.3", features = [
    "wtsapi32",
    "processthreadsapi",
    "winbase",
    "handleapi",
    "securitybaseapi",
    "userenv",
    "winnt",
] }
```

### WatchdogCrashReport Type (rc-common/src/types.rs addition)

```rust
// Source: designed for this phase, consistent with existing types pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchdogCrashReport {
    pub pod_id: String,
    pub exit_code: Option<i32>,
    pub crash_time: String,       // Utc::now().to_rfc3339()
    pub restart_count: u32,
    pub watchdog_version: String,
}
```

### sc.exe Service Installation (install.bat additions)

```batch
:: Install rc-watchdog as SYSTEM service
sc.exe create RCWatchdog binPath= "C:\RacingPoint\rc-watchdog.exe" start= auto obj= LocalSystem DisplayName= "RaceControl Watchdog"
sc.exe description RCWatchdog "Monitors rc-agent and restarts it in Session 1 after crashes"
sc.exe failure RCWatchdog reset= 3600 actions= restart/5000/restart/10000/restart/30000
sc.exe start RCWatchdog
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| watchdog-rc-agent.cmd (batch loop) | rc-watchdog.exe (SYSTEM service) | Phase 19 | Service starts on boot without login; SCM manages lifecycle |
| HKLM Run key (Session 1 via user login) | SYSTEM service + WTSQueryUserToken spawn | Phase 19 | Survives crashes WITHOUT requiring user logout/login cycle |
| pod_monitor.rs restart via pod-agent HTTP | Watchdog restarts locally on pod | Phase 19 | Faster (local, no network hop); works when pod-agent is dead |
| No crash telemetry from watchdog | WatchdogCrashReport HTTP POST | Phase 19 | racecontrol knows crash count, exit code, time |

**Deprecated/outdated:**
- `watchdog-rc-agent.cmd`: Replaced by rc-watchdog.exe; can be deleted from deploy/ after Phase 19 ships
- HKLM Run key for start-rcagent.bat: Remains as fallback (self_heal.rs still writes it), but watchdog is now the primary restart mechanism

**Note on HKLM Run key coexistence:** The self_heal module writes the HKLM Run key pointing to start-rcagent.bat. This is fine — it provides a second path if the watchdog service is not installed yet (e.g., first-time install). Both can coexist: Run key fires once at user login; watchdog fires on every crash. No conflict.

---

## Open Questions

1. **rc-watchdog config: hardcoded vs TOML**
   - What we know: rc-agent reads rc-agent.toml for pod_id and racecontrol URL. The watchdog needs the same info.
   - What's unclear: Should watchdog have its own rc-watchdog.toml or read rc-agent.toml directly?
   - Recommendation: Read rc-agent.toml (same format; watchdog is a companion). If not found, fall back to hardcoded defaults (pod_id from COMPUTERNAME, racecontrol at 192.168.31.23:8080). Self-heal pattern: if rc-agent.toml missing, skip HTTP report (non-fatal).

2. **racecontrol endpoint for WatchdogCrashReport**
   - What we know: Crash report needs a POST handler in racecontrol.
   - What's unclear: Where in racecontrol to add it. Options: new `watchdog.rs` module in api/, or add to existing `pod_monitor.rs` / `ws/mod.rs`.
   - Recommendation: New `api/watchdog.rs` handler at `POST /api/pods/:pod_id/watchdog-crash` — consistent with existing REST pattern in racecontrol. No WS needed (watchdog is not the agent process).

3. **Exit code capture**
   - What we know: SVC-03 requires exit code in crash report. But if rc-agent is killed by `taskkill /F`, there is no graceful exit and the exit code may be 1 or a Windows error code.
   - What's unclear: Can the watchdog actually observe the rc-agent exit code?
   - Recommendation: The watchdog detects absence via tasklist polling, not via process handle. It cannot observe the exit code from an unrelated process. Report `exit_code: None` when process simply disappeared. Only report a code if watchdog had spawned the process and holds a HANDLE. For Phase 19, `None` is acceptable for killed processes; a future phase could track child process handles.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in (`cargo test`) |
| Config file | Cargo.toml per crate, workspace root |
| Quick run command | `cargo test -p rc-common && cargo test -p rc-watchdog` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate && cargo test -p rc-watchdog` |

### Phase Requirements to Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SVC-01 | Service installs and appears in SCM | manual-only | `sc.exe query RCWatchdog` on pod | N/A |
| SVC-02 | Watchdog detects absent process within 10s | unit | `cargo test -p rc-watchdog -- test_process_detection` | Wave 0 |
| SVC-02 | spawn_in_session1 returns error when no active session | unit | `cargo test -p rc-watchdog -- test_no_session_graceful` | Wave 0 |
| SVC-02 | restart grace window prevents double-spawn | unit | `cargo test -p rc-watchdog -- test_double_restart_prevention` | Wave 0 |
| SVC-03 | WatchdogCrashReport serializes to expected JSON | unit | `cargo test -p rc-common -- test_watchdog_crash_report_roundtrip` | Wave 0 |
| SVC-03 | racecontrol handler logs crash report as pod activity | unit | `cargo test -p racecontrol-crate -- test_watchdog_crash_handler` | Wave 0 |
| SVC-04 | install.bat sc commands accepted by SCM | manual-only | Run install.bat on Pod 8, verify `sc.exe qfailure RCWatchdog` | N/A |

**Manual-only justification for SVC-01 and SVC-04:** Windows SCM interactions require running as Administrator on a real pod. Cannot be meaningfully tested in cargo test without mocking the entire SCM API.

### Sampling Rate

- **Per task commit:** `cargo test -p rc-common && cargo test -p rc-watchdog`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate && cargo test -p rc-watchdog`
- **Phase gate:** Full suite green + Pod 8 canary `tasklist /v` confirms Session# = 1

### Wave 0 Gaps

- [ ] `crates/rc-watchdog/src/main.rs` — crate does not exist yet; entire crate is Wave 0 setup
- [ ] `crates/rc-watchdog/src/service.rs` — process polling + restart logic + tests
- [ ] `crates/rc-watchdog/src/session.rs` — Session 1 spawn via WinAPI + unit tests (mocked for non-Windows)
- [ ] `crates/rc-watchdog/src/reporter.rs` — HTTP crash report + unit tests
- [ ] `crates/rc-common/src/types.rs` — `WatchdogCrashReport` struct + serde test
- [ ] `crates/racecontrol/src/api/watchdog.rs` — crash report handler + unit test

---

## Sources

### Primary (HIGH confidence)

- docs.rs/windows-service/0.8 — ServiceMain, define_windows_service!, service_control_handler, ServiceManager
- mullvad/windows-service-rs CHANGELOG.md — version 0.8.0 released 2025-02-19, MSRV 1.60
- microsoft.github.io/windows-docs-rs — WTSQueryUserToken, CreateProcessAsUserW signatures
- docs.rs/winapi — wtsapi32::WTSQueryUserToken confirmed available
- Project codebase — rc-common::watchdog::EscalatingBackoff (100 tests pass), self_heal.rs patterns, rc-agent Cargo.toml (winapi 0.3 already present), pod_monitor.rs restart pattern
- Microsoft Learn — sc.exe failure syntax (reset=, actions=)

### Secondary (MEDIUM confidence)

- gist.github.com/petemoore — Session 1 spawn pattern (WTSQueryUserToken + DuplicateTokenEx + CreateProcessAsUserW), verified against winapi docs
- murrayju/CreateProcessAsUser — Session 1 spawn reference implementation
- evotec.xyz — sc failure PowerShell equivalent (confirmed sc.exe vs sc alias issue)

### Tertiary (LOW confidence)

- users.rust-lang.org discussion on Windows service + interactive process — community experience, not official docs

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — windows-service 0.8 confirmed on docs.rs; winapi already in rc-agent Cargo.toml
- Architecture: HIGH — patterns verified from official Mullvad examples + Win32 API docs; project conventions confirmed from codebase read
- Pitfalls: HIGH — Session 0 isolation and WTSGetActiveConsoleSessionId pitfalls are well-documented Win32 behaviour; double-restart pitfall derived from direct codebase analysis

**Research date:** 2026-03-15
**Valid until:** 2026-04-15 (stable Win32 APIs; windows-service crate rarely changes)
