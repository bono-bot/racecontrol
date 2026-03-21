# Phase 103: Pod Guard Module - Research

**Researched:** 2026-03-21 IST
**Domain:** Rust process monitoring, Windows registry audit, tokio background task, sysinfo 0.33, winreg (via `reg` command), 512KB log rotation
**Confidence:** HIGH — based entirely on direct codebase inspection, no external sources needed

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Two-cycle grace period: process must appear in 2 consecutive scans before kill action — prevents transient Windows processes
- Self-exclusion: first filter skips current PID + parent PID + any process named `rc-agent.exe` unconditionally
- PID identity verification: verify process name + creation time match before kill to prevent PID reuse race
- Pod binary guard: detect `racecontrol.exe` on a pod = CRITICAL severity, zero grace period (standing rule #2)
- Severity tiers: KILL (immediate after grace), ESCALATE (log + WS alert, wait for staff), MONITOR (log only)
- Auto-start audit runs on startup + every 5 minutes (not every scan cycle — registry reads are heavier)
- Backup removed entries to `C:\RacingPoint\autostart-backup.json` before deletion
- Initial deploy in `report_only` mode — log violations without killing. Switch to `kill_and_report` after Pod 8 canary
- rc-agent fetches merged whitelist from racecontrol via `GET /api/v1/guard/whitelist/pod-{N}` on WS connect, falls back to empty whitelist (report-only) if fetch fails
- Log file: `C:\RacingPoint\process-guard.log` with 512KB rotation
- Violations reported via `AgentMessage::ProcessViolation` over existing WS channel

### Claude's Discretion
- Internal data structures for tracking consecutive scan hits
- How to integrate the background task with the existing event_loop.rs select! macro
- sysinfo::System refresh strategy (reuse existing instance or create new)
- Wildcard matching implementation for process names

### Deferred Ideas (OUT OF SCOPE)
- LLM classification for ESCALATE-tier unknowns (v12.2)
- Auto-whitelisting workflow via staff approval (v12.2)
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| PROC-01 | Continuous process scan (configurable interval, default 60s) comparing running processes against whitelist | sysinfo 0.33 API confirmed in kiosk.rs — `System::new()` + `refresh_processes(ProcessesToUpdate::All, true)` + `sys.processes()` iteration. spawn_blocking required (100-300ms blocking call). |
| PROC-02 | Auto-kill non-whitelisted processes with self-exclusion safety (never kill guard, rc-agent, racecontrol) | Kill via `taskkill /PID <pid> /F` in spawn_blocking (same pattern as pre_flight.rs:248). Self-exclusion via `std::process::id()` + `std::env::current_exe()` checked before whitelist lookup. Case-insensitive `eq_ignore_ascii_case()` for name comparison. |
| PROC-03 | PID identity verification (name + creation time) before kill to prevent PID reuse race | sysinfo `process.start_time()` returns u64 (seconds since epoch). Verify name (from snapshot) + start_time match fresh lookup before kill. If name changed, PID was reused — log and skip. |
| PROC-04 | Pod binary guard — detect rc-agent/racecontrol running on wrong machine (standing rule #2), CRITICAL severity with zero grace period | Machine identity from `config.pod.number` (already in AgentConfig). Any process named `racecontrol.exe` on a pod is CRITICAL/WrongMachineBinary — bypass two-cycle grace. |
| PROC-05 | Severity tiers per violation: KILL (immediate), ESCALATE (warn staff, auto-kill after TTL), MONITOR (log only) | Tier determined by whitelist entry field (from MachineWhitelist). KILL uses taskkill. ESCALATE sends WS alert + holds in memory. MONITOR log-only. Default tier for unlisted processes = KILL (deny-by-default). |
| AUTO-01 | HKCU/HKLM Run key audit — enumerate all values, flag non-whitelisted entries | Registry audit via `reg query HKCU\...\Run` and `reg query HKLM\...\Run` via `std::process::Command`. (winreg crate NOT currently in rc-agent Cargo.toml — use `reg` command same as self_heal.rs, or add winreg as new crate dep). |
| AUTO-02 | Startup folder audit — scan `%AppData%\...\Startup` for non-whitelisted shortcuts | `std::fs::read_dir` on `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup`. Use `std::env::var("APPDATA")` for path resolution. Also scan `C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Startup` (all-users). |
| AUTO-04 | Three-stage enforcement progression: LOG → ALERT → REMOVE (configurable per entry) | Track per-entry stage in a `HashMap<String, AutoStartStage>` struct. Stage progression controlled by `autostart_enforcement` config field per entry. Backup to `C:\RacingPoint\autostart-backup.json` before any REMOVE action. |
| ALERT-01 | Violation report via WebSocket to racecontrol on every kill/escalation | `AgentMessage::ProcessViolation(ProcessViolation { ... })` sent via `guard_violation_tx` mpsc channel. Channel drained in event_loop.rs select!. Types already defined in rc-common (Phase 101). |
| ALERT-04 | Append-only audit log per machine (`process-guard.log`, 512KB rotation) | Same pattern as `self_monitor.rs` `log_event()`: `OpenOptions::new().create(true).append(true)`, check `metadata().len() > MAX_LOG_BYTES` → truncate to 0. Log path: `C:\RacingPoint\process-guard.log`. |
| DEPLOY-01 | Process guard module in rc-agent (all 8 pods), report-only mode for safe rollout | MachineWhitelist.violation_action defaults to "report_only" (confirmed in types.rs). Guard enabled field in ProcessGuardConfig with `#[serde(default = "default_true")]` pattern. Deploy to Pod 8 canary first (standing rule). |
</phase_requirements>

---

## Summary

Phase 103 creates `process_guard.rs` in rc-agent: a self-contained background task that scans running processes against the whitelist fetched from racecontrol, kills confirmed violations, audits HKCU/HKLM Run keys and the Startup folder, and reports all actions over the existing WebSocket channel.

The codebase already has all the building blocks. The process scan pattern is in `kiosk.rs` (sysinfo 0.33, spawn_blocking wrapper). The log rotation pattern (512KB truncate-on-overflow) is in `self_monitor.rs`. The kill-by-PID pattern (taskkill via spawn_blocking) is in `pre_flight.rs`. The WS send pattern (mpsc channel → event_loop.rs select!) is in `AppState.ws_exec_result_tx`. The registry command invocation is in `self_heal.rs` (reg.exe via std::process::Command, no winreg crate). All four rc-common types are already defined from Phase 101 (MachineWhitelist, ProcessViolation, ViolationType, AgentMessage::ProcessViolation).

**Primary recommendation:** Build process_guard.rs as a `tokio::spawn` background task with a dedicated `mpsc::Sender<AgentMessage>` channel. Do NOT add new dependencies — all required libraries (sysinfo 0.33, winapi 0.3, std::process::Command for reg.exe) are already in scope.

---

## Standard Stack

### Core (already in rc-agent Cargo.toml)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| sysinfo | 0.33 | Process enumeration — `System::new()`, `refresh_processes()`, `sys.processes()` | Already used in kiosk.rs, game_process.rs, self_test.rs. DO NOT upgrade past 0.33 (breaking API in 0.38). |
| winapi | 0.3 | Windows process handle operations if PID identity verification requires GetProcessTimes | Already in `[target.'cfg(windows)'.dependencies]` with processthreadsapi, winnt, handleapi features. |
| tokio | workspace | spawn, spawn_blocking, mpsc, interval | Already in workspace deps. |
| serde / serde_json | workspace | ProcessViolation serialization | Already in workspace deps. |
| chrono | workspace | `Utc::now().to_rfc3339()` for violation timestamps | Already in workspace deps. |
| anyhow | workspace | Error propagation | Already in workspace deps. |
| tracing | workspace | Log events | Already in workspace deps. |

### Supporting (add to rc-agent Cargo.toml)
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| walkdir | 2 | Startup folder recursive scan | Needed for `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup` directory traversal. Cleaner than std::fs::read_dir for nested paths. Add as `walkdir = "2"` in rc-agent Cargo.toml. |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `reg.exe` via Command for registry audit | `winreg` crate | winreg crate NOT currently in rc-agent Cargo.toml and adding it conflicts with existing `winapi 0.3` approach. `reg query` via Command matches self_heal.rs pattern exactly — no new dep needed. |
| `taskkill /PID` via Command for kill | WinAPI TerminateProcess directly | taskkill matches game_process.rs and pre_flight.rs patterns. Direct WinAPI would require new unsafe code with handle management. No benefit for this use case. |
| walkdir for directory scan | std::fs::read_dir | walkdir adds clarity for recursive scan. std::fs::read_dir is sufficient for flat Startup folder scan but walkdir is already recommended in STATE-v12.1.md. |

**Installation:**
```bash
# In crates/rc-agent/Cargo.toml [dependencies] section:
walkdir = "2"
```

**Version verification:** walkdir 2.5.0 is the current stable release (confirmed in STATE-v12.1.md research decision).

---

## Architecture Patterns

### Recommended Project Structure

New file:
```
crates/rc-agent/src/
└── process_guard.rs    NEW — pod guard module
```

Modified files:
```
crates/rc-agent/src/
├── config.rs           + ProcessGuardConfig struct
├── app_state.rs        + guard_whitelist: Arc<RwLock<MachineWhitelist>>
│                       + guard_violation_tx: mpsc::Sender<AgentMessage>
│                       + guard_violation_rx: mpsc::Receiver<AgentMessage>
├── main.rs             + fetch whitelist on WS connect
│                       + spawn process_guard background task
│                       + wire guard_violation_rx into AppState
└── event_loop.rs       + drain guard_violation_rx in select! loop
```

### Pattern 1: Background Task (tokio::spawn + interval)

**What:** `process_guard::spawn(config, whitelist, ws_tx)` is called once in `main.rs` after AppState init. It uses `tokio::spawn` and a `tokio::time::interval` for the scan cycle. Scan body uses `tokio::task::spawn_blocking` because sysinfo::refresh_processes() blocks for 100-300ms.

**When to use:** This is the established pattern for all background daemons in rc-agent (self_monitor, failure_monitor, billing_guard, udp_heartbeat). All use `tokio::spawn` in main.rs.

**Example (from self_monitor.rs pattern):**
```rust
// Source: crates/rc-agent/src/self_monitor.rs lines 31-42
pub fn spawn(config: ProcessGuardConfig, whitelist: Arc<RwLock<MachineWhitelist>>, tx: mpsc::Sender<AgentMessage>) {
    tokio::spawn(async move {
        // 60s amnesty window on startup before first enforcement
        tokio::time::sleep(Duration::from_secs(60)).await;

        let mut scan_interval = tokio::time::interval(
            Duration::from_secs(config.scan_interval_secs)
        );
        let mut audit_interval = tokio::time::interval(Duration::from_secs(300)); // 5min

        loop {
            tokio::select! {
                _ = scan_interval.tick() => run_scan_cycle(&whitelist, &tx).await,
                _ = audit_interval.tick() => run_autostart_audit(&whitelist, &tx).await,
            }
        }
    });
}
```

### Pattern 2: sysinfo Process Scan (spawn_blocking)

**What:** sysinfo::System::new() + refresh_processes() blocks the thread for 100-300ms. Must use `tokio::task::spawn_blocking` for async context safety. Reuse kiosk.rs exact pattern.

**Example (from kiosk.rs lines 646-668):**
```rust
// Source: crates/rc-agent/src/kiosk.rs lines 648-668
// kiosk.rs warns: "Always call from tokio::task::spawn_blocking, never from the async event loop directly"
let violations = tokio::task::spawn_blocking(move || {
    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    // process.start_time() returns u64 seconds since epoch — use for PID identity verification
    sys.processes()
        .filter(|(pid, process)| {
            pid.as_u32() > 4  // skip system PIDs 0 and 4
            && !process.name().to_string_lossy().is_empty()
        })
        .map(|(pid, process)| (
            pid.as_u32(),
            process.name().to_string_lossy().to_lowercase(),
            process.exe().map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
            process.start_time(),  // u64 — for PID reuse check
        ))
        .collect::<Vec<_>>()
}).await?;
```

### Pattern 3: Kill via taskkill (spawn_blocking)

**What:** PID-targeted kill using `taskkill /F /PID <pid>`. Must use `spawn_blocking` in async context. Verify process name + start_time match before kill (PID reuse guard).

**Example (from pre_flight.rs lines 247-251):**
```rust
// Source: crates/rc-agent/src/pre_flight.rs lines 247-251
let kill_result = tokio::task::spawn_blocking(move || {
    std::process::Command::new("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .output()
}).await;
```

**PID identity guard (Phase 103 addition):**
```rust
// Before kill: re-check that PID still maps to same process name + start_time
// If mismatch → PID was reused → skip kill, log warning
let current = sys.process(sysinfo::Pid::from_u32(pid));
match current {
    Some(p) if p.name().to_string_lossy().to_lowercase() == snapshot_name
               && p.start_time() == snapshot_start_time => {
        // Safe to kill — identity confirmed
    }
    _ => {
        tracing::warn!("PID {} reused before kill — skipping (was {})", pid, snapshot_name);
        return;
    }
}
```

### Pattern 4: Registry Audit via reg.exe Command

**What:** Use `std::process::Command::new("reg").args(["query", key_path])` to enumerate Run keys. This matches self_heal.rs (no winreg crate required). Parse output lines to extract value name + data.

**Example (from self_heal.rs pattern):**
```rust
// Source: crates/rc-agent/src/self_heal.rs lines 207-222
// Query HKCU Run key (same pattern works for HKLM)
let output = std::process::Command::new("reg")
    .args(["query", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run"])
    .creation_flags(CREATE_NO_WINDOW)
    .output()?;
// Parse stdout lines: "    ValueName    REG_SZ    C:\path\to\exe"
// Split on whitespace, first token = value name
```

### Pattern 5: Log Rotation (512KB truncate)

**What:** Append-only log with size check before each write. On overflow, truncate to 0 (NOT rename — simpler). Same pattern as self_monitor.rs.

**Example (from self_monitor.rs lines 146-168):**
```rust
// Source: crates/rc-agent/src/self_monitor.rs lines 147-168
pub fn log_guard_event(event: &str) {
    use std::fs::OpenOptions;
    use std::io::Write;
    const GUARD_LOG: &str = r"C:\RacingPoint\process-guard.log";
    const MAX_LOG_BYTES: u64 = 512 * 1024;

    if let Ok(meta) = std::fs::metadata(GUARD_LOG) {
        if meta.len() > MAX_LOG_BYTES {
            let _ = std::fs::write(GUARD_LOG, b"");  // truncate
        }
    }
    let line = format!("[{}] {}\n", chrono::Utc::now().to_rfc3339(), event);
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(GUARD_LOG) {
        let _ = f.write_all(line.as_bytes());
    }
}
```

### Pattern 6: mpsc Channel for WS Violation Reporting

**What:** Guard sends violations via a dedicated `mpsc::Sender<AgentMessage>`. Event loop drains the receiver in select!. This matches the existing `ws_exec_result_tx` / `ws_exec_result_rx` pattern in AppState.

**Example (from main.rs lines 460, 684-685):**
```rust
// Source: crates/rc-agent/src/main.rs lines 460, 684-685
// In main.rs setup:
let (guard_violation_tx, guard_violation_rx) = mpsc::channel::<AgentMessage>(32);
// Stored in AppState, guard_violation_rx drained in event_loop.rs select!

// In event_loop.rs select! loop (new arm mirrors ws_exec_result handling):
Some(msg) = state.guard_violation_rx.recv() => {
    let json = serde_json::to_string(&msg)?;
    if ws_tx.send(Message::Text(json.into())).await.is_err() {
        break;
    }
}
```

### Pattern 7: Config in AgentConfig (ProcessGuardConfig)

**What:** New `[process_guard]` TOML section in rc-agent.toml. Add `ProcessGuardConfig` struct to config.rs with `#[serde(default)]` on AgentConfig field. Matches KioskConfig and PreflightConfig patterns.

**Example (from config.rs lines 34-44):**
```rust
// Source: crates/rc-agent/src/config.rs lines 34-44
#[derive(Debug, Deserialize)]
pub struct ProcessGuardConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_scan_interval")]
    pub scan_interval_secs: u64,
}

impl Default for ProcessGuardConfig {
    fn default() -> Self {
        Self { enabled: true, scan_interval_secs: 60 }
    }
}
// fn default_true() already exists in config.rs — reuse it
```

### Pattern 8: Whitelist in AppState (Arc<RwLock>)

**What:** `guard_whitelist: Arc<RwLock<MachineWhitelist>>` in AppState. Fetched on WS connect in main.rs (HTTP GET, stored into guard_whitelist). Guard task reads under read lock each scan cycle. UpdateProcessWhitelist handler in ws_handler.rs acquires write lock.

**Note:** AppState already imports `tokio::sync::{mpsc, watch}` — needs `RwLock` added to import.

### Anti-Patterns to Avoid

- **Merging into kiosk.rs:** kiosk.rs is session-scoped with LLM classification. Process guard is always-on, no LLM. Keep them separate.
- **Polling whitelist on a separate timer:** Fetch on WS connect only. Accept server push for mid-session changes. No extra timer.
- **Killing without grace period (except CRITICAL):** Two-cycle grace is mandatory for KILL-tier. Only `WrongMachineBinary` violations get zero grace.
- **Name-only kill (no PID verify):** Always verify `name + start_time` match before calling taskkill.
- **Blocking async task with sysinfo:** ALWAYS use `spawn_blocking` for scan. kiosk.rs documents this at line 646.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Process enumeration | Custom WMI or CreateToolhelp32Snapshot wrapper | sysinfo 0.33 (already in deps) | Already integrated, tested, cross-platform; kiosk.rs confirms it works |
| Log rotation | Custom ring buffer or file rename | self_monitor.rs `log_event()` pattern | Already proven at 512KB threshold, 4 lines of code |
| Registry key enumeration | Raw WinAPI RegEnumValue | `reg query` via Command | self_heal.rs uses Command for reg; no winreg dep needed |
| Consecutive scan tracking | Disk-backed persistence | `HashMap<String, u32>` in module scope (OnceLock/Mutex) | kiosk.rs uses exact same pattern for `unknown_sightings()` |
| WS message dispatch | New transport layer | Existing `ws_exec_result_tx` mpsc pattern | Already wired in AppState and event_loop.rs |

**Key insight:** Every sub-problem in this phase has an existing solved pattern in rc-agent. The task is assembly, not invention.

---

## Common Pitfalls

### Pitfall 1: sysinfo Called from Async Context Without spawn_blocking
**What goes wrong:** sysinfo::refresh_processes() blocks the tokio thread for 100-300ms. Called directly in async code, this stalls the event loop and causes heartbeat timeouts.
**Why it happens:** kiosk.rs documents this at line 646 — the warning is easy to miss when writing new code.
**How to avoid:** Every call to `System::refresh_processes()` must be inside `tokio::task::spawn_blocking`.
**Warning signs:** Heartbeat intervals start drifting, WS timeouts correlate with guard scan timing.

### Pitfall 2: PID Reuse Race Between Snapshot and Kill
**What goes wrong:** Between `sys.processes()` snapshot and `taskkill /PID` call, the target PID may be reused by a different process (possibly rc-agent itself).
**Why it happens:** Two separate syscalls with a time gap. On a busy gaming pod, PID reuse can happen in <50ms.
**How to avoid:** Before kill, do a fresh sysinfo lookup of the PID and verify `name + start_time` match snapshot values. Mismatch = PID reused = skip kill with warning log.
**Warning signs:** rc-agent crashes correlate with guard scan timing; random pod disconnects.

### Pitfall 3: winreg Crate Conflicts with winapi 0.3
**What goes wrong:** Adding `winreg = "0.52"` to Cargo.toml alongside `winapi = "0.3"` may create duplicate type definitions for Windows handle types.
**Why it happens:** STATE-v12.1.md explicitly warns: "Do NOT add `windows = "0.58"` — conflicts with existing `winapi 0.3`". The winreg crate may pull in conflicting dependencies.
**How to avoid:** Use `reg query` / `reg add` / `reg delete` via `std::process::Command` as self_heal.rs already does. No winreg crate needed.
**Warning signs:** Compile error with duplicate type definitions in winapi/windows crates.

### Pitfall 4: Self-Exclusion Via Name Check (Insufficient)
**What goes wrong:** Checking `name == "rc-agent.exe"` before scanning is not sufficient. The guard binary might be renamed, or a different binary might appear with that name.
**How to avoid:** Exclude by own PID (`std::process::id()`) unconditionally. Also exclude by canonical exe path (`std::env::current_exe()`). These cannot be spoofed by name-masquerading.
**Warning signs:** Guard exits within one scan cycle with no log output.

### Pitfall 5: Auto-Start REMOVE Before LOG/ALERT
**What goes wrong:** Removing a registry Run key before logging/alerting silently breaks services with no recovery path (auto-start entries don't come back like processes do).
**How to avoid:** Three-stage progression LOG → ALERT → REMOVE enforced in code. Stage tracked per-entry in `HashMap<String, AutoStartStage>`. Default stage = LOG. REMOVE only after explicit config (`autostart_enforcement = "remove"`). Always backup to `C:\RacingPoint\autostart-backup.json` before deletion.
**Warning signs:** Kiosk or services stop loading after pod restart without any deploy event.

### Pitfall 6: Grace Period Counter Reset on Agent Restart
**What goes wrong:** The `HashMap<String, u32>` tracking consecutive scan hits lives in process memory. rc-agent restart (self-monitor relaunch) resets it to 0. A violation process that triggered the restart survives indefinitely by restarting rc-agent.
**How to avoid:** At Phase 103 scale this is acceptable — report_only mode means no kills during initial rollout. Document as known behavior. When switching to kill_and_report, the two-cycle grace (60-120 seconds) is short enough that this edge case has no real-world impact.
**Warning signs:** Not a problem in report_only mode. Monitor in Phase 104.

### Pitfall 7: Case-Sensitive Process Name Comparison
**What goes wrong:** `name == "Steam.exe"` misses `steam.exe`, `STEAM.EXE`, etc. Windows process names are case-insensitive.
**How to avoid:** All comparisons use `.to_lowercase()` on both sides (same as kiosk.rs line 658: `process.name().to_string_lossy().to_lowercase()`). Whitelist entries normalized to lowercase at load time.
**Warning signs:** Known-bad processes slip through; whitelist entries must be exact-case to work.

---

## Code Examples

### Complete scan_cycle structure (synthesis of codebase patterns):
```rust
// Source: synthesized from kiosk.rs + pre_flight.rs patterns
async fn run_scan_cycle(
    whitelist: &Arc<RwLock<MachineWhitelist>>,
    sighting_map: &Mutex<HashMap<String, u32>>,
    violation_tx: &mpsc::Sender<AgentMessage>,
    own_pid: u32,
    own_name: String,
) {
    let wl = whitelist.read().await.clone();
    let process_set: HashSet<String> = wl.processes.iter()
        .map(|s| s.to_lowercase())
        .collect();

    // spawn_blocking because sysinfo blocks 100-300ms (kiosk.rs line 646 warning)
    let snapshot = tokio::task::spawn_blocking(move || {
        let mut sys = sysinfo::System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        sys.processes().map(|(pid, p)| (
            pid.as_u32(),
            p.name().to_string_lossy().to_lowercase(),
            p.exe().map(|e| e.to_string_lossy().to_string()).unwrap_or_default(),
            p.start_time(),
        )).collect::<Vec<_>>()
    }).await.unwrap_or_default();

    for (pid, name, exe_path, start_time) in snapshot {
        // Self-exclusion: unconditional, before any whitelist check
        if pid == own_pid || name == own_name { continue; }
        if pid <= 4 { continue; }  // System/Idle PIDs
        if process_set.contains(&name) { continue; }

        // CRITICAL zero-grace: racecontrol.exe on a pod
        if name == "racecontrol.exe" {
            // immediate kill + WS report (PROC-04)
            ...
            continue;
        }

        // Two-cycle grace (PROC-02): increment sighting, act on 2nd
        let count = { /* update sighting_map */ };
        if count < 2 {
            log_guard_event(&format!("FIRST_SIGHTING: {} (PID {})", name, pid));
            // send ProcessViolation with consecutive_count=1, action_taken="reported"
        } else {
            if wl.violation_action == "kill_and_report" {
                // PID identity verify before kill (PROC-03)
                // taskkill in spawn_blocking
            }
            // send ProcessViolation with consecutive_count=count, action_taken="killed"/"reported"
        }
    }
}
```

### Registry audit via reg command (AUTO-01):
```rust
// Source: self_heal.rs pattern (lines 207-222)
#[cfg(windows)]
fn query_run_keys(hive: &str) -> Vec<(String, String)> {
    // hive = "HKCU" or "HKLM"
    let key = format!(r"{}\Software\Microsoft\Windows\CurrentVersion\Run", hive);
    let output = std::process::Command::new("reg")
        .args(["query", &key])
        .creation_flags(0x08000000)  // CREATE_NO_WINDOW
        .output()
        .unwrap_or_default();
    // Parse: lines like "    ValueName    REG_SZ    C:\path\to.exe"
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| l.contains("REG_SZ") || l.contains("REG_EXPAND_SZ"))
        .filter_map(|l| {
            let parts: Vec<&str> = l.trim().splitn(3, "    ").collect();
            if parts.len() >= 2 { Some((parts[0].trim().to_string(), parts.last().unwrap_or(&"").trim().to_string())) }
            else { None }
        })
        .collect()
}
```

### Startup folder audit (AUTO-02):
```rust
// Source: std::fs::read_dir pattern, uses APPDATA env var
fn audit_startup_folder() -> Vec<String> {
    let mut entries = Vec::new();
    let paths = [
        std::env::var("APPDATA").ok().map(|a|
            format!(r"{}\Microsoft\Windows\Start Menu\Programs\Startup", a)),
        Some(r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Startup".to_string()),
    ];
    for path in paths.iter().flatten() {
        if let Ok(rd) = std::fs::read_dir(path) {
            for entry in rd.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    entries.push(name.to_string());
                }
            }
        }
    }
    entries
}
```

### AppState additions:
```rust
// Source: app_state.rs inspection — add two fields following ws_exec_result pattern (lines 42-43)
pub(crate) guard_whitelist: Arc<tokio::sync::RwLock<rc_common::types::MachineWhitelist>>,
pub(crate) guard_violation_tx: mpsc::Sender<AgentMessage>,
pub(crate) guard_violation_rx: mpsc::Receiver<AgentMessage>,
// Note: guard_violation_rx will be moved into event_loop drain; tx clone given to guard task
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual process audit (keyword search) | Whitelist-inversion (deny-by-default) | v12.1 trigger incident | Steam/leaderboard/watchdog all missed by keyword search |
| No auto-start enforcement | LOG → ALERT → REMOVE progression | v12.1 | Prevents silent service breakage from over-aggressive removal |
| kiosk.rs session-scoped monitoring | Always-on background daemon | Phase 103 | Covers idle time, non-session machines |

**Deprecated/outdated:**
- `ALLOWED_PROCESSES` static slice in kiosk.rs: that list is for kiosk session enforcement, NOT for process_guard.rs. Process guard uses the MachineWhitelist fetched from racecontrol.

---

## Open Questions

1. **winreg vs reg.exe for RunOnce keys**
   - What we know: self_heal.rs uses `reg.exe` via Command for HKLM Run. The CONTEXT.md says "winreg crate usage for registry operations" under Reusable Assets but STATE-v12.1.md says DO NOT add windows = "0.58" (conflicting crate).
   - What's unclear: Does self_heal.rs actually use winreg? Inspection confirms NO — it uses `reg` command. The CONTEXT.md reference to "winreg crate" in self_heal.rs appears to be an outdated/incorrect note.
   - Recommendation: Use `reg query` via Command (no new dep). Also audit HKCU\...\RunOnce and HKLM\...\RunOnce (not just Run).

2. **Sighting counter persistence across WS reconnects**
   - What we know: The `HashMap<String, u32>` sighting map lives in the guard task's local state (inside tokio::spawn closure). WS reconnect does NOT restart the guard task (background task lives for binary lifetime, same as self_monitor.rs).
   - What's unclear: Does the guard task need to receive the AppState reference or just the whitelist Arc?
   - Recommendation: Guard task receives `Arc<RwLock<MachineWhitelist>>` clone only. Sighting map is local to the task. Survives WS reconnects correctly.

3. **UpdateProcessWhitelist handler in ws_handler.rs**
   - What we know: protocol.rs line 469 defines `CoreToAgentMessage::UpdateProcessWhitelist { whitelist }`. The wildcard arm was added at Phase 101.
   - What's unclear: Whether Phase 103 should implement the handler now (replacing wildcard) or defer to Phase 104.
   - Recommendation: Implement the handler in Phase 103 since the AppState field is being added anyway. It's 5 lines of code and completes the integration.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test (`cargo test`) |
| Config file | none — inline `#[cfg(test)]` modules |
| Quick run command | `cargo test -p rc-agent-crate process_guard` |
| Full suite command | `cargo test -p rc-agent-crate && cargo test -p rc-common` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PROC-01 | scan_interval_secs config default = 60 | unit | `cargo test -p rc-agent-crate process_guard::tests::default_config_has_60s_interval` | ❌ Wave 0 |
| PROC-02 | self_exclusion_filters_own_pid | unit | `cargo test -p rc-agent-crate process_guard::tests::self_exclusion_own_pid_skipped` | ❌ Wave 0 |
| PROC-02 | self_exclusion_filters_rc_agent_name | unit | `cargo test -p rc-agent-crate process_guard::tests::self_exclusion_rc_agent_name_skipped` | ❌ Wave 0 |
| PROC-03 | name_mismatch_aborts_kill | unit | `cargo test -p rc-agent-crate process_guard::tests::pid_identity_mismatch_skips_kill` | ❌ Wave 0 |
| PROC-04 | racecontrol_on_pod_is_critical | unit | `cargo test -p rc-agent-crate process_guard::tests::racecontrol_exe_is_critical_violation` | ❌ Wave 0 |
| PROC-05 | severity_tier_kill_triggers_kill_action | unit | `cargo test -p rc-agent-crate process_guard::tests::kill_tier_violation_action` | ❌ Wave 0 |
| AUTO-01 | reg_output_parser_extracts_value_names | unit | `cargo test -p rc-agent-crate process_guard::tests::parse_reg_output_extracts_names` | ❌ Wave 0 |
| AUTO-02 | startup_folder_path_resolves_from_appdata | unit | `cargo test -p rc-agent-crate process_guard::tests::startup_folder_path_resolution` | ❌ Wave 0 |
| AUTO-04 | log_before_remove_progression | unit | `cargo test -p rc-agent-crate process_guard::tests::autostart_log_before_remove` | ❌ Wave 0 |
| ALERT-01 | violation_serializes_as_agent_message | unit | `cargo test -p rc-common process_guard_types_tests` (exists ✅) | ✅ exists |
| ALERT-04 | log_rotation_truncates_at_512kb | unit | `cargo test -p rc-agent-crate process_guard::tests::log_rotates_at_512kb` | ❌ Wave 0 |
| DEPLOY-01 | default_whitelist_is_report_only | unit | `cargo test -p rc-common process_guard_types_tests::machine_whitelist_default_has_report_only_action` (exists ✅) | ✅ exists |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-agent-crate process_guard`
- **Per wave merge:** `cargo test -p rc-agent-crate && cargo test -p rc-common`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/rc-agent/src/process_guard.rs` — module to create (no tests possible until file exists)
- [ ] `#[cfg(test)] mod tests` block in process_guard.rs — covers PROC-01 through ALERT-04
- [ ] Test for log rotation (ALERT-04) requires tempfile (already in `[dev-dependencies]`)
- [ ] ProcessGuardConfig Default impl test — add to config.rs tests block

*(Existing: `cargo test -p rc-common process_guard_types_tests` covers ALERT-01 and DEPLOY-01 default — 2 tests already green)*

---

## Sources

### Primary (HIGH confidence)
- Direct inspection: `crates/rc-agent/src/kiosk.rs` — sysinfo 0.33 usage, ALLOWED_PROCESSES, spawn_blocking warning, sighting counter pattern
- Direct inspection: `crates/rc-agent/src/self_monitor.rs` — 512KB log rotation pattern, tokio::spawn background task pattern
- Direct inspection: `crates/rc-agent/src/pre_flight.rs` — spawn_blocking + taskkill /PID pattern
- Direct inspection: `crates/rc-agent/src/app_state.rs` — AppState structure, ws_exec_result_tx/rx channel pattern
- Direct inspection: `crates/rc-agent/src/event_loop.rs` — ConnectionState, select! loop structure
- Direct inspection: `crates/rc-agent/src/ws_handler.rs` — CoreToAgentMessage dispatch pattern
- Direct inspection: `crates/rc-agent/src/self_heal.rs` — reg.exe via Command (confirms no winreg crate used)
- Direct inspection: `crates/rc-agent/src/config.rs` — AgentConfig struct, KioskConfig/PreflightConfig Default pattern
- Direct inspection: `crates/rc-agent/src/main.rs` — background task spawn pattern, ws_exec_result_tx wiring
- Direct inspection: `crates/rc-agent/Cargo.toml` — confirmed sysinfo = "0.33", winapi = "0.3", NO winreg crate
- Direct inspection: `crates/rc-common/src/types.rs` — MachineWhitelist, ProcessViolation, ViolationType (Phase 101)
- Direct inspection: `crates/rc-common/src/protocol.rs` — AgentMessage::ProcessViolation, ProcessGuardStatus, CoreToAgentMessage::UpdateProcessWhitelist (Phase 101)
- `.planning/phases/103-pod-guard-module/103-CONTEXT.md` — locked decisions, integration points
- `.planning/REQUIREMENTS-v12.1.md` — PROC-01 through DEPLOY-01 requirement text
- `.planning/STATE-v12.1.md` — decisions log (sysinfo 0.33 pin, winreg conflict warning, walkdir 2)
- `.planning/research/ARCHITECTURE.md` — system architecture, data flow, integration patterns
- `.planning/research/PITFALLS.md` — 12 pitfalls with prevention strategies

### Secondary (MEDIUM confidence)
- None needed — all findings from direct source inspection.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — directly verified from Cargo.toml, no guessing
- Architecture: HIGH — directly verified from main.rs, app_state.rs, event_loop.rs
- Pitfalls: HIGH — drawn from direct incident record + code inspection of kiosk.rs warnings
- API surface: HIGH — all four rc-common types confirmed present from Phase 101

**Research date:** 2026-03-21 IST
**Valid until:** 2026-05-21 (stable codebase, no fast-moving dependencies)

**Critical discovery:** `winreg` crate is NOT in rc-agent Cargo.toml and self_heal.rs uses `reg.exe` via Command instead. The CONTEXT.md mention of "winreg crate" in self_heal.rs is inaccurate. Registry audit must use `reg query` via Command to match existing patterns and avoid dep conflicts.
