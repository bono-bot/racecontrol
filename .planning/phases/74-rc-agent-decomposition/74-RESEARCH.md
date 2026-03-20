# Phase 74: rc-agent Decomposition - Research

**Researched:** 2026-03-20 IST
**Domain:** Rust module extraction - large binary crate decomposition
**Confidence:** HIGH

## Summary

Phase 74 decomposes a 3,404-line `main.rs` into four focused modules: `config.rs`, `app_state.rs`, `ws_handler.rs`, and `event_loop.rs`. The Phase 73 characterization tests (TEST-01, TEST-02, TEST-03) are green - the safety net is in place. This is the highest-risk phase in v11.0 because the event loop contains a `select!` body with 14 arms and 15+ shared mutable variables that cross module boundaries.

The extraction order is strict: config types first (zero runtime dependencies), then AppState (depends on config types), then ws_handler (depends on both), then event_loop (depends on everything). Each extraction must be followed by `cargo test -p rc-agent-crate` green before proceeding. The `select!` dispatch body itself - the 22 match arms inside the `msg = ws_rx.next()` arm - is deferred to v12.0 (DECOMP-05). Phase 74 only moves the `select!` call site and its surrounding context into `event_loop.rs`.

The dominant risk is the `select!` body referencing local variables that are defined in `main()` - `game_process`, `adapter`, `lock_screen`, `ffb`, `crash_recovery`, `launch_state`, etc. These must be bundled into a `ConnectionState` struct that is passed to the event loop by mutable reference. Alternatively, they can remain local to the inner loop body inside `event_loop.rs`; the key insight is that `event_loop.rs` houses the `loop { select! { ... } }` construct, not just dispatching functions for each arm.

**Primary recommendation:** Use mutable-ref passing via `ConnectionState` struct for all inner-loop variables. Keep the `select!` arms intact inside `event_loop.rs` - do not split them into sub-handlers (v12.0). Each extraction step must individually compile and pass `cargo test`.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| DECOMP-01 | rc-agent config types extracted from main.rs to config.rs (<500 lines) | All config types identified: AgentConfig, PodConfig, CoreConfig, WheelbaseConfig, TelemetryPortsConfig, GamesConfig, KioskConfig, plus 8 default fns, detect_installed_games(), is_steam_app_installed(), validate_config(), load_config(), config_search_paths() - totals ~350 lines. Zero runtime dependencies on AppState or WS - safest first step. |
| DECOMP-02 | rc-agent AppState struct and shared state extracted to app_state.rs | No struct named AppState exists yet - this means extracting the variables initialized in main() before the reconnect loop: heartbeat_status, ffb, detector, pod_info, pod_id, adapter, failure_monitor channels, ws_exec channels, kiosk, lock_screen, overlay, ai_result channels. These become fields on a new AppState struct. |
| DECOMP-03 | rc-agent WebSocket message handler extracted to ws_handler.rs | The `msg = ws_rx.next()` select arm (lines 1816-2745) handles 22 CoreToAgentMessage variants. This arm becomes handle_ws_message() taking &mut ConnectionState + &mut AppState + &mut ws_tx, returning HandleResult (Continue or Break). |
| DECOMP-04 | rc-agent event loop select! body extracted to event_loop.rs using ConnectionState struct pattern | The inner `loop { select! { ... } }` (lines 1086-2747) becomes `event_loop::run(&mut AppState, ws_tx, ws_rx) -> Result<()>`. ConnectionState holds all per-connection variables: crash_recovery, launch_state, blank_timer/blank_timer_armed, current_driver_name, last_ffb_percent/preset, session_max_speed_kmh, session_race_position, plus the 6 interval timers. |
</phase_requirements>

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Rust module system | n/a (built-in) | `mod foo;` + `pub use` | Standard Rust module pattern - no external crates needed |
| `pub(crate)` visibility | n/a | Restrict pub items to crate scope | Prevents accidental API leakage from config to ws_handler |
| All current rc-agent dependencies | (unchanged) | No new deps needed | Phase 74 adds zero new dependencies - pure reorganization |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Single ConnectionState struct | Multiple Arc-Mutex wraps | Arc-Mutex adds lock contention and is wrong for single-task locals; ConnectionState is passed by &mut ref, no allocation overhead |
| Moving select! arms into sub-handlers | Keep select! monolithic in event_loop.rs | Sub-handlers require ConnectionState/ReconnectState split - too risky for Phase 74, deferred to v12.0 (DECOMP-05) |
| Separate crates | Modules within rc-agent-crate | Separate crates require Cargo.toml workspace changes - far more disruptive |

---

## Architecture Patterns

### Target Project Structure
```
crates/rc-agent/src/
  main.rs            # ~150 lines: #[tokio::main], init sequence, reconnect loop shell
  config.rs          # ~350 lines: all TOML types, load/validate, detect_installed_games
  app_state.rs       # ~150 lines: AppState struct + initialization helpers
  ws_handler.rs      # ~400 lines: handle_ws_message() dispatching 22 CoreToAgentMessage variants
  event_loop.rs      # ~500 lines: ConnectionState struct + inner select! loop
  ac_launcher.rs     # (existing, unchanged)
  billing_guard.rs   # (existing, unchanged - Phase 73 tests here)
  ... (all other existing modules unchanged)
```

### Pattern 1: Config Module Extraction (DECOMP-01)

**What:** Move all structs/enums/fns from main.rs that deal only with TOML deserialization, validation, and config search. None of these touch tokio, WebSocket, or runtime state.

**Exact items to move to config.rs:**
- `AgentConfig` struct (line 51) - add `pub` to struct and all fields
- `default_auto_end_orphan_session_secs()` fn (line 71) - make `pub(crate)`
- `KioskConfig` struct + Default impl (lines 73-83) - add `pub`
- `default_true()` fn (line 85) - keep `pub(crate)` or private
- `GamesConfig` struct (lines 87-105) - add `pub`
- `detect_installed_games()` fn (lines 111-152) - make `pub(crate)`
- `is_steam_app_installed()` fn (lines 155-161) - can stay private inside config.rs
- `PodConfig` struct (lines 163-172) - add `pub`
- `CoreConfig` struct (lines 174-178) - add `pub`
- `WheelbaseConfig` struct + Default impl (lines 180-195) - add `pub`
- `TelemetryPortsConfig` struct + Default impl (lines 197-209) - add `pub`
- `default_sim_ip()`, `default_sim_port()`, `default_core_url()`, `default_wheelbase_vid()`, `default_wheelbase_pid()`, `default_telemetry_ports()` (lines 211-216)
- `validate_config()` fn (lines 2809-2836) - make `pub(crate)`
- `config_search_paths()` fn (lines 2838-2853) - make `pub(crate)`
- `load_config()` fn (lines 2855-2875) - make `pub(crate)`

**Do NOT move to config.rs:**
- `LaunchState` enum (line 218) - inner-loop state, belongs in event_loop.rs
- `CrashRecoveryState` enum (line 229) - contains tokio::time::Sleep pin, belongs in event_loop.rs
- `WS_MAX_CONCURRENT_EXECS` + `WS_EXEC_SEMAPHORE` (lines 249-250) - belongs in ws_handler.rs
- `handle_ws_exec()` fn (lines 254-319) - belongs in ws_handler.rs
- `PANIC_HOOK_ACTIVE` + `PANIC_LOCK_STATE` statics (lines 387-389) - stay in main.rs

**config.rs imports needed:** `serde`, `anyhow`, `toml`, `rc_common::types::SimType`, `crate::ai_debugger::AiDebuggerConfig`, `crate::game_process::GameExeConfig`

**After extraction, main.rs adds:**
```rust
mod config;
use config::{load_config, AgentConfig};
```

### Pattern 2: AppState Struct (DECOMP-02)

**What:** Bundle the long-lived variables initialized in main() before the reconnect loop into a single `AppState` struct. These variables persist across WebSocket reconnections.

**Fields for AppState (pre-loop, reconnect-surviving variables):**
```rust
pub struct AppState {
    pub pod_id: String,
    pub pod_info: PodInfo,
    pub config: AgentConfig,
    pub ffb: std::sync::Arc<FfbController>,
    pub detector: DrivingDetector,
    pub adapter: Option<Box<dyn SimAdapter>>,
    pub heartbeat_status: std::sync::Arc<udp_heartbeat::HeartbeatStatus>,
    pub failure_monitor_tx: tokio::sync::watch::Sender<failure_monitor::FailureMonitorState>,
    pub ws_exec_result_tx: tokio::sync::mpsc::Sender<rc_common::protocol::AgentMessage>,
    pub ws_exec_result_rx: tokio::sync::mpsc::Receiver<rc_common::protocol::AgentMessage>,
    pub ai_result_rx: tokio::sync::mpsc::Receiver<ai_debugger::AiDebugSuggestion>,
    pub ai_result_tx: tokio::sync::mpsc::Sender<ai_debugger::AiDebugSuggestion>,
    pub kiosk: KioskManager,
    pub kiosk_enabled: bool,
    pub lock_screen: LockScreenManager,
    pub lock_event_rx: tokio::sync::mpsc::Receiver<LockScreenEvent>,
    pub overlay: OverlayManager,
    pub signal_rx: tokio::sync::mpsc::Receiver<DetectorSignal>,
    pub heartbeat_event_rx: tokio::sync::mpsc::Receiver<udp_heartbeat::HeartbeatEvent>,
    pub last_launch_error: debug_server::LastLaunchError,
    pub agent_start_time: std::time::Instant,
    pub exe_dir: std::path::PathBuf,
    pub heal_result: self_heal::HealResult,
    pub crash_recovery_startup: bool,
    pub startup_self_test_verdict: Option<String>,
    pub startup_probe_failures: u8,
    pub lock_screen_bound: bool,
    pub remote_ops_bound: bool,
    pub hid_detected: bool,
    // Outer-loop state that survives reconnect
    pub game_process: Option<game_process::GameProcess>,
    pub last_ac_status: Option<AcStatus>,
    pub ac_status_stable_since: Option<std::time::Instant>,
    pub launch_state: LaunchState,
}
```

**Critical note on game_process placement:** `game_process` is declared at line 705 (before reconnect loop) and referenced in the Register message build at line 988 (outer loop header). It must be in AppState, NOT ConnectionState.

**SimAdapter Send bound check:** Before implementing AppState containing `Box<dyn SimAdapter>`, verify `SimAdapter: Send` in `sims/mod.rs`. If not Send, keep adapter as a local in main() passed by `&mut` to event_loop::run().

### Pattern 3: ConnectionState Struct (for DECOMP-04)

**What:** Bundle all variables declared at the TOP of the inner loop (lines 1058-1084) that reset on every new WebSocket connection.

**Fields for ConnectionState (re-initialized each inner loop iteration):**
```rust
pub struct ConnectionState {
    // 6 interval timers
    pub heartbeat_interval: tokio::time::Interval,
    pub telemetry_interval: tokio::time::Interval,
    pub detector_interval: tokio::time::Interval,
    pub game_check_interval: tokio::time::Interval,
    pub kiosk_interval: tokio::time::Interval,
    pub overlay_topmost_interval: tokio::time::Interval,
    // Auto-blank timer
    pub blank_timer: std::pin::Pin<Box<tokio::time::Sleep>>,
    pub blank_timer_armed: bool,
    // Session lifecycle state
    pub crash_recovery: CrashRecoveryState,
    pub last_launch_args_stored: Option<String>,
    pub current_driver_name: Option<String>,
    // FFB tracking
    pub last_ffb_percent: u8,
    pub last_ffb_preset: String,
    // Telemetry accumulators
    pub session_max_speed_kmh: f32,
    pub session_race_position: Option<u32>,
    // WS connect time for grace window
    pub ws_connect_time: tokio::time::Instant,
}
```

**LaunchState and CrashRecoveryState location:** These belong in event_loop.rs (where ConnectionState is defined), NOT config.rs. CrashRecoveryState contains `tokio::time::Sleep` - importing tokio in config.rs would be architecturally wrong.

### Pattern 4: ws_handler.rs Function Signature

```rust
// ws_handler.rs
pub enum HandleResult {
    Continue,
    Break, // connection lost -> reconnect
}

pub type WsTx = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>
    >,
    tokio_tungstenite::tungstenite::Message,
>;

pub async fn handle_ws_message(
    text: &str,
    state: &mut AppState,
    conn: &mut ConnectionState,
    ws_tx: &mut WsTx,
) -> HandleResult
```

Also move to ws_handler.rs:
- `WS_MAX_CONCURRENT_EXECS` const
- `WS_EXEC_SEMAPHORE` static
- `handle_ws_exec()` fn

### Pattern 5: event_loop.rs Entry Point

```rust
// event_loop.rs
pub type WsRx = futures_util::stream::SplitStream<
    tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>
    >,
>;

pub async fn run(
    state: &mut AppState,
    mut ws_tx: WsTx,  // separate from AppState to avoid borrow conflict in select!
    mut ws_rx: WsRx,
) -> Result<()>
```

Inside `run()`: create `ConnectionState::new()`, enter `loop { select! { ... } }`. The select! arms remain monolithic - no sub-handler dispatch (v12.0).

**Borrow conflict rule:** ws_tx MUST be a separate parameter, not a field of AppState. The select! macro borrows ws_tx mutably (for send) and AppState mutably (for other fields) in the same arm - two separate borrows of the same AppState would be rejected by the borrow checker.

### Anti-Patterns to Avoid

- **Moving select! arm bodies into sub-handlers:** Deferred to DECOMP-05/v12.0. Do not attempt in Phase 74.
- **Using Arc-Mutex for per-connection variables:** ConnectionState is passed as &mut - no lock overhead.
- **Batching two extractions in one plan:** Each module must be its own plan with a cargo test gate.
- **Moving game_process to ConnectionState:** It is referenced in the Register message build (outer loop line 988) - it survives reconnects, so it belongs in AppState.
- **Placing LaunchState/CrashRecoveryState in config.rs:** These are runtime state enums containing tokio types. They belong in event_loop.rs.
- **Forgetting pub modifiers:** All config types are private in main.rs scope. They need `pub` added when extracted to config.rs.

---

## Complete Variable Classification

This is the critical pre-work. Every variable in the event loop classified by scope:

### Outer Loop Variables (survive reconnect -> AppState)
| Variable | Line | Evidence of outer-loop scope |
|----------|------|------------------------------|
| `reconnect_attempt` | 924 | outer loop counter, reset only on successful connect |
| `startup_complete_logged` | 925 | once-per-lifetime flag |
| `startup_report_sent` | 926 | once-per-lifetime flag |
| `ws_disconnected_at` | 929 | SESSION-04 grace window, set in outer loop |
| `game_process` | 705 | Referenced in Register message at line 988 (outer loop) |
| `last_ac_status` | 708 | Declared before outer loop; used only in inner loop - can be in ConnectionState if reset on each connect |
| `ac_status_stable_since` | 709 | Same as above |
| `launch_state` | 710 | Same as above - referenced in LaunchState::Idle init |

Note: `last_ac_status`, `ac_status_stable_since`, and `launch_state` are declared before the outer loop but never read between the inner loop's `break` and the next iteration's `connect_async`. They CAN go in ConnectionState - verify no reads exist in the outer loop reconnect logic before deciding.

### Inner Loop Variables (reset per connection -> ConnectionState)
| Variable | Line | Reset value |
|----------|------|-------------|
| `heartbeat_interval` | 1058 | interval(5s) |
| `telemetry_interval` | 1059 | interval(100ms) |
| `detector_interval` | 1060 | interval(100ms) |
| `game_check_interval` | 1061 | interval(2s) |
| `kiosk_interval` | 1062 | interval(5s) |
| `overlay_topmost_interval` | 1063 | interval(10s) |
| `blank_timer` | 1065 | sleep(86400s) dormant |
| `blank_timer_armed` | 1067 | false |
| `crash_recovery` | 1071 | CrashRecoveryState::Idle |
| `last_launch_args_stored` | 1073 | None |
| `current_driver_name` | 1076 | None |
| `last_ffb_percent` | 1079 | 70 |
| `last_ffb_preset` | 1080 | "medium" |
| `session_max_speed_kmh` | 1083 | 0.0 |
| `session_race_position` | 1084 | None |

### Pre-Loop Variables (initialized in main() before any loop -> AppState)
| Variable | Lines |
|----------|-------|
| `pod_id`, `pod_info`, `sim_type` | 566-602 |
| `detector` | 628 |
| `ffb` | 631 |
| `signal_rx` | 658 |
| `adapter` | 661 |
| `ai_result_tx`, `ai_result_rx` | 713-716 |
| `ws_exec_result_tx`, `ws_exec_result_rx` | 716-717 |
| `failure_monitor_tx` | 719-721 |
| `kiosk`, `kiosk_enabled` | 723-730 |
| `lock_screen`, `lock_event_rx` | 734-735 |
| `overlay` | 770 |
| `last_launch_error` | 775-776 |
| `heartbeat_status` | 799 |
| `heartbeat_event_rx` | 800 |
| `agent_start_time` | 556 |
| `config` | 510 |
| `exe_dir` | 493-496 |
| `heal_result` | 497 |
| `crash_recovery_startup` (startup detect) | 479 |
| startup report fields | 908-913 |

---

## select! Arm Inventory (all 14 arms)

| # | Arm | Lines | Variables Touched | Risk |
|---|-----|-------|-------------------|------|
| 1 | `heartbeat_interval.tick()` | 1088-1104 | ws_tx, detector, game_process, lock_screen, last_ffb_preset, pod_info | LOW |
| 2 | `telemetry_interval.tick()` | 1105-1222 | adapter, overlay, ws_tx, game_process, last_ac_status, ac_status_stable_since, launch_state, failure_monitor_tx, pod_id | MEDIUM |
| 3 | `signal_rx.recv()` | 1224-1239 | detector, heartbeat_status, failure_monitor_tx, ws_tx, pod_id | LOW |
| 4 | `detector_interval.tick()` | 1241-1259 | detector, failure_monitor_tx, ws_tx, pod_id | LOW |
| 5 | `game_check_interval.tick()` | 1261-1397 | game_process, heartbeat_status, failure_monitor_tx, ws_tx, config.ai_debugger, ai_result_tx, lock_screen, ffb, adapter, pod_id | HIGH |
| 6 | `ai_result_rx.recv()` | 1399-1468 | ai_debugger, ws_tx, pod_id, config, lock_screen, heartbeat_status, agent_start_time, detector, launch_state | MEDIUM |
| 7 | `kiosk_interval.tick()` | 1470-1554 | kiosk, kiosk_enabled, ws_exec_result_tx, config.ai_debugger, pod_id | MEDIUM |
| 8 | `overlay_topmost_interval.tick()` | 1557-1568 | overlay, kiosk, kiosk_enabled | LOW |
| 9 | `blank_timer` | 1570-1583 | blank_timer_armed, heartbeat_status, lock_screen, ffb, ws_tx, pod_id | LOW |
| 10 | `crash_recovery timer` | 1586-1766 | crash_recovery, game_process, adapter, ws_tx, failure_monitor_tx, pod_id, ffb, lock_screen, overlay, launch_state, last_launch_args_stored, current_driver_name, last_ac_status, ac_status_stable_since | HIGH |
| 11 | `lock_event_rx.recv()` | 1768-1780 | ws_tx, pod_id | LOW |
| 12 | `ws_exec_result_rx.recv()` | 1782-1789 | ws_tx | LOW |
| 13 | `heartbeat_event_rx.recv()` | 1791-1815 | heartbeat_status, ws_connect_time | LOW |
| 14 | `msg = ws_rx.next()` | 1816-2745 | ALL state variables, 22 CoreToAgentMessage variants | VERY HIGH |

**ws_rx arm CoreToAgentMessage variants handled (22 total):**
BillingStarted, BillingTick, BillingStopped, SessionEnded, LaunchGame (AC branch + generic branch), StopGame, ShowPinLockScreen, ShowQrLockScreen, ClearLockScreen, BlankScreen, SubSessionEnded, ShowAssistanceScreen, EnterDebugMode, SettingsUpdated, SetTransmission, SetFfb, SetAssist (abs/tc/transmission), SetFfbGain, QueryAssistState, PinFailed, Ping, Exec, ApproveProcess, RejectProcess, RunSelfTest, other (warn)

---

## Extraction Risk Analysis

### Risk Order for Plan Splitting
1. **Plan 74-01:** config.rs (DECOMP-01) - no runtime deps, pure TOML types, ~350 lines
2. **Plan 74-02:** app_state.rs (DECOMP-02) - group pre-loop variables, verify SimAdapter Send
3. **Plan 74-03:** ws_handler.rs (DECOMP-03) - extract ws_rx.next() arm into handle_ws_message()
4. **Plan 74-04:** event_loop.rs (DECOMP-04) - wrap inner loop in run(), define ConnectionState

Each plan ends with `cargo test -p rc-agent-crate` gate. Plan 74-04 adds `cargo build --release --bin rc-agent` for the final binary verification.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Visibility between modules | Custom re-export glue | `pub use crate::config::AgentConfig;` in main.rs | Standard Rust module system |
| Sharing ws_tx across select! arms | Arc-Mutex around SplitSink | Pass `&mut ws_tx` as function parameter | select! requires exclusive mutable access to ws_tx - Arc-Mutex would deadlock inside select! |
| ConnectionState initialization | Complex builder pattern | Plain `ConnectionState::new()` struct literal | 15 fields, all obvious defaults |
| Type alias for SplitSink | Inline the 150-char type everywhere | Define `WsTx` and `WsRx` type aliases in event_loop.rs | Readability; Rust type aliases are zero-cost |

---

## Common Pitfalls

### Pitfall 1: game_process in ConnectionState

**What goes wrong:** Developer places `game_process` in ConnectionState because it is declared inside the reconnect loop body.

**Why it happens:** `game_process` is declared at line 705 which is before the outer loop, but inside `main()`. It looks like outer-loop setup code. However the Register message at line 988 reads `game_process.as_ref().map(|g| g.state)` - this is the outer loop reconnect header, meaning game_process must survive the break out of the inner loop.

**How to avoid:** Place game_process in AppState. Test: grep for game_process reads between `break;` and the next `connect_async` call.

**Warning signs:** After placing game_process in ConnectionState, the Register message build can no longer access it without threading ConnectionState through the outer loop - that is the signal it is in the wrong struct.

### Pitfall 2: LaunchState/CrashRecoveryState in config.rs

**What goes wrong:** Researcher places LaunchState (line 218) and CrashRecoveryState (line 229) in config.rs because they appear before the reconnect loop in main.rs.

**Why it happens:** Their position in the file looks like config/type definitions. But CrashRecoveryState contains a `std::pin::Pin<Box<tokio::time::Sleep>>` field - importing tokio in config.rs would be wrong.

**How to avoid:** LaunchState and CrashRecoveryState are runtime state machines, not configuration. They belong in event_loop.rs alongside ConnectionState.

**Warning signs:** config.rs imports tokio - that is always a red flag for this module.

### Pitfall 3: Forgetting pub on Config Types

**What goes wrong:** config.rs compiles but main.rs cannot access `config::PodConfig`, `config::GamesConfig`, etc.

**Why it happens:** In main.rs, these structs had no visibility modifier (defaulting to private within the main.rs file scope, which is the crate root for a binary). When moved to config.rs they need explicit `pub`.

**How to avoid:** Add `pub` to every struct definition and every field that is read outside config.rs. If tests in config.rs reference private fields, keep those fields private.

### Pitfall 4: Borrow Checker Conflicts in select! with &mut AppState

**What goes wrong:** select! tries to borrow `state.ws_exec_result_rx` mutably for recv() AND `state.lock_screen` for a method call in the same arm - both go through `&mut AppState`.

**Why it happens:** The Rust borrow checker sees multiple mutable sub-borrows from a single `&mut AppState` as a conflict in complex match arm bodies.

**How to avoid:** Keep all channel receivers (signal_rx, ws_exec_result_rx, heartbeat_event_rx, ai_result_rx, lock_event_rx) as either separate parameters to event_loop::run() OR as fields of AppState accessed via separate local borrows at the top of each arm. The practical solution is to pass ws_tx/ws_rx separately and let AppState hold everything else - the compiler will surface specific conflicts during compilation.

**Warning signs:** Compiler error "cannot borrow `state` as mutable more than once at a time" inside select! arms.

### Pitfall 5: Tests Break After config.rs Extraction

**What goes wrong:** The `#[cfg(test)]` mod in main.rs (lines 3064-3404) uses `use super::*`. After extraction, `validate_config()`, `config_search_paths()`, `load_config()`, `detect_installed_games()` etc. are no longer in `super` scope.

**Why it happens:** `super::*` pulls from main.rs scope. After extraction, these functions are in `crate::config`.

**How to avoid:** Either add `pub(crate) use config::*;` to main.rs (re-exports everything), or move the config tests to config.rs where they naturally belong. Recommended: move `validate_config_*`, `test_config_search_paths_*`, `test_load_config_*`, `test_installed_games_*`, `test_is_steam_app_installed_*` tests to config.rs in Plan 74-01. Keep reconnect/session/crash tests in main.rs.

### Pitfall 6: PANIC_HOOK_ACTIVE and PANIC_LOCK_STATE Must Stay in main.rs

**What goes wrong:** Developer moves the two panic statics to app_state.rs during DECOMP-02.

**Why it happens:** They look like "shared state" that belongs in AppState.

**How to avoid:** These are process-global singletons used inside a sync panic hook installed in `main()`. The panic hook is a `Box<dyn Fn>` closure that captures `ffb_vid`/`ffb_pid` by copy and references these statics by path. Changing their path requires updating the panic hook closure. Keep them in main.rs.

---

## Code Examples

### Config Module - Shell

```rust
// crates/rc-agent/src/config.rs
// Source: main.rs lines 50-216 + 2809-2875

use anyhow::Result;
use serde::Deserialize;
use crate::ai_debugger::AiDebuggerConfig;
use crate::game_process::GameExeConfig;
use rc_common::types::SimType;

#[derive(Debug, Deserialize)]
pub struct AgentConfig {
    pub pod: PodConfig,
    pub core: CoreConfig,
    #[serde(default)]
    pub wheelbase: WheelbaseConfig,
    // ... all fields pub
}

pub fn load_config() -> Result<AgentConfig> { /* unchanged body */ }
pub(crate) fn validate_config(config: &AgentConfig) -> Result<()> { /* unchanged body */ }
pub(crate) fn config_search_paths() -> Vec<std::path::PathBuf> { /* unchanged body */ }
pub(crate) fn detect_installed_games(games: &GamesConfig) -> Vec<SimType> { /* unchanged body */ }
```

### WsTx Type Alias

```rust
// crates/rc-agent/src/event_loop.rs
// Source: required to make ws_handler.rs function signatures readable

pub type WsTx = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>
    >,
    tokio_tungstenite::tungstenite::Message,
>;
pub type WsRx = futures_util::stream::SplitStream<
    tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>
    >,
>;
```

### ConnectionState Initialization

```rust
// crates/rc-agent/src/event_loop.rs
impl ConnectionState {
    pub fn new() -> Self {
        Self {
            heartbeat_interval: tokio::time::interval(Duration::from_secs(5)),
            telemetry_interval: tokio::time::interval(Duration::from_millis(100)),
            detector_interval: tokio::time::interval(Duration::from_millis(100)),
            game_check_interval: tokio::time::interval(Duration::from_secs(2)),
            kiosk_interval: tokio::time::interval(Duration::from_secs(5)),
            overlay_topmost_interval: tokio::time::interval(Duration::from_secs(10)),
            blank_timer: Box::pin(tokio::time::sleep(Duration::from_secs(86400))),
            blank_timer_armed: false,
            crash_recovery: CrashRecoveryState::Idle,
            last_launch_args_stored: None,
            current_driver_name: None,
            last_ffb_percent: 70,
            last_ffb_preset: "medium".to_string(),
            session_max_speed_kmh: 0.0,
            session_race_position: None,
            ws_connect_time: tokio::time::Instant::now(),
        }
    }
}
```

### main.rs After All Extractions (~150 lines)

```rust
// The target state of main.rs after Phase 74 complete
mod config;
mod app_state;
mod ws_handler;
mod event_loop;
// ... all existing mods

use config::load_config;
use app_state::AppState;
use event_loop::run;

static PANIC_HOOK_ACTIVE: AtomicBool = AtomicBool::new(false);
static PANIC_LOCK_STATE: OnceLock<Arc<Mutex<lock_screen::LockScreenState>>> = OnceLock::new();

#[tokio::main]
async fn main() -> Result<()> {
    // panic hook install (stays in main.rs)
    // single-instance guard
    // log cleanup
    // self-heal
    // config load
    // tracing init
    // AppState::new(&config).await  <- most init moves here
    // reconnect loop:
    //   connect_async
    //   register
    //   run(&mut state, ws_tx, ws_rx).await  <- inner loop
    //   disconnect handling
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Monolithic main.rs (~3400 lines) | Focused modules (<500 lines each) | Phase 74 | Enables isolated testing; reduces merge conflicts on feature work |
| Variables scattered in main() | AppState + ConnectionState structs | Phase 74 | Allows event_loop::run() to be called with typed state |

**Deferred/out of scope for Phase 74:**
- DECOMP-05: select! dispatch into sub-handlers per message type (v12.0)
- DECOMP-06: lock_screen state machine as standalone module (v12.0)

---

## Open Questions

1. **SimAdapter Send bound**
   - What we know: `adapter: Option<Box<dyn SimAdapter>>` - if AppState contains it and AppState must be Send (for tokio spawn), SimAdapter must be Send
   - What's unclear: Does SimAdapter trait have a Send bound? AssettoCorsaAdapter uses shared memory - is it Send?
   - Recommendation: Read `crates/rc-agent/src/sims/mod.rs` before writing app_state.rs. If SimAdapter is not Send, pass adapter separately as `&mut Option<Box<dyn SimAdapter>>` to event_loop::run() rather than embedding in AppState.

2. **ws_exec_result_rx placement**
   - What we know: ws_exec_result_rx is read in select! arm 12 (inner loop only). ws_exec_result_tx is cloned for billing_guard, failure_monitor, kiosk tasks.
   - Recommendation: Put ws_exec_result_rx in AppState alongside the tx (they are created together). It is read only in the inner loop via `&mut state.ws_exec_result_rx` - the borrow checker handles this via field borrows.

---

## Validation Architecture

nyquist_validation is enabled in .planning/config.json.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test + tokio::test (1.x workspace) |
| Config file | Cargo.toml [dev-dependencies] |
| Quick run command | `cargo test -p rc-agent-crate 2>&1 \| tail -20` |
| Full suite command | `cargo test -p rc-agent-crate` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DECOMP-01 | config.rs exists <500 lines, all config types accessible from main.rs | build + existing tests | `cargo build --bin rc-agent && cargo test -p rc-agent-crate` | Wave 0 - created in Plan 74-01 |
| DECOMP-02 | app_state.rs exists <500 lines, AppState struct compiles | build + existing tests | `cargo build --bin rc-agent && cargo test -p rc-agent-crate` | Wave 0 - created in Plan 74-02 |
| DECOMP-03 | ws_handler.rs exists <500 lines, handle_ws_message() handles all 22 variants | build + existing tests | `cargo build --bin rc-agent && cargo test -p rc-agent-crate` | Wave 0 - created in Plan 74-03 |
| DECOMP-04 | event_loop.rs exists, main.rs <500 lines, release build passes | build + line count | `wc -l crates/rc-agent/src/main.rs && cargo build --release --bin rc-agent && cargo test -p rc-agent-crate` | Wave 0 - created in Plan 74-04 |

### Sampling Rate
- **Per extraction plan completion:** `cargo test -p rc-agent-crate` - MUST be green before next extraction begins (non-negotiable gate per Refactor Second rule)
- **Per wave merge:** `cargo build --release --bin rc-agent && cargo test -p rc-agent-crate`
- **Phase gate:** `wc -l crates/rc-agent/src/main.rs` output <500 + all tests green + `cargo build --release --bin rc-agent` passes Pod 8 canary

### Wave 0 Gaps
- [ ] `crates/rc-agent/src/config.rs` - created in Plan 74-01
- [ ] `crates/rc-agent/src/app_state.rs` - created in Plan 74-02
- [ ] `crates/rc-agent/src/ws_handler.rs` - created in Plan 74-03
- [ ] `crates/rc-agent/src/event_loop.rs` - created in Plan 74-04

No new test files needed. Phase 73 tests in billing_guard.rs, failure_monitor.rs, and ffb_controller.rs are the characterization tests. DECOMP requirements are verified by `cargo build` success (structural correctness) + existing tests green (behavioral correctness).

---

## Sources

### Primary (HIGH confidence)
- `crates/rc-agent/src/main.rs` - full 3,404-line file read, every struct/fn/static/const catalogued with line numbers
- `crates/rc-agent/Cargo.toml` - full dependency list read
- `.planning/REQUIREMENTS.md` - DECOMP-01..04 requirements text confirmed
- `.planning/STATE.md` - v11.0 accumulated context and constraints
- `.planning/phases/73-critical-business-tests/73-VERIFICATION.md` - Phase 73 green confirmed (9/9 tests pass, all three requirements satisfied)
- `CLAUDE.md` - project standing rules, deployment constraints

### Secondary (MEDIUM confidence)
- Rust reference: module system (mod, pub use, pub(crate)) - well-understood, no external verification needed
- tokio 1.x docs: select! macro borrowing rules - known constraint, confirmed by tokio 1.x documentation pattern

---

## Metadata

**Confidence breakdown:**
- Variable classification (AppState vs ConnectionState): HIGH - every variable identified by exact line number from direct source read
- Extraction order (config -> app_state -> ws_handler -> event_loop): HIGH - locked decision from STATE.md Accumulated Context
- select! arm inventory (14 arms, 22 WS message variants): HIGH - directly enumerated from main.rs full read
- WsTx type alias pattern: HIGH - standard Rust pattern for complex future sink types
- SimAdapter Send bound question: LOW - requires reading sims/mod.rs trait definition (not read in this research session)
- ws_exec_result_rx placement question: MEDIUM - reasoning from ownership patterns, but not verified against compiler

**Research date:** 2026-03-20 IST
**Valid until:** 2026-04-20 (stable codebase - no external dependency changes expected for this phase)
