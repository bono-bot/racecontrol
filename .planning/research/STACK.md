# Stack Research: Pre-Flight Session Checks (v11.1)

**Domain:** rc-agent Windows service — pre-session health gate
**Researched:** 2026-03-21
**Confidence:** HIGH

---

## Existing Stack (Do Not Change)

Everything below is already compiled into rc-agent. Do not add duplicates.

| Technology | Version | Already Used For |
|------------|---------|-----------------|
| `tokio` (full) | 1 | Async runtime, `spawn_blocking`, `select!`, intervals, timers |
| `winapi` | 0.3 | HID, process, winuser, wingdi — features already include `winuser` + `wingdi` |
| `sysinfo` | 0.33 | Disk, memory, process scan (used in `self_test.rs`, `kiosk.rs`, `game_process.rs`) |
| `hidapi` | 2 | Wheelbase HID detection (`ffb_controller.rs`) |
| `reqwest` | 0.12 | HTTP client for orphan session end, Ollama queries |
| `tokio-tungstenite` | 0.26 | WebSocket client (WS connectivity already tracked in `AppState`) |
| `anyhow` + `thiserror` | 1 / 2 | Error handling throughout |
| `tracing` | 0.1 | Structured logging |
| `serde` + `serde_json` | 1 | Message serialization for WS notification |

**Key insight:** Zero new Rust crates are needed. Every capability required for pre-flight checks is already in the dependency graph.

---

## New Capabilities Needed and How to Implement Them

### 1. Display Validation — Window Position Check

**Requirement:** Verify the lock screen Edge window is visible and positioned at (0,0) on the primary monitor, not hidden off-screen or on a wrong monitor.

**Approach: `winapi::um::winuser::GetWindowRect` + `FindWindowW`**

Both `FindWindowW` and `GetWindowRect` are in `winapi::um::winuser`, already enabled via the `"winuser"` feature in `rc-agent/Cargo.toml`. No feature additions needed.

`FindWindowW` is already called in `ac_launcher.rs` (line 1352), `ffb_controller.rs` (line 383), and `kiosk.rs` (line 913). `GetWindowRect` is in the same `winuser` module — it takes an `HWND` and returns a `RECT` with `left`, `top`, `right`, `bottom` fields.

`get_virtual_screen_bounds()` already exists in `lock_screen.rs` and uses `GetSystemMetrics` (SM_XVIRTUALSCREEN etc.) to get the primary monitor origin. Reuse it directly.

**What to check:**
- `FindWindowW(NULL, "Racing Point\0")` returns non-null HWND — lock screen Edge is running
- `GetWindowRect(hwnd, &mut rect)` succeeds — window handle is valid
- `rect.left` and `rect.top` match the virtual screen origin from `get_virtual_screen_bounds()` (within ±50px tolerance for resize border artifacts per Microsoft DwmGetWindowAttribute note)
- `rect.right - rect.left` >= primary monitor width (window is maximized, not minimized/partial)

**Call site:** Must be in `tokio::task::spawn_blocking` — Win32 calls are blocking and the same pattern is used throughout the codebase (see `failure_monitor.rs` `is_game_process_hung()`).

**Confidence:** HIGH — `winuser` feature confirmed in `Cargo.toml` line 62. `FindWindowW` + `GetWindowRect` confirmed available in `winapi 0.3 winuser` module.

**What NOT to do:**
- Do NOT take a screenshot and do pixel comparison — GDI `GetDC`/`GetPixel` is overkill. Window position check is sufficient and faster.
- Do NOT add the `windows` crate (Microsoft's newer binding) — you already have `winapi 0.3` and mixing them causes duplicate symbol issues.
- Do NOT add `win-screenshot` or any external screenshot crate — not needed.

---

### 2. ConspitLink Hardware Check — Process + HID

**Requirement:** Verify ConspitLink is running and the wheelbase HID device is present.

**Approach: `sysinfo` (processes) + existing `hidapi`**

ConspitLink process check: `sysinfo::System::new()` + `refresh_processes()` scan for `ConspitLink.exe` by name. This is the exact pattern used in `kiosk.rs` (`enforce_process_whitelist_blocking`) and `game_process.rs` (`find_game_pid`). Copy that pattern.

Wheelbase HID check: `AppState.hid_detected` is already set at startup in `main.rs`. For pre-flight, re-probe `FfbController::probe_hid()` (or equivalent in `ffb_controller.rs`) to get current state rather than using stale startup state. `hidapi` is already a dep.

**Auto-fix:** If ConspitLink is not running, shell-launch it via `std::process::Command` (same approach as `ac_launcher.rs` game launch). The ConspitLink exe path is in `AgentConfig`.

**Call site:** `spawn_blocking` — both `sysinfo::refresh_processes()` and `hidapi` enumeration are blocking (same warning comment in `kiosk.rs` line 619-621).

**Confidence:** HIGH — direct reuse of established patterns in the codebase.

---

### 3. Network Check — WebSocket + UDP Heartbeat

**Requirement:** Confirm WS is connected and UDP heartbeat is alive before the session starts.

**Approach: Read from existing `AppState` atomics**

- WS connectivity: `AppState` already tracks the connection. In `ws_handler.rs`, the pre-flight check runs inside the established WS connection handler for `BillingStarted` — so if we receive `BillingStarted`, WS is by definition connected. No additional check needed.
- UDP heartbeat: `AppState.heartbeat_status` is an `Arc<HeartbeatStatus>` with `billing_active` atomic already. Check `udp_heartbeat::HeartbeatStatus` for last-seen timestamp or alive flag. Look at what fields exist on `HeartbeatStatus` to read UDP liveness. This is a lock-free atomic read — no `spawn_blocking` needed.

**Confidence:** HIGH — state already maintained, just needs to be read at BillingStarted time.

---

### 4. Game Check — Orphaned Process Detection

**Requirement:** Kill any orphaned AC/F1 processes before the new session starts.

**Approach: `game_process::find_orphaned_game_processes()` (or equivalent)**

The pattern is already in `game_process.rs`: `sysinfo` scan for known game process names. Wrap in `spawn_blocking`. If found, kill via `taskkill /F /IM <name>.exe` (same approach as `ac_launcher.rs` cleanup). This is the existing orphan detection path — pre-flight just triggers it proactively.

**Confidence:** HIGH — direct reuse of `game_process.rs` + `kiosk.rs` patterns.

---

### 5. Billing Check — Stuck Session Detection

**Requirement:** Verify no billing session is already active when `BillingStarted` fires.

**Approach: Read `FailureMonitorState.billing_active`**

`AppState.failure_monitor_tx` is a `watch::Sender<FailureMonitorState>`. Subscribe a receiver and read `billing_active`. If `true` when `BillingStarted` arrives, the previous session is stuck. Auto-fix: trigger orphan-end HTTP call (same path as `billing_guard.rs` `attempt_orphan_end()`).

**Confidence:** HIGH — reuses `billing_guard.rs` pattern directly.

---

### 6. System Checks — Disk + Memory

**Requirement:** Disk > 1GB on C:, free memory > 2GB.

**Approach: `sysinfo::Disks` + `sysinfo::System::refresh_memory()`**

These are the exact probes already in `self_test.rs` (`probe_disk()` line 342, `probe_memory()` line 385). Copy them verbatim into the pre-flight module. The thresholds match the milestone spec.

**Confidence:** HIGH — code already exists in `self_test.rs`, copy-paste with attribution.

---

### 7. Pre-Flight Gate Pattern

**Requirement:** Run all checks concurrently, block on results, then either proceed or show "Maintenance Required" screen.

**Approach: `tokio::join!` over async check fns, each wrapping `spawn_blocking` for Win32/sysinfo calls**

```rust
// Inside ws_handler.rs BillingStarted arm, before existing session setup:
let (display_ok, hw_ok, game_ok, billing_ok, system_ok) = tokio::join!(
    preflight::check_display(&state),
    preflight::check_hardware(&state),
    preflight::check_game_orphans(),
    preflight::check_billing(&state),
    preflight::check_system(),
);
```

Each `check_*` function returns a `PreflightResult { ok: bool, detail: String, auto_fixed: bool }`. Aggregate: if any `ok == false && !auto_fixed`, show `MaintenanceRequired` lock screen state and send WS alert. If all pass (or auto-fixed), continue into existing `BillingStarted` handling.

This is a pure Rust async pattern using `tokio::join!` — no new crate needed.

**Confidence:** HIGH — `tokio::join!` is idiomatic for concurrent independent async tasks. Pattern matches existing `self_test.rs` probe runner (which runs probes concurrently with `tokio::time::timeout`).

---

### 8. "Maintenance Required" Lock Screen State

**Requirement:** New `LockScreenState` variant that blocks the pod and shows a staff-callout message.

**Approach: Add variant to existing `LockScreenState` enum in `lock_screen.rs`**

```rust
/// Pod blocked — pre-flight check failed and auto-fix could not resolve.
/// Only cleared by staff action (PIN or server command).
MaintenanceRequired {
    reason: String,        // human-readable failure description
    failed_checks: Vec<String>,  // list of check names that failed
},
```

Add to the existing enum in `lock_screen.rs`. The HTML template served by the lock screen HTTP server gets a new template branch. The existing `LockScreenManager::show_*` pattern handles state transitions — add `show_maintenance(reason, failed_checks)`.

**Confidence:** HIGH — trivial enum extension, same pattern as existing `ConfigError { message }` and `Lockdown { message }` variants already in the enum.

---

### 9. Staff Notification

**Requirement:** Alert staff via WS + kiosk dashboard badge when pre-flight fails and auto-fix doesn't resolve.

**Approach: Existing `AgentMessage` WS protocol**

Send an `AgentMessage::PodFailureAlert` (or define a new `PreflightFailed` variant in `rc-common/src/protocol.rs`) over the established WS connection. The server-side kiosk dashboard already processes `AgentMessage` variants to update the fleet view. A new message variant is the correct extension point.

For the kiosk dashboard badge: racecontrol server receives the `PreflightFailed` message and marks the pod status accordingly in the fleet health state. Pods in `MaintenanceRequired` state show a distinct badge in the kiosk fleet view.

**Confidence:** HIGH — established WS message protocol, existing `AgentMessage` enum in rc-common. Pattern matches `BillingAnomaly`, `PodFailure` message types.

---

## New Module: `preflight.rs`

Create `crates/rc-agent/src/preflight.rs`. This module:
- Owns the `PreflightResult` struct and `PreflightCheckName` enum
- Contains all `check_*` async functions
- Exposes a single `run_all(state: &AppState) -> Vec<PreflightResult>` function
- Called from `ws_handler.rs` in the `BillingStarted` arm

No new files anywhere else. No changes to `Cargo.toml`. No changes to `rc-common` unless a new `AgentMessage` variant is added (which it should be for server-side tracking).

---

## What NOT to Add

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `windows` crate (Microsoft) | Conflicts with existing `winapi 0.3`. Two different Win32 bindings in one binary cause symbol conflicts. | `winapi 0.3` — already present, already has `winuser` + `wingdi` features |
| `win-screenshot` / `screenshots` crate | GDI screenshot for display validation is >10x more code and slower than `GetWindowRect` position check | `GetWindowRect` via existing `winapi::um::winuser` |
| `image` crate | No pixel analysis needed — window position check is sufficient | N/A |
| Any new HTTP client | Already have `reqwest 0.12` for staff notification fallback | `reqwest` (existing) |
| `async-std` | tokio is already the runtime | `tokio` (existing) |
| Separate `preflight` crate | Overkill for a single module | `preflight.rs` inside `rc-agent/src/` |
| Polling pre-flight every N seconds | Pre-flight runs once on `BillingStarted`, not continuously | Event-driven: trigger on `BillingStarted` |
| External health-check framework | No value over direct tokio::join! | `tokio::join!` (built-in) |

---

## Cargo.toml Changes Required

**rc-agent/Cargo.toml:** None. Zero new dependencies.

**winapi features (already present, no changes):**
```toml
winapi = { version = "0.3", features = [
    "processthreadsapi", "winnt", "handleapi",
    "winuser",   # FindWindowW, GetWindowRect, GetSystemMetrics — already here
    "memoryapi", "basetsd", "synchapi", "errhandlingapi",
    "winerror",
    "wingdi",    # GetDC, GetPixel if needed — already here
    "libloaderapi"
]}
```

**rc-common/Cargo.toml:** Add `PreflightFailed` variant to `AgentMessage` enum (source change, not dep change).

---

## Integration Points

| Where | What Changes |
|-------|-------------|
| `ws_handler.rs` `BillingStarted` arm | Insert `preflight::run_all(&state).await` call before existing session setup code. If any check fails unrecoverably, call `state.lock_screen.show_maintenance(...)` and return early from the arm. |
| `lock_screen.rs` `LockScreenState` enum | Add `MaintenanceRequired { reason, failed_checks }` variant. Add HTML template branch. Add `LockScreenManager::show_maintenance()` method. |
| `rc-common/src/protocol.rs` `AgentMessage` | Add `PreflightFailed { pod_id, failed_checks, timestamp }` variant. |
| `app_state.rs` | No changes — all needed state is already in `AppState`. |
| New file: `rc-agent/src/preflight.rs` | All check logic lives here. |

---

## Version Compatibility

All capabilities are built on existing, already-compiled dependencies. No version compatibility concerns.

| Capability | Dep | Version | Status |
|------------|-----|---------|--------|
| `GetWindowRect` | `winapi::um::winuser` | 0.3 | Already in Cargo.toml |
| `FindWindowW` | `winapi::um::winuser` | 0.3 | Already used in ac_launcher.rs |
| Process scan | `sysinfo` | 0.33 | Already used in kiosk.rs |
| Disk/memory | `sysinfo::Disks` / `System` | 0.33 | Already used in self_test.rs |
| HID probe | `hidapi` | 2 | Already in ffb_controller.rs |
| Async gate | `tokio::join!` | 1 | Already the runtime |
| WS message | `tokio-tungstenite` | 0.26 | Already the WS client |

---

## Sources

- `crates/rc-agent/Cargo.toml` — confirmed `winuser` and `wingdi` features already present (line 62)
- `crates/rc-agent/src/lock_screen.rs` — confirmed `FindWindowW` / `GetWindowRect` available, `get_virtual_screen_bounds()` exists for monitor origin
- `crates/rc-agent/src/self_test.rs` — confirmed disk/memory probe pattern via `sysinfo 0.33`
- `crates/rc-agent/src/kiosk.rs` — confirmed `sysinfo` process scan pattern in `spawn_blocking`
- `crates/rc-agent/src/failure_monitor.rs` — confirmed `EnumWindows` / `spawn_blocking` pattern for Win32 from async context
- `crates/rc-agent/src/billing_guard.rs` — confirmed `attempt_orphan_end()` HTTP call pattern for auto-fix
- [winapi 0.3 docs — GetWindowRect](https://docs.rs/winapi/latest/winapi/um/winuser/fn.GetWindowRect.html) — confirmed in `winuser` module
- [Rust forum — GetWindowRect window position](https://users.rust-lang.org/t/how-to-get-window-position-and-size-of-a-different-process/79224) — MEDIUM confidence (community source, consistent with docs)

---

*Stack research for: v11.1 Pre-Flight Session Checks (rc-agent)*
*Researched: 2026-03-21 IST*
