# Phase 98: MaintenanceRequired Lock Screen + Display Checks - Research

**Researched:** 2026-03-21
**Domain:** rc-agent Rust — LockScreenManager, pre_flight gate, WebSocket handler, WinAPI
**Confidence:** HIGH — all findings from direct source-code inspection of the live codebase

## Summary

Phase 98 wires together three things that Phase 97 deliberately left incomplete: (1) the lock screen must show a branded `MaintenanceRequired` state when `pre_flight::run()` returns `PreFlightResult::MaintenanceRequired`, (2) the agent must auto-retry pre-flight every 30 seconds while blocked, and (3) two display checks (HTTP probe + GetWindowRect) must be added to `pre_flight.rs` as concurrent checks alongside HID and ConspitLink.

All the plumbing already exists. `ClearMaintenance` is a unit variant in `CoreToAgentMessage` (added in 97-01) with a round-trip test but no handler yet. `PreFlightFailed` is already sent on failure in `ws_handler.rs` (97-02) — the comment at line 160 literally says "Phase 98 will add MaintenanceRequired lock screen state here". The 14th `LockScreenState` variant (`MaintenanceRequired`) does not exist yet in `lock_screen.rs` — it must be added.

`AppState` has no `in_maintenance` flag. The retry loop needs a flag that persists across `BillingStarted` messages. The cleanest place for this is `AppState` as an `AtomicBool` (same pattern as `billing_active` in `heartbeat_status`) — or a plain `bool` field on `AppState` directly (no `Arc` needed since `AppState` is `&mut` inside the event loop context via `ConnectionState`). The auto-retry is a 30-second tokio interval on `ConnectionState` (inner loop), not `AppState`, so it resets on reconnect — which is acceptable since the maintenance state survives in `AppState`.

**Primary recommendation:** Add `MaintenanceRequired` as the 14th `LockScreenState` variant, add `in_maintenance: AtomicBool` to `AppState`, add the retry interval to `ConnectionState`, add `DISP-01` and `DISP-02` checks to `pre_flight.rs`, wire the `ClearMaintenance` handler in `ws_handler.rs`.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| PF-04 | Lock screen shows "Maintenance Required" state when pre-flight fails after auto-fix | Add `LockScreenState::MaintenanceRequired` variant + `show_maintenance_required()` method + HTML renderer; call from ws_handler.rs at line 160 |
| PF-05 | `PreFlightFailed` AgentMessage sent to racecontrol with failed check details | Already sent at ws_handler.rs lines 151-157 — verify it's still correct; no new code needed for PF-05 itself |
| PF-06 | Pod auto-retries pre-flight every 30s while in `MaintenanceRequired` state | Add `maintenance_retry_interval` to `ConnectionState`; add select! arm in event_loop; re-run `pre_flight::run()` every 30s when `state.in_maintenance.load()` is true |
| DISP-01 | Lock screen HTTP server responding on port 18923 | Add `check_lock_screen_http()` async fn in `pre_flight.rs` — TCP connect probe to 127.0.0.1:18923, GET /health, expect 200 |
| DISP-02 | Lock screen window position validated via GetWindowRect (centered on primary monitor) | Add `check_window_rect()` in `pre_flight.rs` — `spawn_blocking` with raw `GetWindowRect` WinAPI (same pattern as `GetSystemMetrics` in lock_screen.rs) |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| tokio | (existing) | Async runtime, intervals, timeouts | Already in rc-agent |
| tokio::time::interval | (existing) | 30s retry loop | Standard tokio interval; same pattern as heartbeat_interval in ConnectionState |
| winapi | 0.3 (existing) | GetWindowRect, FindWindowA | Already in Cargo.toml for rc-agent; lock_screen.rs uses raw `unsafe extern "system"` without winapi crate |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| std::sync::atomic::AtomicBool | stdlib | `in_maintenance` flag on AppState | Shared between ws_handler and event_loop without Mutex |
| tokio::net::TcpStream | (existing) | HTTP probe to :18923 | Direct TCP connect — no reqwest, no external dep |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `AtomicBool` on AppState | `bool` on ConnectionState | ConnectionState resets on reconnect; `in_maintenance` must survive reconnect, so AppState is correct |
| Raw WinAPI `extern "system"` | `winapi` crate | lock_screen.rs already uses raw `unsafe extern "system"` for GetSystemMetrics — use the same pattern, no new dep |
| tokio HTTP client | Direct TcpStream + raw HTTP | Lock screen HTTP server is minimal raw TCP; matching probe approach is simpler and has no dep |

**Installation:** No new dependencies required. All needed libraries are already in `crates/rc-agent/Cargo.toml`.

## Architecture Patterns

### Recommended Project Structure

Changes span three files, one new state variant:

```
crates/rc-agent/src/
├── lock_screen.rs       # Add MaintenanceRequired variant + show_maintenance_required() + HTML renderer
├── pre_flight.rs        # Add check_lock_screen_http() + check_window_rect()
├── ws_handler.rs        # Add ClearMaintenance handler; call show_maintenance_required() at line 160
├── event_loop.rs        # Add maintenance_retry_interval to ConnectionState; add select! arm
└── app_state.rs         # Add in_maintenance: Arc<AtomicBool>
```

### Pattern 1: Adding LockScreenState::MaintenanceRequired

**What:** 14th enum variant in `LockScreenState`, with a `failures: Vec<String>` field for display.
**When to use:** Triggered by `PreFlightResult::MaintenanceRequired` in ws_handler.rs.

```rust
// In lock_screen.rs — add to LockScreenState enum after Lockdown:
/// Pre-flight checks failed — pod blocked until staff clears or auto-retry succeeds.
MaintenanceRequired {
    failures: Vec<String>,
},
```

Add to `render_page()` match arm:
```rust
LockScreenState::MaintenanceRequired { failures } => render_maintenance_required_page(failures),
```

Add `show_maintenance_required()` method on `LockScreenManager` (same pattern as `show_lockdown()`):
```rust
pub fn show_maintenance_required(&mut self, failures: Vec<String>) {
    {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        *state = LockScreenState::MaintenanceRequired { failures };
    }
    self.launch_browser();
}
```

Add `is_maintenance_required()` query method (same pattern as `is_blanked()`):
```rust
pub fn is_maintenance_required(&self) -> bool {
    let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
    matches!(*state, LockScreenState::MaintenanceRequired { .. })
}
```

### Pattern 2: HTML Renderer for MaintenanceRequired

**What:** `render_maintenance_required_page()` — branded fullscreen error page.
**Style:** Follow `render_lockdown_page()` exactly — same `page_shell()`, same CSS, add 5s auto-reload.

```rust
fn render_maintenance_required_page(failures: &[String]) -> String {
    // List each failure as a bullet
    let failure_list = failures.iter()
        .map(|f| format!("<li style='margin:6px 0;color:#ccc'>{}</li>", html_escape(f)))
        .collect::<Vec<_>>()
        .join("\n");
    page_shell(
        "Racing Point — Maintenance",
        &format!(r#"<div style="text-align:center;padding-top:20vh">
<div style="font-family:Enthocentric,sans-serif;font-size:2.5em;color:#E10600;margin-bottom:20px">MAINTENANCE REQUIRED</div>
<div style="font-size:1.1em;color:#fff;margin-bottom:24px">Staff have been notified. This pod is temporarily unavailable.</div>
<ul style="list-style:none;padding:0;max-width:600px;margin:0 auto 32px">{}</ul>
<div style="font-size:0.85em;color:#5A5A5A">This pod will automatically recover once the issue is resolved.</div>
</div>
<script>setTimeout(function(){{location.reload()}},5000)</script>"#, failure_list),
    )
}
```

### Pattern 3: in_maintenance AtomicBool on AppState

**What:** `pub(crate) in_maintenance: std::sync::Arc<std::sync::atomic::AtomicBool>` added to `AppState`.
**When to use:** Set `true` on `PreFlightResult::MaintenanceRequired`; set `false` on `ClearMaintenance` or successful retry.

```rust
// In app_state.rs AppState struct:
pub(crate) in_maintenance: std::sync::Arc<std::sync::atomic::AtomicBool>,

// Initialized in main.rs:
in_maintenance: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
```

### Pattern 4: ws_handler.rs Changes

**Line 160 replacement** — currently a comment "Phase 98 will add MaintenanceRequired lock screen state here". Replace with:

```rust
pre_flight::PreFlightResult::MaintenanceRequired { failures } => {
    let failure_strings: Vec<String> = failures.iter().map(|f| f.detail.clone()).collect();
    let pod_id = state.config.pod.number.to_string();
    // PF-05: send PreFlightFailed to server
    let msg = AgentMessage::PreFlightFailed {
        pod_id,
        failures: failure_strings.clone(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    if let Ok(json) = serde_json::to_string(&msg) {
        let _ = ws_tx.send(Message::Text(json.into())).await;
    }
    // PF-04: show maintenance required lock screen
    state.lock_screen.show_maintenance_required(failure_strings);
    // PF-06: arm maintenance flag so retry loop fires
    state.in_maintenance.store(true, Ordering::Relaxed);
    return Ok(HandleResult::Continue);
}
```

**CRITICAL:** The existing code at lines 147-162 already sends `PreFlightFailed`. The refactor must NOT duplicate the send — the current structure sends it and then returns `Continue`. Phase 98 adds `show_maintenance_required()` and `in_maintenance.store(true)` into that same branch, replacing the "Phase 98" comment.

**ClearMaintenance handler** — add new arm to the `match core_msg` in `ws_handler.rs`:

```rust
CoreToAgentMessage::ClearMaintenance => {
    tracing::info!("ClearMaintenance received from server — clearing maintenance state");
    state.in_maintenance.store(false, Ordering::Relaxed);
    state.lock_screen.show_idle_pin_entry();
}
```

### Pattern 5: Auto-Retry Loop (PF-06)

**What:** 30-second interval in `ConnectionState`; when `state.in_maintenance` is true, re-run `pre_flight::run()`.
**Where:** `event_loop.rs` `ConnectionState` struct gets a new field; the select! dispatch gets a new arm.

```rust
// In event_loop.rs ConnectionState:
pub(crate) maintenance_retry_interval: tokio::time::Interval,
```

Initialized with `tokio::time::interval(Duration::from_secs(30))`.

In the select! dispatch:
```rust
_ = conn.maintenance_retry_interval.tick() => {
    if !state.in_maintenance.load(Ordering::Relaxed) {
        continue;
    }
    tracing::info!("Maintenance retry: re-running pre-flight checks");
    let ffb_ref: &dyn crate::ffb_controller::FfbBackend = state.ffb.as_ref();
    match pre_flight::run(state, ffb_ref).await {
        pre_flight::PreFlightResult::Pass => {
            tracing::info!("Maintenance retry: pre-flight passed — clearing maintenance");
            state.in_maintenance.store(false, Ordering::Relaxed);
            state.lock_screen.show_idle_pin_entry();
            // Optionally send PreFlightPassed message to server
        }
        pre_flight::PreFlightResult::MaintenanceRequired { failures } => {
            let failure_strings: Vec<String> = failures.iter().map(|f| f.detail.clone()).collect();
            tracing::warn!("Maintenance retry: still failing — {:?}", failure_strings);
            // Refresh lock screen with updated failures
            state.lock_screen.show_maintenance_required(failure_strings);
        }
    }
}
```

### Pattern 6: DISP-01 — HTTP Probe Check

**What:** `async fn check_lock_screen_http() -> CheckResult` in `pre_flight.rs`.
**How:** TCP connect to `127.0.0.1:18923` with 2s timeout, send minimal HTTP GET, read response, check for `200`.

```rust
async fn check_lock_screen_http() -> CheckResult {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let connect = tokio::time::timeout(
        Duration::from_secs(2),
        tokio::net::TcpStream::connect("127.0.0.1:18923"),
    ).await;
    match connect {
        Ok(Ok(mut stream)) => {
            let _ = stream.write_all(b"GET /health HTTP/1.0\r\nHost: 127.0.0.1\r\n\r\n").await;
            let mut buf = [0u8; 256];
            let n = stream.read(&mut buf).await.unwrap_or(0);
            let resp = String::from_utf8_lossy(&buf[..n]);
            if resp.starts_with("HTTP/1.") && resp.contains("200") {
                CheckResult { name: "lock_screen_http", status: CheckStatus::Pass, detail: "Lock screen HTTP server responding on :18923".into() }
            } else {
                CheckResult { name: "lock_screen_http", status: CheckStatus::Fail, detail: format!("Lock screen HTTP server returned unexpected response: {}", resp.lines().next().unwrap_or("")) }
            }
        }
        Ok(Err(e)) => CheckResult { name: "lock_screen_http", status: CheckStatus::Fail, detail: format!("Lock screen HTTP connect failed: {}", e) },
        Err(_) => CheckResult { name: "lock_screen_http", status: CheckStatus::Fail, detail: "Lock screen HTTP server timeout (>2s) on :18923".into() },
    }
}
```

No auto-fix for DISP-01 (HTTP server is started at agent startup and bound; if it fails, it cannot self-heal without agent restart).

### Pattern 7: DISP-02 — GetWindowRect Check

**What:** `async fn check_window_rect() -> CheckResult` in `pre_flight.rs`.
**How:** `spawn_blocking` with raw `unsafe extern "system"` WinAPI calls — same pattern as `GetSystemMetrics` in `lock_screen.rs`. Find Edge kiosk window via `FindWindowA` or enumerate processes, call `GetWindowRect`, compare against primary monitor dimensions.

```rust
#[cfg(windows)]
async fn check_window_rect() -> CheckResult {
    let result = spawn_blocking(|| {
        unsafe extern "system" {
            fn GetSystemMetrics(nIndex: i32) -> i32;
            fn FindWindowA(lpClassName: *const u8, lpWindowName: *const u8) -> *mut std::ffi::c_void;
            fn GetWindowRect(hWnd: *mut std::ffi::c_void, lpRect: *mut [i32; 4]) -> i32;
        }
        let screen_w = unsafe { GetSystemMetrics(0) };  // SM_CXSCREEN
        let screen_h = unsafe { GetSystemMetrics(1) };  // SM_CYSCREEN
        // Find msedge kiosk window — class name "Chrome_WidgetWin_1" is Edge's window class
        let class_name = b"Chrome_WidgetWin_1\0";
        let hwnd = unsafe { FindWindowA(class_name.as_ptr(), std::ptr::null()) };
        if hwnd.is_null() {
            return CheckResult {
                name: "window_rect",
                status: CheckStatus::Warn,
                detail: "Lock screen Edge window not found (may not be launched yet)".into(),
            };
        }
        let mut rect = [0i32; 4]; // left, top, right, bottom
        let ok = unsafe { GetWindowRect(hwnd, &mut rect) };
        if ok == 0 {
            return CheckResult {
                name: "window_rect",
                status: CheckStatus::Warn,
                detail: "GetWindowRect failed — window may have closed".into(),
            };
        }
        let w = rect[2] - rect[0];
        let h = rect[3] - rect[1];
        // Accept if window covers at least 90% of screen (allows minor border offsets)
        if w >= (screen_w * 9 / 10) && h >= (screen_h * 9 / 10) {
            CheckResult { name: "window_rect", status: CheckStatus::Pass, detail: format!("Lock screen window {}x{} covers screen {}x{}", w, h, screen_w, screen_h) }
        } else {
            CheckResult { name: "window_rect", status: CheckStatus::Fail, detail: format!("Lock screen window {}x{} too small for screen {}x{}", w, h, screen_w, screen_h) }
        }
    }).await.unwrap_or_else(|e| CheckResult {
        name: "window_rect",
        status: CheckStatus::Fail,
        detail: format!("spawn_blocking panicked in window_rect check: {}", e),
    });
    result
}

#[cfg(not(windows))]
async fn check_window_rect() -> CheckResult {
    CheckResult { name: "window_rect", status: CheckStatus::Pass, detail: "Window rect check skipped (non-Windows)".into() }
}
```

**No auto-fix for DISP-02.** If the window is too small, that indicates a display configuration problem (monitor disconnected, resolution changed) that cannot be auto-fixed.

### Pattern 8: Wiring DISP checks into run()

Add `check_lock_screen_http()` and `check_window_rect()` to `run_concurrent_checks()` via `tokio::join!`:

```rust
async fn run_concurrent_checks(
    ffb: &dyn FfbBackend,
    billing_active: bool,
    has_game_process: bool,
    game_pid: Option<u32>,
) -> Vec<CheckResult> {
    let (hid, conspit, orphan, http, rect) = tokio::join!(
        check_hid(ffb),
        check_conspit(),
        check_orphan_game(billing_active, has_game_process, game_pid),
        check_lock_screen_http(),
        check_window_rect(),
    );
    vec![hid, conspit, orphan, http, rect]
}
```

The existing 5-second hard timeout wrapping `run_concurrent_checks()` still applies — DISP-01 has its own internal 2s timeout so it cannot block the join.

### Anti-Patterns to Avoid

- **Do not put `in_maintenance` in `ConnectionState`** — that struct resets on every WebSocket reconnect. Maintenance state must survive reconnect to prevent billing from starting on a failed pod immediately after reconnect.
- **Do not add a `maintenance_retry_interval` tick when not in maintenance** — the `if !state.in_maintenance.load()` guard in the select! arm is mandatory to avoid wasted pre-flight runs every 30s on healthy pods.
- **Do not use name-based window search (`FindWindowA` with title)** — lock screen window title changes per state. Use class name `Chrome_WidgetWin_1` which is stable for all Edge instances.
- **Do not use reqwest for the HTTP probe** — no reqwest dep in rc-agent; use raw TcpStream (same as `wait_for_self_ready()` in LockScreenManager).
- **Do not call `health_response_body()` from pre_flight.rs** — that function is in lock_screen.rs. The probe only needs to check for HTTP 200 status line.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| HTTP probe | Custom HTTP client | Raw TcpStream connect + write | `wait_for_self_ready()` already does this exact pattern in LockScreenManager |
| Window enumeration | EnumWindows callback | FindWindowA with class name | Single window, no callback complexity needed |
| Retry loop | Custom timer + counter | tokio::time::interval | Same as heartbeat_interval in ConnectionState |
| Maintenance HTML | New page framework | `page_shell()` + inline format! | All 13 existing pages use this pattern |

**Key insight:** The entire codebase uses inline raw WinAPI (`unsafe extern "system"`) without the `winapi` crate for lock screen code. DISP-02 must follow the same pattern — adding a new crate dep for two function calls is unnecessary.

## Common Pitfalls

### Pitfall 1: Duplicate PreFlightFailed Send
**What goes wrong:** Phase 97 already sends `PreFlightFailed` at ws_handler.rs lines 151-157. If Phase 98 moves or restructures this code block carelessly, the message gets sent twice.
**Why it happens:** The "Phase 98" comment at line 160 is inside the `MaintenanceRequired` arm that already contains the send at lines 151-157. The planner might think the send needs to be added, not recognizing it's already there.
**How to avoid:** Read ws_handler.rs lines 147-162 in full before writing Task plans. The send is present; Phase 98 adds `show_maintenance_required()` call and `in_maintenance.store(true)` — that's all.
**Warning signs:** If a task plan says "add PreFlightFailed send" — it's already there.

### Pitfall 2: health_response_body Missing MaintenanceRequired
**What goes wrong:** `health_response_body()` in lock_screen.rs matches on `LockScreenState` variants. After adding `MaintenanceRequired`, the Rust compiler will error because the match is non-exhaustive.
**Why it happens:** `health_response_body()` has an explicit match at lines 1292-1300. Adding a new variant without updating this match = compile error.
**How to avoid:** `MaintenanceRequired` should return `"degraded"` in `health_response_body()` (pod is not ready for customers). Add it to the non-active states list.
**Warning signs:** `cargo build` fails with "non-exhaustive patterns" after adding the variant.

### Pitfall 3: render_page() Non-Exhaustive Match
**What goes wrong:** Same as above — `render_page()` at line 880 explicitly matches all `LockScreenState` variants. Adding `MaintenanceRequired` without a renderer arm = compile error.
**How to avoid:** Add `LockScreenState::MaintenanceRequired { failures } => render_maintenance_required_page(failures)` to the match in `render_page()` at the same time as adding the variant.

### Pitfall 4: is_idle_or_blanked() Must Include MaintenanceRequired
**What goes wrong:** `is_idle_or_blanked()` at line 451 matches `Hidden | ScreenBlanked | Disconnected | StartupConnecting`. If `MaintenanceRequired` is not added, then `is_active()` will return `true` for a maintenance-blocked pod, which is semantically wrong.
**How to avoid:** Add `LockScreenState::MaintenanceRequired { .. }` to the `is_idle_or_blanked()` match. A pod in maintenance is not serving a customer — it's idle from the customer's perspective.

### Pitfall 5: FindWindowA Returns First Edge Window Found
**What goes wrong:** If there are multiple Edge windows open (unlikely on a kiosk pod, but possible if close_browser() races), `FindWindowA` returns the first one found which may not be the lock screen window.
**Why it happens:** Pods are kiosk-only — no user Edge sessions should exist. `close_browser()` calls `taskkill /IM msedge.exe` aggressively before launch. In practice, only one Edge window exists.
**How to avoid:** Return `Warn` (not `Fail`) if window is not found — the lock screen may not have been launched yet if this check runs very early. Only `Fail` if the window is found but too small.

### Pitfall 6: 30s Retry Fires During Active Session (After ClearMaintenance)
**What goes wrong:** If `ClearMaintenance` clears `in_maintenance` flag and then a customer starts a session, the 30s interval may still fire. The guard `if !state.in_maintenance.load()` prevents the actual retry, but the tick still fires.
**How to avoid:** The guard is sufficient — early return from the select! arm when `in_maintenance` is false is cheap and correct.

## Code Examples

Verified patterns from live codebase:

### Raw WinAPI Pattern (from lock_screen.rs lines 28-44)
```rust
#[cfg(windows)]
fn get_virtual_screen_bounds() -> (i32, i32, i32, i32) {
    unsafe extern "system" {
        fn GetSystemMetrics(nIndex: i32) -> i32;
    }
    let x = unsafe { GetSystemMetrics(76) };
    // ...
}
```
Use identical structure for `GetWindowRect` — declare in `unsafe extern "system"` block inside the `spawn_blocking` closure or at function scope.

### LockScreenManager show_ method pattern (from lock_screen.rs lines 514-523)
```rust
pub fn show_lockdown(&mut self, message: &str) {
    {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        *state = LockScreenState::Lockdown { message: message.to_string() };
    }
    self.launch_browser();
}
```
`show_maintenance_required(failures: Vec<String>)` follows this exactly.

### ConnectionState interval field pattern (from event_loop.rs lines 60-66)
```rust
pub(crate) heartbeat_interval: tokio::time::Interval,
pub(crate) telemetry_interval: tokio::time::Interval,
// ...
pub(crate) kiosk_interval: tokio::time::Interval,
```
Add `pub(crate) maintenance_retry_interval: tokio::time::Interval,` to this list.

### Pre-flight gate return pattern (from ws_handler.rs lines 147-162)
```rust
pre_flight::PreFlightResult::MaintenanceRequired { failures } => {
    // ... send PreFlightFailed ...
    // Phase 98 comment here — replace with:
    // state.lock_screen.show_maintenance_required(failure_strings);
    // state.in_maintenance.store(true, Ordering::Relaxed);
    return Ok(HandleResult::Continue);
}
```

### TcpStream HTTP probe pattern (from lock_screen.rs lines 237-259 — wait_for_self_ready)
```rust
let timeout_result = tokio::time::timeout(
    tokio::time::Duration::from_millis(100),
    tokio::net::TcpStream::connect(addr),
).await;
match timeout_result {
    Ok(Ok(_stream)) => { /* connected */ }
    _ => { /* not ready */ }
}
```
DISP-01 uses the same pattern with a 2s timeout and adds a `GET /health` write + read.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Phase 97 left a comment "Phase 98 adds lock screen here" | Phase 98 fills that comment in | Now (Phase 98) | One surgical edit to ws_handler.rs |
| ClearMaintenance had no handler | Phase 98 adds the handler | Now (Phase 98) | One new match arm in ws_handler.rs |
| 3 concurrent pre-flight checks | 5 concurrent pre-flight checks (+ DISP-01 + DISP-02) | Now (Phase 98) | `run_concurrent_checks()` gains 2 more `tokio::join!` arms |

**Deprecated/outdated:**
- The comment at ws_handler.rs line 160 (`// Phase 98 will add MaintenanceRequired lock screen state here`) — this is a Phase 97 TODO that Phase 98 resolves.

## Open Questions

1. **`FindWindowA` class name for Edge kiosk**
   - What we know: Edge uses `Chrome_WidgetWin_1` window class (same Chromium base as Chrome). This is well-established in Windows automation.
   - What's unclear: Whether Edge kiosk mode uses a different class name. Kiosk mode typically uses the same class.
   - Recommendation: Return `CheckStatus::Warn` (not `Fail`) if window not found — the check is advisory, not blocking. A Warn result does not trigger `MaintenanceRequired`.

2. **Should DISP-02 be Warn-only (never Fail)?**
   - What we know: REQUIREMENTS.md says "Lock screen window position validated via GetWindowRect (centered on primary monitor)". No explicit failure severity.
   - What's unclear: Whether a mispositioned window should block a session.
   - Recommendation: Make DISP-02 return `Warn` on position mismatch, `Fail` only if window is completely missing or clearly wrong size (< 50% of screen). A pod with Edge slightly off-center should not block billing.

3. **PreFlightPassed message on successful retry**
   - What we know: `AgentMessage::PreFlightPassed` variant exists (added 97-01). Phase 97 sends it on the initial Pass path. When maintenance auto-clears, should it also send PreFlightPassed?
   - What's unclear: Whether the server currently handles PreFlightPassed for retry-clear events.
   - Recommendation: Send `PreFlightPassed` on successful auto-retry clear — consistent, server can ignore if not ready.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[tokio::test]` + `mockall` 0.13 |
| Config file | `crates/rc-agent/Cargo.toml` (dev-dependencies: mockall, tokio test-util) |
| Quick run command | `cargo test -p rc-agent-crate lock_screen` and `cargo test -p rc-agent-crate pre_flight` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PF-04 | `LockScreenState::MaintenanceRequired` renders branded HTML | unit | `cargo test -p rc-agent-crate lock_screen::tests::maintenance_required_renders_html` | ❌ Wave 0 |
| PF-04 | `health_response_body` returns `degraded` for MaintenanceRequired | unit | `cargo test -p rc-agent-crate lock_screen::tests::health_degraded_for_maintenance_required` | ❌ Wave 0 |
| PF-04 | `is_idle_or_blanked` returns true for MaintenanceRequired | unit | `cargo test -p rc-agent-crate lock_screen::tests::maintenance_required_is_idle` | ❌ Wave 0 |
| PF-05 | Already tested by 97-01 round-trip test for PreFlightFailed | unit | `cargo test -p rc-common protocol::tests::test_pre_flight_failed_round_trip` | ✅ |
| PF-06 | `in_maintenance` AtomicBool transitions (set on failure, clear on pass) | unit | `cargo test -p rc-agent-crate pre_flight::tests::test_maintenance_flag_lifecycle` | ❌ Wave 0 |
| DISP-01 | HTTP probe passes when server is listening | unit | `cargo test -p rc-agent-crate pre_flight::tests::test_lock_screen_http_pass` | ❌ Wave 0 |
| DISP-01 | HTTP probe fails when server is not listening | unit | `cargo test -p rc-agent-crate pre_flight::tests::test_lock_screen_http_fail` | ❌ Wave 0 |
| DISP-02 | Window rect check returns Pass on non-Windows | unit | `cargo test -p rc-agent-crate pre_flight::tests::test_window_rect_non_windows` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-agent` (runs all rc-agent unit tests)
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol`
- **Phase gate:** Full suite green + `cargo build --bin rc-sentry` (stdlib-only constraint check) before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `lock_screen::tests::maintenance_required_renders_html` — add to bottom of lock_screen.rs test module
- [ ] `lock_screen::tests::health_degraded_for_maintenance_required` — add to lock_screen.rs test module
- [ ] `lock_screen::tests::maintenance_required_is_idle` — add to lock_screen.rs test module
- [ ] `pre_flight::tests::test_lock_screen_http_pass` — bind ephemeral port, check passes (same pattern as `wait_for_self_ready_succeeds_when_port_open` in lock_screen.rs)
- [ ] `pre_flight::tests::test_lock_screen_http_fail` — no server bound, check fails
- [ ] `pre_flight::tests::test_window_rect_non_windows` — trivial: returns Pass on non-Windows

## Sources

### Primary (HIGH confidence)
- Direct source: `crates/rc-agent/src/lock_screen.rs` — all 1700 lines read; LockScreenState enum, render_page(), health_response_body(), show_ methods, WinAPI pattern
- Direct source: `crates/rc-agent/src/pre_flight.rs` — full file read; PreFlightResult, run(), run_concurrent_checks(), check patterns
- Direct source: `crates/rc-agent/src/ws_handler.rs` lines 130-185 — BillingStarted arm, pre-flight gate, "Phase 98" comment at line 160
- Direct source: `crates/rc-agent/src/event_loop.rs` lines 1-80 — ConnectionState struct, interval fields
- Direct source: `crates/rc-agent/src/app_state.rs` lines 1-58 — AppState struct fields
- Direct source: `crates/rc-common/src/protocol.rs` — ClearMaintenance unit variant, round-trip test verified
- Direct source: `.planning/phases/97-rc-common-protocol-pre-flight-rs-framework-hardware-checks/97-02-SUMMARY.md` — what 97 built, key decisions, line-number references

### Secondary (MEDIUM confidence)
- `Chrome_WidgetWin_1` window class for Edge/Chromium: established Windows automation fact, consistent across all Chromium-based browsers including Edge; not verified against current Edge docs but universally used in Windows UI automation contexts

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new deps; all patterns from live codebase
- Architecture: HIGH — line-number-precise locations for every change; enum variant count and "Phase 98" comment verified in source
- Pitfalls: HIGH — non-exhaustive match risks are compile-time errors; verified by reading all match arms
- DISP-02 window class name: MEDIUM — Chrome_WidgetWin_1 is correct for Chromium/Edge but not verified from Microsoft docs for latest Edge kiosk mode

**Research date:** 2026-03-21 IST
**Valid until:** 2026-04-21 (stable codebase, internal APIs)
