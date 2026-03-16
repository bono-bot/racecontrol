# Phase 18: Startup Self-Healing - Research

**Researched:** 2026-03-15
**Domain:** Windows startup repair (config files, batch scripts, registry keys) from Rust + WebSocket startup reporting
**Confidence:** HIGH

## Summary

Phase 18 adds three capabilities to rc-agent: (1) self-repair of missing config file, start script, and registry key on every startup, (2) a startup status report sent to racecontrol immediately after WebSocket connection, and (3) a phased startup log that persists to disk even if rc-agent crashes mid-startup for post-mortem analysis.

The self-repair work follows the exact pattern established by Phase 16 (firewall.rs): a new module with a synchronous public function called early in main(), using `std::process::Command` for registry operations, `std::fs` for file operations, and non-fatal error handling (log warning + continue on failure). The config repair is the most nuanced piece -- rc-agent currently calls `load_config()` which fails and exits if no config file exists. Phase 18 must intercept this failure: when config is missing, regenerate it from an embedded template using `include_str!()` with the pod number derived from the hostname (Pod PCs are named "Pod-1" through "Pod-8") or from a minimal fallback. The start script and registry key repairs are straightforward file writes and `reg add` commands using patterns already in the codebase (lock_screen.rs, install.bat).

The startup report is a new `AgentMessage::StartupReport` variant sent once per WebSocket connection, immediately after `Register`. racecontrol receives it, logs it, and stores it in the pod state. No new protocol complexity -- just a new enum variant with flat fields (version, uptime_secs, config_hash, crash_recovery flag). The crash recovery flag is set when rc-agent detects a prior unclean shutdown (e.g., stale PID file or startup log from a previous run that ended mid-phase).

The phased startup log writes to `C:\RacingPoint\rc-agent-startup.log` at each phase of startup (config loaded, firewall configured, HTTP server started, WebSocket connected). If rc-agent crashes, the log shows the last phase reached. This is a simple append-to-file pattern -- not the tracing system, but a dedicated text file written with `std::fs::write` / `std::fs::OpenOptions::append`.

**Primary recommendation:** Create `self_heal.rs` module following the firewall.rs pattern. Three public functions: `repair_config()`, `repair_start_script()`, `repair_registry_key()`. Add `AgentMessage::StartupReport` to protocol. Add `startup_log::write_phase()` helper. Wire all into main.rs startup sequence.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| HEAL-01 | rc-agent verifies config file, start script, and registry key on every startup -- repairs if missing | `self_heal.rs` module with `repair_config()`, `repair_start_script()`, `repair_registry_key()` called before `load_config()` in main.rs. Config template embedded via `include_str!()`, start script content as const string, registry key via `reg add` command. |
| HEAL-02 | rc-agent reports startup status to racecontrol immediately after WebSocket connect (version, uptime, config hash, crash recovery flag) | New `AgentMessage::StartupReport` variant in protocol.rs. Sent once after `Register` message in the reconnection loop. Config hash computed via simple CRC/hash of config file contents. Crash recovery flag set when previous startup log shows incomplete startup. |
| HEAL-03 | Startup errors are captured to a log file before rc-agent exits (for post-mortem) | Phased startup log at `C:\RacingPoint\rc-agent-startup.log` written with `std::fs::OpenOptions::append`. Each startup phase writes a timestamped line. On crash, last line shows where it stopped. |
</phase_requirements>

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| std::fs | stdlib | File existence checks, reading/writing config + start script + startup log | No extra dep; same pattern as existing config_search_paths() |
| std::process::Command | stdlib | Registry operations via `reg add` / `reg query` | Same pattern as lock_screen.rs suppress_notifications(), firewall.rs |
| toml | 0.8 (workspace) | Parse and validate regenerated config | Already in Cargo.toml, used by load_config() |
| tracing | 0.1 (workspace) | Structured logging | Already used project-wide |
| include_str!() | stdlib | Embed config template at compile time | Zero-cost, no file I/O at runtime for template |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| std::collections::hash_map::DefaultHasher | stdlib | Compute config hash for startup report | Lightweight hash -- no crypto needed, just change detection |
| chrono | 0.4 (workspace) | Timestamp startup log entries | Already in Cargo.toml |
| gethostname (or std::env) | stdlib/env | Derive pod number from hostname for config regeneration | `std::env::var("COMPUTERNAME")` on Windows -- no extra dep |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `reg add` via Command | winreg crate (pure Rust registry API) | winreg adds a dependency; `reg` command matches existing lock_screen.rs pattern and is tested on all 8 pods. For 3 registry operations at startup, subprocess overhead is negligible. |
| include_str!() for template | Download template from racecontrol | Network-dependent defeats the purpose -- self-healing must work offline |
| DefaultHasher for config hash | sha256/md5 | Overkill -- hash is for change detection not security. DefaultHasher is zero-dep. |
| Phased startup log file | tracing file appender (already exists) | The tracing appender writes structured logs. The startup log needs a simple, human-readable, append-only file that survives even if tracing init fails. They serve different purposes. |

**No new dependencies needed.** Everything is already in the workspace Cargo.toml or the standard library.

## Architecture Patterns

### New Files

```
crates/rc-agent/src/
  self_heal.rs          # NEW -- repair_config(), repair_start_script(), repair_registry_key()
  startup_log.rs        # NEW -- write_phase(), detect_crash_recovery()
  main.rs               # MODIFIED -- wire self-heal before load_config, startup log, startup report
crates/rc-common/src/
  protocol.rs           # MODIFIED -- add AgentMessage::StartupReport variant
crates/racecontrol/src/
  ws/mod.rs             # MODIFIED -- handle StartupReport message (log + store)
```

### Pattern 1: Pre-Config Self-Heal (HEAL-01)

**What:** Before calling `load_config()`, run self-heal checks. If the config file is missing, regenerate it. If the start script or registry key is missing, repair them. All repairs are logged but non-fatal -- if repair fails, the existing error handling in `load_config()` catches it.

**When to use:** Every startup, unconditionally.

**Key insight -- startup ordering:**

Current main.rs startup flow:
```
1. Single-instance mutex
2. Logging init (tracing)
3. Early lock screen server
4. load_config()              <-- FAILS if config missing, exits process
5. Firewall auto-config
6. Remote ops HTTP server
7. ... rest of initialization
8. WebSocket reconnection loop
```

Phase 18 inserts self-heal BEFORE step 4:
```
1. Single-instance mutex
2. Logging init (tracing)
3. Startup log: "phase=init"
4. Early lock screen server
5. Startup log: "phase=lock_screen"
6. self_heal::run()           <-- NEW: checks config, script, registry
7. Startup log: "phase=self_heal_complete"
8. load_config()
9. Startup log: "phase=config_loaded"
10. Firewall auto-config
11. Startup log: "phase=firewall"
12. Remote ops HTTP server
13. Startup log: "phase=http_server"
14. ... rest of initialization
15. WebSocket connection + StartupReport
```

**Example:**
```rust
// Source: follows firewall.rs pattern
pub struct SelfHealResult {
    pub config_repaired: bool,
    pub script_repaired: bool,
    pub registry_repaired: bool,
    pub errors: Vec<String>,
}

pub fn run(exe_dir: &Path) -> SelfHealResult {
    let mut result = SelfHealResult {
        config_repaired: false,
        script_repaired: false,
        registry_repaired: false,
        errors: Vec::new(),
    };

    // Check and repair config file
    let config_path = exe_dir.join("rc-agent.toml");
    if !config_path.exists() {
        match repair_config(&config_path) {
            Ok(()) => {
                tracing::warn!("[self-heal] Regenerated missing config: {}", config_path.display());
                result.config_repaired = true;
            }
            Err(e) => {
                tracing::error!("[self-heal] Failed to repair config: {}", e);
                result.errors.push(format!("config: {}", e));
            }
        }
    }

    // Check and repair start script
    let script_path = exe_dir.join("start-rcagent.bat");
    if !script_path.exists() {
        match repair_start_script(&script_path) {
            Ok(()) => {
                tracing::warn!("[self-heal] Regenerated missing start script: {}", script_path.display());
                result.script_repaired = true;
            }
            Err(e) => {
                tracing::error!("[self-heal] Failed to repair start script: {}", e);
                result.errors.push(format!("script: {}", e));
            }
        }
    }

    // Check and repair HKLM Run key
    if !registry_key_exists() {
        match repair_registry_key(exe_dir) {
            Ok(()) => {
                tracing::warn!("[self-heal] Recreated missing HKLM Run key");
                result.registry_repaired = true;
            }
            Err(e) => {
                tracing::error!("[self-heal] Failed to repair registry key: {}", e);
                result.errors.push(format!("registry: {}", e));
            }
        }
    }

    result
}
```

### Pattern 2: Config Regeneration from Embedded Template

**What:** The config template is embedded at compile time via `include_str!("../../deploy/rc-agent.template.toml")`. At runtime, the pod number is derived from the Windows hostname (`COMPUTERNAME` environment variable, expected pattern "Pod-N" or "Pod N"). The template placeholders `{pod_number}` and `{pod_name}` are replaced.

**Why hostname-based:** When the config file is deleted, no other source of truth exists for the pod number. The hostname is set during initial Windows setup and is stable. Each pod PC has a unique hostname following the "Pod-N" pattern. This is a reasonable fallback -- the alternative (hardcoding a default pod number) would cause conflicts if two pods regenerated configs simultaneously.

**Fallback:** If hostname parsing fails (unexpected format), log an error and do NOT regenerate config. A wrong pod number is worse than no config. Let the existing `load_config()` error path handle it (shows branded error on lock screen, exits).

```rust
const CONFIG_TEMPLATE: &str = include_str!("../../deploy/rc-agent.template.toml");

fn repair_config(config_path: &Path) -> Result<()> {
    let pod_number = detect_pod_number()?;
    let pod_name = format!("Pod {}", pod_number);

    let content = CONFIG_TEMPLATE
        .replace("{pod_number}", &pod_number.to_string())
        .replace("{pod_name}", &pod_name);

    // Validate before writing -- parse it back to catch template errors
    let _: toml::Value = toml::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Generated config is invalid TOML: {}", e))?;

    std::fs::write(config_path, &content)?;
    Ok(())
}

fn detect_pod_number() -> Result<u32> {
    let hostname = std::env::var("COMPUTERNAME")
        .map_err(|_| anyhow::anyhow!("COMPUTERNAME not set"))?;

    // Expected patterns: "Pod-1", "Pod-2", ..., "Pod-8"
    // Also handle "POD-1", "pod1", "Pod 1" etc.
    let digits: String = hostname.chars().filter(|c| c.is_ascii_digit()).collect();
    let num: u32 = digits.parse()
        .map_err(|_| anyhow::anyhow!("Cannot parse pod number from hostname '{}'", hostname))?;

    if num < 1 || num > 8 {
        return Err(anyhow::anyhow!("Pod number {} from hostname '{}' is out of range 1-8", num, hostname));
    }

    Ok(num)
}
```

### Pattern 3: Start Script Repair (Hardcoded Content)

**What:** The start-rcagent.bat content is simple and stable (13 lines). Embed it as a const string and write it verbatim. No template variables needed -- the script references `C:\RacingPoint` which is the same on all pods.

**Critical detail -- CRLF:** The bat file MUST use CRLF line endings on Windows. Use `\r\n` explicitly in the const string, or write with a helper that ensures CRLF.

```rust
const START_SCRIPT_CONTENT: &str = "\
@echo off\r\n\
cd /d C:\\RacingPoint\r\n\
start \"\" /D C:\\RacingPoint rc-agent.exe\r\n";
```

Note: The existing start-rcagent.bat includes netsh, taskkill, and binary-swap logic. For self-healing, the minimal script (cd + start) is sufficient -- the firewall is handled by rc-agent itself (Phase 16), and binary swaps happen via deploy (Phase 20). A simpler script is more resilient.

### Pattern 4: Registry Key Repair

**What:** Check if `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run\RCAgent` exists using `reg query`. If absent, recreate it using `reg add` pointing to `C:\RacingPoint\start-rcagent.bat`.

**Registry interaction uses the same pattern as lock_screen.rs and install.bat:**

```rust
fn registry_key_exists() -> bool {
    let output = Command::new("reg")
        .args(["query",
               r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
               "/v", "RCAgent"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    match output {
        Ok(o) => o.status.success(),
        Err(_) => false,  // Can't check -- assume missing
    }
}

fn repair_registry_key(exe_dir: &Path) -> Result<()> {
    let script_path = exe_dir.join("start-rcagent.bat");
    let output = Command::new("reg")
        .args(["add",
               r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
               "/v", "RCAgent",
               "/d", &script_path.display().to_string(),
               "/f"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("reg add failed: {}", stderr.trim()));
    }
    Ok(())
}
```

**Admin requirement:** `reg add` to HKLM requires admin privileges. rc-agent already runs elevated (via HKLM Run key that launches start-rcagent.bat, which inherits elevation from the admin session). If privileges are missing, the `reg add` fails gracefully with a logged error.

### Pattern 5: Startup Report via WebSocket (HEAL-02)

**What:** New `AgentMessage::StartupReport` sent once, immediately after the `Register` message in the reconnection loop.

```rust
// In protocol.rs
AgentMessage::StartupReport {
    pod_id: String,
    version: String,
    uptime_secs: u64,
    config_hash: String,
    crash_recovery: bool,
    repairs: Vec<String>,  // e.g. ["config", "registry_key"]
}
```

**Fields:**
- `version`: `env!("CARGO_PKG_VERSION")` -- compile-time version string (currently "0.1.0")
- `uptime_secs`: seconds since `agent_start_time` (already tracked in main.rs)
- `config_hash`: hex string of DefaultHasher applied to config file bytes
- `crash_recovery`: true if previous startup log shows incomplete (no "phase=complete" line)
- `repairs`: list of items that were repaired this startup (empty if nothing was broken)

**Core side (ws/mod.rs):** Handle the new variant by logging it and optionally storing in a new `pod_startup_reports` field on AppState (or just enriching the existing pods RwLock). For Phase 18, logging is sufficient -- Phase 21 (Fleet Dashboard) will display this data.

### Pattern 6: Phased Startup Log (HEAL-03)

**What:** A simple text file at `C:\RacingPoint\rc-agent-startup.log` that records each phase of startup with a timestamp. On each startup, the previous log is overwritten (not appended -- we only care about the most recent startup).

```
2026-03-15T14:30:01Z phase=init
2026-03-15T14:30:01Z phase=lock_screen
2026-03-15T14:30:01Z phase=self_heal repairs=config,registry_key
2026-03-15T14:30:02Z phase=config_loaded pod=3
2026-03-15T14:30:02Z phase=firewall status=configured
2026-03-15T14:30:02Z phase=http_server port=8090
2026-03-15T14:30:03Z phase=websocket connected=true
2026-03-15T14:30:03Z phase=complete
```

**Implementation:** A `startup_log` module with two functions:
- `write_phase(phase: &str, details: &str)` -- appends a line to the log file
- `detect_crash_recovery() -> bool` -- reads the existing log file (from previous run), returns true if the last line is NOT "phase=complete"

The very first call to `write_phase` overwrites the file (truncate). Subsequent calls append. This is done by checking if the file was already opened this run (static flag or passing a file handle).

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use std::fs::OpenOptions;
use std::io::Write;

const LOG_PATH: &str = r"C:\RacingPoint\rc-agent-startup.log";
static LOG_INITIALIZED: AtomicBool = AtomicBool::new(false);

pub fn write_phase(phase: &str, details: &str) {
    let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let line = if details.is_empty() {
        format!("{} phase={}\n", timestamp, phase)
    } else {
        format!("{} phase={} {}\n", timestamp, phase, details)
    };

    let result = if !LOG_INITIALIZED.swap(true, Ordering::SeqCst) {
        // First write this run -- truncate
        std::fs::write(LOG_PATH, &line)
    } else {
        // Subsequent writes -- append
        OpenOptions::new().append(true).open(LOG_PATH)
            .and_then(|mut f| f.write_all(line.as_bytes()))
    };

    if let Err(e) = result {
        // Best effort -- don't crash if log write fails
        eprintln!("[startup-log] Failed to write: {}", e);
    }
}

pub fn detect_crash_recovery() -> bool {
    match std::fs::read_to_string(LOG_PATH) {
        Ok(content) => {
            let last_line = content.lines().last().unwrap_or("");
            !last_line.contains("phase=complete")
        }
        Err(_) => false,  // No previous log -- not a crash recovery
    }
}
```

### Anti-Patterns to Avoid

- **Writing config without validating it first:** Always parse the regenerated TOML back and validate before writing to disk. A malformed config is worse than no config.
- **Silently overwriting existing config:** Only regenerate when the file is MISSING, never when it exists but has different content. Config changes should be intentional.
- **Using the winreg crate for 3 registry operations:** Adds a dependency for minimal benefit. The `reg` command is already proven in this codebase.
- **Making startup report block WebSocket connection:** Send StartupReport as fire-and-forget after Register. If it fails, log and continue.
- **Using tracing for the startup log:** The startup log must work even if tracing initialization fails. It is a separate, simpler mechanism.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Config template | Custom serialization | include_str!() + string replace | Template already exists at deploy/rc-agent.template.toml; reuse it |
| Registry operations | Windows API bindings | `reg` command via std::process::Command | Proven pattern in lock_screen.rs and install.bat; no new deps |
| Config hashing | Custom hash function | std::collections::hash_map::DefaultHasher | Standard library; consistent enough for change detection |
| CRLF handling | Manual byte manipulation | Explicit `\r\n` in const strings | Batch files need CRLF; embedding it directly avoids conversion bugs |

**Key insight:** This is infrastructure code that must be maximally reliable. Prefer simple, proven patterns over clever abstractions. Every self-heal function should be testable in isolation with filesystem mocks (tempdir).

## Common Pitfalls

### Pitfall 1: Config Regeneration with Wrong Pod Number
**What goes wrong:** If hostname detection fails or returns the wrong number, a regenerated config will cause the pod to register as the wrong pod in racecontrol, creating ghost pods and session conflicts.
**Why it happens:** Hostname format assumptions break if someone renames the PC.
**How to avoid:** (1) Validate pod number is 1-8 after parsing. (2) If hostname doesn't match expected pattern, DO NOT regenerate config -- let load_config() fail with a clear error. (3) Log the hostname and detected number at WARN level so post-mortem is easy.
**Warning signs:** Two pods registering with the same pod_id; core sees duplicate registrations.

### Pitfall 2: Race Between Config Repair and load_config()
**What goes wrong:** self_heal::run() writes a config file, but load_config() still fails because it's looking in a different directory (CWD vs exe-dir).
**Why it happens:** config_search_paths() checks exe_dir first, then CWD. self_heal must write to the SAME path that load_config() will check first.
**How to avoid:** Use the same `exe_dir` logic from config_search_paths() -- the path returned by `std::env::current_exe().parent()`. Self-heal writes to that exact path.
**Warning signs:** Config file exists at C:\RacingPoint\rc-agent.toml but load_config() reports "No config file found" (it's looking in the wrong directory).

### Pitfall 3: Start Script with LF Line Endings
**What goes wrong:** `start-rcagent.bat` with Unix LF line endings (\n only) fails silently on Windows -- cmd.exe either ignores lines or misinterprets them.
**Why it happens:** Rust's default write uses the platform's line ending on some APIs but not all. String literals with `\n` write LF, not CRLF.
**How to avoid:** Explicitly use `\r\n` in the const string. Write with `std::fs::write()` (which does byte-exact writes, no conversion). Verify in tests that the output contains `\r\n`.
**Warning signs:** Start script exists but rc-agent doesn't auto-start after reboot.

### Pitfall 4: Registry Repair Without Admin Privileges
**What goes wrong:** `reg add HKLM\...` fails because rc-agent isn't running elevated. The self-heal reports success (because the check ran) but the repair failed.
**Why it happens:** If start-rcagent.bat was deleted and rc-agent was started manually (double-click), it may not have admin privileges.
**How to avoid:** Check the return code of `reg add`. Log failure as WARN, not ERROR (it's expected in dev/testing). The registry repair is best-effort -- the critical path is config repair.
**Warning signs:** Registry key repair attempted but `reg add` returns exit code 1 with "Access is denied."

### Pitfall 5: Startup Log Fails Before Tracing Init
**What goes wrong:** startup_log::write_phase() is called before tracing is initialized, so if the log write fails, there's no way to know.
**Why it happens:** The startup log is designed to work before tracing. But the file might not be writable (permissions, disk full).
**How to avoid:** Use `eprintln!()` as fallback for log write failures (goes to stderr which is captured by the parent process / watchdog). Never panic on log write failure.
**Warning signs:** Empty or missing rc-agent-startup.log after a crash.

### Pitfall 6: Startup Report Sent on Every Reconnect
**What goes wrong:** StartupReport is sent after every WebSocket reconnect, not just on initial startup. This pollutes logs with repeated startup reports from brief disconnections.
**Why it happens:** The reconnection loop in main.rs runs Register on every connect.
**How to avoid:** Send StartupReport only on the FIRST successful WebSocket connection (when `reconnect_attempt == 0` is reset). Use a `startup_report_sent` bool flag that's set after the first successful send.
**Warning signs:** racecontrol logs show startup reports every few seconds during a flaky connection period.

## Code Examples

Verified patterns from the existing codebase:

### Registry Query (from lock_screen.rs pattern)
```rust
// Source: lock_screen.rs:529 -- uses reg command for Windows registry
fn registry_key_exists() -> bool {
    let mut cmd = Command::new("reg");
    cmd.args([
        "query",
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
        "/v", "RCAgent",
    ]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    match cmd.output() {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}
```

### Non-Fatal Startup Operation (from firewall.rs)
```rust
// Source: main.rs:384-392 -- firewall configure is non-fatal
match firewall::configure() {
    firewall::FirewallResult::Configured => {
        tracing::info!("Firewall configured");
    }
    firewall::FirewallResult::Failed(msg) => {
        tracing::warn!("Firewall config failed: {} -- continuing anyway", msg);
    }
}
```

### Config Loading Path Detection (from main.rs:1927-1941)
```rust
// Source: main.rs -- config_search_paths()
fn config_search_paths() -> Vec<std::path::PathBuf> {
    let mut paths = Vec::new();
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            paths.push(exe_dir.join("rc-agent.toml"));
        }
    }
    paths.push(PathBuf::from("rc-agent.toml"));
    paths.push(PathBuf::from("/etc/racecontrol/rc-agent.toml"));
    paths
}
```

### WebSocket Message Sending (from main.rs:604-619)
```rust
// Source: main.rs -- Register message sent after WS connect
let register_msg = AgentMessage::Register(PodInfo { ... });
let json = serde_json::to_string(&register_msg)?;
if ws_tx.send(Message::Text(json.into())).await.is_err() {
    tracing::warn!("Failed to register with core");
    continue;  // reconnect
}
```

### Config Template (actual production file)
```toml
# Source: deploy/rc-agent.template.toml
[pod]
number = {pod_number}
name = "{pod_name}"
sim = "assetto_corsa"
sim_ip = "127.0.0.1"
sim_port = 9996

[core]
url = "ws://192.168.31.23:8080/ws/agent"

[games.assetto_corsa]
steam_app_id = 244210
use_steam = true

[games.f1_25]
steam_app_id = 2488620
use_steam = true

[games.le_mans_ultimate]
steam_app_id = 1564310
use_steam = true

[games.forza]
steam_app_id = 2440510
use_steam = true

[games.iracing]
exe_path = "C:\\Program Files (x86)\\iRacing\\iRacingSim64DX11.exe"
use_steam = false

[ai_debugger]
enabled = true
ollama_url = "http://192.168.31.27:11434"
ollama_model = "qwen2.5-coder:14b"
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| fix-firewall.bat (CRLF-sensitive) | firewall.rs (Rust, Phase 16) | 2026-03-15 | Batch files eliminated for firewall; Phase 18 extends this pattern to config/script/registry |
| Manual config copy via pendrive | install.bat + deploy-staging | 2026-03-11 | Config deployed via install.bat; Phase 18 adds self-repair when file is deleted |
| No crash diagnostics | Tracing to rc-agent.log | Phase 1 | rc-agent.log exists but is overwritten per session; Phase 18 adds a persistent startup-specific log |
| No startup report | Heartbeat only | Phase 1 | Heartbeat has pod info but no version/uptime/config hash; Phase 18 adds explicit startup report |

**Deprecated/outdated:**
- `fix-firewall.bat`: Superseded by firewall.rs in Phase 16
- `watchdog-rcagent.bat`: Deleted in Phase 1 cleanup; Phase 19 will add Rust watchdog
- The old `rc-agent.example.toml` uses `[agent]` section names; production uses `[pod]` and `[core]`. The template.toml is the authoritative format.

## Open Questions

1. **Pod hostname format verification**
   - What we know: Pods are named "Pod-1" through "Pod-8" based on install.bat and MEMORY.md
   - What's unclear: Are all 8 pod hostnames confirmed to follow this exact pattern? Any spaces vs dashes inconsistency?
   - Recommendation: Parse flexibly (extract digits, validate 1-8). Log the raw hostname at WARN when regenerating config so discrepancies are visible.

2. **Config template staleness**
   - What we know: deploy/rc-agent.template.toml exists and matches production configs
   - What's unclear: If the template is updated in the repo but not yet compiled into a deployed binary, pods will regenerate stale configs
   - Recommendation: This is acceptable -- the template will always be current as of the build. Config changes require a new binary deploy anyway.

3. **Start script content -- minimal vs full**
   - What we know: Current start-rcagent.bat has netsh, taskkill, and binary-swap logic. Self-healed version should be minimal.
   - What's unclear: Does Phase 19 (Watchdog) need the full script or minimal?
   - Recommendation: Use the full current script content for compatibility. Phase 19 can simplify it later. Embed the full content as a const.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | Cargo.toml workspace |
| Quick run command | `cargo test -p rc-agent-crate -- self_heal` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| HEAL-01a | Config repair: generates valid TOML from template + pod number | unit | `cargo test -p rc-agent-crate -- self_heal::tests::test_repair_config -x` | Wave 0 |
| HEAL-01b | Config repair: detects pod number from hostname | unit | `cargo test -p rc-agent-crate -- self_heal::tests::test_detect_pod_number -x` | Wave 0 |
| HEAL-01c | Config repair: rejects invalid pod numbers (0, 9, non-numeric) | unit | `cargo test -p rc-agent-crate -- self_heal::tests::test_detect_pod_number_invalid -x` | Wave 0 |
| HEAL-01d | Start script repair: writes CRLF content | unit | `cargo test -p rc-agent-crate -- self_heal::tests::test_repair_start_script_crlf -x` | Wave 0 |
| HEAL-01e | Registry key check: reg query parsing | unit | `cargo test -p rc-agent-crate -- self_heal::tests::test_registry_key -x` | Wave 0 |
| HEAL-01f | Self-heal skips repair when files exist | unit | `cargo test -p rc-agent-crate -- self_heal::tests::test_no_repair_when_exists -x` | Wave 0 |
| HEAL-02a | StartupReport serde roundtrip | unit | `cargo test -p rc-common -- protocol::tests::test_startup_report -x` | Wave 0 |
| HEAL-02b | Config hash is deterministic | unit | `cargo test -p rc-agent-crate -- self_heal::tests::test_config_hash -x` | Wave 0 |
| HEAL-03a | Startup log write_phase creates file | unit | `cargo test -p rc-agent-crate -- startup_log::tests::test_write_phase -x` | Wave 0 |
| HEAL-03b | Crash recovery detection from incomplete log | unit | `cargo test -p rc-agent-crate -- startup_log::tests::test_detect_crash -x` | Wave 0 |
| HEAL-03c | No crash recovery from complete log | unit | `cargo test -p rc-agent-crate -- startup_log::tests::test_no_crash -x` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-agent-crate -- self_heal && cargo test -p rc-common -- protocol::tests::test_startup_report`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/rc-agent/src/self_heal.rs` -- covers HEAL-01 (config, script, registry repair)
- [ ] `crates/rc-agent/src/startup_log.rs` -- covers HEAL-03 (phased startup log)
- [ ] `AgentMessage::StartupReport` variant in protocol.rs -- covers HEAL-02 (serde tests)

## Sources

### Primary (HIGH confidence)
- Codebase: `crates/rc-agent/src/main.rs` -- current startup flow, load_config(), config_search_paths()
- Codebase: `crates/rc-agent/src/firewall.rs` -- reference pattern for self-heal module structure
- Codebase: `crates/rc-agent/src/lock_screen.rs:529` -- registry interaction via `reg` command
- Codebase: `crates/rc-common/src/protocol.rs` -- current AgentMessage enum (where StartupReport goes)
- Codebase: `deploy/rc-agent.template.toml` -- config template with placeholders
- Codebase: `deploy-staging/install.bat:123` -- registry key setup pattern (reg add HKLM)
- Codebase: `deploy-staging/start-rcagent.bat` -- current start script content

### Secondary (MEDIUM confidence)
- Windows documentation: `reg query` returns exit code 0 on found, 1 on not found -- consistent with observed behavior in install.bat
- Windows documentation: HKLM Run key format is `REG_SZ` value pointing to executable/script path

### Tertiary (LOW confidence)
- Pod hostname format ("Pod-N") -- from MEMORY.md, not verified on all 8 pods. Parsing should be flexible.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries already in workspace, no new deps
- Architecture: HIGH -- follows proven firewall.rs pattern, all interactions verified in codebase
- Pitfalls: HIGH -- derived from actual production incidents (CRLF bugs, admin privilege failures, config path mismatches)
- Config regeneration: MEDIUM -- hostname parsing depends on actual pod hostnames matching expected pattern

**Research date:** 2026-03-15
**Valid until:** 2026-04-15 (stable domain -- Windows registry and file operations don't change)
