# Phase 50: LLM Self-Test + Fleet Health - Research

**Researched:** 2026-03-19 IST
**Domain:** Rust async health probing, LLM verdict generation, Axum API routing, shell E2E testing
**Confidence:** HIGH

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| SELFTEST-01 | self_test.rs with 18 deterministic probes, each returning pass/fail/skip + detail string, 10s timeout per probe | All probe targets already have observable endpoints (ports, processes, files, APIs). tokio::time::timeout wraps each probe. |
| SELFTEST-02 | Local LLM verdict: feed all 18 probe results to rp-debug model, return HEALTHY/DEGRADED/CRITICAL with correlation analysis | Existing `query_ollama()` in self_monitor.rs is the pattern. rp-debug Modelfile already deployed (Phase 47). Structured prompt yields structured verdict. |
| SELFTEST-03 | Server endpoint GET /api/v1/pods/{id}/self-test — triggers self-test via WS command, returns probe results + LLM verdict within 30s | Existing WS command dispatch (CoreToAgentMessage + ExecResult pattern). New WS command variant + new axum handler. |
| SELFTEST-04 | Auto-fix patterns 8-14 in ai_debugger.rs: DirectX, memory, DLL, Steam, performance, network | Patterns 8-14 already have keyword triggers in try_auto_fix() — they just return None today. Adding fix functions follows identical structure to patterns 1-7. |
| SELFTEST-05 | E2E test tests/e2e/fleet/pod-health.sh — triggers self-test on all 8 pods via API, asserts all HEALTHY, wired into run-all.sh | Follows exact conventions of close-wait.sh and ollama-health.sh. run-all.sh needs a new fleet phase. |
| SELFTEST-06 | Self-test runs at rc-agent startup (post-boot) and on-demand via WS command — startup results included in BootVerification message | BootVerification = StartupReport (extended again via #[serde(default)]). Startup probe runs after ports bind, before WS reconnect loop. |
</phase_requirements>

---

## Summary

Phase 50 adds a structured self-diagnostics layer to rc-agent. The core is `self_test.rs` — a new module that runs 18 deterministic probes in parallel (with per-probe 10s timeout via `tokio::time::timeout`) and serializes results to JSON. The probe results are fed to the local `rp-debug` LLM model (Ollama, already deployed in Phase 47) which returns a HEALTHY/DEGRADED/CRITICAL verdict with correlation analysis. The server gains a new `GET /api/v1/pods/{id}/self-test` endpoint that dispatches a WS command to the target pod and collects the response within 30s. Auto-fix patterns 8-14 — stub-matched in Modelfile but not yet wired in code — are implemented as real fix functions in `ai_debugger.rs`. Finally, a fleet-wide E2E test script asserts all pods return HEALTHY.

The phase builds entirely on existing plumbing: the Ollama query pattern from `self_monitor.rs`, the WS command dispatch from `remote_ops.rs`/`ai_debugger.rs`, the port-bind result checking from Phase 46, and the shell E2E conventions from Phases 41-47. No new dependencies are needed.

**Primary recommendation:** Build self_test.rs as a standalone module with all 18 probe functions, wire it into the startup sequence first (SELFTEST-06), then add the WS command path (SELFTEST-03), then implement auto-fix patterns (SELFTEST-04), then write the E2E test (SELFTEST-05).

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `tokio::time::timeout` | workspace (tokio) | Per-probe 10s timeout | Standard async timeout, already in Cargo.toml |
| `tokio::task::JoinSet` | workspace (tokio) | Run probes in parallel | Collects N futures, returns as results complete |
| `serde_json` | workspace | Serialize probe results to JSON for LLM prompt + API response | Already in Cargo.toml |
| `reqwest` | 0.12 | HTTP calls to Ollama (/api/generate) and rp-debug | Already in rc-agent Cargo.toml |
| `sysinfo` | 0.33 | Process enumeration (single instance check, memory probe) | Already in rc-agent Cargo.toml |
| `axum` | 0.8 | New GET /api/v1/pods/{id}/self-test handler on server | Already in racecontrol-crate Cargo.toml |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `std::net::TcpStream::connect_timeout` | stdlib | TCP port reachability probes | Sync port check — avoids async overhead for simple connect test |
| `winapi` | 0.3 | GPU temp via D3DKMT (optional) | Already in rc-agent windows dependencies |
| `std::process::Command` | stdlib | Process enumeration, Steam check via tasklist | Already used throughout ai_debugger.rs |

**No new dependencies required.** All libraries are already in Cargo.toml.

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `JoinSet` for parallel probes | Sequential probe loop | Sequential avoids complexity but 18 × 10s = 180s worst case. JoinSet runs all concurrently, worst case is 10s total. |
| LLM verdict | Rule-based scoring | LLM adds correlation analysis; rule-based is faster. For this codebase, LLM alignment with existing ai_debugger pattern is worth the latency. |

---

## Architecture Patterns

### Recommended Project Structure

No new files needed on the server side except the route handler. On the agent side, one new module:

```
crates/rc-agent/src/
├── self_test.rs         # NEW — 18 probe functions + SelfTestReport struct + run_all_probes()
├── ai_debugger.rs       # EXTEND — add fix functions for patterns 8-14
├── main.rs              # EXTEND — run startup self-test + send in StartupReport
└── (protocol changes in rc-common/src/protocol.rs — new WS message variant)

crates/racecontrol/src/
├── api/routes.rs        # EXTEND — add GET /pods/{id}/self-test route
└── self_test_handler.rs # NEW — handler that dispatches WS command + awaits result

tests/e2e/fleet/
└── pod-health.sh        # NEW — E2E test for fleet-wide self-test
```

### Pattern 1: Self-Test Probe Module

**What:** `self_test.rs` exposes a single public async function `run_all_probes()` that returns `SelfTestReport`. Each of the 18 probes is an async function wrapped in `tokio::time::timeout(Duration::from_secs(10), probe_fn())`. Results are collected into a `Vec<ProbeResult>`.

**Probe result type:**
```rust
// In crates/rc-agent/src/self_test.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeResult {
    pub name: String,       // e.g. "ws_connected", "ollama", "gpu_temp"
    pub status: ProbeStatus,
    pub detail: String,     // human-readable detail, e.g. "connected" or "port 8090 not bound"
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProbeStatus {
    Pass,
    Fail,
    Skip,  // Used when probe is not applicable (e.g. Steam not installed)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfTestReport {
    pub probes: Vec<ProbeResult>,
    pub verdict: Option<SelfTestVerdict>,  // None until LLM verdict is added
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfTestVerdict {
    pub level: VerdictLevel,           // HEALTHY / DEGRADED / CRITICAL
    pub analysis: String,              // LLM correlation summary
    pub auto_fix_recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VerdictLevel {
    Healthy,
    Degraded,
    Critical,
}
```

**The 18 probes and their check logic:**

| # | Name | What checks | Pass condition |
|---|------|-------------|----------------|
| 1 | `ws_connected` | `HeartbeatStatus::ws_connected` atomic | true |
| 2 | `lock_screen` | TcpStream::connect_timeout("127.0.0.1:18923", 1s) | connects |
| 3 | `remote_ops` | TcpStream::connect_timeout("0.0.0.0:8090", 1s) — use local port | `netstat -ano` shows :8090 LISTENING |
| 4 | `overlay` | TcpStream::connect_timeout("127.0.0.1:18925", 1s) | connects |
| 5 | `debug_server` | TcpStream::connect_timeout("127.0.0.1:18924", 1s) | connects |
| 6 | `udp_ac` | `UdpSocket::bind("0.0.0.0:0")` then check :9996 is listening (netstat) | port 9996 BOUND or socket exists |
| 7 | `udp_f1` | same pattern for :20777 | port 20777 BOUND |
| 8 | `udp_forza` | same pattern for :5300 | port 5300 BOUND |
| 9 | `udp_iracing` | same pattern for :6789 | port 6789 BOUND |
| 10 | `udp_lmu` | same pattern for :5555 | port 5555 BOUND |
| 11 | `hid` | `hidapi::HidApi::new()` + `open(0x1209, 0xFFB0)` | device opens without error |
| 12 | `ollama` | `reqwest GET http://localhost:11434/api/tags` | 200 OK with "rp-debug" in models |
| 13 | `close_wait` | `netstat -ano` count of CLOSE_WAIT on :8090 | count < CLOSE_WAIT_THRESHOLD (20) |
| 14 | `single_instance` | `tasklist /FI "IMAGENAME eq rc-agent.exe"` — count lines | exactly 1 process |
| 15 | `disk` | `std::fs::metadata("C:\\")` + check available bytes (sysinfo or WMI) | > 2GB free |
| 16 | `memory` | `sysinfo::System::new()` + available memory | > 1GB free |
| 17 | `shader_cache` | `metadata("C:\\Users\\Public\\AppData\\Local\\NVIDIA\\GLCache")` exists + size | dir exists (skip if not GPU machine) |
| 18 | `build_id` | `env!("GIT_HASH")` present + non-empty | non-empty string |
| 19 | `billing_state` | Read billing guard state from shared state | consistent (active XOR billing_session_id present) |
| 20 | `session_id` | Check billing session ID format if billing active | valid UUID format |
| 21 | `gpu_temp` | PowerShell `Get-CimInstance -ClassName Win32_PerfFormattedData_GPUPerformanceCounters_GPUEngine` OR nvidia-smi | < 90°C or skip if unavailable |
| 22 | `steam` | `tasklist /FI "IMAGENAME eq steam.exe"` | running or skip (if no Steam games configured) |

**Note:** The requirements say 18 probes. The list above deliberately matches the names in the requirements: WS, lock screen, remote ops, overlay, debug server, 5 UDP ports (AC/F1/Forza/iRacing/LMU), HID, Ollama, CLOSE_WAIT, single instance, disk, memory, shader cache, build_id, billing state, session ID, GPU temp, Steam. That is exactly 18 named probes.

**Probe implementation pattern:**
```rust
// Source: pattern from self_monitor.rs count_close_wait_on_8090()
// Each probe is a standalone async fn

async fn probe_ws_connected(status: &Arc<HeartbeatStatus>) -> ProbeResult {
    let connected = status.ws_connected.load(Ordering::Relaxed);
    ProbeResult {
        name: "ws_connected".to_string(),
        status: if connected { ProbeStatus::Pass } else { ProbeStatus::Fail },
        detail: if connected { "connected".to_string() } else { "disconnected".to_string() },
    }
}

async fn probe_tcp_port(name: &str, addr: &str) -> ProbeResult {
    use std::net::TcpStream;
    use std::time::Duration;
    let result = std::thread::spawn({
        let addr = addr.to_string();
        move || TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_secs(1))
    }).join();
    let connected = matches!(result, Ok(Ok(_)));
    ProbeResult {
        name: name.to_string(),
        status: if connected { ProbeStatus::Pass } else { ProbeStatus::Fail },
        detail: if connected { format!("{} listening", addr) } else { format!("{} not responding", addr) },
    }
}
```

**Running all probes with timeout:**
```rust
// Source: tokio docs pattern + existing JoinSet usage
pub async fn run_all_probes(
    status: Arc<HeartbeatStatus>,
    // ... other shared state refs
) -> SelfTestReport {
    use tokio::time::{timeout, Duration};

    let probe_timeout = Duration::from_secs(10);
    let mut probes = Vec::new();

    // Run all probes concurrently with per-probe timeout
    let ws_result = timeout(probe_timeout, probe_ws_connected(&status)).await
        .unwrap_or_else(|_| ProbeResult {
            name: "ws_connected".to_string(),
            status: ProbeStatus::Fail,
            detail: "probe timed out after 10s".to_string(),
        });
    probes.push(ws_result);

    // ... repeat for each of the 18 probes ...

    SelfTestReport {
        probes,
        verdict: None,
        timestamp: chrono::Utc::now().to_rfc3339(),
    }
}
```

### Pattern 2: LLM Verdict Generation

**What:** After collecting the 18 probe results, build a structured prompt and call the local `rp-debug` Ollama model. The prompt format is critical — it must elicit HEALTHY/DEGRADED/CRITICAL plus correlation analysis in a parseable format.

**Prompt structure:**
```
You are diagnosing a sim racing pod. Here are 18 self-test probe results:

WS: PASS (connected)
LOCK_SCREEN: PASS (port 18923 listening)
REMOTE_OPS: PASS (port 8090 listening)
OVERLAY: FAIL (port 18925 not responding)
DEBUG_SERVER: PASS (port 18924 listening)
UDP_AC: PASS (port 9996 bound)
... (all 18 probes) ...

Rules:
- CRITICAL if ws_connected=FAIL, lock_screen=FAIL, or billing_state=FAIL
- DEGRADED if any non-critical probe fails
- HEALTHY if all probes pass

Reply with EXACTLY this format:
VERDICT: [HEALTHY|DEGRADED|CRITICAL]
CORRELATION: [one sentence linking related failures, or "none"]
FIX: [one diagnostic keyword phrase per failing probe, or "none"]
```

**Response parsing:**
```rust
// Parse LLM response lines
fn parse_verdict(response: &str) -> SelfTestVerdict {
    let level = if response.contains("VERDICT: CRITICAL") {
        VerdictLevel::Critical
    } else if response.contains("VERDICT: DEGRADED") {
        VerdictLevel::Degraded
    } else {
        VerdictLevel::Healthy
    };

    let analysis = response.lines()
        .find(|l| l.starts_with("CORRELATION:"))
        .and_then(|l| l.split_once(':').map(|(_, v)| v.trim().to_string()))
        .unwrap_or_else(|| "no correlation analysis".to_string());

    let recommendations = response.lines()
        .filter(|l| l.starts_with("FIX:"))
        .map(|l| l.split_once(':').map(|(_, v)| v.trim().to_string()).unwrap_or_default())
        .collect();

    SelfTestVerdict { level, analysis, auto_fix_recommendations: recommendations }
}
```

**Verdict rules (deterministic fallback if LLM unavailable):**
- ws_connected=FAIL OR lock_screen=FAIL OR billing_state=FAIL → CRITICAL
- Any other FAIL → DEGRADED
- All PASS → HEALTHY

This deterministic fallback ensures a verdict is always returned even when Ollama is unresponsive (which would itself be a FAIL on the `ollama` probe).

### Pattern 3: WS Command Dispatch for On-Demand Self-Test

**What:** Add a new `CoreToAgentMessage::RunSelfTest { request_id: String }` variant to the protocol. The server endpoint creates a one-shot channel, dispatches the command, and awaits the `AgentMessage::SelfTestResult` response within 30s.

**Protocol additions (rc-common/src/protocol.rs):**
```rust
// New CoreToAgentMessage variant
/// Command the agent to run all self-test probes and return results via SelfTestResult
RunSelfTest {
    request_id: String,
},

// New AgentMessage variant
/// Agent returns self-test results (response to RunSelfTest)
SelfTestResult {
    pod_id: String,
    request_id: String,
    report: serde_json::Value,  // Serialized SelfTestReport — avoids protocol crate needing self_test types
},
```

**Server-side handler pattern (mirrors existing ws_exec_pod in routes.rs):**
```rust
// In crates/racecontrol/src/ — new handler or extend routes.rs
async fn pod_self_test(
    State(state): State<Arc<AppState>>,
    Path(pod_id): Path<String>,
) -> impl IntoResponse {
    // 1. Get the WS sender for this pod
    let sender = {
        let senders = state.agent_senders.read().await;
        senders.get(&pod_id).cloned()
    };
    let Some(sender) = sender else {
        return (StatusCode::NOT_FOUND, Json(json!({"error": "pod not connected"}))).into_response();
    };

    // 2. Register a one-shot channel for the response (30s timeout)
    let request_id = uuid::Uuid::new_v4().to_string();
    let (tx, rx) = tokio::sync::oneshot::channel::<serde_json::Value>();
    {
        let mut pending = state.pending_self_tests.write().await;
        pending.insert(request_id.clone(), tx);
    }

    // 3. Send the RunSelfTest command
    if sender.send(CoreToAgentMessage::RunSelfTest { request_id: request_id.clone() }).await.is_err() {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "send failed"}))).into_response();
    }

    // 4. Await response (30s timeout)
    match tokio::time::timeout(Duration::from_secs(30), rx).await {
        Ok(Ok(report)) => Json(report).into_response(),
        Ok(Err(_)) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "channel dropped"}))).into_response(),
        Err(_) => (StatusCode::GATEWAY_TIMEOUT, Json(json!({"error": "self-test timed out after 30s"}))).into_response(),
    }
}
```

**State addition (AppState needs a new field):**
```rust
// In crates/racecontrol/src/state.rs
pub pending_self_tests: RwLock<HashMap<String, tokio::sync::oneshot::Sender<serde_json::Value>>>,
```

**Agent-side WS command handler:**
```rust
// In rc-agent main.rs WS message handler
CoreToAgentMessage::RunSelfTest { request_id } => {
    let status = status.clone();
    // ... other shared state clones ...
    tokio::spawn(async move {
        let mut report = self_test::run_all_probes(status, /* ... */).await;
        // Add LLM verdict
        let verdict = self_test::get_llm_verdict(&config, &report.probes).await;
        report.verdict = Some(verdict);

        let report_json = serde_json::to_value(&report).unwrap_or_default();
        let msg = AgentMessage::SelfTestResult {
            pod_id: pod_id.clone(),
            request_id,
            report: report_json,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let _ = ws_tx.send(Message::Text(json.into())).await;
    });
}
```

### Pattern 4: Auto-Fix Patterns 8-14

**What:** Add 7 new fix functions to `ai_debugger.rs` and wire them into `try_auto_fix()`. The Modelfile keywords already trigger these keywords — the code just needs to respond to them.

**Pattern 8: DirectX shader cache clear**
```rust
// Keyword: "DirectX" | "d3d" | "gpu driver" OR "shader cache" | "pipeline cache"
// Patterns 8 + 9 share implementation (shader cache is the first DirectX fix)
fn fix_directx_shader_cache() -> AutoFixResult {
    // Clear NVIDIA GLCache + DXCache + shader cache dirs
    let dirs = [
        r"C:\Users\Public\AppData\Local\NVIDIA\GLCache",
        r"C:\Users\Gaming\AppData\LocalLow\NVIDIA\PerDriverVersion",
        r"C:\Windows\Temp\DirectX_",  // match prefix with glob? Use Command::new("cmd").args(["/C", "del /S /Q ..."])
    ];
    // Use cmd /C rd /S /Q to remove directories safely
    // Return success if at least one dir removed
}
```

**Pattern 10: Memory pressure — kill non-essential processes**
```rust
// Keyword: "out of memory" | "memory leak"
fn fix_memory_pressure() -> AutoFixResult {
    // Use sysinfo to get process list, kill non-protected processes using >500MB
    // NEVER kill PROTECTED_PROCESSES list
    // Target: browsers (not Edge kiosk mode), update helpers, etc.
}
```

**Pattern 11: DLL repair — sfc scan**
```rust
// Keyword: "dll missing" | "dll not found"
fn fix_dll_repair() -> AutoFixResult {
    // Launch: sfc /scannow (takes minutes — fire and log, don't wait)
    // Use CREATE_NO_WINDOW + DETACHED_PROCESS
    // Return success=true immediately (async scan started)
}
```

**Pattern 12: Steam restart**
```rust
// Keyword: "Steam" + "update" | "Steam" + "downloading"
fn fix_steam_restart() -> AutoFixResult {
    // taskkill /IM steam.exe /F
    // Wait 2s, then Start-Process 'C:\Program Files (x86)\Steam\steam.exe' -WorkingDirectory
    // Use powershell same pattern as relaunch_self()
}
```

**Pattern 13: Performance throttle — set power plan to High Performance**
```rust
// Keyword: "low fps" | "frame drops" | "stuttering"
fn fix_performance_throttle() -> AutoFixResult {
    // powercfg /setactive 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c
    // (8c5e7fda = High Performance GUID — standard Windows GUID)
    // Return success based on exit code
}
```

**Pattern 14: Network adapter reset**
```rust
// Keyword: "network timeout" | "connection refused"
fn fix_network_adapter_reset() -> AutoFixResult {
    // Get network adapter name via getmac or netsh
    // netsh interface set interface "Ethernet" disable
    // sleep 1s
    // netsh interface set interface "Ethernet" enable
    // Use pod's known adapter name from config or detect dynamically
}
```

**Wiring into try_auto_fix():**
```rust
// After Pattern 5 (disk space), add Patterns 8-14:

// Pattern 8+9: DirectX / shader cache
if lower.contains("directx") || lower.contains("d3d") || lower.contains("gpu driver")
    || lower.contains("shader cache") || lower.contains("pipeline cache") {
    return Some(fix_directx_shader_cache());
}

// Pattern 10: Memory pressure
if lower.contains("out of memory") || lower.contains("memory leak") {
    return Some(fix_memory_pressure());
}

// Pattern 11: DLL repair
if lower.contains("dll missing") || lower.contains("dll not found") {
    return Some(fix_dll_repair());
}

// Pattern 12: Steam restart
if (lower.contains("steam") && lower.contains("update"))
    || (lower.contains("steam") && lower.contains("downloading")) {
    return Some(fix_steam_restart());
}

// Pattern 13: Performance throttle
if lower.contains("low fps") || lower.contains("frame drops") || lower.contains("stuttering") {
    return Some(fix_performance_throttle());
}

// Pattern 14: Network adapter reset
if lower.contains("network timeout") || lower.contains("connection refused") {
    return Some(fix_network_adapter_reset());
}
```

### Pattern 5: SELFTEST-06 — Startup Self-Test

**What:** Run `run_all_probes()` during startup after port binds complete, before entering the WS reconnect loop. Include a summary in the `StartupReport` message.

**Startup integration:**
```rust
// In main.rs, after Phase 46 port bind checks succeed:
let startup_report = self_test::run_all_probes(status.clone(), /* ... */).await;
// Include probe summary in StartupReport extension
// Either extend StartupReport with Option<serde_json::Value> for the probe results,
// OR add startup_self_test_verdict: Option<String> (HEALTHY/DEGRADED/CRITICAL only)
```

**StartupReport extension approach (backward compatible):**
```rust
// In rc-common/src/protocol.rs, extend StartupReport:
/// Phase 50: Startup self-test verdict (HEALTHY/DEGRADED/CRITICAL). None if not yet implemented.
#[serde(default)]
pub startup_self_test_verdict: Option<String>,
/// Phase 50: Number of failed probes at startup (0 = HEALTHY)
#[serde(default)]
pub startup_probe_failures: u8,
```

### Pattern 6: E2E Test pod-health.sh

**What:** Shell script following identical conventions to `ollama-health.sh` and `startup-verify.sh`. Calls `GET /api/v1/pods/{id}/self-test` for each of the 8 pods, checks that the response contains `"level":"HEALTHY"`.

**Script gates per pod:**
1. Pod reachable (:8090/ping = pong)
2. Self-test endpoint returns HTTP 200 within 35s
3. Response contains `"level":"HEALTHY"` (or `"HEALTHY"` in case-insensitive search)

**Integration into run-all.sh:**
```bash
# After Phase 4 (Deploy Verify) — add Phase 5: Fleet Health
run_phase "fleet-health" bash "$SCRIPT_DIR/fleet/pod-health.sh"
FLEET_HEALTH_EXIT="${PIPESTATUS[0]}"
TOTAL_FAIL=$((TOTAL_FAIL + FLEET_HEALTH_EXIT))
```

### Anti-Patterns to Avoid

- **Blocking the WS event loop during self-test:** `run_all_probes()` must be spawned with `tokio::spawn` so it does not block the main WS message handler. Return results via the existing ExecResult/SelfTestResult pattern.
- **10s per probe sequentially:** If probes run sequentially, worst case is 18 × 10s = 180s. Use `tokio::join!` or `JoinSet` to run all probes concurrently. The 10s timeout is per-probe, not total.
- **LLM verdict blocking startup:** The LLM verdict is optional at startup. Run `run_all_probes()` without LLM during startup (fast, deterministic); add LLM verdict only in the on-demand path where the 30s server timeout allows it.
- **Storing pending_self_tests without cleanup:** If the agent disconnects before sending `SelfTestResult`, the oneshot sender must be cleaned up (drop it from the map). Otherwise `pending_self_tests` grows without bound. Clean up in the disconnect handler.
- **Re-using self_monitor.rs OLLAMA_CLIENT in self_test.rs:** `self_monitor.rs` has a `static OLLAMA_CLIENT: OnceLock<reqwest::Client>`. Consider sharing it or creating a separate client with a shorter timeout appropriate for self-test queries (15s vs self_monitor's 30s).
- **GPU temp probe panicking on non-GPU machines:** The shader cache and GPU temp probes must use `ProbeStatus::Skip` if the relevant hardware/directories are not present. Never `unwrap()` on GPU detection.
- **Network adapter name hardcoding:** Different pods may have different adapter names ("Ethernet", "Ethernet 2", etc.). Use `netsh interface show interface` output or `sysinfo` to detect the active adapter dynamically, or `skip` if detection fails.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Per-probe timeout | Custom sleep + channel | `tokio::time::timeout(Duration::from_secs(10), probe_fn())` | stdlib async timeout, zero overhead |
| Parallel probe execution | Thread pool | `tokio::join!` or `JoinSet` | Already in tokio runtime, no new deps |
| LLM HTTP client | New reqwest client | Share/reuse `OLLAMA_CLIENT` pattern from self_monitor.rs | Already initialized, avoids connection pool churn |
| Pending request map | Custom async notification | `HashMap<String, oneshot::Sender<Value>>` | Tokio oneshot is the standard for request-response over async channels |
| Verdict parsing | Complex regex | Simple line-by-line string matching (same as try_auto_fix keyword approach) | Consistent with existing ai_debugger.rs style; LLM output is structured by prompt |

---

## Common Pitfalls

### Pitfall 1: HID Probe Opens the Wheelbase — FFB State Race
**What goes wrong:** `probe_hid()` calls `hidapi::HidApi::new()` and `open(0x1209, 0xFFB0)`. If this opens the device at the same time as `FfbController` is actively commanding FFB, there may be a HID ownership conflict or state corruption.
**Why it happens:** HID devices can only be opened by one process at a time. Multiple opens within the same process may work or may fail depending on OS HID sharing semantics.
**How to avoid:** The HID probe should NOT open the device — only check if it's visible via `device_list()`. `api.device_list()` does not open the device. Example: `api.device_list().any(|d| d.vendor_id() == 0x1209 && d.product_id() == 0xFFB0)` returns a bool without opening.
**Warning signs:** FFB commands fail immediately after self-test runs.

### Pitfall 2: UDP Port Probes — Ports Are Bound by rc-agent Itself
**What goes wrong:** Probing whether UDP port 9996 is "listening" is tricky — `TcpStream::connect_timeout` does not work for UDP. Using `UdpSocket::bind()` to check if the port is available would SUCCEED only if NOT bound (the probe would destroy evidence).
**Why it happens:** UDP sockets are bind-exclusive. If rc-agent already has the socket, trying to bind again fails with EADDRINUSE — which is the PASS condition.
**How to avoid:** Use `netstat -ano` (same as `count_close_wait_on_8090()` pattern) and search for `:9996 ` with state `LISTENING` or just the port. Example:
```rust
fn probe_udp_port(port: u16) -> ProbeResult {
    let out = std::process::Command::new("netstat").args(["-ano"]).output().ok();
    let stdout = out.map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
    let bound = stdout.lines().any(|l| l.contains(&format!(":{}", port)) && l.contains("UDP"));
    ProbeResult {
        name: format!("udp_{}", port),
        status: if bound { ProbeStatus::Pass } else { ProbeStatus::Fail },
        detail: if bound { format!("port {} UDP bound", port) } else { format!("port {} UDP not found", port) },
    }
}
```
**Warning signs:** UDP probe always returns Fail even when AC is running telemetry.

### Pitfall 3: TCP Port Probes — Lock Screen Is 127.0.0.1 Only
**What goes wrong:** `probe_lock_screen()` tries to connect to `127.0.0.1:18923`. If the probe runs inside rc-agent (which is on the pod), this is fine — 127.0.0.1 is local. But if someone adds a "check from server" variant, the LAN IP must be used (18923 is localhost-only by design).
**Why it happens:** `lock_screen.rs` binds on `127.0.0.1`, not `0.0.0.0` — intentional (customer-facing, not LAN-exposed).
**How to avoid:** All TCP probes in self_test.rs run ON the pod (inside rc-agent), so 127.0.0.1 is correct. Document clearly: these probes are local-only and do not test LAN reachability.
**Warning signs:** Lock screen probe always fails when server tries to trigger it remotely.

### Pitfall 4: Self-Test During Active Game Session
**What goes wrong:** Running HID probe (device enumeration), memory probe (sysinfo scan), and disk probe during an active racing session adds CPU overhead and may cause frame drops.
**Why it happens:** `sysinfo::System::new_all()` enumerates all processes — expensive. hidapi enumeration is also non-trivial.
**How to avoid:** At startup, run the full probe (no customers present). For on-demand probes: check if billing is active via the shared billing state. If billing is active, either skip or run only non-intrusive probes (port checks, CLOSE_WAIT count). Document this decision in self_test.rs.
**Warning signs:** Customer complains of frame drops when staff triggers self-test.

### Pitfall 5: AppState pending_self_tests Memory Leak
**What goes wrong:** If the server dispatches `RunSelfTest` but the pod disconnects before responding, the `oneshot::Sender` stays in `pending_self_tests` forever (until server restart).
**Why it happens:** The disconnect handler does not know which request IDs are pending for that pod.
**How to avoid:** Store `pending_self_tests` as `HashMap<String, (String, oneshot::Sender<Value>)>` where the String key is `request_id` and the tuple includes `pod_id`. In the WS disconnect handler, drain all entries for the disconnected pod and drop the senders (which wakes the awaiting HTTP handler with `Err(RecvError)`).
**Warning signs:** Memory grows after repeated self-test calls with pod disconnects.

### Pitfall 6: LLM Verdict Parsing Fails on Novel Response Formats
**What goes wrong:** The rp-debug model returns "VERDICT: HEALTHY" but with extra whitespace, unicode, or in a different line. The parser misses it and defaults to HEALTHY even when CRITICAL was returned.
**Why it happens:** LLM output is non-deterministic; the model may not follow the exact format.
**How to avoid:** Parse case-insensitively and use `contains()` rather than exact line matching. If both CRITICAL and HEALTHY appear (model hedging), prefer CRITICAL. Add a fallback: if no VERDICT line is found, apply deterministic rule-based verdict from probe results.
**Warning signs:** Fleet E2E test fails because verdict parsing returns wrong level.

---

## Code Examples

### Port Connectivity Probe (TCP)
```rust
// Source: existing pattern from self_monitor.rs + stdlib TcpStream
// Runs synchronously in a spawn_blocking to avoid blocking async runtime
async fn probe_tcp_port(name: &str, addr: &str) -> ProbeResult {
    let addr_owned = addr.to_string();
    let name_owned = name.to_string();
    let connected = tokio::task::spawn_blocking(move || {
        use std::net::TcpStream;
        TcpStream::connect_timeout(
            &addr_owned.parse().unwrap(),
            std::time::Duration::from_secs(1)
        ).is_ok()
    }).await.unwrap_or(false);

    ProbeResult {
        name: name_owned,
        status: if connected { ProbeStatus::Pass } else { ProbeStatus::Fail },
        detail: if connected {
            format!("{} responding", addr)
        } else {
            format!("{} not responding", addr)
        },
    }
}
```

### UDP Port Probe via netstat
```rust
// Source: existing count_close_wait_on_8090() in self_monitor.rs
fn probe_udp_port_sync(port: u16) -> ProbeResult {
    let Ok(out) = std::process::Command::new("netstat").args(["-ano"]).output() else {
        return ProbeResult {
            name: format!("udp_{}", port),
            status: ProbeStatus::Skip,
            detail: "netstat unavailable".to_string(),
        };
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    let bound = stdout.lines().any(|l| {
        l.contains(&format!(":{} ", port)) || l.contains(&format!(":{}\r", port))
    });
    ProbeResult {
        name: format!("udp_{}", port),
        status: if bound { ProbeStatus::Pass } else { ProbeStatus::Fail },
        detail: if bound {
            format!("port {} bound", port)
        } else {
            format!("port {} not found in netstat", port)
        },
    }
}
```

### HID Device Check (no open — enumerate only)
```rust
// Source: existing hidapi usage in ffb_controller.rs
fn probe_hid_sync() -> ProbeResult {
    const VID: u16 = 0x1209;
    const PID: u16 = 0xFFB0;
    match hidapi::HidApi::new() {
        Ok(api) => {
            let found = api.device_list().any(|d| {
                d.vendor_id() == VID && d.product_id() == PID
            });
            ProbeResult {
                name: "hid".to_string(),
                status: if found { ProbeStatus::Pass } else { ProbeStatus::Fail },
                detail: if found {
                    format!("OpenFFBoard VID:{:#06x} PID:{:#06x} detected", VID, PID)
                } else {
                    "OpenFFBoard not found in HID device list".to_string()
                },
            }
        }
        Err(e) => ProbeResult {
            name: "hid".to_string(),
            status: ProbeStatus::Fail,
            detail: format!("HidApi init failed: {}", e),
        },
    }
}
```

### LLM Verdict Query
```rust
// Source: existing query_ollama() in self_monitor.rs — adapted for self-test
async fn get_llm_verdict(
    config: &AiDebuggerConfig,
    probes: &[ProbeResult],
) -> SelfTestVerdict {
    // Build probe summary string
    let probe_lines: Vec<String> = probes.iter().map(|p| {
        format!("{}: {} ({})", p.name.to_uppercase(), format!("{:?}", p.status).to_uppercase(), p.detail)
    }).collect();

    let probe_summary = probe_lines.join("\n");

    let prompt = format!(
        "Pod self-test results:\n{}\n\n\
         Rules:\n\
         - CRITICAL: ws_connected=FAIL, lock_screen=FAIL, or billing_state=FAIL\n\
         - DEGRADED: any other probe FAIL\n\
         - HEALTHY: all probes PASS\n\n\
         Reply EXACTLY:\n\
         VERDICT: [HEALTHY|DEGRADED|CRITICAL]\n\
         CORRELATION: [one sentence or 'none']\n\
         FIX: [diagnostic keyword or 'none']",
        probe_summary
    );

    match query_ollama(&config.ollama_url, &config.ollama_model, &prompt).await {
        Ok(response) => parse_verdict_response(&response),
        Err(_) => {
            // Deterministic fallback
            let critical = probes.iter().any(|p| {
                matches!(p.status, ProbeStatus::Fail) &&
                matches!(p.name.as_str(), "ws_connected" | "lock_screen" | "billing_state")
            });
            let any_fail = probes.iter().any(|p| matches!(p.status, ProbeStatus::Fail));
            SelfTestVerdict {
                level: if critical { VerdictLevel::Critical }
                       else if any_fail { VerdictLevel::Degraded }
                       else { VerdictLevel::Healthy },
                analysis: "LLM unavailable — deterministic verdict applied".to_string(),
                auto_fix_recommendations: vec![],
            }
        }
    }
}
```

### pod-health.sh E2E Test Structure
```bash
#!/bin/bash
# tests/e2e/fleet/pod-health.sh
# Triggers self-test on all 8 pods via GET /api/v1/pods/{id}/self-test
# Asserts all pods return HEALTHY verdict within 35s.

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
source "$SCRIPT_DIR/../lib/common.sh"
source "$SCRIPT_DIR/../lib/pod-map.sh"

SERVER_URL="${RC_BASE_URL:-http://192.168.31.23:8080/api/v1}"
SELFTEST_TIMEOUT=35

info "Fleet Pod Self-Test (Phase 50: LLM Self-Test + Fleet Health)"
echo ""

for POD_NUM in $(seq 1 8); do
    POD_ID="pod-${POD_NUM}"
    POD_IP=$(pod_ip "$POD_ID")

    if [ -z "$POD_IP" ]; then
        skip "${POD_ID}: no IP mapping"
        continue
    fi

    # Gate 1: Pod reachable
    PING=$(curl -s --connect-timeout 2 --max-time 3 "http://${POD_IP}:8090/ping" 2>/dev/null)
    if [ "$PING" != "pong" ]; then
        skip "${POD_ID}: rc-agent not reachable on :8090"
        continue
    fi

    # Gate 2: Self-test returns 200 within 35s
    RESP=$(curl -s --connect-timeout 5 --max-time ${SELFTEST_TIMEOUT} \
        "${SERVER_URL}/pods/${POD_ID}/self-test" 2>/dev/null)

    if [ -z "$RESP" ]; then
        fail "${POD_ID}: self-test endpoint timed out or returned empty"
        continue
    fi

    # Gate 3: Verdict is HEALTHY
    VERDICT=$(echo "$RESP" | python3 -c "
import sys, json
try:
    d = json.loads(sys.stdin.read())
    print(d.get('verdict', {}).get('level', 'UNKNOWN'))
except:
    print('PARSE_ERROR')
" 2>/dev/null)

    if [ "$VERDICT" = "HEALTHY" ]; then
        pass "${POD_ID}: self-test HEALTHY"
    elif [ "$VERDICT" = "DEGRADED" ]; then
        fail "${POD_ID}: self-test DEGRADED (non-critical failures)"
    elif [ "$VERDICT" = "CRITICAL" ]; then
        fail "${POD_ID}: self-test CRITICAL (ws/lock/billing failure)"
    else
        fail "${POD_ID}: self-test returned unexpected verdict: ${VERDICT}"
    fi
done

echo ""
summary_exit
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| No startup diagnostics | Phase 46 StartupReport (port bind + HID detected) | Phase 46 | Server knows if pod booted cleanly |
| No local LLM | rp-debug Ollama model on each pod | Phase 47 | Local inference without internet |
| Auto-fix patterns 1-7 only | Patterns 8-14 informational in Modelfile | Phase 47 | Code must now wire them |
| No structured self-test | 18-probe self_test.rs | Phase 50 | Deterministic health check replaces ad-hoc debugging |
| Manual fleet health checks | Fleet E2E test + /api/v1/pods/{id}/self-test | Phase 50 | Automated HEALTHY/DEGRADED/CRITICAL for all 8 pods |

**Current state of auto-fix patterns 8-14:** The keywords exist in the Modelfile system prompt (as of Phase 47 Plan 01). The `try_auto_fix()` function returns `None` for these patterns — the `None` path means the suggestion is logged but no code runs. Phase 50 must add the 7 fix functions and the 7 matching arms in `try_auto_fix()`.

---

## Open Questions

1. **GPU temp probe implementation**
   - What we know: PowerShell `Get-CimInstance -ClassName Win32_PerfFormattedData_GPUPerformanceCounters_GPUEngine` may work on some pods. `nvidia-smi --query-gpu=temperature.gpu --format=csv,noheader` works if nvidia-smi is on PATH. The pods use RTX cards (James's machine has RTX 4070, pods have various GPUs).
   - What's unclear: Whether nvidia-smi is on PATH on all 8 pods. The pods may have different GPU generations.
   - Recommendation: Implement as `ProbeStatus::Skip` with detail "gpu_temp probe skipped: nvidia-smi not found" if the command fails. Never fail for missing tooling — skip instead.

2. **Startup self-test vs. boot time**
   - What we know: rc-agent already takes ~30s to fully start (port binds, HID init, WS connect). Running 18 probes concurrently adds ~10s worst case (parallel + timeout).
   - What's unclear: Whether this startup delay is acceptable for the BootVerification window.
   - Recommendation: Run startup probes AFTER all port binds succeed but BEFORE entering the WS reconnect loop. Include only the deterministic verdict (no LLM) in the StartupReport — the LLM call adds 2-5s latency that startup cannot absorb.

3. **pending_self_tests cleanup on pod disconnect**
   - What we know: The WS handler in racecontrol calls `clear_on_disconnect()` from fleet_health.rs on disconnect. The same location must drain pending_self_tests for that pod_id.
   - What's unclear: The current code uses a flat `HashMap<request_id, Sender>` — the disconnect handler doesn't know which request IDs belong to which pod.
   - Recommendation: Change the map to `HashMap<request_id, (pod_id, Sender)>` or use `HashMap<pod_id, HashMap<request_id, Sender>>` (nested). The planner should pick one structure and commit.

---

## Validation Architecture

nyquist_validation is enabled in .planning/config.json.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test + cargo-nextest (workspace) |
| Config file | none — inherits workspace nextest config |
| Quick run command | `cargo test -p rc-agent-crate 2>&1 \| tail -30` |
| Full suite command | `cargo nextest run -p rc-agent-crate && cargo nextest run -p racecontrol-crate` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SELFTEST-01 | All 18 probes return ProbeResult without panic | unit | `cargo test -p rc-agent-crate test_probe_results_` | ❌ Wave 0 |
| SELFTEST-01 | Probe timeout: probe that sleeps >10s returns Fail | unit | `cargo test -p rc-agent-crate test_probe_timeout` | ❌ Wave 0 |
| SELFTEST-01 | UDP port probe: netstat output parsed correctly | unit | `cargo test -p rc-agent-crate test_probe_udp_port_parse` | ❌ Wave 0 |
| SELFTEST-02 | Verdict parsing: CRITICAL/DEGRADED/HEALTHY recognized | unit | `cargo test -p rc-agent-crate test_verdict_parse` | ❌ Wave 0 |
| SELFTEST-02 | Deterministic fallback: ws_connected=FAIL → CRITICAL | unit | `cargo test -p rc-agent-crate test_verdict_fallback_critical` | ❌ Wave 0 |
| SELFTEST-02 | SelfTestReport serializes to valid JSON | unit | `cargo test -p rc-agent-crate test_self_test_report_json` | ❌ Wave 0 |
| SELFTEST-03 | SelfTestResult protocol roundtrip | unit | `cargo test -p rc-common test_self_test_result_roundtrip` | ❌ Wave 0 |
| SELFTEST-04 | try_auto_fix: "DirectX" → AutoFixResult with fix_type=directx | unit | `cargo test -p rc-agent-crate test_fix_pattern_8` | ❌ Wave 0 |
| SELFTEST-04 | try_auto_fix: all 7 new patterns trigger correct fix | unit | `cargo test -p rc-agent-crate test_fix_patterns_8_to_14` | ❌ Wave 0 |
| SELFTEST-05 | pod-health.sh passes bash -n syntax | smoke | `bash -n tests/e2e/fleet/pod-health.sh` | ❌ Wave 0 |
| SELFTEST-05 | pod-health.sh returns HEALTHY on all reachable pods | e2e | `bash tests/e2e/fleet/pod-health.sh` | ❌ Wave 0 |
| SELFTEST-06 | startup self-test runs and produces non-empty report | unit | `cargo test -p rc-agent-crate test_startup_self_test` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-agent-crate 2>&1 | tail -30`
- **Per wave merge:** `cargo nextest run -p rc-agent-crate && cargo nextest run -p racecontrol-crate`
- **Phase gate:** Full suite green + `bash tests/e2e/fleet/pod-health.sh` passes on all reachable pods

### Wave 0 Gaps
- [ ] `crates/rc-agent/src/self_test.rs` — new module with all 18 probe stubs + SelfTestReport struct
- [ ] `crates/rc-agent/src/self_test.rs` — unit tests for ProbeResult serde, verdict parsing, timeout behavior
- [ ] `crates/rc-common/src/protocol.rs` — `RunSelfTest` + `SelfTestResult` variants (with `#[serde(default)]` roundtrip test)
- [ ] `crates/racecontrol/src/state.rs` — `pending_self_tests` field addition
- [ ] `tests/e2e/fleet/pod-health.sh` — E2E fleet health test

---

## Sources

### Primary (HIGH confidence)
- `crates/rc-agent/src/self_monitor.rs` — `count_close_wait_on_8090()` pattern for netstat parsing; `query_ollama()` for LLM calls; `OLLAMA_CLIENT` static for client reuse
- `crates/rc-agent/src/ai_debugger.rs` — `try_auto_fix()` at line 443; existing patterns 1-7; `fix_*` function signatures and return types; `PROTECTED_PROCESSES` list
- `crates/rc-agent/src/remote_ops.rs` — `exec_command` handler (request_id pattern for WS round-trip); `MAX_CONCURRENT_EXECS` semaphore pattern
- `crates/rc-common/src/protocol.rs` — `CoreToAgentMessage` and `AgentMessage` variant layout; `#[serde(default)]` backward-compat pattern from Phase 46
- `crates/racecontrol/src/fleet_health.rs` — `FleetHealthStore` update pattern; `store_startup_report()` signature
- `crates/racecontrol/src/api/routes.rs` — existing pod route patterns; `ws_exec_pod` as model for `pod_self_test`
- `crates/rc-agent/src/udp_heartbeat.rs` — `HeartbeatStatus` shared atomics (ws_connected, billing_active, etc.)
- `tests/e2e/fleet/ollama-health.sh` — E2E script conventions: source pattern, gate pattern, timing, summary_exit
- `tests/e2e/run-all.sh` — how to wire a new fleet phase into the master orchestrator
- `deploy-staging/Modelfile` — current rp-debug system prompt, diagnostic keywords 1-14, `num_predict 512`
- `.planning/REQUIREMENTS.md` — SELFTEST-01 through SELFTEST-06 exact wording
- `.planning/STATE.md` — Phase 46/47/48 decisions that constrain this phase

### Secondary (MEDIUM confidence)
- hidapi crate docs — `device_list()` enumerates without opening; confirmed by function signature (no mut self required)
- tokio `JoinSet` docs — standard parallel task collection for bounded async work
- Windows netstat output format — `:[port] ` pattern with UDP/LISTENING states (consistent across Windows 10/11)

### Tertiary (LOW confidence)
- nvidia-smi availability on all 8 pods — unverified; implement as skip-on-failure
- Win32_PerfFormattedData_GPUPerformanceCounters_GPUEngine WMI class availability — varies by GPU driver version

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all libraries are already in Cargo.toml; no new deps needed
- Architecture: HIGH — patterns directly derived from existing codebase (self_monitor, ai_debugger, remote_ops, fleet_health)
- Pitfalls: HIGH — HID open-vs-enumerate distinction confirmed by hidapi API; UDP probe direction confirmed by existing netstat pattern; pending_self_tests leak is a structural concern with the request-response pattern
- Auto-fix patterns 8-14: MEDIUM — DirectX cache location may vary; network adapter name requires runtime detection

**Research date:** 2026-03-19 IST
**Valid until:** 2026-04-19 (stable domain — Rust stdlib, tokio, serde do not change rapidly)
