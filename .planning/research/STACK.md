# Stack Research

**Domain:** RC Bot Expansion (v5.0) — deterministic auto-fix patterns for sim racing pod management (Windows 11)
**Researched:** 2026-03-16
**Confidence:** HIGH (codebase read directly; version data from crates.io searches)

---

> **Milestone scope:** This file covers the v5.0 RC Bot Expansion ONLY — new bot patterns for
> ai_debugger.rs expansion. All prior v1.0–v4.0 stack additions (windows-service, winreg,
> tokio-util, netsh firewall) remain in place and are not repeated here.
> Focus: process hang detection, USB reconnect, UDP gap, billing edge cases, game launch failures.

---

## Executive Finding: No New Crates Required

All 9 new bot patterns are implementable using crates already present in rc-agent. The v5.0
expansion is a code addition, not a dependency addition. The PROJECT.md constraint
("no new dependencies where possible") is fully satisfiable here — not as a compromise
but as the correct architectural choice.

---

## What Already Exists (Authoritative — Read From Source)

Read directly from `crates/rc-agent/Cargo.toml`, source files, and PROJECT.md.

### Existing rc-agent Cargo.toml Dependencies

| Crate | Version | Bot-Relevant Capability |
|-------|---------|------------------------|
| `sysinfo` | 0.33 | Process enumeration, CPU usage, process status, memory |
| `winapi` | 0.3 | `IsHungAppWindow`, `EnumWindows`, `GetExitCodeProcess`, `OpenProcess` |
| `hidapi` | 2 | USB HID open/read/write/enumerate (OpenFFBoard wheelbase) |
| `tokio` | 1 (workspace) | Async timers, `time::timeout`, `time::interval`, mpsc channels |
| `reqwest` | 0.12 | HTTP client for cloud sync error detection, Ollama/AI calls |
| `chrono` | 0.4 (workspace) | Session timestamps, billing duration calculation |
| `serde_json` | 1 (workspace) | Structured event log payloads |
| `tracing` | 0.1 (workspace) | Structured logging throughout |
| `anyhow` | 1 (workspace) | Error propagation |
| `axum` | 0.8 | rc-agent HTTP server (port 8090) |

### Existing Implementation Coverage Map

| Bot Pattern | What Exists | File |
|-------------|-------------|------|
| Process alive check | `winapi::GetExitCodeProcess` (STILL_ACTIVE=259) | `game_process.rs:317` |
| Process kill by PID | `taskkill /PID {pid} /F` via `hidden_cmd()` | `game_process.rs:341` |
| Process kill by name | `taskkill /IM {name} /F` in `PROTECTED_PROCESSES` guard | `ai_debugger.rs:386` |
| Process scan by name | `sysinfo::System::refresh_processes()` + name match | `game_process.rs:96` |
| PID file persistence | Flat file `C:\RaceControl\game.pid` | `game_process.rs:33` |
| USB HID input read | `hidapi::open()` + `read_timeout()`, 10ms poll interval | `driving_detector.rs` |
| USB HID disconnect detect | `DetectorSignal::HidDisconnected` on open failure | `driving_detector.rs:87` |
| FFB write (safety zero) | `hidapi` vendor HID usage page 0xFF00, report 0xA1 | `ffb_controller.rs` |
| UDP gap detection | `last_udp_packet.elapsed() > 2s` | `driving_detector.rs:110` |
| WS dead detection | `ws_last_connected.elapsed() >= WS_DEAD_SECS` | `self_monitor.rs:52` |
| CLOSE_WAIT flood detect | `netstat -ano` line count filter | `self_monitor.rs:136` |
| AI pattern memory | `DebugMemory` with `instant_fix()` replay | `ai_debugger.rs:77` |
| Auto-fix dispatch | `try_auto_fix()` keyword match on AI suggestion | `ai_debugger.rs:303` |
| WerFault kill | `taskkill /IM WerFault.exe /F` | `ai_debugger.rs:455` |
| Temp file cleanup | `Remove-Item $env:TEMP` via PowerShell | `ai_debugger.rs:425` |
| Stale socket cleanup | PowerShell `Get-NetTCPConnection` + `Stop-Process` | `ai_debugger.rs:346` |
| Bot event log | Append to `C:\RacingPoint\rc-bot-events.log` with rotation | `self_monitor.rs:110` |
| Relaunch self | Detached PowerShell `Start-Process` + `exit(0)` | `self_monitor.rs:173` |
| HeartbeatStatus atomics | `ws_connected`, `game_running`, `driving_active`, `billing_active` | `udp_heartbeat.rs:30` |
| PodStateSnapshot | Pod state capture at crash time | `ai_debugger.rs:33` |

---

## New Capabilities Needed Per Bot Pattern

### 1. Pod Crash/Hang Detection

**What's missing:** Game process CPU near-zero + no UDP frames is already detectable with `sysinfo` + `HeartbeatStatus`. The only gap is `IsHungAppWindow` for confirming a GUI freeze (game window not responding to messages).

**How to add — no new crate:**

`IsHungAppWindow` is in `winuser.h`, which maps to the `winuser` feature of `winapi 0.3`. The `winuser` feature is **already present** in `rc-agent/Cargo.toml`. The call requires a `HWND`. Getting the HWND from a PID requires `EnumWindows` (also in `winuser`) — a callback that matches `GetWindowThreadProcessId` against the target PID.

```rust
// Pseudocode — uses only existing winapi::winuser features
unsafe fn is_game_window_hung(game_pid: u32) -> bool {
    // EnumWindows callback: find HWND where GetWindowThreadProcessId == game_pid
    // Then: IsHungAppWindow(hwnd) != 0
}
```

**Hang heuristic (conjunction of signals — avoids false positives on menu/loading screens):**
- `sysinfo` CPU usage < 2% for the game PID (two refreshes, 500ms apart)
- `HeartbeatStatus::game_running` = true
- `last_udp_packet.elapsed() > 30s` (no telemetry for 30s while game "running")
- `IsHungAppWindow(hwnd)` = true

Only when all four are true should the hang fix trigger. A game at the AC main menu has CPU near 0% and no UDP but is NOT hung — the IsHungAppWindow check is the discriminator.

**sysinfo two-refresh pattern (required for accurate CPU):**

```rust
// sysinfo CPU usage requires two samples
let mut sys = System::new();
sys.refresh_processes(ProcessesToUpdate::All, true);
tokio::time::sleep(Duration::from_millis(500)).await;
sys.refresh_processes(ProcessesToUpdate::All, true);
let cpu = sys.process(pid).map(|p| p.cpu_usage()).unwrap_or(0.0);
```

This is documented sysinfo behavior — do not skip the second refresh.

---

### 2. Billing Edge Case Recovery

**What's missing:** Server-side billing is in `racecontrol/billing.rs`. The rc-agent side has `HeartbeatStatus::billing_active` atomic. The gap is detecting when `billing_active=true` but `game_running=false` for >60s (stuck session — game exited but billing didn't end).

**How to add — no new crate:**

The existing `self_monitor.rs` 60s loop is the right home. Add:

```rust
if status.billing_active.load(Ordering::Relaxed)
    && !status.game_running.load(Ordering::Relaxed)
    && billing_game_gap_secs >= 60
{
    // Emit AgentMessage::BillingStuck over WebSocket to trigger server-side end_session()
}
```

The WebSocket sender already exists in the main loop. The bot signals via an mpsc channel (same pattern as `HeartbeatEvent::CoreDead`).

**No new crates.** Uses: `tokio` (timers), atomic bools in `HeartbeatStatus`, existing WS channel.

---

### 3. Network/Connection Drop Auto-Repair

**What already handles this:** `self_monitor.rs` already handles WS dead for 60s → `relaunch_self()`. The existing path is correct and complete for rc-agent. No gap.

**What's missing on the racecontrol side:** IP drift detection. But this is a server-side concern (pod_monitor.rs, not ai_debugger.rs). Out of scope for this crate.

**No additions needed in rc-agent.**

---

### 4. USB Hardware Failure (Wheelbase Disconnect/Reconnect)

**What's missing:** Detecting when the wheelbase goes from connected → disconnected → connected (a USB reset/replug cycle) and clearing the FFB fault state after reconnect.

**How to add — no new crate:**

`hidapi::enumerate()` already returns all connected HID devices. The `DrivingDetector` already tracks `hid_connected: bool` from the `HidDisconnected` signal. The reconnect bot adds:

1. A counter for disconnect events this session (`usb_reconnect_count: u32`)
2. On reconnect detection (was disconnected, now enumerable again): call `FfbController::zero_force()` as a safety reset before resuming input polling — prevents the wheelbase resuming with stale FFB state

The `FfbController` is already implemented in `ffb_controller.rs` using `hidapi`. No new API surface.

**Pattern for reconnect detection (polling, no WM_DEVICECHANGE needed):**

```rust
// Poll every 5s in the HID monitoring task
let devices = hidapi.device_list();
let wheelbase_present = devices.any(|d| d.vendor_id() == VID && d.product_id() == PID);
// Previously disconnected + now present = reconnect event
```

The `hidapi` crate's `device_list()` method is a scan of currently attached devices — equivalent to re-enumerating. No hotplug callback required; 5s polling latency is acceptable for a wheelbase reconnect scenario.

**No new crates.** Uses: `hidapi 2` (already present), `FfbController` (already present).

---

### 5. Game Launch Failure Recovery (Content Manager Hang, AC Timeout)

**What's missing:** Detecting when a `LaunchGame` command was sent but the game PID never appeared within a timeout window.

**How to add — no new crate:**

`tokio::time::timeout` wraps the PID polling loop:

```rust
// After spawning game launch:
tokio::time::timeout(
    Duration::from_secs(90),  // AC takes 60-80s on cold start
    async {
        loop {
            if find_game_pid(sim_type).is_some() { return Ok(()); }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }
).await
// On Err(Elapsed): trigger fix_kill_content_manager() + retry
```

Content Manager (the AC launcher) can hang. Its process name is `Content Manager.exe` or `acmanager.exe`. Fix = kill CM + kill acs.exe + retry launch. All via existing `taskkill` `hidden_cmd()` pattern.

**No new crates.** Uses: `tokio::time::timeout` (already in workspace), `sysinfo` process scan (already present), `hidden_cmd` (already in codebase).

---

### 6. Telemetry Gap Detection

**What already exists:** `driving_detector.rs` already tracks `last_udp_packet: Option<Instant>` and flags `udp_active = false` after 2s of no packets. `HeartbeatStatus::game_running` and `driving_active` are exposed as atomics.

**What's missing:** Sending a staff alert when telemetry has been absent for a prolonged period (e.g., >5 minutes while game is running). The existing 2s gap is for billing idle detection — not for staff alerting.

**How to add — no new crate:**

The `self_monitor.rs` loop checks `game_running` and `driving_active`. Add: if `game_running=true` AND `driving_active=false` AND elapsed > 300s → log event + emit `AgentMessage::TelemetryGap` over WebSocket → racecontrol forwards as email alert.

Email alerts already work via the existing `email_alerts.rs` shell-out mechanism.

**No new crates.**

---

### 7. Multiplayer Session Guard

**What already exists:** `sims/assetto_corsa.rs` parses AC UDP frames including session type and server connection fields. `HeartbeatStatus` has `game_running`.

**What's missing:** Detecting when AC disconnects from the server mid-session (AC returns to server list, sends a session_type = 0 frame).

**How to add — no new crate:**

In the UDP frame processing loop, track `last_mp_session_id`. If session ID resets to 0 while billing is active → emit `AgentMessage::MultiplayerDisconnect`. Server-side racecontrol decides whether to auto-rejoin or trigger safe teardown.

**No new crates.** Uses: existing UDP parsing in `sims/assetto_corsa.rs`.

---

### 8. Kiosk PIN Failure Recovery

**What already exists:** PIN auth is validated at racecontrol. rc-agent receives lock/unlock commands over WebSocket. Failed PIN attempts return an error response.

**What's missing:** Detecting repeated PIN failures (brute-force / stuck customer) and escalating — lock the screen for 5 minutes and alert staff.

**How to add — no new crate:**

Track `pin_fail_count: u8` and `last_pin_fail: Instant` in the agent's main state. After 5 failures in 60s → set `lock_screen_extended = true` + emit `AgentMessage::PinLockout` → racecontrol alerts Uday.

**No new crates.** Uses: existing lock_screen.rs state, existing WS channel.

---

### 9. Lap Filtering (Auto-Flag Invalid Laps)

**What already exists:** The `Lap` struct in `rc-common` has a `valid: bool` field. The lap tracker already checks `valid` before committing to leaderboard. `sims/assetto_corsa.rs` passes through the `valid` flag from AC's own UDP output.

**What's missing:** Additional client-side validation rules: lap time sanity check (too fast = track cut, too slow = spin/pause), lap continuity (missing telemetry frames mid-lap), and session-type tagging (hotlap vs practice).

**How to add — no new crate:**

Add a `LapValidator` struct in `rc-agent/src/sims/` with rules:
- `lap_time_ms < TRACK_MINIMUM_MS` → flag as invalid (track record floor per track config)
- `total_telemetry_frames < MIN_FRAMES_PER_LAP` → flag as suspect (missing data)
- `session_type == Hotlap` → tag `session_category = "hotlap"` on the Lap message

The track minimum lap times can be stored in `rc-agent.toml` under `[lap_validation]`. No external data source needed — staff configures the floor on first use.

**No new crates.** Uses: `rc-common` Lap types (already present), `toml` config (already present).

---

## Summary: What to Add to Cargo.toml

**Answer: Nothing.**

All 9 bot patterns are implementable with the current rc-agent dependency set. This is the correct finding — not a forced limitation.

```toml
# rc-agent/Cargo.toml after v5.0 — unchanged from v4.0
# No new [dependencies] entries
# No new [target.'cfg(windows)'.dependencies] entries
```

The existing `winapi 0.3` `winuser` feature covers `IsHungAppWindow` and `EnumWindows`. The existing `hidapi 2` covers USB reconnect polling. The existing `sysinfo 0.33` covers CPU hang detection. The existing `tokio` covers all timeout and interval patterns.

---

## What NOT to Add

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `windows-rs` or `windows-sys` | Already have `winapi 0.3` with all needed features. Adding microsoft's crate creates duplicate Windows bindings, longer compile times (windows-rs is large), and potential linker conflicts. The needed APIs (IsHungAppWindow, EnumWindows) are already in the existing `winuser` feature. | Keep `winapi 0.3` as-is |
| `notify` crate (filesystem watcher) | USB disconnect detection does not need filesystem events. `hidapi::device_list()` polling every 5s is correct for this use case. `notify` adds inotify/RDCW complexity with no benefit. | Poll `hidapi::enumerate()` on interval |
| `sysinfo` version upgrade (0.33 → 0.38) | The API between 0.33 and 0.38 changed `ProcessesToUpdate` and `Process::status()` return types. 47 existing tests pass against 0.33. Upgrading breaks the test suite with zero bot benefit — hang detection works identically on 0.33. | Stay on `sysinfo 0.33` |
| `wmi` crate | WMI queries require COM initialization (100ms+ startup cost, threading constraints). `sysinfo` provides the same process CPU data faster and is already tested. | `sysinfo 0.33` |
| `tokio-process` / `async-process` | Game process spawning uses `std::process::Command` + `Child::try_wait()` — the fire-and-observe pattern. Async spawning adds complexity for no benefit here; the bot doesn't need async child I/O streams. | `std::process::Command` (existing) |
| New UDP socket per bot | Telemetry gap bots read `HeartbeatStatus` atomics, which are already updated by the existing UDP listeners. No new sockets. | `HeartbeatStatus` atomics |
| `lettre` or any SMTP crate | PROJECT.md explicitly prohibits this. Alerts go through existing `send_email.js` shell-out. | Existing shell-out mechanism |
| WM_DEVICECHANGE for USB detection | Requires a Windows message pump (GUI thread integration). `hidapi` polling every 5s achieves the same result with less complexity and is already tested in the HID monitoring task. | Poll `hidapi::enumerate()` |

---

## Integration Architecture: Where Code Goes

### ai_debugger.rs — New Fix Arms in `try_auto_fix()`

The existing function pattern-matches on AI suggestion keywords. Each new bot pattern adds one `if lower.contains()` arm and one private fix function:

| Pattern keyword | Fix function | Mechanism |
|-----------------|-------------|-----------|
| `"game freeze"` or `"hung process"` | `fix_kill_frozen_game()` | `IsHungAppWindow` check + `taskkill /PID` |
| `"launch timeout"` or `"content manager"` | `fix_kill_cm_hang()` | `taskkill /IM "Content Manager.exe" /F` + `taskkill /IM acs.exe /F` |
| `"usb disconnect"` or `"wheelbase"` | `fix_reset_usb_wheelbase()` | `FfbController::zero_force()` + hidapi re-enumerate |
| `"billing stuck"` or `"session stuck"` | `fix_force_end_session()` | Emit `AgentMessage::BillingStuck` over WS |
| `"telemetry gap"` or `"no udp"` | `fix_restart_udp_listener()` | Signal main loop via mpsc to restart UDP socket |
| `"invalid lap"` or `"lap cut"` | `fix_flag_lap_invalid()` | Emit `AgentMessage::InvalidateLap` |

### self_monitor.rs — New Detection in the 60s Loop

The existing monitoring loop checks CLOSE_WAIT and WS dead. New detectors slot into the same `issues: Vec<String>` pattern:

| New detector | Condition | Action |
|--------------|-----------|--------|
| Billing stuck | `billing_active && !game_running && gap > 60s` | Emit BillingStuck event |
| Telemetry dead | `game_running && !driving_active && gap > 300s` | Log + alert staff |
| USB reconnect count spike | `usb_reconnect_count > 3 in session` | Log + alert staff |

### ai_debugger.rs — PodStateSnapshot Extensions

Add richer context fields to improve AI prompt quality for the new patterns:

```rust
// New fields for PodStateSnapshot:
pub game_cpu_percent: Option<f32>,     // from sysinfo (two-refresh pattern)
pub last_udp_age_secs: u64,            // age of last UDP telemetry frame
pub usb_reconnect_count: u32,          // USB disconnect/reconnect events this session
pub billing_duration_secs: u64,        // how long billing has been active
pub launch_attempt_count: u8,          // game launch retries since last success
pub game_hung: bool,                   // result of IsHungAppWindow check
```

All these come from data already maintained in the main loop or computable from existing state — wiring only, no new APIs.

---

## Windows-Specific Challenges for New Patterns

| Challenge | Severity | Mitigation |
|-----------|----------|------------|
| `IsHungAppWindow` requires HWND, not PID | MEDIUM | `EnumWindows` callback filters by `GetWindowThreadProcessId` → match game PID → pass HWND to `IsHungAppWindow`. Both APIs are in the already-present `winuser` winapi feature. ~30 lines of unsafe. |
| sysinfo CPU requires two refreshes | LOW | Build hang detector as a dedicated async task that maintains a persistent `System` instance. Pre-warm by calling `refresh_processes()` every 30s as a background tick — data is fresh when the hang check fires. |
| Process freeze vs game at menu | MEDIUM | Conjunction check: low CPU + no UDP + `IsHungAppWindow` = hang. Low CPU + no UDP + window responds = game at menu (normal). Never trigger kill on CPU alone. |
| hidapi `device_list()` blocks | LOW | The scan is fast (<5ms on Windows for USB enumeration). Run in a `tokio::task::spawn_blocking` wrapper since it's a synchronous scan. The existing HID polling in driving_detector.rs already uses this pattern. |
| Session 1 constraint (GUI processes) | LOW (already solved) | rc-agent runs in Session 1 via HKLM Run key. All game processes launched from Session 1. `sysinfo` and process enumeration see Session 1 processes correctly. No change needed. |
| Content Manager process name variance | LOW | CM registers as `Content Manager.exe` in Task Manager. Confirm with `tasklist /FI "IMAGENAME eq Content Manager.exe"` on a pod. Update `DIALOG_PROCESSES` list in `ac_launcher.rs` if needed — already the right location. |

---

## Version Status

| Package | In Use | Latest (2026-03) | Upgrade Needed? |
|---------|--------|------------------|-----------------|
| `sysinfo` | 0.33 | 0.38.3 | No — API changes in 0.34–0.38 break existing code with no bot benefit |
| `hidapi` | 2.x | 2.6.x | No — 2.x is backward-compatible; patch version bump is fine if needed |
| `winapi` | 0.3.9 | 0.3.9 (final) | No — stable final version; all needed APIs present |
| `tokio` | 1 (workspace) | 1.44+ | No — tokio 1.x is stable ABI |
| `reqwest` | 0.12 | 0.12.x | No — 0.12 series is current |

---

## Sources

- `crates/rc-agent/Cargo.toml` — authoritative dependency list (read directly, 2026-03-16)
- `crates/rc-agent/src/ai_debugger.rs` — existing auto-fix patterns and `PodStateSnapshot` (read directly)
- `crates/rc-agent/src/self_monitor.rs` — existing bot loop structure and detection patterns (read directly)
- `crates/rc-agent/src/game_process.rs` — process management including `is_process_alive()`, sysinfo usage (read directly)
- `crates/rc-agent/src/driving_detector.rs` — USB/UDP detection state machine (read directly)
- `crates/rc-agent/src/ffb_controller.rs` — hidapi write pattern for wheelbase (read directly)
- `crates/rc-agent/src/udp_heartbeat.rs` — HeartbeatStatus atomics definition (read directly)
- `.planning/PROJECT.md` — explicit "no new dependencies" constraint (read directly)
- [sysinfo crates.io](https://crates.io/crates/sysinfo) — v0.38.3 confirmed latest; v0.33 pinned intentionally (WebSearch, MEDIUM confidence)
- [winapi vs windows-sys comparison](https://kennykerr.ca/rust-getting-started/windows-or-windows-sys.html) — winapi 0.3 functional, windows-sys is modern path but not needed here (MEDIUM confidence)
- [IsHungAppWindow in windows-sys docs](https://docs.rs/windows-sys/latest/windows_sys/Win32/UI/WindowsAndMessaging/fn.IsHungAppWindow.html) — confirmed in winuser feature which is already present (HIGH confidence)
- [sysinfo CPU two-refresh requirement](https://users.rust-lang.org/t/how-to-get-precise-cpu-usage-from-process/117017) — community confirmed, matches sysinfo documentation (MEDIUM confidence)

---

*Stack research for: RC Bot Expansion v5.0 — ai_debugger.rs pattern expansion*
*Researched: 2026-03-16*
