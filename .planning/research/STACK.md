# Stack Research: RC Sentry AI Debugger (v11.2)

**Domain:** Crash diagnostics and AI debugging watchdog in existing Rust binary
**Researched:** 2026-03-21 IST
**Confidence:** HIGH (all versions verified against actual Cargo.toml files in repo; Ollama HTTP pattern verified from rc-agent/src/ai_debugger.rs)

---

## Critical Architectural Constraint

rc-sentry is intentionally pure `std` — no tokio, no async, thread-per-connection model.
The "no new crate dependencies" constraint means: do not add crates not already present
in the workspace `Cargo.lock`. The workspace already locks `tokio`, `reqwest`, `serde_json`,
`chrono`, and `tracing` — these can be referenced in rc-sentry's `Cargo.toml` without
changing the lock file.

**Decision: Keep rc-sentry pure std. Use std::thread for watchdog loop. Use std::net::TcpStream for Ollama HTTP call.**

Rationale: adding `tokio` to rc-sentry's `[dependencies]` increases binary size, requires
runtime initialization in `main()`, and breaks the deliberate design of rc-sentry as a
minimal fallback tool. The watchdog loop is a single background thread sleeping 5 seconds
— no async needed. The Ollama query is a single blocking HTTP request on a dedicated
thread — no reqwest needed either.

---

## Existing Crates Already Present in rc-sentry (No Change)

| Crate | Version | Already Used For |
|-------|---------|-----------------|
| `serde` | workspace `"1"` | JSON deserialization |
| `serde_json` | workspace `"1"` | debug-memory.json read/write |
| `tracing` | workspace `"0.1"` | structured logging |
| `tracing-subscriber` | workspace `"0.3"` | log output |
| `toml` | workspace `"0.8"` | config file reading |
| `rc-common` | path dep | shared exec primitives |
| `sysinfo` | `"0.33"` | /processes endpoint (already present) |

All of these are already in `crates/rc-sentry/Cargo.toml`.

---

## New Crates to Add to rc-sentry Cargo.toml

### Required Additions

| Crate | Version | Purpose | Why |
|-------|---------|---------|-----|
| `tokio` | workspace `"1"` | — | **NOT NEEDED** — see architectural decision above |
| `reqwest` | `"0.12"` | — | **NOT NEEDED** — Ollama via std HTTP (see below) |
| `chrono` | workspace `"0.4"` | Timestamps in crash records, last_seen field | Already workspace dep; needed for DebugIncident.last_seen |
| `anyhow` | workspace `"1"` | Error propagation in watchdog/log analysis | Already workspace dep |

**Net new crates added to rc-sentry Cargo.toml: `chrono`, `anyhow`**

Both are already in the workspace and locked — zero new entries in `Cargo.lock`.

---

## Implementation Patterns

### Pattern 1: Health Polling Watchdog (std::thread)

Use a background thread started at `main()` that polls `localhost:8090/health` every 5
seconds using `std::net::TcpStream` with a raw HTTP/1.1 GET request. No reqwest, no tokio.

```rust
// Cargo.toml: no new deps required
// Uses: std::net::TcpStream, std::thread, std::time::Duration (all std)

fn start_watchdog(config: SentryConfig) {
    std::thread::Builder::new()
        .name("agent-watchdog".into())
        .spawn(move || watchdog_loop(config))
        .expect("watchdog thread");
}

fn poll_agent_health(port: u16) -> bool {
    // Raw HTTP GET to localhost:8090/health via std::net::TcpStream
    // Returns true if 200 received within 3s timeout
    use std::net::TcpStream;
    use std::io::{Read, Write};
    let addr = format!("127.0.0.1:{}", port);
    let Ok(mut stream) = TcpStream::connect_timeout(
        &addr.parse().unwrap(),
        std::time::Duration::from_secs(3),
    ) else {
        return false;
    };
    let _ = stream.write_all(b"GET /health HTTP/1.0\r\nHost: localhost\r\n\r\n");
    let mut buf = [0u8; 64];
    let _ = stream.read(&mut buf);
    buf.starts_with(b"HTTP/1.0 200") || buf.starts_with(b"HTTP/1.1 200")
}
```

**Why not reqwest:** reqwest requires tokio runtime. A 5-second polling loop does not need
async — it blocks intentionally. `std::net::TcpStream` is 20 lines and zero dependencies.

**Why not tokio::time::interval:** Same reason — rc-sentry has no tokio runtime.

### Pattern 2: Crash Log Analysis (std::fs)

Read startup_log, stderr capture, panic output using `std::fs::read_to_string`.
Parse for known crash signatures with string matching. No new crates needed.

```rust
// All std — no new deps
fn read_crash_logs(base_path: &str) -> CrashLogBundle {
    let startup_log = std::fs::read_to_string(
        format!("{}\\startup_log.txt", base_path)
    ).unwrap_or_default();
    let stderr_capture = std::fs::read_to_string(
        format!("{}\\rc-agent-stderr.txt", base_path)
    ).unwrap_or_default();
    // Truncate to last 4KB to match rc-sentry's 64KB safety discipline
    CrashLogBundle { startup_log: tail_4k(&startup_log), stderr_capture: tail_4k(&stderr_capture) }
}
```

### Pattern 3: Pattern Memory JSON (serde + serde_json + chrono)

Port `DebugMemory` and `DebugIncident` structs directly from
`crates/rc-agent/src/ai_debugger.rs`. The implementation is already proven — atomic
write via temp file + rename, 100-entry cap, success_count sorting.

`chrono` is the only addition needed (for `Utc::now().to_rfc3339()` in `last_seen`).

```toml
# Add to crates/rc-sentry/Cargo.toml:
chrono = { workspace = true }
anyhow = { workspace = true }
```

Key difference from rc-agent version: rc-sentry's `DebugMemory` keys on log content
patterns (not SimType/exit_code), since sentry sees crash logs, not game crash events.
Pattern key = first crash signature line hash (no SimType available in sentry context).

### Pattern 4: Ollama HTTP Query (std::net::TcpStream)

Use a raw HTTP POST to `192.168.31.27:11434/api/generate` via `std::net::TcpStream`.
Build the JSON body with `serde_json::to_string`. Parse response with `serde_json`.
No reqwest, no tokio, no ollama-rs.

```rust
// Uses: std::net::TcpStream + serde_json (already present)
fn query_ollama_blocking(url: &str, model: &str, prompt: &str) -> anyhow::Result<String> {
    // Build JSON body
    let body = serde_json::to_string(&serde_json::json!({
        "model": model,
        "prompt": prompt,
        "stream": false,
    }))?;
    // Raw HTTP/1.1 POST via TcpStream (Ollama accepts HTTP/1.0 and HTTP/1.1)
    // 30s timeout to match rc-agent's existing query_ollama timeout
    // Parse response: serde_json::from_str on the body portion
    Ok(extract_ollama_response(&body)?)
}
```

**Why not ollama-rs crate:** Adds reqwest + tokio deps. Overkill for a single blocking
call in a std thread.

**Why not reqwest blocking feature:** Internally bundles its own tokio runtime — adds
~800KB to binary size for one HTTP call.

### Pattern 5: Anti-Cheat Safe Process Monitoring

**Do NOT use:** `sysinfo::System::processes()`, `tasklist`, Windows `CreateToolhelp32Snapshot`,
`OpenProcess`, `NtQuerySystemInformation`, or any process-enumeration API.

Easy Anti-Cheat (F1 25) and iRacing both flag external process inspection as cheat tooling.

**Use instead:** HTTP health endpoint polling (`localhost:8090/health`). If rc-agent is
alive, the health endpoint responds. If rc-agent is dead, the TCP connection is refused.
This is identical to what a load balancer health check does — zero anti-cheat surface.

For process restart (after crash confirmed), use `std::process::Command` to spawn
`start-rcagent.bat` — this is command execution, not process inspection, and is
anti-cheat safe.

```rust
// SAFE: spawn rc-agent via bat file (same as HKLM Run key does)
fn restart_agent() -> bool {
    std::process::Command::new("cmd")
        .args(["/C", r"C:\RacingPoint\start-rcagent.bat"])
        .spawn()
        .is_ok()
}
// UNSAFE (DO NOT USE for games): sysinfo::System::processes()
// UNSAFE (DO NOT USE for games): win32 CreateToolhelp32Snapshot
```

The `/processes` endpoint on rc-sentry itself uses `sysinfo` — but that endpoint is
invoked by staff tools, not by the watchdog. The watchdog must never call sysinfo on game
processes.

---

## Cargo.toml Changes Required

### `crates/rc-sentry/Cargo.toml` — Add 2 lines

```toml
[dependencies]
# existing deps unchanged...
serde = { workspace = true }
serde_json = { workspace = true }
toml = { workspace = true }
rc-common = { path = "../rc-common" }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
sysinfo = "0.33"
# NEW (both already workspace-locked, zero Cargo.lock changes):
chrono = { workspace = true }
anyhow = { workspace = true }
```

**No other Cargo.toml changes needed.**

---

## What NOT to Add

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `tokio` in rc-sentry | Requires async runtime init, breaks std thread model, bloats binary | `std::thread` + `std::time::Duration` |
| `reqwest` in rc-sentry | Pulls in tokio runtime internally even with blocking feature | `std::net::TcpStream` raw HTTP |
| `ollama-rs` crate | New dep not in workspace; wraps reqwest/tokio; overkill for one call | Raw TcpStream POST + serde_json |
| `sysinfo::processes()` for game detection | Triggers Easy Anti-Cheat (F1 25) and iRacing anti-cheat | HTTP health endpoint polling only |
| `winapi::um::tlhelp32` process snapshot | Same anti-cheat violation risk as sysinfo | HTTP health endpoint polling only |
| Adding new watchdog crate (`tokio-watchdog`) | External dep for trivial loop; rc-sentry must have minimal deps | `loop { sleep(5s); poll() }` |

---

## Alternatives Considered

| Recommended | Alternative | Why Not |
|-------------|-------------|---------|
| `std::net::TcpStream` raw HTTP for Ollama | `reqwest` blocking | reqwest bundles mini tokio runtime; +800KB binary |
| `std::thread` watchdog loop | `tokio::time::interval` | no tokio runtime in rc-sentry; adding it contradicts deliberate design |
| Port `DebugMemory` from rc-agent | Shared `rc-common` DebugMemory type | DebugMemory is context-specific; rc-sentry keys on log patterns, rc-agent on SimType/exit_code |
| HTTP health poll for liveness | `sysinfo::processes()` | sysinfo triggers anti-cheat on gaming pods |

---

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| `serde_json = "1"` | `serde = "1"` | Already workspace co-versioned |
| `chrono = "0.4"` | `serde = "1"` (via `features = ["serde"]`) | Already workspace dep with serde feature |
| `anyhow = "1"` | All workspace crates | Already workspace dep |
| `sysinfo = "0.33"` | No conflict with new additions | Already in rc-sentry |

---

## Integration Points

| Feature | Integrates With | Mechanism |
|---------|----------------|-----------|
| Health watchdog | rc-agent `:8090/health` endpoint | Raw HTTP GET from background std::thread |
| Crash log reader | `C:\RacingPoint\startup_log.txt`, `rc-agent-stderr.txt` | `std::fs::read_to_string` |
| Pattern memory | `C:\RacingPoint\debug-memory.json` | serde_json read/write (atomic rename) |
| Ollama query | James `.27:11434/api/generate` | Raw HTTP POST via TcpStream |
| Fleet reporting | `192.168.31.23:8080/api/v1/fleet/...` | Raw HTTP POST (same TcpStream pattern) |
| rc-agent restart | `C:\RacingPoint\start-rcagent.bat` | `std::process::Command` |

---

## Sources

- `crates/rc-sentry/Cargo.toml` — confirmed current deps (pure std, serde_json, sysinfo)
- `crates/rc-sentry/src/main.rs` — confirmed thread-per-connection, no tokio runtime
- `crates/rc-agent/src/ai_debugger.rs` — `query_ollama()` and `DebugMemory` patterns (proven)
- `crates/rc-agent/Cargo.toml` — confirmed `reqwest = "0.12"` already workspace-locked
- `Cargo.toml` (workspace) — confirmed `chrono`, `anyhow`, `serde_json` as workspace deps
- WebSearch: reqwest 0.12.x confirmed as latest stable 0.12 series (0.13 exists but not in workspace)
- WebSearch: Ollama `/api/generate` REST endpoint confirmed stable (stream: false for blocking response)

---
*Stack research for: RC Sentry AI Debugger (v11.2)*
*Researched: 2026-03-21 IST*
