# Phase 321: External Monitoring & Alert Chain — Research

**Researched:** 2026-04-06
**Domain:** Rust/Windows systems — rc-sentry watchdog extension, GDI screen capture, WhatsApp Evolution API via pure std::net, pod_healer sentry fallback
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**MON-01: Detection Method**
Dual detection — health poll (existing) + `tasklist /FI "IMAGENAME eq rc-agent.exe"` as secondary.
Implementation: Add `check_process_alive()` to watchdog.rs. If tasklist says dead + health fails = immediate crash (skip hysteresis). If only health fails = keep existing hysteresis.

**MON-02: Server Recovery via Sentry**
pod_healer gets a sentry fallback path. When :8090 unreachable but :8091 reachable, POST to `http://{pod_ip}:8091/exec` with `{"cmd": "schtasks /Run /TN StartRCAgent"}`.

**MON-03: Backoff + MAINTENANCE_MODE**
Keep existing implementation. Already meets MON-03 requirements. No changes needed.

**MON-04: WhatsApp Alert Path**
Add direct HTTP POST to WhatsApp Evolution API from rc-sentry. Deploy COMMS_PSK to all pods via rc-sentry.toml.
Implementation: Add `alert_config` section to rc-sentry.toml: `whatsapp_url`, `whatsapp_number`, `comms_psk`. New `fn send_whatsapp_alert()` in tier1_fixes.rs using std::net TcpStream HTTP POST (no reqwest — keep pure std).

**MON-05: Screenshot-Based Blanking Verification**
Use Windows GDI `BitBlt` + `GetPixel` sampling in rc-sentry directly. Sample 9 points on each monitor. If >80% match expected blanking color (#1A1A1A ± tolerance), blanking is verified.
Implementation: New module `screen_verify.rs` with `fn verify_blanking() -> BlankingStatus`. Called after watchdog detects rc-agent restart + blanking command sent.

### Claude's Discretion
None specified — all five decisions are locked.

### Deferred Ideas (OUT OF SCOPE)
- Full Playwright screenshot comparison (belongs in v43.0, already done)
- rc-sentry → server WS channel for real-time monitoring dashboard (future phase)
- Multi-monitor blanking verification per-display (current 9-point sampling sufficient for MVP)
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| MON-01 | rc-sentry detects rc-agent death within 30s via tasklist + health poll | watchdog.rs FSM is ready to extend; `tasklist /FI` pattern proven in StepPidLiveness |
| MON-02 | pod_healer falls back to sentry /exec when :8090 down but :8091 up | Existing sentry_post() pattern in pod_healer already calls :8091/exec; need new branch when agent is dead |
| MON-03 | 3 restarts in 10 min → stop, auto-clear after backoff, WhatsApp alert | RestartTracker + MAINTENANCE_MODE + auto-clear already implemented; confirmed no code changes needed |
| MON-04 | WhatsApp alert reaches Uday when rc-agent dies; COMMS_PSK on pods | Evolution API endpoint confirmed; pure std::net pattern exists in escalate_to_whatsapp(); SentryConfig needs alert_config section |
| MON-05 | rc-sentry captures screenshot and verifies blanking color on all monitors | New module needed; winapi crate already in Cargo.toml; needs gdi32+wingdi features added |
</phase_requirements>

---

## Summary

Phase 321 builds on an already solid rc-sentry foundation. The core watchdog FSM (watchdog.rs, 485 lines), crash handler (tier1_fixes.rs, 1414 lines), and MAINTENANCE_MODE logic (main.rs) are all production-hardened from v11.2 and v31.0. This phase adds five discrete capabilities, three of which are extensions to existing code and two of which require new modules.

The most important discovery: **MON-03 requires zero code changes**. The RestartTracker, MAINTENANCE_MODE auto-clear, and escalation threshold are all live. This frees a full plan slot for MON-05 which is the most complex new work.

For MON-04 (WhatsApp alerts), the existing `escalate_to_whatsapp()` already posts to the server's `/api/v1/fleet/alert` endpoint — which is indirect and fails if the server is down. The locked decision requires a **direct HTTP POST to Evolution API** bypassing the server entirely. The Evolution API pattern is thoroughly documented in billing.rs and auth/mod.rs: `POST {whatsapp_url}/message/sendText/{instance}` with `apikey: {api_key}` header and `{"number": "...", "text": "..."}` body. This must be implemented using pure std::net (no reqwest — rc-sentry has no async runtime).

For MON-05 (screen_verify.rs), the winapi crate is already a dependency but **gdi32 and wingdi features are not listed** in Cargo.toml. These must be added. The GDI pattern is: `GetDC(NULL)` → `BitBlt` pixels into a compatible DC → `GetPixel` for each sample point → compare to `#1A1A1A` (RGB: 26, 26, 26). The CONTEXT.md confirms 9 sample points and ±tolerance. The module must be `#[cfg(windows)]`-gated and safe to call from the sentry-crash-handler thread.

**Primary recommendation:** Plan in 4 units — (1) MON-01 watchdog extension, (2) MON-02 pod_healer fallback, (3) MON-04 alert config + direct WhatsApp, (4) MON-05 screen_verify. MON-03 is confirmed no-op.

---

## Standard Stack

### Core (all already in rc-sentry)
| Library | Version | Purpose | Notes |
|---------|---------|---------|-------|
| winapi | 0.3 | Windows API bindings | Already in Cargo.toml; needs gdi32+wingdi features added for MON-05 |
| serde + serde_json | workspace | JSON serialization | Already used in tier1_fixes |
| toml | workspace | Config parsing | SentryConfig already uses this |
| tracing | workspace | Structured logging | Already throughout rc-sentry |
| sysinfo | 0.33 | System introspection | Already used in main.rs handle_processes |
| rc-common | path | Shared types (CrashDiagResult, RecoveryLogger) | Already imported |

### New winapi features needed for MON-05
Add to `[target.'cfg(windows)'.dependencies]` in `crates/rc-sentry/Cargo.toml`:
```toml
winapi = { version = "0.3", features = [
    "consoleapi",
    "errhandlingapi",
    "handleapi",
    "processthreadsapi",
    "securitybaseapi",
    "userenv",
    "winbase",
    "winnt",
    "wtsapi32",
    # NEW for MON-05:
    "wingdi",
    "winuser",
] }
```

**No new crate dependencies** — all implementation uses existing winapi + std::net.

**Installation:** No `cargo add` needed. Feature additions only.

---

## Architecture Patterns

### Recommended Project Structure (new files)
```
crates/rc-sentry/src/
├── watchdog.rs          # EXTEND: add check_process_alive() + dual-detection logic
├── tier1_fixes.rs       # EXTEND: add alert_config field usage, send_whatsapp_alert()
├── sentry_config.rs     # EXTEND: add AlertConfig struct + alert_config field
├── main.rs              # NO CHANGES (MON-03 confirmed no-op; crash handler already calls tier1_fixes)
├── session1_spawn.rs    # NO CHANGES
├── screen_verify.rs     # NEW: BlankingStatus enum + verify_blanking() function
└── debug_memory.rs      # NO CHANGES
crates/racecontrol/src/
└── pod_healer.rs        # EXTEND: add sentry fallback branch in run_graduated_recovery()
```

### Pattern 1: Dual-Detection in watchdog.rs (MON-01)

The watchdog loop currently runs `poll_health()` each tick. The extension adds a secondary check: `check_process_alive()` via tasklist. The FSM decision logic changes only in the `Healthy → Suspect` transition:

**What:** When health fails AND tasklist also shows process dead → skip hysteresis, emit crash immediately. When health fails but tasklist shows process alive → enter Suspect as today.

**Why:** A dead process is unambiguous. A slow/unresponsive-but-alive process warrants hysteresis. Combining both signals eliminates the 15s false-negative window for true deaths.

```rust
// Source: inferred from StepPidLiveness in tier1_fixes.rs (verified pattern)
fn check_process_alive(process_name: &str) -> bool {
    #[cfg(test)]
    { return true; }
    #[cfg(not(test))]
    {
        let mut cmd = std::process::Command::new("tasklist");
        cmd.args(["/FI", &format!("IMAGENAME eq {}", process_name)]);
        #[cfg(windows)]
        { cmd.creation_flags(0x08000000); }
        match cmd.output() {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                stdout.to_lowercase().contains(&process_name.to_lowercase())
            }
            Err(_) => true,  // tasklist failed — assume alive, don't false-positive
        }
    }
}
```

The watchdog loop must call both checks each tick and apply this logic:
```
health=false AND process=false  → immediate WatchdogState::Crashed (skip hysteresis)
health=false AND process=true   → WatchdogState::Suspect(1) (existing path)
health=true  AND process=false  → WatchdogState::Suspect(1) (process in TIME_WAIT or starting)
health=true  AND process=true   → WatchdogState::Healthy (existing path)
```

### Pattern 2: AlertConfig in sentry_config.rs (MON-04)

`SentryConfig` gets a new nested struct. This mirrors the existing `MeshConfig` pattern exactly.

```rust
/// Direct WhatsApp alert configuration for MON-04.
/// Bypasses server relay — alerts fire even when server is down.
#[derive(Clone, Deserialize)]
pub struct AlertConfig {
    /// Whether direct WhatsApp alerts are enabled.
    #[serde(default)]
    pub enabled: bool,
    /// Evolution API base URL (e.g. "http://bono-vps:8080")
    #[serde(default)]
    pub whatsapp_url: String,
    /// Evolution API instance name (e.g. "staff-alerts")
    #[serde(default)]
    pub whatsapp_instance: String,
    /// Evolution API key (apikey header value)
    #[serde(default)]
    pub whatsapp_api_key: String,
    /// Phone number to alert (E.164 without +, e.g. "917075778180")
    #[serde(default)]
    pub whatsapp_number: String,
    /// COMMS_PSK value (for future mesh use, stored alongside alert config)
    #[serde(default)]
    pub comms_psk: String,
}
```

The TOML on each pod gets:
```toml
[alert_config]
enabled = true
whatsapp_url = "http://srv1422716.hstgr.cloud:8080"
whatsapp_instance = "racing-point-staff"
whatsapp_api_key = "..."
whatsapp_number = "917075778180"
comms_psk = "85d1d06c806b3cc5159676bbed35e29ef0a60661e442a683c2c5a345f2036df0"
```

**Debug impl must redact credentials** — follow the MeshConfig `Debug` pattern exactly (psk field → "[REDACTED]", api_key → "[REDACTED]").

### Pattern 3: send_whatsapp_alert() using pure std::net (MON-04)

The existing `escalate_to_whatsapp()` POSTs to server's `/api/v1/fleet/alert`. The new function POSTs directly to the Evolution API endpoint. Pattern is identical to the existing raw HTTP POST in the function — just different URL and headers.

```rust
// Source: auth/mod.rs + billing.rs (Evolution API pattern, verified)
// Adapted to pure std::net (no reqwest) to match rc-sentry's sync architecture
pub fn send_whatsapp_alert(pod_id: &str, message: &str) {
    let cfg = sentry_config::load();
    let alert = &cfg.alert_config;
    if !alert.enabled || alert.whatsapp_url.is_empty() {
        tracing::debug!(target: LOG_TARGET, "WhatsApp direct alert disabled or not configured");
        return;
    }

    let body = serde_json::json!({
        "number": alert.whatsapp_number,
        "text": message,
    });
    let body_str = match serde_json::to_string(&body) {
        Ok(b) => b,
        Err(_) => return,
    };

    // Path: POST /message/sendText/{instance}
    let path = format!("/message/sendText/{}", alert.whatsapp_instance);
    // Parse host:port from whatsapp_url
    let host_port = alert.whatsapp_url
        .trim_start_matches("http://")
        .trim_start_matches("https://");

    let request = format!(
        "POST {} HTTP/1.0\r\n\
         Host: {}\r\n\
         Content-Type: application/json\r\n\
         apikey: {}\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n{}",
        path, host_port, alert.whatsapp_api_key, body_str.len(), body_str
    );

    let addr = match host_port.parse::<std::net::SocketAddr>() {
        Ok(a) => a,
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, "invalid whatsapp_url '{}': {}", host_port, e);
            return;
        }
    };
    let timeout = std::time::Duration::from_secs(5);
    match std::net::TcpStream::connect_timeout(&addr, timeout) {
        Ok(mut stream) => {
            if let Err(e) = stream.write_all(request.as_bytes()) {
                tracing::warn!(target: LOG_TARGET, "WhatsApp direct alert write failed: {}", e);
            } else {
                tracing::info!(target: LOG_TARGET, "WhatsApp direct alert sent for {}", pod_id);
            }
        }
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, "WhatsApp direct alert connect failed: {}", e);
        }
    }
}
```

**Call sites:** Called from `escalate_to_whatsapp()` AFTER the existing server POST (dual path: server relay + direct). Also called from `enter_maintenance_mode()` alongside the existing server POST.

### Pattern 4: pod_healer sentry fallback (MON-02)

The existing `run_graduated_recovery()` in pod_healer.rs already has a PRE-WoL block (line ~951) that checks if rc-sentry is alive and retries rc-agent restart via sentry before sending WoL. The CONTEXT.md decision says to use `schtasks /Run /TN StartRCAgent` — BUT the standing rule explicitly says **NEVER use `schtasks /Run /TN StartRCAgent` from non-interactive context**. This contradicts the CONTEXT.md decision.

**Resolution:** The CONTEXT.md decision appears to have been made without full awareness of the standing rule. The correct command (used throughout pod_healer.rs already) is:
1. `sc start RCWatchdog` — ensure watchdog is running
2. `taskkill /F /IM rc-agent.exe` — kill rc-agent; RCWatchdog respawns in Session 1

This is exactly what the existing PRE-WoL block does. The MON-02 change is about adding an **earlier** branch: currently, pod_healer only checks sentry in the WoL step. MON-02 means adding this check in `TierOneRestart` when `:8090` is down — which is already done in the current `TierOneRestart` handler at line ~759.

**Actual gap:** The current `TierOneRestart` step already checks `:8091` and uses `sc start RCWatchdog + taskkill`. The RESEARCH finding is that **MON-02 may be largely already implemented**. The planner must verify whether the existing `TierOneRestart` sentry exec path satisfies the CONTEXT decision or if the `schtasks` command is strictly required.

If the planner decides the existing path satisfies MON-02, this becomes a verification-only plan.

### Pattern 5: screen_verify.rs (MON-05)

New module. GDI approach is Session-1 safe (rc-sentry runs in Session 1 context). The blanking color is `#1A1A1A` = RGB(26, 26, 26).

```rust
// Source: Windows GDI documentation (winapi crate)
// Runs from the sentry-crash-handler thread — must not block > 100ms
#[cfg(windows)]
pub fn verify_blanking() -> BlankingStatus {
    use winapi::um::wingdi::{BitBlt, CreateCompatibleBitmap, CreateCompatibleDC,
                              DeleteDC, DeleteObject, GetPixel, SRCCOPY};
    use winapi::um::winuser::GetDC;

    const EXPECTED_R: u8 = 26;   // #1A
    const EXPECTED_G: u8 = 26;   // #1A
    const EXPECTED_B: u8 = 26;   // #1A
    const TOLERANCE: u8 = 10;
    const SAMPLE_POINTS: &[(i32, i32)] = &[
        (100, 100), (500, 100), (900, 100),    // top row
        (100, 500), (500, 500), (900, 500),    // middle row
        (100, 900), (500, 900), (900, 900),    // bottom row
    ];

    let hdc = unsafe { GetDC(std::ptr::null_mut()) };
    if hdc.is_null() {
        return BlankingStatus::Unknown("GetDC failed".to_string());
    }
    // RAII guard to release DC
    struct DcGuard(*mut _);
    impl Drop for DcGuard {
        fn drop(&mut self) {
            unsafe { winapi::um::winuser::ReleaseDC(std::ptr::null_mut(), self.0); }
        }
    }
    let _guard = DcGuard(hdc);

    let mut matching = 0usize;
    for &(x, y) in SAMPLE_POINTS {
        let pixel = unsafe { GetPixel(hdc, x, y) };
        let r = (pixel & 0xFF) as u8;
        let g = ((pixel >> 8) & 0xFF) as u8;
        let b = ((pixel >> 16) & 0xFF) as u8;
        let close_r = r.abs_diff(EXPECTED_R) <= TOLERANCE;
        let close_g = g.abs_diff(EXPECTED_G) <= TOLERANCE;
        let close_b = b.abs_diff(EXPECTED_B) <= TOLERANCE;
        if close_r && close_g && close_b {
            matching += 1;
        }
    }

    let pct = (matching * 100) / SAMPLE_POINTS.len();
    if pct >= 80 {
        BlankingStatus::Blanked { matching_points: matching, total_points: SAMPLE_POINTS.len() }
    } else {
        BlankingStatus::NotBlanked { matching_points: matching, total_points: SAMPLE_POINTS.len() }
    }
}

#[derive(Debug, Clone)]
pub enum BlankingStatus {
    Blanked { matching_points: usize, total_points: usize },
    NotBlanked { matching_points: usize, total_points: usize },
    Unknown(String),
}
```

**Important:** `GetPixel()` is the simplest approach and sufficient for 9 points. `BitBlt` is not required for sampling — it's only needed for full capture. GetPixel directly queries pixel color at a coordinate without needing a compatible DC. This simplifies the implementation considerably.

**Feature gate:** Add `screen-verify` feature to Cargo.toml, or gate behind `#[cfg(feature = "tier1-fixes")]` since it logically belongs with fixes.

**Call site:** Called from the crash handler in main.rs, after restart + spawn verification succeeds. The result is logged. If `NotBlanked`, a WhatsApp alert is sent via `send_whatsapp_alert()`.

### Anti-Patterns to Avoid

- **`schtasks /Run /TN StartRCAgent` via rc-sentry exec:** Runs in Session 0, cannot launch GUI. Standing rule explicitly forbids this. Use `sc start RCWatchdog` + `taskkill /IM rc-agent.exe` instead.
- **Using reqwest in rc-sentry:** rc-sentry has no async runtime (pure std). All HTTP is via `std::net::TcpStream`. Never add reqwest.
- **OnceLock for AlertConfig:** SentryConfig already uses `OnceLock` — AlertConfig is a nested field, not a separate singleton. No second OnceLock needed.
- **GetDC(NULL) from Session 0:** `GetDC(NULL)` with a null HWND gets the entire virtual screen DC, but returns NULL or captures wrong content in non-interactive sessions. Since rc-sentry runs in Session 1 (same as rc-agent), this is safe.
- **Calling verify_blanking() too frequently:** GDI calls have overhead. Cap at once per restart cycle, not in the polling loop.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| HTTP POST to Evolution API | Custom HTTP framing | Reuse existing escalate_to_whatsapp() raw TCP pattern | Already written, proven, no reqwest dependency |
| Tasklist process check | Custom process enumeration | `std::process::Command::new("tasklist")` | Anti-cheat safe, already used in StepPidLiveness |
| Screen pixel sampling | Full screenshot comparison | `winapi::GetPixel()` with 9 fixed points | GetPixel is synchronous, ~1µs per call, no bitmap allocation |
| Config credential redaction | Custom Debug impl | Follow MeshConfig `Debug` pattern exactly | Already tested pattern in same codebase |

---

## Common Pitfalls

### Pitfall 1: schtasks /Run /TN StartRCAgent causes Session 0 restart
**What goes wrong:** rc-agent restarts but runs in Session 0. All GUI operations (Edge launch, game launch, blanking) silently fail. `ws_connected: true` but `edge_process_count: 0`.
**Why it happens:** Task Scheduler runs tasks as SYSTEM in a non-interactive session unless configured otherwise.
**How to avoid:** Never use schtasks for rc-agent restart. Use `sc start RCWatchdog` + kill. RCWatchdog uses `WTSQueryUserToken` + `CreateProcessAsUser` to spawn in Session 1.
**Warning signs:** After restart, tasklist shows `rc-agent.exe` in Sessions column as `Services` not `Console`.

### Pitfall 2: OnceLock config is cached — TOML changes require restart
**What goes wrong:** Updating rc-sentry.toml with new AlertConfig doesn't take effect because `CONFIG: OnceLock<SentryConfig>` is initialized on first call.
**Why it happens:** `get_or_init()` only runs once per process lifetime.
**How to avoid:** AlertConfig values must be set before rc-sentry starts. Deploy the updated TOML then restart rc-sentry (this is already part of the deploy sequence).
**Warning signs:** Alerts still going to old URL/number after TOML update.

### Pitfall 3: GetPixel coordinates exceed screen bounds
**What goes wrong:** On a pod with a different monitor configuration, GetPixel at (900, 900) returns CLR_INVALID (0xFFFFFFFF) if the screen is smaller than expected.
**Why it happens:** Sample point coordinates are hardcoded against a 1920x1080 assumption.
**How to avoid:** Check that all sample points are within `GetSystemMetrics(SM_CXVIRTUALSCREEN)` x `GetSystemMetrics(SM_CYVIRTUALSCREEN)`. Return `BlankingStatus::Unknown` for out-of-bounds points rather than counting them.
**Warning signs:** All 9 points return 0xFFFFFFFF (CLR_INVALID = -1 as COLORREF).

### Pitfall 4: WhatsApp URL includes path prefix
**What goes wrong:** `host_port.parse::<SocketAddr>()` fails if `whatsapp_url` is `http://host:8080/api` (path included).
**Why it happens:** Some Evolution API deployments have a base path prefix.
**How to avoid:** Strip scheme prefix, then take only the `host:port` portion before the first `/`. The URL path is constructed separately as `/message/sendText/{instance}`.
**Warning signs:** `invalid whatsapp_url` warning in sentry logs.

### Pitfall 5: MAINTENANCE_MODE race during dual-detection
**What goes wrong:** With dual-detection, a process killed for deploy (MAINTENANCE_MODE suppressed) triggers the fast-crash path because `check_process_alive()` returns false + `poll_health()` returns false simultaneously.
**Why it happens:** `kill_watchdog_restart` flag suppresses the crash handler, but dual-detection fires before the flag is checked.
**How to avoid:** Check `restart_suppressed` (from `sentry_flags`) BEFORE the dual-detection fast-path, same as the existing single-detection path.
**Warning signs:** MAINTENANCE_MODE entered during intentional deploy.

### Pitfall 6: Feature flag for screen_verify not in default features
**What goes wrong:** `screen_verify` module exists but isn't compiled into production builds if not added to `default` features.
**Why it happens:** Cargo feature gates.
**How to avoid:** Add feature gate and include in `default = [...]` in Cargo.toml, or simply keep the module always-on with `#[cfg(windows)]` guard.

---

## Code Examples

### Current crash flow (verified from codebase)

```
watchdog::spawn()
  → polls /health every 5s
  → Healthy → Suspect(1) → Suspect(2) → Crashed (after 3 failures = 15s)
  → sends CrashContext via mpsc channel

main.rs crash-handler thread
  → receives CrashContext from crash_rx
  → calls tier1_fixes::handle_crash(&ctx, &mut tracker)
  → if consecutive_failures >= 3: tier1_fixes::escalate_to_whatsapp(...)
  → logs to recovery JSONL
```

### Current escalate_to_whatsapp() flow (verified from tier1_fixes.rs)

```rust
// Current path: POST to server's /api/v1/fleet/alert at 192.168.31.23:8080
// MON-04 adds: ALSO POST directly to Evolution API at whatsapp_url
pub fn escalate_to_whatsapp(pod_id, failure_count, last_error, last_escalation) {
    // 5-minute cooldown check
    // Builds JSON body with pod_id, message, severity
    // POSTs to "POST /api/v1/fleet/alert HTTP/1.0" at SERVER_ADDR
    // Sets *last_escalation = Some(Instant::now())
}
```

### Current pod_healer sentry exec pattern (verified from pod_healer.rs line ~794)

```rust
// TierOneRestart step — already calls /exec via sentry_post():
let exec_url = format!("http://{}:8091/exec", pod.ip_address);
// Step A: sc start RCWatchdog
state.sentry_post(&exec_url)
    .json(&json!({ "cmd": "sc start RCWatchdog", "timeout": 10 }))
    .timeout(Duration::from_secs(15))
    .send().await;
// Step B: taskkill /F /IM rc-agent.exe → RCWatchdog respawns in Session 1
state.sentry_post(&exec_url)
    .json(&json!({ "cmd": "taskkill /F /IM rc-agent.exe", "timeout": 10 }))
    .timeout(Duration::from_secs(15))
    .send().await;
```

### Evolution API endpoint (verified from billing.rs and auth/mod.rs)

```rust
// Pattern used in billing.rs line 4444 and auth/mod.rs line 1149
let url = format!("{}/message/sendText/{}", evo_url, evo_instance);
let body = json!({ "number": wa_phone, "text": message });
// Header: "apikey": evo_key
// This is the REQWEST version — rc-sentry must use std::net TCP equivalent
```

Phone number format for Evolution API: E.164 without `+`, e.g. `"917075778180"` for +91 70 7577 8180.

---

## Runtime State Inventory

This is NOT a rename/refactor phase. However, COMMS_PSK deployment to pods is a configuration change:

| Category | Items Found | Action Required |
|----------|-------------|-----------------|
| Stored data | None — no DB records reference COMMS_PSK | None |
| Live service config | rc-sentry.toml on all 8 pods currently has NO `[alert_config]` section | Deploy new TOML to all 8 pods + restart rc-sentry on each |
| OS-registered state | rc-sentry runs via HKLM Run key `start-rcsentry.bat` | Restart needed after TOML deploy; use `sc stop/start` or kill via pod_healer |
| Secrets/env vars | COMMS_PSK = `85d1d06c806b3cc5159676bbed35e29ef0a60661e442a683c2c5a345f2036df0` (from MEMORY.md) | Add to rc-sentry.toml `[alert_config]` section; NOT an env var in rc-sentry |
| Build artifacts | rc-sentry.exe must be rebuilt with new features (wingdi/winuser for MON-05, new config struct) | `cargo build --release --bin rc-sentry` + full pod deploy |

**COMMS_PSK is stored in TOML (not env var) for rc-sentry** — this is by design per CONTEXT.md. The existing MeshConfig has `psk` in TOML. AlertConfig follows the same pattern.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| winapi crate (wingdi, winuser) | MON-05 screen_verify | Must add features | 0.3.x (existing) | None — Windows-only feature |
| Evolution API server (Bono VPS :8080) | MON-04 direct WhatsApp | Known live | Unknown | Fall back to server relay (existing escalate_to_whatsapp) |
| RCWatchdog service on pods | MON-02 + MON-01 restart | Deployed v31.0 | Current | None — required for Session 1 restart |
| rc-sentry.toml on all 8 pods | MON-04 COMMS_PSK deploy | Present (default only) | n/a | Alerts disabled (alert_config.enabled=false by default) |

**Missing dependencies with no fallback:**
- `wingdi` and `winuser` winapi features — must be added to Cargo.toml before building

**Missing dependencies with fallback:**
- Evolution API direct path — falls back to existing server relay if Bono VPS is unreachable

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in (`cargo test`) |
| Config file | None — rc-sentry uses `#[cfg(test)]` guards in each fn |
| Quick run command | `cargo test -p rc-sentry` |
| Full suite command | `cargo test -p rc-sentry -p racecontrol` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| MON-01 | check_process_alive() returns false for nonexistent process | unit | `cargo test -p rc-sentry check_process_alive` | ❌ Wave 0 |
| MON-01 | dual-detection fast-crash when both health + process fail | unit | `cargo test -p rc-sentry fsm_dual_detection_immediate_crash` | ❌ Wave 0 |
| MON-01 | hysteresis preserved when only health fails | unit | `cargo test -p rc-sentry fsm_health_fail_process_alive_stays_suspect` | ❌ Wave 0 |
| MON-02 | TierOneRestart sends sc start + taskkill to :8091/exec | unit | `cargo test -p racecontrol graduated_recovery_uses_sentry_exec` | ✅ (pattern exists, new test needed) |
| MON-03 | NO CHANGES — existing tests cover this | unit | `cargo test -p rc-sentry maintenance_mode` | ✅ (existing) |
| MON-04 | send_whatsapp_alert() constructs correct HTTP request | unit | `cargo test -p rc-sentry send_whatsapp_alert_formats_request` | ❌ Wave 0 |
| MON-04 | send_whatsapp_alert() respects disabled flag | unit | `cargo test -p rc-sentry send_whatsapp_alert_disabled` | ❌ Wave 0 |
| MON-05 | verify_blanking() returns Unknown when GetDC fails (cfg(test) mock) | unit | `cargo test -p rc-sentry verify_blanking_unknown` | ❌ Wave 0 |
| MON-05 | BlankingStatus::Blanked when 9/9 points match | unit | `cargo test -p rc-sentry blanking_status_all_match` | ❌ Wave 0 |
| MON-05 | BlankingStatus::NotBlanked when < 80% match | unit | `cargo test -p rc-sentry blanking_status_not_blanked` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-sentry`
- **Per wave merge:** `cargo test -p rc-sentry -p racecontrol`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/rc-sentry/src/screen_verify.rs` — covers MON-05 (new file)
- [ ] New test functions in `crates/rc-sentry/src/watchdog.rs` — covers MON-01 dual-detection
- [ ] New test functions in `crates/rc-sentry/src/tier1_fixes.rs` — covers MON-04 formatting tests

*(All rc-sentry tests use `#[cfg(test)]` guards within the same file — no separate test file needed. The test pattern is established by the existing 12+ tests in watchdog.rs and tier1_fixes.rs.)*

---

## Open Questions

1. **MON-02 Already Implemented?**
   - What we know: pod_healer `TierOneRestart` (line ~759) already checks `:8091/health`, and if alive, POSTs `sc start RCWatchdog + taskkill /F /IM rc-agent.exe` to `:8091/exec`. This is functionally what MON-02 requires.
   - What's unclear: The CONTEXT.md decision says "POST to `/exec` with `schtasks /Run /TN StartRCAgent`" which contradicts the standing rule. The existing code uses the correct approach.
   - Recommendation: Plan for MON-02 as a verification + documentation task. If the existing behavior already passes the success criterion (pod recovers when :8090 down + :8091 up), the plan is: write a test that verifies this path, document it as done.

2. **Evolution API URL for Bono VPS**
   - What we know: billing.rs uses `state.config.auth.evolution_url` (from racecontrol.toml). The actual URL for the staff phone is not hardcoded in the codebase — it lives in racecontrol.toml.
   - What's unclear: The exact evolution_url, evolution_instance, and evolution_api_key values that should go into rc-sentry.toml on the pods.
   - Recommendation: Planner must include a task for Uday to provide the Evolution API credentials for the staff alert WhatsApp number. The code structure is ready; the credentials must be obtained from racecontrol.toml on the server.

3. **GetPixel vs BitBlt for screen_verify**
   - What we know: GetPixel() directly returns a COLORREF for any (x,y) coordinate on the virtual screen DC. No memory allocation needed for 9 points.
   - What's unclear: Whether GetPixel has higher per-call overhead than BitBlt+pixel-read on some drivers.
   - Recommendation: Use GetPixel. For 9 points it's trivially fast. Only switch to BitBlt if GetPixel proves unreliable in testing.

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| HTTP-only crash detection (15s hysteresis) | Dual: HTTP health + tasklist process check | Phase 321 (this phase) | Detection time: 15s → <5s for true deaths |
| WhatsApp via server relay only | WhatsApp via server relay + direct Evolution API | Phase 321 (this phase) | Alert survives server downtime |
| pod_healer Tier 1 uses sentry exec (implicit) | Explicit verification that sentry fallback covers MON-02 | Phase 321 (this phase) | No behavior change, explicit coverage |
| No blanking verification in rc-sentry | GDI pixel sampling post-restart | Phase 321 (this phase) | Closes "sentry says restarted but screen wrong" blind spot |

---

## Sources

### Primary (HIGH confidence)
- `crates/rc-sentry/src/watchdog.rs` — Full FSM read: 485 lines, confirmed patterns
- `crates/rc-sentry/src/tier1_fixes.rs` — escalate_to_whatsapp(), RestartTracker, MAINTENANCE_MODE logic: lines 596-744
- `crates/rc-sentry/src/main.rs` — Crash handler thread, feature gates, /exec endpoint: lines 165-354
- `crates/rc-sentry/src/sentry_config.rs` — SentryConfig + MeshConfig pattern: 176 lines full read
- `crates/rc-sentry/Cargo.toml` — Features, winapi feature list: confirmed
- `crates/racecontrol/src/pod_healer.rs` — TierOneRestart sentry exec path: lines 697-853, PRE-WoL path: lines 951-980
- `crates/racecontrol/src/billing.rs` — Evolution API sendText pattern: lines 4388-4454
- `crates/racecontrol/src/auth/mod.rs` — send_otp_whatsapp Evolution API pattern: lines 1127-1149
- `.planning/phases/321-external-monitoring-alert-chain/321-CONTEXT.md` — All decisions confirmed verbatim

### Secondary (MEDIUM confidence)
- `CLAUDE.md` — Standing rules: Session 1 restart path, schtasks prohibition, MAINTENANCE_MODE behavior
- `.planning/ROADMAP.md` — Phase 321 success criteria (lines 481-491)
- `comms-link/send-message.js` — COMMS_PSK auth pattern confirmed (not used in rc-sentry directly)

### Tertiary (LOW confidence)
- winapi crate GetPixel behavior in Session 1 — based on Windows API documentation knowledge; not verified against actual pod hardware

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all deps already in Cargo.toml; only feature additions needed
- Architecture: HIGH — all five patterns verified from actual source files
- Pitfalls: HIGH — schtasks/Session 0 pitfall is documented in standing rules; GetPixel bounds pitfall is known Windows API behavior
- MON-02 implementation status: MEDIUM — code looks complete but needs runtime verification

**Research date:** 2026-04-06 IST
**Valid until:** 2026-05-06 (stable domain — no external API changes expected)

## Project Constraints (from CLAUDE.md)

| Constraint | Impact on This Phase |
|------------|---------------------|
| No `.unwrap()` in production Rust | All new code in watchdog.rs, tier1_fixes.rs, screen_verify.rs must use `?`, `.ok()`, or match |
| Static CRT (`+crt-static`) | Already in `.cargo/config.toml` — no change needed; new winapi features compile statically |
| `#[cfg(test)]` guards on all fix functions | screen_verify.rs and check_process_alive() must return mock results under test |
| `touch build.rs` before release builds | Required before `cargo build --release --bin rc-sentry` after new commits |
| Pod deploy: canary on Pod 8 first | Deploy new rc-sentry.exe to Pod 8 first, verify session=Console, then fleet |
| MAINTENANCE_MODE sentinel check before restart | Any new restart path must check `kill_watchdog_restart` flag from sentry-flags.json |
| Session 1 enforcement | check_process_alive() and screen_verify.rs are passive reads — no Session 0 risk |
| Single-binary-tier policy | rc-sentry has no per-pod feature variants; new features in default build |
| Bat CRLF + no parentheses | If rc-sentry.toml deploy requires a bat file, use goto labels not if/else parentheses |
| Git LOGBOOK entry per commit | Every commit to watchdog.rs, tier1_fixes.rs, sentry_config.rs, screen_verify.rs needs LOGBOOK entry |
| Auto-push + notify after every commit | All commits must be pushed and Bono notified |
| Deploy parity | rc-sentry change on pods does NOT require cloud deploy (rc-sentry only runs on pods/server, not Bono VPS) |
