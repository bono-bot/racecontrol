# Phase 24: Crash, Hang, Launch + USB Bot Patterns - Research

**Researched:** 2026-03-16
**Domain:** rc-agent bot expansion — deterministic fix handlers for game freeze, launch timeout, USB reconnect
**Confidence:** HIGH — all findings derived from direct source file inspection (ai_debugger.rs, self_monitor.rs, game_process.rs, driving_detector.rs, ffb_controller.rs, main.rs, types.rs, protocol.rs)

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| CRASH-01 | Bot detects game freeze (UDP silent 30s + IsHungAppWindow) and kills/restarts game without staff intervention | `last_udp_packet` tracked in DrivingDetector; `is_process_alive()` exists in game_process.rs; `IsHungAppWindow` via existing winapi winuser feature; `fix_kill_stale_game()` already exists in ai_debugger.rs |
| CRASH-02 | Bot detects launch timeout (game not running 90s after launch command) and kills Content Manager + retries launch | `LaunchState::WaitingForLive { launched_at }` already exists in main.rs (180s timeout); Content Manager kill via `taskkill /IM "Content Manager.exe"` via hidden_cmd; new 90s threshold for game-not-visible (vs 180s for AcStatus::Live) |
| CRASH-03 | Bot zeros FFB torque before any game kill in teardown sequence (safety ordering) | `FfbController::zero_force()` implemented in ffb_controller.rs; already wired on game crash (line 978); must add to new fix handlers in ai_debugger.rs |
| UI-01 | Bot suppresses Windows error dialogs before any process kill | `fix_kill_error_dialogs()` exists in ai_debugger.rs; current impl only kills WerFault.exe — must also add `taskkill /IM WerFaultSecure.exe` and `taskkill /IM msedge.exe` (crash reporter dialogs) |
| USB-01 | Bot polls for wheelbase USB reconnect (hidapi 5s scan) and restarts FFB controller when device re-appears | `hidapi::HidApi::device_list()` exists in ffb_controller.rs; `DrivingDetector.is_hid_connected()` tracks state; new polling loop needed in failure_monitor.rs |
</phase_requirements>

---

## Summary

Phase 24 implements five bot patterns that eliminate the most common staff walk-to-pod interventions: game freeze, launch timeout, USB disconnect, FFB safety, and error dialog suppression. The foundation from Phase 23 is fully in place — `PodFailureReason` enum, 5 `AgentMessage` variants, and `is_pod_in_recovery()` are all committed and green. Phase 24 builds on top without touching protocol or common types.

The codebase already contains most of the fix infrastructure. `fix_kill_stale_game()`, `fix_kill_error_dialogs()`, `FfbController::zero_force()`, and `DrivingDetector.is_hid_connected()` all exist. What's missing is the detection layer (a `failure_monitor.rs` task that polls shared state) and new fix arms in `try_auto_fix()`. The FFB zero-before-kill ordering for crash is already wired in main.rs (line 978); it needs to be replicated inside the new fix handlers so DebugMemory replay paths also fire it.

The key constraint is that the existing `LaunchState` FSM in main.rs already handles a 180s launch timeout for AcStatus::Live (BILL-01 from an earlier context). CRASH-02 requires a separate, shorter timeout for the case where the game process itself never appears (Content Manager hang). These are distinct conditions that need distinct detection: `LaunchState::WaitingForLive` covers AC status; a new `game_process::find_game_pid()` poll in failure_monitor.rs covers process-level non-appearance.

**Primary recommendation:** Create `failure_monitor.rs` as a new async task in rc-agent. It reads shared atomics + detector state, constructs synthetic suggestion strings, and calls `try_auto_fix()`. Add 5 new fix arms + 3 new fix functions. Total scope: 2 files modified (ai_debugger.rs, main.rs), 1 file created (failure_monitor.rs). No new crates.

---

## Standard Stack

### Core — No New Dependencies

All capabilities needed for Phase 24 are in the existing rc-agent dependency set.

| Crate | Version | Phase 24 Use | How Used |
|-------|---------|--------------|----------|
| `winapi` | 0.3.9 | `IsHungAppWindow`, `EnumWindows` | Game freeze detection — winuser feature already present |
| `sysinfo` | 0.33 | CPU usage check (two-refresh) | Hang heuristic: low CPU + no UDP + hung window |
| `hidapi` | 2.x | `device_list()` USB reconnect poll | 5s scan for VID:0x1209 PID:0xFFB0 reappearance |
| `tokio` | 1 | `time::interval`, `task::spawn_blocking` | failure_monitor polling loop + hidapi blocking calls |
| `std::process::Command` | stdlib | `taskkill`, `powershell` | `hidden_cmd()` pattern already in ai_debugger.rs |

**Cargo.toml: no changes required.**

### What NOT to Add

| Avoid | Why |
|-------|-----|
| `windows-rs` / `windows-sys` | Already have `winapi 0.3` with winuser feature covering all needed APIs |
| WM_DEVICECHANGE USB hotplug | Requires a Windows message pump; `hidapi::device_list()` polling at 5s is correct and simpler |
| `sysinfo` upgrade to 0.38 | API changed in 0.34+; 47 passing tests against 0.33; hang detection identical on both |
| `notify` crate | Not needed for USB polling — hidapi enumerate is the right approach |

---

## Architecture Patterns

### Recommended File Structure

```
crates/rc-agent/src/
├── ai_debugger.rs       MODIFY: 5 new try_auto_fix arms + 3 new fix fns + PodStateSnapshot fields
├── self_monitor.rs      NO CHANGE — existing CLOSE_WAIT/WS-dead detection stays separate
├── failure_monitor.rs   NEW: polling loop for CRASH-01, CRASH-02, USB-01 detection
└── main.rs              MODIFY: spawn failure_monitor task, pass shared state
```

### Pattern 1: failure_monitor.rs — Single Detection Task

**What:** A new `pub fn spawn(...)` function (matching self_monitor.rs style) that starts a tokio task. Polls every 5s. Reads `HeartbeatStatus` atomics plus module-local state passed in. When it detects a condition, constructs a synthetic suggestion string with canonical keywords, then calls `try_auto_fix()`.

**Why separate from self_monitor.rs:** `self_monitor.rs` handles CLOSE_WAIT floods and WS-dead — agent liveness, not game session failures. Mixing bot game patterns into it would violate single-responsibility and make the 60s interval too coarse for USB/freeze detection (which needs 5s).

**Polling interval:** 5s — matches hidapi USB poll frequency from STACK.md. For game freeze the 30s UDP silence threshold means 5 polls before triggering; acceptable latency.

**Spawn signature (follows self_monitor.rs pattern):**

```rust
// Source: self_monitor.rs lines 30-35 (existing pattern to match)
pub fn spawn(
    status: Arc<HeartbeatStatus>,
    game_state_rx: watch::Receiver<FailureMonitorState>,
) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(30)).await; // startup grace period
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            check_all_conditions(&status, &game_state_rx).await;
        }
    });
}
```

**`FailureMonitorState` struct (new, passed from main.rs via watch channel):**

```rust
pub struct FailureMonitorState {
    pub game_pid: Option<u32>,
    pub last_udp_packet: Option<Instant>,
    pub hid_connected: bool,
    pub launch_started_at: Option<Instant>,
    pub billing_active: bool,  // mirrors HeartbeatStatus::billing_active
}
```

The `watch::Sender<FailureMonitorState>` sits in main.rs. It is updated wherever these fields change in the main event loop.

### Pattern 2: try_auto_fix() — New Arms (ai_debugger.rs)

**What:** Add 5 new `if lower.contains()` arms before the final `None` return. Each arm checks the billing guard inside the fix function (not at the call site), because DebugMemory replay can bypass call-site guards.

**Canonical synthetic string conventions (must match arm conditions exactly):**

| Failure Class | Canonical Synthetic String | try_auto_fix arm trigger |
|--------------|---------------------------|--------------------------|
| CRASH-01: game frozen | `"Game frozen — IsHungAppWindow true + UDP silent 30s relaunch acs.exe"` | `"game frozen"` + `"relaunch"` |
| CRASH-02: launch timeout | `"launch timeout — Content Manager hang kill cm process"` | `"launch timeout"` + `"content manager"` |
| CRASH-03: FFB zero | Injected into CRASH-01 and CRASH-02 fix functions directly, not via keyword | N/A — safety ordering inside fix fn |
| UI-01: error dialogs | `"werfault crash dialog error dialog suppress before kill"` | existing `"werfault"` arm — extend to suppress more |
| USB-01: wheelbase reconnect | `"Wheelbase usb reset required — HID reconnected VID:0x1209 PID:0xFFB0"` | `"wheelbase"` + `"usb reset"` |

**New arms to add in order (specific before general):**

```rust
// Source: ai_debugger.rs try_auto_fix() — ADD before final None

// CRASH-01: Game frozen (USB hang + UDP silence)
if lower.contains("game frozen") && lower.contains("relaunch") {
    return Some(fix_frozen_game(snapshot));
}

// CRASH-02: Launch timeout / Content Manager hang
if lower.contains("launch timeout") || (lower.contains("content manager") && lower.contains("kill cm")) {
    return Some(fix_launch_timeout(snapshot));
}

// UI-01: Extended error dialog suppression (keep before game-kill arms)
// NOTE: existing arm checks "werfault" || "error dialog" || "crash dialog"
// Extend the existing fix_kill_error_dialogs() body — no new arm needed

// USB-01: Wheelbase USB reconnect detected — restart FFB
if lower.contains("wheelbase") && lower.contains("usb reset") {
    return Some(fix_usb_reconnect(snapshot));
}
```

### Pattern 3: New Fix Functions (ai_debugger.rs)

**CRASH-01: `fix_frozen_game(snapshot: &PodStateSnapshot) -> AutoFixResult`**

Ordering is safety-critical:
1. Gate: `if !snapshot.billing_active { return early_no_billing_result() }` — DO NOT SKIP, DebugMemory can replay this
2. Zero FFB first: call `FfbController::new(0x1209, 0xFFB0).zero_force()` via hidden synchronous call
3. Kill error dialogs: `taskkill /IM WerFault.exe /F` (existing pattern)
4. Kill game processes: `taskkill /IM acs.exe /F` etc. (existing `fix_kill_stale_game()` body)
5. Return `AutoFixResult { fix_type: "fix_frozen_game", ... }`

Note: `FfbController` uses `hidapi` which is synchronous. In try_auto_fix() (called from `spawn_blocking` in main.rs), this is fine — no async needed.

**CRASH-02: `fix_launch_timeout(snapshot: &PodStateSnapshot) -> AutoFixResult`**

1. Kill Content Manager by name: `taskkill /IM "Content Manager.exe" /F`
2. Kill by alternate name: `taskkill /IM acmanager.exe /F` (fallback — CM may report either name)
3. Kill acs.exe in case it spawned then hung: `taskkill /IM acs.exe /F`
4. Return `AutoFixResult { fix_type: "fix_launch_timeout", ... }`

No billing gate here — launch timeout can happen before billing fully activates. The detection in failure_monitor.rs gates on `launch_started_at.is_some()`.

**USB-01: `fix_usb_reconnect(snapshot: &PodStateSnapshot) -> AutoFixResult`**

1. Zero FFB as safety reset: `FfbController::new(0x1209, 0xFFB0).zero_force()`
2. Log reconnect event
3. Return `AutoFixResult { fix_type: "fix_usb_reconnect", success: true, ... }`

The fix does NOT re-open the HID device for input — that's driving_detector.rs's job on its next 100ms poll cycle. This fix just ensures the wheelbase starts clean (no stale FFB state) when driving_detector.rs picks it back up.

### Pattern 4: PodStateSnapshot Expansion (ai_debugger.rs)

Add new fields with `#[serde(default)]` — no migration cost since snapshot is ephemeral (never persisted to disk in this form):

```rust
// ADD to PodStateSnapshot — source: ai_debugger.rs lines 34-44
#[serde(default)]
pub last_udp_secs_ago: Option<u64>,          // seconds since last UDP frame (None = never received)
#[serde(default)]
pub game_launch_elapsed_secs: Option<u64>,   // seconds since LaunchGame received (None = not launching)
#[serde(default)]
pub hid_last_error: bool,                    // true if driving_detector last saw HidDisconnected
```

These 3 fields are the minimum needed by the new fix functions. Existing tests construct `PodStateSnapshot` with named fields — adding `#[serde(default)]` fields requires updating all existing test struct constructions to add the new fields (or using `..Default::default()` if Default is derived).

**CRITICAL for test compatibility:** `PodStateSnapshot` derives `Debug, Clone, Serialize` but NOT `Default`. Either add `#[derive(Default)]` (requires `Option<DrivingState>` to already be `Option`-wrapped, which it is), OR update all 8 test snapshot constructions to include the new fields explicitly.

Recommendation: add `#[derive(Default)]` to `PodStateSnapshot` and use `..Default::default()` in tests. Requires confirming all field types implement Default (they do: `bool` defaults false, `Option<T>` defaults None, `u64` defaults 0).

### Pattern 5: IsHungAppWindow Detection

**What:** For CRASH-01, the hang heuristic requires all 4 conditions simultaneously:
1. `HeartbeatStatus::game_running` = true
2. `last_udp_packet.elapsed() > 30s` (from FailureMonitorState)
3. CPU usage < 2% (sysinfo two-refresh pattern)
4. `IsHungAppWindow(hwnd)` = true

Conditions 1-3 are cheap. Condition 4 requires a Windows API call via `EnumWindows`. Only evaluate condition 4 if 1-3 are already true — avoids the `EnumWindows` overhead on every 5s tick.

**Implementation sketch for failure_monitor.rs:**

```rust
// Source: STACK.md §1 — IsHungAppWindow pattern
#[cfg(windows)]
fn is_game_window_hung(game_pid: u32) -> bool {
    use std::sync::atomic::{AtomicBool, Ordering};
    use winapi::um::winuser::{EnumWindows, IsHungAppWindow, GetWindowThreadProcessId};

    static FOUND: AtomicBool = AtomicBool::new(false);
    static FOUND_PID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

    // EnumWindows callback — unsafe extern "system" fn
    unsafe extern "system" fn callback(hwnd: winapi::shared::windef::HWND, lparam: winapi::shared::minwindef::LPARAM) -> winapi::shared::minwindef::BOOL {
        let target_pid = lparam as u32;
        let mut window_pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut window_pid as *mut u32);
        if window_pid == target_pid {
            if IsHungAppWindow(hwnd) != 0 {
                FOUND.store(true, Ordering::Relaxed);
                FOUND_PID.store(window_pid, Ordering::Relaxed);
            }
            return 0; // stop enumeration
        }
        1 // continue
    }

    FOUND.store(false, Ordering::Relaxed);
    unsafe { EnumWindows(Some(callback), game_pid as _); }
    FOUND.load(Ordering::Relaxed)
}
```

Note: The `static AtomicBool` approach is a simplification — in practice the closure needs thread-local or a callback struct with the target PID. The clean approach is to use a thread-local to pass state to the callback. Verify on Windows — this is the standard pattern for EnumWindows in Rust.

**sysinfo two-refresh requirement (HIGH confidence — documented sysinfo behavior):**

```rust
// Must take two samples 500ms apart before reading cpu_usage()
let mut sys = System::new();
sys.refresh_processes(ProcessesToUpdate::All, true);
tokio::time::sleep(Duration::from_millis(500)).await;
sys.refresh_processes(ProcessesToUpdate::All, true);
let cpu = sys.process(Pid::from_u32(game_pid))
    .map(|p| p.cpu_usage())
    .unwrap_or(100.0); // default 100% = "not hung" to avoid false positive
```

This must run in a `tokio::task::spawn_blocking` call because `sysinfo::System::refresh_processes()` is synchronous and blocks for 500ms. failure_monitor.rs is already an async task — use `spawn_blocking` for the two-refresh sequence.

### Pattern 6: USB Reconnect Polling (failure_monitor.rs)

**What:** Track previous HID connection state. When state transitions from disconnected → connected, fire the synthetic suggestion string.

```rust
// Poll in failure_monitor every 5s
let api = hidapi::HidApi::new().ok();
let wheelbase_present = api.as_ref().map(|a| {
    a.device_list().any(|d| d.vendor_id() == 0x1209 && d.product_id() == 0xFFB0)
}).unwrap_or(false);

// Detect reconnect: was false, now true
if !prev_hid_connected && wheelbase_present && billing_active {
    let synthetic = "Wheelbase usb reset required — HID reconnected VID:0x1209 PID:0xFFB0";
    // Build snapshot, call try_auto_fix(synthetic, &snapshot)
}
prev_hid_connected = wheelbase_present;
```

`hidapi::HidApi::new()` is blocking (USB enumeration). Run via `spawn_blocking`. The existing `ffb_controller.rs:open_vendor_interface()` already uses this pattern.

Also send `AgentMessage::HardwareFailure` to the server when disconnect is detected (billing_active = true), using the `WheelbaseDisconnected` variant of `PodFailureReason`.

### Anti-Patterns to Avoid

- **Calling fix functions directly from failure_monitor.rs:** Always go through `try_auto_fix(synthetic_str, &snapshot)`. DebugMemory pattern learning only fires through this path. Bypassing it means the bot gets dumber over time on recurring failures.

- **Skipping FFB zero in new fix handlers:** `fix_frozen_game()` and any future game-kill handlers MUST call `FfbController::zero_force()` before any `taskkill` call. The ordering is a safety requirement, not a preference. The 8Nm torque from a Conspit Ares with no game feedback is a physical hazard.

- **Using billing_active at the call site only:** The DebugMemory `instant_fix()` path calls `try_auto_fix()` directly, bypassing any call-site guard in failure_monitor.rs. The billing check must live inside each fix function.

- **EnumWindows on every 5s tick:** Expensive if called constantly. Gate: only call `is_game_window_hung()` when conditions 1-3 (game running + UDP silent 30s + low CPU) are already true.

- **Merging CRASH-01 and CRASH-02 detection:** They are distinct. CRASH-01 is a running game that froze. CRASH-02 is a game that never started after a launch command. Different conditions, different fix handlers, different detection paths in failure_monitor.rs.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| USB hang detection | WM_DEVICECHANGE message pump | `hidapi::device_list()` polling every 5s | WM_DEVICECHANGE requires a Windows message pump (GUI thread); polling with hidapi is already the pattern used in ffb_controller.rs |
| Process CPU check | `/proc/` parsing or WMI | `sysinfo 0.33` | Already in Cargo.toml, already tested; WMI has 100ms+ COM startup cost |
| Game window hung check | CPU + memory heuristics alone | `IsHungAppWindow` winapi | CPU can be low during loading screens; IsHungAppWindow is the authoritative hung-window API |
| Error dialog suppression | UI Automation, WMI | `taskkill /IM WerFault.exe /F` | Already implemented and working; extend the existing function, don't replace it |

---

## Common Pitfalls

### Pitfall 1: LaunchState vs Failure Monitor Duplication

**What goes wrong:** main.rs already has `LaunchState::WaitingForLive { launched_at }` with a 180s timeout. If failure_monitor.rs also implements a launch timeout check, both fire and produce duplicate fixes.

**Root cause:** Two systems monitoring the same condition independently.

**How to avoid:** The existing `LaunchState` covers the case where the game launches but never reaches `AcStatus::Live` (the AC-specific status). CRASH-02 is a different condition: Content Manager hangs and `acs.exe` never starts at all. Detection: `launch_started_at.elapsed() > 90s AND find_game_pid(sim_type).is_none()`. This is process-level absence, not AcStatus. No duplication if scoped correctly.

**Warning sign:** Both paths triggering on the same launch attempt — add a `launch_timeout_fired: bool` flag in FailureMonitorState to suppress duplicate firing within the same launch attempt.

### Pitfall 2: PodStateSnapshot Construction — Missing New Fields in Tests

**What goes wrong:** Adding new fields to `PodStateSnapshot` breaks all 8 existing test constructions in ai_debugger.rs (lines 531-651) with compile error "missing field".

**Root cause:** Struct literal construction in Rust requires all fields.

**How to avoid:** Add `#[derive(Default)]` to `PodStateSnapshot`. Update test constructions to use struct update syntax: `PodStateSnapshot { pod_id: "...".into(), pod_number: 1, ..Default::default() }`. This is backward-compatible and reduces test boilerplate for new fields.

**Warning sign:** `cargo test -p rc-agent-crate` fails to compile after adding new PodStateSnapshot fields.

### Pitfall 3: FfbController in spawn_blocking

**What goes wrong:** `FfbController::zero_force()` uses `hidapi` which is synchronous. Calling it directly in an async context (without `spawn_blocking`) blocks the tokio event loop.

**Root cause:** try_auto_fix() is already called via `spawn_blocking` in main.rs (line 1016). The fix functions are correctly in a blocking context when invoked from main.rs's ai_result handler. But if failure_monitor.rs calls try_auto_fix() from a tokio async task without wrapping in spawn_blocking, the hidapi call blocks the executor.

**How to avoid:** In failure_monitor.rs, wrap `try_auto_fix()` calls in `tokio::task::spawn_blocking`. Consistent with the existing pattern in main.rs.

**Warning sign:** Tokio warning "blocking operation may block the thread pool" in logs.

### Pitfall 4: is_pod_in_recovery() is Server-Side Only

**What goes wrong:** `is_pod_in_recovery()` lives in `racecontrol/src/pod_healer.rs`, not rc-common. rc-agent cannot call it directly.

**Root cause:** Phase 23 decision: `WatchdogState` is server-local; `is_pod_in_recovery` was never moved to rc-common.

**How to avoid:** failure_monitor.rs CANNOT check `is_pod_in_recovery()`. Instead, use a local "recovery in progress" flag in FailureMonitorState that main.rs sets when it receives a server-initiated recovery command (e.g., `CoreToAgentMessage::StopSession` or `StopGame`). While a server-commanded recovery is in progress, failure_monitor suppresses autonomous fixes.

**Warning sign:** failure_monitor.rs trying to import anything from `racecontrol` crate.

### Pitfall 5: IsHungAppWindow with Thread-Locals and EnumWindows

**What goes wrong:** `EnumWindows` callback is `extern "system"` and cannot capture local variables. Passing state to the callback requires a global or thread-local.

**Root cause:** The Windows callback ABI prohibits closures that capture state.

**How to avoid:** Use `thread_local!` for a `Cell<bool>` + `Cell<u32>` that the callback writes to. The outer function reads them after `EnumWindows` returns. Since failure_monitor runs in a single `spawn_blocking` thread per call, thread-locals are safe. See the pattern sketch in Architecture Patterns §5.

---

## Code Examples

### Verified: fix_kill_stale_game() — existing reference implementation

```rust
// Source: ai_debugger.rs lines 386-423 — existing working pattern
fn fix_kill_stale_game() -> AutoFixResult {
    let game_exes = ["acs.exe", "AssettoCorsa.exe", "F1_25.exe", ...];
    for exe in &game_exes {
        if PROTECTED_PROCESSES.iter().any(|p| p.eq_ignore_ascii_case(exe)) { continue; }
        let _ = hidden_cmd("taskkill").args(["/IM", exe, "/F"]).output();
    }
    AutoFixResult { fix_type: "kill_stale_game".to_string(), detail: ..., success: true }
}
```

### Verified: FfbController::zero_force() — existing safety command

```rust
// Source: ffb_controller.rs lines 53-78
pub fn zero_force(&self) -> Result<bool, String> {
    let device = match self.open_vendor_interface() {
        Some(dev) => dev,
        None => return Ok(false), // device not found — skip silently
    };
    self.send_vendor_cmd(&device, CMD_ESTOP, 1)?;    // emergency stop
    let _ = self.send_vendor_cmd(&device, CMD_FFB_ACTIVE, 0); // belt+suspenders
    Ok(true)
}
```

### Verified: LaunchState tracking — existing main.rs pattern

```rust
// Source: main.rs lines 181-188, 819-861
enum LaunchState {
    Idle,
    WaitingForLive { launched_at: std::time::Instant, attempt: u8 },
    Live,
}
// Detection: launched_at.elapsed() > Duration::from_secs(180) — existing 3min gate
// CRASH-02 needs: launched_at.elapsed() > 90s AND find_game_pid() is None
```

### Verified: hidden_cmd pattern — used throughout ai_debugger.rs

```rust
// Source: ai_debugger.rs lines 334-342
fn hidden_cmd(program: &str) -> std::process::Command {
    let mut cmd = std::process::Command::new(program);
    #[cfg(windows)] { use std::os::windows::process::CommandExt; cmd.creation_flags(0x08000000); }
    cmd
}
// Content Manager kill: hidden_cmd("taskkill").args(["/IM", "Content Manager.exe", "/F"]).output()
```

### Verified: HardwareFailure message sending pattern (ws/mod.rs stub shows the shape)

```rust
// Source: protocol.rs lines 111-116 (AgentMessage::HardwareFailure variant)
AgentMessage::HardwareFailure {
    pod_id: pod_id.clone(),
    reason: PodFailureReason::WheelbaseDisconnected,
    detail: "VID:0x1209 PID:0xFFB0 USB disconnect detected".to_string(),
}
```

---

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|-----------------|--------|
| Manual staff walk to pod on game freeze | CRASH-01 bot detects in 30s, kills + staff notified via WS | Staff no longer walk to pod for freeze |
| Content Manager hang requires manual kill | CRASH-02 bot kills CM + acs.exe on 90s timeout | Launch failures self-resolve |
| FFB safety only wired on billing crash (main.rs line 978) | CRASH-03: all new fix handlers also zero FFB | Safety ordering is universal, not just one code path |
| WerFault dismissed only via AI suggestion path | UI-01: deterministic pre-kill suppression | Customer never sees crash dialogs during bot recovery |
| USB reconnect requires staff physical re-seat | USB-01: bot detects reconnect, resets FFB, resumes input | Single USB cable wiggle self-heals |

**Phase 23 shipped (already done):**
- `PodFailureReason` enum with all 21 variants — `WheelbaseDisconnected`, `GameFrozen`, `ContentManagerHang`, `LaunchTimeout`, `FfbFault` all available
- 5 new `AgentMessage` variants — `HardwareFailure` ready to send for USB-01
- `is_pod_in_recovery()` in pod_healer.rs — server-side guard against concurrent fixes
- ws/mod.rs stub arms for all 5 new variants (log-only stubs)

---

## Open Questions

1. **Content Manager process name on pods**
   - What we know: STACK.md says "Content Manager.exe" (with space) is the Task Manager name; `acmanager.exe` is an alternate
   - What's unclear: Which name appears in `tasklist` on the actual pods — depends on install method
   - Recommendation: fix_launch_timeout() should kill BOTH names with separate taskkill calls. Belt-and-suspenders, both are safe to attempt.

2. **EnumWindows thread-local safety in spawn_blocking**
   - What we know: `thread_local!` is safe for spawn_blocking since Tokio uses a thread pool; each spawn_blocking call gets a thread
   - What's unclear: Whether the same thread is reused between is_game_window_hung() calls, which could cause stale thread-local state
   - Recommendation: Always reset the thread-local FOUND flag to false at the top of is_game_window_hung() before calling EnumWindows. Include a test that verifies the flag resets correctly.

3. **launch_started_at access from failure_monitor.rs**
   - What we know: `LaunchState` is local to main.rs's event loop, not shared via HeartbeatStatus
   - What's unclear: The cleanest way to expose it — HeartbeatStatus atomic or FailureMonitorState watch channel
   - Recommendation: Add `launch_started_at: Option<Instant>` to FailureMonitorState (watch channel pattern). main.rs updates it when LaunchState transitions to WaitingForLive. This keeps FailureMonitorState as the single struct for failure_monitor inputs.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` (no external test runner) |
| Config file | `Cargo.toml` (workspace) |
| Quick run command | `cargo test -p rc-agent-crate 2>&1` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CRASH-01 | freeze detection fires when UDP silent + hung | unit | `cargo test -p rc-agent-crate -- freeze` | Wave 0 |
| CRASH-01 | fix_frozen_game() calls FFB zero before kill | unit | `cargo test -p rc-agent-crate -- fix_frozen_game` | Wave 0 |
| CRASH-01 | billing gate blocks fix when billing inactive | unit | `cargo test -p rc-agent-crate -- fix_frozen_game_no_billing` | Wave 0 |
| CRASH-02 | fix_launch_timeout() kills CM by both names | unit | `cargo test -p rc-agent-crate -- fix_launch_timeout` | Wave 0 |
| CRASH-02 | try_auto_fix dispatches on "launch timeout" keyword | unit | `cargo test -p rc-agent-crate -- auto_fix_launch_timeout` | Wave 0 |
| CRASH-03 | FFB zero fires before game kill in fix_frozen_game | unit | `cargo test -p rc-agent-crate -- ffb_zero_before_kill_ordering` | Wave 0 |
| UI-01 | fix_kill_error_dialogs extended suppression | unit | `cargo test -p rc-agent-crate -- kill_error_dialogs_extended` | Wave 0 |
| USB-01 | try_auto_fix dispatches on wheelbase+usb reset | unit | `cargo test -p rc-agent-crate -- auto_fix_usb_reconnect` | Wave 0 |
| USB-01 | fix_usb_reconnect zeros FFB on reconnect | unit | `cargo test -p rc-agent-crate -- fix_usb_reconnect_ffb_zero` | Wave 0 |
| USB-01 | HardwareFailure message sent on disconnect | unit | `cargo test -p rc-agent-crate -- hardware_failure_disconnect_msg` | Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test -p rc-agent-crate`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `crates/rc-agent/src/failure_monitor.rs` — new file, all detection unit tests
- [ ] `crates/rc-agent/src/ai_debugger.rs` test additions — 5 new fix arm tests + billing gate tests
- [ ] `PodStateSnapshot` Default derive — required before test struct update syntax works

*(No new test framework install needed — `#[test]` is built-in)*

---

## Sources

### Primary (HIGH confidence)

- Direct inspection: `crates/rc-agent/src/ai_debugger.rs` — 767 lines, full file (2026-03-16)
- Direct inspection: `crates/rc-agent/src/self_monitor.rs` — 219 lines, full file (2026-03-16)
- Direct inspection: `crates/rc-agent/src/game_process.rs` — 447 lines, full file (2026-03-16)
- Direct inspection: `crates/rc-agent/src/driving_detector.rs` — 282 lines, full file (2026-03-16)
- Direct inspection: `crates/rc-agent/src/ffb_controller.rs` — 312 lines, full file (2026-03-16)
- Direct inspection: `crates/rc-agent/src/udp_heartbeat.rs` — 163 lines, full file (2026-03-16)
- Direct inspection: `crates/rc-agent/src/main.rs` — lines 1-1050 inspected (2026-03-16)
- Direct inspection: `crates/rc-common/src/types.rs` — PodFailureReason enum confirmed (2026-03-16)
- Direct inspection: `crates/rc-common/src/protocol.rs` — 5 new AgentMessage variants confirmed (2026-03-16)
- Direct inspection: `crates/racecontrol/src/pod_healer.rs` — is_pod_in_recovery() at line 775 (2026-03-16)
- Direct inspection: `crates/racecontrol/src/ws/mod.rs` — stub arms confirmed lines 508-522 (2026-03-16)
- Direct inspection: `.planning/research/ARCHITECTURE.md` — v5.0 architecture (2026-03-16)
- Direct inspection: `.planning/research/FEATURES.md` — feature patterns (2026-03-16)
- Direct inspection: `.planning/research/STACK.md` — no new crates needed (2026-03-16)
- Direct inspection: `.planning/REQUIREMENTS.md` — Phase 24 requirements (2026-03-16)
- Direct inspection: `.planning/STATE.md` — Phase 23 complete, decisions locked (2026-03-16)

### Secondary (MEDIUM confidence)

- IsHungAppWindow in winapi 0.3 winuser feature — confirmed present via existing Cargo.toml features already pulling winuser
- sysinfo two-refresh pattern — documented in sysinfo crate, referenced in STACK.md with citation

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — verified directly from Cargo.toml; no new crates needed
- Architecture: HIGH — verified from reading actual source files; patterns match existing code
- Pitfalls: HIGH — PodStateSnapshot struct literal issue is a real compile error; LaunchState duplication is a real design conflict identified from reading both main.rs and requirements; is_pod_in_recovery server-side-only confirmed from pod_healer.rs location
- Fix handler ordering: HIGH — FFB zero before kill already wired at main.rs line 978; pattern must be replicated in new fix handlers

**Research date:** 2026-03-16
**Valid until:** 2026-04-16 (stable Rust/Windows API domain — 30 days)
