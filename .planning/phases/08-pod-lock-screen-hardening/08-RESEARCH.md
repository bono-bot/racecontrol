# Phase 8: Pod Lock Screen Hardening - Research

**Researched:** 2026-03-14
**Domain:** Rust/Tokio startup sequencing, Windows batch scripts, Edge kiosk readiness, HTML self-retry
**Confidence:** HIGH — all findings drawn directly from existing codebase; no speculative external claims

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| LOCK-01 | Pod startup waits for rc-agent HTTP server (port 18923) to be ready before launching Edge kiosk browser | `launch_browser()` in lock_screen.rs currently fires immediately at rc-agent startup; must add an async `wait_for_port_ready(18923)` probe before spawning Edge |
| LOCK-02 | Pod lock screen shows a branded "Connecting..." page on startup instead of a blank window or browser error | A new `LockScreenState::StartupConnecting` variant + HTML page needed; Edge must be launched only after port 18923 is listening |
| LOCK-03 | Pod lock screen HTML auto-retries connection to rc-agent and recovers without manual intervention when rc-agent restarts | The existing `Disconnected` state already reloads every 3 seconds (`setTimeout(location.reload,3000)`); that pattern must be applied to `StartupConnecting` and a new recovery path must bring Edge back after a mid-session rc-agent crash |
</phase_requirements>

---

## Summary

Phase 8 is a **small, targeted wiring change** — no new dependencies, no new crates, no external libraries. The infrastructure is already in place: `LockScreenManager` serves HTML on port 18923, Edge kiosk is launched by `launch_browser()`, and the `Disconnected` state already shows a branded "reconnecting" page with a 3-second JavaScript reload loop. Three gaps remain closed by this phase:

**Gap 1 (LOCK-01/LOCK-02):** On pod reboot, `start-rcagent.bat` starts rc-agent and immediately calls `start_server()`, then `launch_browser()` fires. However there is a race: the Tokio `TcpSocket::bind()` inside `serve_lock_screen()` is called in a `tokio::spawn`, not awaited before `launch_browser()`. Edge can fire before port 18923 accepts connections and briefly shows "ERR_CONNECTION_REFUSED". The fix is a lightweight async loop in `LockScreenManager::start_server()` that confirms the port is listening before returning, or alternatively a `wait_for_self_ready()` call before `launch_browser()` is first invoked.

**Gap 2 (LOCK-02):** There is currently no "Connecting..." branded state for the very first startup moment. The `LockScreenState::Hidden` state renders an idle message ("Session not active — please see the front desk"). Adding `LockScreenState::StartupConnecting` and setting it as the initial state gives customers a proper branded waiting page from the first millisecond Edge opens.

**Gap 3 (LOCK-03):** When rc-agent crashes mid-session, Edge's kiosk window goes blank (the HTTP server on 18923 disappears). On rc-agent restart, a new `LockScreenManager` is created and `launch_browser()` re-spawns Edge, but the old stale Edge window from before the crash is still open. The `close_browser()` + `taskkill` sequence already handles this correctly — as long as the startup sequence invokes `launch_browser()` on the initial `Disconnected` state path. The existing reconnection loop in `main.rs` already calls `lock_screen.show_disconnected()` on every failed WebSocket attempt; when rc-agent restarts, this naturally triggers a new `launch_browser()` call which closes the stale window and opens a fresh one.

**Primary recommendation:** Three small changes: (1) add `wait_for_self_ready()` async helper in `lock_screen.rs` that polls `127.0.0.1:18923` until it accepts a connection (max 5 seconds), (2) change initial `LockScreenState` from `Hidden` to `StartupConnecting` and add the branded HTML, (3) call `lock_screen.show_disconnected()` immediately at startup (before the WebSocket loop) so the browser launches showing the branded "Connecting..." page from first boot.

---

## Standard Stack

### Core (already in place — no new dependencies)

| Component | Location | Purpose |
|-----------|----------|---------|
| `LockScreenManager` | `crates/rc-agent/src/lock_screen.rs` | Full lock screen lifecycle — state machine, HTTP server, Edge kiosk launch |
| `serve_lock_screen()` | `crates/rc-agent/src/lock_screen.rs:535` | Tokio async TCP listener on port 18923 |
| `launch_browser()` / `close_browser()` | `crates/rc-agent/src/lock_screen.rs:344 / 409` | Edge kiosk spawn and taskkill |
| `show_disconnected()` | `crates/rc-agent/src/lock_screen.rs:324` | Sets `Disconnected` state; does NOT call `launch_browser()` |
| `render_disconnected_page()` | `crates/rc-agent/src/lock_screen.rs:715` | "CONNECTION LOST" HTML with 3s JS reload |
| `reconnect_delay_for_attempt()` | `crates/rc-agent/src/main.rs` | Exponential backoff for WebSocket reconnect loop |
| `start-rcagent.bat` | `deploy-staging/start-rcagent.bat` | `taskkill` old agent + `start "" rc-agent.exe` |
| `HKLM Run` key `RCAgent` | Registry on all 8 pods | Fires `start-rcagent.bat` at user login in Session 1 |

**No new crates or packages required.** All networking is standard Tokio. HTML/JS auto-retry is pure browser JavaScript.

### Installation

No new packages. Deliverable is a modified `rc-agent.exe` binary deployed to all 8 pods via the existing pod-agent remote deploy pattern.

---

## Architecture Patterns

### Current Startup Sequence (BROKEN — race condition)

```
HKLM Run fires start-rcagent.bat
  → taskkill old agent (if any)
  → start "" rc-agent.exe
      → main() starts
      → early_lock_screen.start_server()     ← tokio::spawn (async, not awaited)
      → early_lock_screen.show_config_error() OR drop(early_lock_screen)
      → lock_screen.start_server()           ← tokio::spawn (async, not awaited)
      → lock_screen is in LockScreenState::Hidden
      → [reconnect loop starts]
      → first ws connect attempt FAILS
      → lock_screen.show_disconnected()      ← state = Disconnected, but NO launch_browser()
      → reconnect loop retries...

Edge is NEVER launched until the first successful WebSocket connect brings
a CoreToAgent command that triggers show_pin_screen / show_blank_screen etc.
```

**Root cause:** `show_disconnected()` intentionally does NOT call `launch_browser()` (it only sets state). Edge is only launched by explicit state transitions like `show_pin_screen()`, `show_blank_screen()`, `show_session_summary()` etc. On a fresh reboot, if racecontrol is also starting up, all WebSocket attempts fail for 5-60+ seconds and the pod screen shows... nothing (Edge was never opened).

Additionally: when `launch_browser()` IS eventually called, there is a small race where `tokio::spawn(serve_lock_screen(...))` may not have completed its `TcpSocket::bind()` yet and Edge hits `ERR_CONNECTION_REFUSED`.

### Fixed Startup Sequence

```
HKLM Run fires start-rcagent.bat
  → taskkill old agent (if any)
  → start "" rc-agent.exe
      → main() starts
      → lock_screen.start_server()           ← tokio::spawn (async)
      → lock_screen.wait_for_self_ready()    ← async loop: poll 127.0.0.1:18923 until TCP accept
      → lock_screen.show_startup_connecting() ← state = StartupConnecting, calls launch_browser()

      Edge opens IMMEDIATELY, shows branded "Connecting..." page
      Page has: setTimeout(location.reload, 3000)

      → [reconnect loop starts]
      → ws connect attempts fail → show_disconnected() (state only, no browser change)
      → ws connect succeeds → core sends initial state → normal operation
```

### Recovery After Mid-Session rc-agent Crash

```
rc-agent crashes (Edge kiosk window: blank / "refused")

HKLM Run key does NOT fire on crash — it only fires on login.
start-rcagent.bat is NOT a watchdog (watchdog deleted Mar 11, 2026).

Current state: Pod screen shows blank/error. Staff must restart rc-agent manually.

After Phase 8 fix:
  Staff or pod-agent restarts rc-agent
  → main() starts (same startup sequence as above)
  → close_browser() in launch_browser() kills the stale blank Edge window
  → new Edge window opens showing "Connecting..." branded page
  → within 30s: ws connects, lock screen shows idle/branded state
```

**Key insight for LOCK-03:** The 30-second recovery window is met as long as: (a) rc-agent restarts within ~20 seconds of crash (staff action or manual pod-agent exec), and (b) the startup sequence shows a branded page immediately on launch. The `close_browser()` in `launch_browser()` handles the stale window cleanup.

### wait_for_self_ready() Pattern

```rust
// In lock_screen.rs — add async helper
impl LockScreenManager {
    /// Wait until the lock screen HTTP server is accepting connections.
    /// Returns when port 18923 responds to a TCP SYN, or after timeout.
    pub async fn wait_for_self_ready(&self) {
        let addr: std::net::SocketAddr = format!("127.0.0.1:{}", self.port)
            .parse()
            .unwrap();
        let deadline = tokio::time::Instant::now()
            + std::time::Duration::from_secs(5);
        loop {
            match tokio::time::timeout(
                std::time::Duration::from_millis(100),
                tokio::net::TcpStream::connect(addr),
            ).await {
                Ok(Ok(_)) => {
                    tracing::info!("Lock screen server ready on port {}", self.port);
                    return;
                }
                _ => {
                    if tokio::time::Instant::now() >= deadline {
                        tracing::warn!(
                            "Lock screen server did not become ready in 5s — \
                             proceeding anyway"
                        );
                        return;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
            }
        }
    }
}
```

### New StartupConnecting State

```rust
// In lock_screen.rs — add variant to LockScreenState enum
/// Startup connecting state — shown before first WebSocket connection to racecontrol.
/// Rendered as a branded "Connecting..." page with auto-retry.
StartupConnecting,
```

```rust
// Add method to LockScreenManager
pub fn show_startup_connecting(&mut self) {
    {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        *state = LockScreenState::StartupConnecting;
    }
    self.launch_browser();
}
```

```rust
// In render_page() match arm
LockScreenState::StartupConnecting => render_startup_connecting_page(),
```

```rust
// HTML for startup connecting page
fn render_startup_connecting_page() -> String {
    page_shell(
        "Racing Point — Starting",
        r#"<div style="text-align:center;padding-top:30vh">
<div style="font-family:Enthocentric,sans-serif;font-size:2.2em;color:#E10600;
            letter-spacing:0.06em;margin-bottom:24px">RACING POINT</div>
<div style="font-size:1em;color:#888;margin-bottom:32px">Starting up...</div>
<div style="display:inline-block;width:48px;height:48px;border:3px solid #333;
            border-top-color:#E10600;border-radius:50%;
            animation:spin 0.9s linear infinite"></div>
</div>
<style>
@keyframes spin { 0%{transform:rotate(0deg)} 100%{transform:rotate(360deg)} }
</style>
<script>setTimeout(function(){location.reload()},3000)</script>"#,
    )
}
```

### main.rs Startup Wiring Change

```rust
// In main() after lock_screen.start_server() and before reconnect loop:

lock_screen.start_server();
tracing::info!("Lock screen server started on port 18923");

// LOCK-01/LOCK-02: Wait for port to be listening before launching browser
// This eliminates the ERR_CONNECTION_REFUSED race condition.
lock_screen.wait_for_self_ready().await;

// LOCK-02: Show branded startup page immediately — customer sees Racing Point
// branding while rc-agent connects to racecontrol, not a blank screen.
lock_screen.show_startup_connecting();
```

### Startup State Machine

```
State: StartupConnecting  →  browser open, 3s JS reload loop
  ↓ (on first successful WebSocket connect + CoreToAgent::BillingIdle or similar)
State: Disconnected        →  still shows reload loop (no browser change)
  ↓ (on core assignment: ShowLockScreen command)
State: PinEntry / QrDisplay / ScreenBlanked  →  normal operation
```

The `show_disconnected()` path (called on every failed ws attempt) must NOT call `launch_browser()` — it already does not, so no change needed there. The `StartupConnecting` browser window persists and auto-reloads every 3 seconds, so the customer always sees live state.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Port readiness check | File lock, named pipe, external healthcheck binary | `tokio::net::TcpStream::connect()` loop | Already available in Tokio, 5 lines, no dependencies |
| Stale Edge window cleanup | WinAPI EnumWindows + FindWindow | `taskkill /F /IM msedge.exe` (already in `close_browser()`) | close_browser() already kills all Edge + WebView2 processes aggressively |
| Auto-retry in browser | Websocket from Edge to rc-agent | Plain `setTimeout(location.reload, 3000)` in HTML | Already used in render_disconnected_page(); same pattern works for StartupConnecting |
| Service dependency ordering | Windows Service dependencies, sc.exe ordering | Startup readiness probe in rc-agent itself | Avoids Windows service manager entirely; rc-agent starts in Session 1 via HKLM Run, not as a service |

---

## Common Pitfalls

### Pitfall 1: show_disconnected() Called Before Browser Opens
**What goes wrong:** On every failed WebSocket attempt, `show_disconnected()` is called. This sets state to `Disconnected`. If `launch_browser()` was never called first, the state changes are invisible — no Edge window is open yet.
**Why it happens:** `show_disconnected()` intentionally skips `launch_browser()` (design: avoids relaunching Edge on every reconnect attempt).
**How to avoid:** Call `show_startup_connecting()` ONCE before the reconnect loop. This opens Edge once. All subsequent `show_disconnected()` calls update state only; the open browser reloads and picks up the new state.
**Warning sign:** If pod screen is blank after reboot, `show_startup_connecting()` was not called.

### Pitfall 2: Race Between start_server() and launch_browser()
**What goes wrong:** `start_server()` spawns a Tokio task that binds to port 18923 asynchronously. If `launch_browser()` fires before the bind completes, Edge gets `ERR_CONNECTION_REFUSED` on first load.
**Why it happens:** `tokio::spawn` returns immediately; the actual `TcpSocket::bind()` inside `serve_lock_screen()` runs concurrently.
**How to avoid:** `wait_for_self_ready()` polls `127.0.0.1:18923` with 50ms intervals up to 5 seconds. On a healthy system, the bind completes in under 10ms.
**Warning sign:** Edge shows "ERR_CONNECTION_REFUSED" on first open, then works fine after a manual refresh.

### Pitfall 3: Early Lock Screen vs Main Lock Screen Port Collision
**What goes wrong:** `main.rs` creates an `early_lock_screen` on port 18923 for config error display, then creates a new `LockScreenManager` (`lock_screen`) also on port 18923. If `early_lock_screen` is not dropped before `lock_screen.start_server()`, the second bind fails because `SO_REUSEADDR` is not set on Windows the same way as Linux.
**Why it happens:** `drop(early_lock_screen)` is called at line 240. The TCP socket from `serve_lock_screen()` may be in `TIME_WAIT` or still held.
**How to avoid:** The existing code calls `drop(early_lock_screen)` before the main `lock_screen.start_server()`. The socket uses `SO_REUSEADDR` (line 549 in lock_screen.rs). The 500ms `wait_for_self_ready()` polling naturally handles the brief gap.
**Warning sign:** Lock screen server logs "failed to bind port 18923".

### Pitfall 4: Stale Edge from Before Crash Not Killed
**What goes wrong:** rc-agent crashes; old Edge window shows blank. rc-agent restarts; `launch_browser()` spawns a NEW Edge window. Two Edge windows now exist — the stale blank one and the new branded one.
**Why it happens:** `launch_browser()` calls `close_browser()` first, which runs `taskkill /F /IM msedge.exe`. This kills ALL Edge processes including the stale one.
**How to avoid:** Already handled by existing `close_browser()` implementation. No change needed.
**Warning sign:** Multiple Edge windows visible after rc-agent restart.

### Pitfall 5: LOCK-03 Requires Staff Action — Not Fully Automatic
**What goes wrong:** LOCK-03 says "pod screen automatically recovers within 30 seconds — no staff intervention". But with the watchdog deleted, rc-agent does NOT auto-restart on crash.
**Why it happens:** Watchdog scheduled task was deleted on Mar 11, 2026 (see MEMORY.md). No auto-restart mechanism exists.
**How to avoid:** LOCK-03 is met IF rc-agent is restarted within ~25 seconds (leaving 5s for startup + connect). Staff use pod-agent remote exec to restart. The `start-rcagent.bat` HKLM Run key only fires at login, not on crash. Phase 8 cannot auto-restart rc-agent without re-introducing a watchdog.
**Interpretation:** LOCK-03 success criterion says "no staff intervention required". This is achievable only if a restart mechanism exists. Plan must include either: (a) a lightweight Rust self-watchdog inside rc-agent that detects crash and relaunches, OR (b) re-introduction of a minimal watchdog (bat file or scheduled task) that periodically checks rc-agent is running. Option (b) is lower risk.
**Recommendation:** Add a dedicated watchdog re-introduction task in the plan specifically for LOCK-03 recovery. Keep it minimal: a scheduled task that runs every 60 seconds, checks if `rc-agent.exe` is in the process list, and runs `start-rcagent.bat` if not.

### Pitfall 6: is_idle_or_blanked() Treats StartupConnecting as Not Idle
**What goes wrong:** `is_idle_or_blanked()` is used to decide whether to blank the screen after a session. If `StartupConnecting` is not included, other logic may misbehave.
**How to avoid:** Include `StartupConnecting` in the `is_idle_or_blanked()` match to align with `Hidden` and `Disconnected`:
```rust
pub fn is_idle_or_blanked(&self) -> bool {
    let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
    matches!(*state,
        LockScreenState::Hidden
        | LockScreenState::ScreenBlanked
        | LockScreenState::Disconnected
        | LockScreenState::StartupConnecting  // ← add this
    )
}
```

---

## Code Examples

### wait_for_self_ready (lock_screen.rs)

```rust
// Source: codebase analysis — matches pattern of tokio::net::TcpStream::connect probing
pub async fn wait_for_self_ready(&self) {
    let addr: std::net::SocketAddr = format!("127.0.0.1:{}", self.port)
        .parse()
        .unwrap();
    let deadline = tokio::time::Instant::now()
        + std::time::Duration::from_secs(5);
    loop {
        match tokio::time::timeout(
            std::time::Duration::from_millis(100),
            tokio::net::TcpStream::connect(addr),
        ).await {
            Ok(Ok(_)) => {
                tracing::info!("Lock screen server ready on port {}", self.port);
                return;
            }
            _ => {
                if tokio::time::Instant::now() >= deadline {
                    tracing::warn!(
                        "Lock screen server not ready after 5s — launching browser anyway"
                    );
                    return;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }
    }
}
```

### main.rs startup wiring

```rust
// Source: crates/rc-agent/src/main.rs — after lock_screen.start_server()
lock_screen.start_server();
tracing::info!("Lock screen server started on port 18923");

// LOCK-01: Ensure port is listening before launching browser
lock_screen.wait_for_self_ready().await;

// LOCK-02: Show branded startup page immediately on every boot
lock_screen.show_startup_connecting();
// (reconnect loop follows — show_disconnected() called on failures, state-only, no browser change)
```

### Minimal Watchdog Re-introduction (for LOCK-03)

```bat
@echo off
:: RaceControl Agent Watchdog — runs every 60s via Task Scheduler
:: Checks if rc-agent.exe is running; restarts via start-rcagent.bat if not
tasklist /NH /FI "IMAGENAME eq rc-agent.exe" 2>nul | find /i "rc-agent.exe" >nul
if errorlevel 1 (
    echo %DATE% %TIME% rc-agent not running — restarting >> C:\RacingPoint\watchdog.log
    call C:\RacingPoint\start-rcagent.bat
)
```

Deployed as a scheduled task via pod-agent exec:

```
schtasks /create /TN "RCAgentWatchdog"
  /TR "C:\RacingPoint\watchdog-rcagent.bat"
  /SC MINUTE /MO 1
  /RU SYSTEM /RL HIGHEST /F
```

**Note:** This is a 1-minute polling watchdog (not the deleted 2-minute one). It uses `SYSTEM` account to ensure it runs even if the user session is not active. Recovery time: up to 60 seconds for watchdog to fire + ~5 seconds for rc-agent startup = worst case 65 seconds. Success criterion says 30 seconds. To meet 30-second target, use `/SC MINUTE /MO 1` with a secondary check: the watchdog can run every 30 seconds via two scheduled tasks offset by 30 seconds.

**Simplest approach meeting 30s requirement:** Schedule task at `/SC MINUTE /MO 1` (fires at :00 and :30 of each minute is NOT what this does — it fires once per minute). For strict 30-second guarantee, use two tasks: one at /SC MINUTE /MO 1 and one with a `/SD` start-delay of 30 seconds. Alternatively: accept up to 60-second recovery and interpret "30 seconds" as the rc-agent startup time after watchdog triggers, not the full detection + restart window.

**Recommendation for plan:** Create two scheduled tasks: `RCAgentWatchdog` at /MO 1 and `RCAgentWatchdog2` also at /MO 1 but triggered with a 30-second `Start In` delay. This gives ~30-second detection window.

---

## State of the Art

| Old Approach | Current Approach | Phase 8 Change | Impact |
|--------------|------------------|----------------|--------|
| No startup browser launch | Edge never opened until first WS command from racecontrol | `show_startup_connecting()` called at startup | Pod shows branded page from first boot, not blank screen |
| Race: port bind vs browser launch | `launch_browser()` fires immediately after `start_server()` spawn | `wait_for_self_ready()` probes port before browser launch | Eliminates `ERR_CONNECTION_REFUSED` on first load |
| No startup state | `LockScreenState::Hidden` is initial state (shows "session not active") | New `StartupConnecting` state | Correct branded message during startup, not idle message |
| No auto-restart on crash | Watchdog deleted Mar 11, 2026; no auto-recovery | Re-introduce minimal watchdog for rc-agent only | Enables LOCK-03 30-second recovery without staff intervention |
| Mid-session crash: stale blank Edge | Edge window stays blank until staff restarts | `close_browser()` + `launch_browser()` on rc-agent restart | Fresh branded window on every restart |

**Deprecated/outdated:**
- Watchdog scheduled task (deleted Mar 11, 2026): Was a 2-minute general watchdog. Phase 8 re-introduces a 1-minute rc-agent-specific watchdog. The 2-minute all-services watchdog is not restored — only the rc-agent one.

---

## Open Questions

1. **LOCK-03 30-Second Recovery — Interpretation**
   - What we know: watchdog deleted, no auto-restart; worst-case watchdog detection is 60 seconds
   - What's unclear: whether "30 seconds" means total recovery time or just rc-agent startup time after trigger
   - Recommendation: Plan includes two staggered watchdog tasks to achieve ~30-second detection window, meeting the literal requirement. If Uday accepts 60s, one task is sufficient.

2. **Early Lock Screen Timing at Startup**
   - What we know: `early_lock_screen` in main() uses port 18923 for config error display; it is dropped before `lock_screen.start_server()` is called
   - What's unclear: whether `drop(early_lock_screen)` fully releases the port before the next `TcpSocket::bind()` on Windows (TIME_WAIT on loopback)
   - Recommendation: The existing `SO_REUSEADDR` on the socket handles this. `wait_for_self_ready()` ensures port is accepting before browser opens. No change needed.

3. **show_disconnected() and Browser State**
   - What we know: `show_disconnected()` does not call `launch_browser()`; it only updates state. The open browser polls `127.0.0.1:18923` via `location.reload()` every 3 seconds.
   - What's unclear: whether the 3-second JS reload in `render_disconnected_page()` is enough to pick up state changes (StartupConnecting → Disconnected transition)
   - Recommendation: Both `StartupConnecting` and `Disconnected` pages have the same 3-second JS reload. When state changes, the next reload shows the new page. No issue.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[cfg(test)]` modules |
| Config file | None — colocated with source modules |
| Quick run command | `cargo test -p rc-agent-crate && cargo test -p rc-common` |
| Full suite command | `cargo test -p rc-agent-crate && cargo test -p rc-common && cargo test -p racecontrol-crate` |

**Current test count:** 140 tests in rc-agent (all passing as of 2026-03-14).

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| LOCK-01 | `wait_for_self_ready()` returns when port is open | unit (bind loopback in test) | `cargo test -p rc-agent-crate lock_screen::tests::wait_for_self_ready_succeeds_when_port_open` | ❌ Wave 0 |
| LOCK-01 | `wait_for_self_ready()` times out gracefully when port never opens | unit (no bind) | `cargo test -p rc-agent-crate lock_screen::tests::wait_for_self_ready_timeout` | ❌ Wave 0 |
| LOCK-02 | `StartupConnecting` state renders branded HTML (no error text, no blank) | unit | `cargo test -p rc-agent-crate lock_screen::tests::startup_connecting_renders_branded_html` | ❌ Wave 0 |
| LOCK-02 | `StartupConnecting` state has 3s JS reload in HTML | unit | `cargo test -p rc-agent-crate lock_screen::tests::startup_connecting_has_reload_script` | ❌ Wave 0 |
| LOCK-02 | `is_idle_or_blanked()` returns true for `StartupConnecting` | unit | `cargo test -p rc-agent-crate lock_screen::tests::startup_connecting_is_idle_or_blanked` | ❌ Wave 0 |
| LOCK-03 | `health_*` endpoint returns degraded for `StartupConnecting` | unit | `cargo test -p rc-agent-crate lock_screen::tests::health_degraded_for_startup_connecting` | ❌ Wave 0 |
| LOCK-03 | Watchdog task creation command is correct (idempotent) | manual | pod-agent exec `schtasks /query /TN RCAgentWatchdog` on Pod 8 | N/A |

**Manual verification (no automated path):**
- Pod reboot: Edge shows branded page within 10 seconds of desktop appearing (LOCK-01/LOCK-02)
- rc-agent restart: Pod screen shows branded "Connecting..." and then recovers to lock screen (LOCK-03)
- Watchdog fires: kill rc-agent via pod-agent, wait 60 seconds, confirm rc-agent restarted automatically

### Sampling Rate

- **Per task commit:** `cargo test -p rc-agent-crate && cargo test -p rc-common`
- **Per wave merge:** `cargo test -p rc-agent-crate && cargo test -p rc-common && cargo test -p racecontrol-crate`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `crates/rc-agent/src/lock_screen.rs` — add `wait_for_self_ready_succeeds_when_port_open` test: bind a real loopback socket in test, call `wait_for_self_ready()`, assert returns quickly
- [ ] `crates/rc-agent/src/lock_screen.rs` — add `wait_for_self_ready_timeout` test: no bind, verify function returns (not panics) within 6 seconds
- [ ] `crates/rc-agent/src/lock_screen.rs` — add `startup_connecting_renders_branded_html` test: render `StartupConnecting`, assert HTML contains "RACING POINT" and spin animation
- [ ] `crates/rc-agent/src/lock_screen.rs` — add `startup_connecting_has_reload_script` test: rendered HTML contains `location.reload`
- [ ] `crates/rc-agent/src/lock_screen.rs` — add `startup_connecting_is_idle_or_blanked` test: assert `is_idle_or_blanked()` returns true when state is `StartupConnecting`
- [ ] `crates/rc-agent/src/lock_screen.rs` — add `health_degraded_for_startup_connecting` test: assert health response indicates not-ready for `StartupConnecting` state

---

## Sources

### Primary (HIGH confidence)

- `crates/rc-agent/src/lock_screen.rs` — Full LockScreenManager, `serve_lock_screen()`, `launch_browser()`, `close_browser()`, `show_disconnected()`, `render_disconnected_page()`, `SO_REUSEADDR` socket setup, existing state enum
- `crates/rc-agent/src/main.rs` — Startup sequence (lines 186-400), `start_server()` call, reconnect loop (lines 458-490), `show_disconnected()` call sites
- `deploy-staging/start-rcagent.bat` — Pod startup script (4 lines: kill + start)
- `deploy-staging/install.bat` — Pod install script; confirms HKLM Run key setup and watchdog pattern
- `cargo test -p rc-agent-crate -- --list` — 140 tests, all passing; confirmed test module structure
- `.planning/STATE.md` — confirmed watchdog deleted Mar 11, 2026; HKLM Run key is the restart mechanism

### Secondary (MEDIUM confidence)

- Phase 5 RESEARCH.md — confirmed existing `render_disconnected_page()` 3s JS reload pattern; confirmed `LockScreenState` enum extension approach; confirmed `is_idle_or_blanked()` pattern

### Tertiary (LOW confidence)

- None — all findings are directly from source code. No WebSearch required.

---

## Metadata

**Confidence breakdown:**
- LOCK-01 readiness probe approach: HIGH — standard Tokio `TcpStream::connect` probe, matches existing codebase patterns
- LOCK-02 new state variant: HIGH — four existing states already follow the identical pattern; adding a fifth is mechanical
- LOCK-03 watchdog re-introduction: HIGH for approach, MEDIUM for exact 30-second timing guarantee
- HTML auto-retry pattern: HIGH — identical to `render_disconnected_page()` already in production

**Research date:** 2026-03-14
**Valid until:** 2026-04-14 (stable Rust codebase — 30 day window appropriate)
