# Architecture Research

**Domain:** Anti-cheat safe mode integration into existing rc-agent pod software
**Researched:** 2026-03-21
**Confidence:** HIGH (based on direct codebase inspection + anti-cheat behaviour research)

---

## Existing Architecture Context

Understanding the current system is essential before describing what changes.

**Deployment topology per pod:**

```
Pod (Windows 11, 192.168.31.x)
  rc-agent.exe         port 8090   Axum HTTP + WebSocket client to racecontrol
  rc-sentry.exe        port 8091   Pure std::net TCP, no tokio, 6 endpoints
```

**rc-agent current state (v11.0 decomposed):**
- `main.rs` spawns all subsystems and enters the WebSocket reconnect loop
- `app_state.rs` — all long-lived state that survives WS reconnections
- `ws_handler.rs` — dispatches `CoreToAgentMessage` from racecontrol
- `event_loop.rs` — main `tokio::select!` loop, heartbeat, telemetry, game lifecycle
- `kiosk.rs` — process allowlist enforcement, LLM classifier, kill-path
- `process_guard.rs` — continuous whitelist scan, autostart audit, kill-path
- `game_process.rs` — spawn/kill/monitor game child process
- `sims/` — `SimAdapter` trait + per-game adapters (AC, F1 25, iRacing, LMU, AC EVO)

**Game launch flow (existing):**

```
Staff kiosk → POST /api/v1/fleet/launch
    ↓
racecontrol → WS CoreToAgentMessage::LaunchGame { sim_type, launch_args }
    ↓
ws_handler.rs::handle_ws_message (LaunchGame arm)
    → game_process::launch()
    → persist_pid(pid)
    ↓
event_loop.rs game_check_interval (every 2s)
    → game_process.is_running()
    → adapter.connect() (when not connected)
    → adapter.read_telemetry() (100ms)
```

**Shared memory adapters (existing, relevant to anti-cheat):**
- `sims/iracing.rs` — opens `IRSDKMemMapFileName` via `OpenFileMapping` + `MapViewOfFile`
- `sims/lmu.rs` — opens `$rFactor2SMMP_Scoring$` and `$rFactor2SMMP_Telemetry$` same way
- `sims/assetto_corsa.rs` — opens `acpmf_physics`, `acpmf_graphics`, `acpmf_static`
- `sims/f1_25.rs` — UDP only (port 20777), no shared memory, no anti-cheat risk
- All adapters use `adapter.connect()` called lazily from event_loop when `!adapter.is_connected()`

**Subsystems with anti-cheat risk (existing):**
- `kiosk.rs` — uses `sysinfo::System::refresh_processes()` to scan running processes. Can kill any non-whitelisted process. Risky if it scans and kills while an anti-cheat driver is watching.
- `process_guard.rs` — same process scanning, runs every 60s. More aggressive: kills CRITICAL binaries with zero grace.
- `overlay.rs` — creates a Win32 overlay window on top of game. Rendered as a separate process window (not DLL injection). Lower risk than SetWindowsHookEx, but EA Javelin may flag unusual window layering.
- Keyboard hook (`SetWindowsHookEx WH_KEYBOARD_LL`) — system-wide low-level keyboard hook installed by rc-agent. This is the **highest risk** behaviour for kernel-level anti-cheat systems. EAC and Javelin actively monitor `SetWindowsHookEx` calls as a known injection vector.

---

## System Overview: v15.0 Safe Mode Integration

```
+----------------------------------------------------------------------+
| racecontrol (server :8080)                                            |
|  LaunchGame WS → { sim_type: F125, launch_args }                     |
+----------------------------------------------------------------------+
                              |  WebSocket
+----------------------------------------------------------------------+
| rc-agent (pod :8090)                                                  |
|                                                                       |
|  ws_handler.rs — LaunchGame dispatch                                  |
|    existing: conn.current_sim_type = Some(launch_sim)                 |
|    NEW:      if safe_mode::requires_safe_mode(launch_sim) {           |
|                  state.safe_mode.enter(launch_sim);                   |
|              }                                                        |
|    existing: game_process::launch() [unchanged]                       |
|                            |                                          |
|                  AppState mutation                                     |
|                            |                                          |
|  +-------------------------v-------------------------------------+    |
|  |  AppState                                                     |    |
|  |    existing: sim_type, game_process, adapter, ...             |    |
|  |    NEW:      safe_mode: SafeModeState                         |    |
|  |               { active: bool,                                 |    |
|  |                 game: Option<SimType>,                        |    |
|  |                 entered_at: Option<Instant> }                 |    |
|  +---+---------------+------------------+------------------+----+    |
|      |               |                  |                  |         |
|  +---v----+  +-------v------+  +--------v---+  +----------v----+     |
|  |kiosk.rs|  |process_guard |  |game_process|  |sims/ adapters  |    |
|  |        |  |.rs           |  |.rs         |  |(iracing, lmu)  |    |
|  |GATED:  |  |GATED:        |  |UNCHANGED   |  |DEFERRED:       |    |
|  |skip    |  |skip kill_and |  |            |  |connect() after |    |
|  |kill in |  |report when   |  |            |  |game is live    |    |
|  |safe    |  |safe_mode     |  |            |  |+ 5s grace      |    |
|  |mode    |  |active        |  |            |  |                |    |
|  +--------+  +--------------+  +------------+  +----------------+    |
|                                                                       |
|  event_loop.rs — game exit detection                                  |
|    existing: game_process.is_running() → false → arm exit_grace_timer|
|    NEW: on exit_grace fire → safe_mode.exit()                         |
|    belt-and-suspenders: ws_handler BillingStopped → safe_mode.exit() |
+----------------------------------------------------------------------+
```

---

## Component Responsibilities

| Component | Status | Anti-cheat Role |
|-----------|--------|-----------------|
| `safe_mode.rs` | **NEW** | SafeModeState struct, `enter()`/`exit()` transitions, `requires_safe_mode(sim)` classification table |
| `app_state.rs` | MODIFIED | Holds `safe_mode: SafeModeState` — persists across WS reconnections (game stays running during transient disconnect) |
| `ws_handler.rs` LaunchGame arm | MODIFIED | Calls `safe_mode.enter(sim)` before `game_process::launch()` |
| `ws_handler.rs` BillingStopped/SessionEnded | MODIFIED | Calls `safe_mode.exit()` as belt-and-suspenders |
| `event_loop.rs` exit grace path | MODIFIED | Calls `safe_mode.exit()` when `exit_grace_timer` fires |
| `event_loop.rs` telemetry branch | MODIFIED | Guards `adapter.connect()` — defers shm connect for protected games until 5s after game is live |
| `kiosk.rs` kill-path | MODIFIED | Checks `state.safe_mode.active` before killing unknown processes |
| `process_guard.rs` kill-path | MODIFIED | Checks `state.safe_mode.active` before `kill_and_report` |
| `game_process.rs` | UNCHANGED | Process spawn/kill/monitor is not anti-cheat risky (killing our own child) |
| `sims/iracing.rs` | reference | `connect()` opens `IRSDKMemMapFileName` — caller in event_loop must gate the call |
| `sims/lmu.rs` | reference | `connect()` opens rF2 shared memory — same gate |
| `sims/assetto_corsa.rs` | reference | AC has no kernel anti-cheat — connect immediately, no gate needed |
| `sims/f1_25.rs` | reference | UDP only, no memory reads — no gate needed |
| `config.rs` | MODIFIED | Add `[anti_cheat]` TOML section with enable flag and per-pod overrides |

---

## Recommended Project Structure

The safe mode feature adds one new module and modifies four existing ones. No new crate is needed.

```
crates/rc-agent/src/
├── safe_mode.rs          NEW — SafeModeState, enter/exit, requires_safe_mode() table
├── app_state.rs          MODIFIED — add safe_mode: SafeModeState field
├── ws_handler.rs         MODIFIED — enter on LaunchGame, exit on BillingStopped/SessionEnded
├── event_loop.rs         MODIFIED — exit on game death; gate adapter.connect()
├── kiosk.rs              MODIFIED — gate kill-path on safe_mode.active
├── process_guard.rs      MODIFIED — gate kill-path on safe_mode.active
└── config.rs             MODIFIED — add [anti_cheat] config section
```

### Structure Rationale

- **`safe_mode.rs` as a new module:** Keeps the classification table (which sims need protection) in one auditable place. An exhaustive `match` in `requires_safe_mode()` ensures a compile error when a new `SimType` variant is added without updating this table.
- **`AppState` as owner:** Safe mode state must survive WebSocket reconnections — the game keeps running through a transient WS drop and must stay in safe mode throughout. Same rationale as all other `AppState` fields.
- **No new crate:** All logic is boolean flag propagation and a lookup table. No external dependencies are needed.

---

## Architectural Patterns

### Pattern 1: Safe Mode State Machine

**What:** `SafeModeState` is a plain struct with three fields — `active: bool`, `game: Option<SimType>`, `entered_at: Option<Instant>`. It is entered on `LaunchGame` for protected sims and exited when the game process dies. All gated subsystems check the `active` flag before performing risky operations.

**When to use:** Any subsystem that calls `kill_process` / `kill_and_report` on processes that are not the active game, or that opens a shared memory handle into the game's address space.

**Trade-offs:** Single bool check adds zero measurable latency. The risk is forgetting to add a check when new risky behaviour is introduced in future phases. Mitigation: document the gating requirement in `safe_mode.rs` module doc.

**Example:**
```rust
// safe_mode.rs

pub struct SafeModeState {
    pub active: bool,
    pub game: Option<SimType>,
    pub entered_at: Option<std::time::Instant>,
}

impl SafeModeState {
    pub fn new() -> Self {
        Self { active: false, game: None, entered_at: None }
    }

    pub fn enter(&mut self, sim: SimType) {
        if self.active {
            tracing::warn!(target: "safe-mode", "enter() called while already active — idempotent");
            self.game = Some(sim);
            return;
        }
        self.active = true;
        self.game = Some(sim);
        self.entered_at = Some(std::time::Instant::now());
        tracing::info!(target: "safe-mode", sim = ?sim, "Anti-cheat safe mode ENTERED");
    }

    pub fn exit(&mut self) {
        if !self.active {
            return; // idempotent
        }
        tracing::info!(target: "safe-mode", game = ?self.game,
            elapsed_secs = ?self.entered_at.map(|t| t.elapsed().as_secs()),
            "Anti-cheat safe mode EXITED");
        self.active = false;
        self.game = None;
        self.entered_at = None;
    }
}

/// Which sims require safe mode?
/// Exhaustive match — compile error when new SimType added without updating this table.
pub fn requires_safe_mode(sim: SimType) -> bool {
    match sim {
        SimType::F125            => true,  // EA Javelin (kernel) — scans external process memory
        SimType::IRacing         => true,  // Epic EOS — bans for unauthorized memory access
        SimType::LeMansUltimate  => true,  // Epic EOS (rF2 engine) — same enforcement
        SimType::AssettoCorsaEvo => true,  // Unknown AC (Early Access) — default protected
        SimType::AssettoCorsa    => false, // No kernel AC — hooks and scanning safe
        SimType::AssettoCorsaRally => false, // No AC confirmed
        SimType::Forza           => false, // No ban risk confirmed for LAN/offline play
        SimType::ForzaHorizon5   => false, // Same
    }
}
```

### Pattern 2: Guard Suspension via Flag Check

**What:** Gated subsystems check `state.safe_mode.active` at the top of their kill-path before executing. Scanning continues — only the kill action is suppressed. This preserves observability without risking a ban.

**When to use:** In `kiosk.rs` before the WARN_BEFORE_ACTION kill path, and in `process_guard.rs` before `kill_and_report`.

**Critical exception:** `process_guard.rs` CRITICAL tier (racecontrol.exe on a pod) must still kill even in safe mode — this protects against standing rule #2 violations regardless of session state.

**Example:**
```rust
// In process_guard.rs run_scan_cycle(), before kill_and_report():
if safe_mode_active {
    tracing::info!(target: LOG_TARGET,
        process = %pname,
        "Safe mode active — logging violation but NOT killing (anti-cheat gate)");
    // Still send ProcessViolation to server for logging, but with kill=false
    return;
}
// Normal kill path continues below
```

### Pattern 3: Deferred Shared Memory Connect

**What:** `SimAdapter::connect()` for iRacing and LMU is deferred until the game has been confirmed live (game is in `GameState::Running`) AND a 5-second grace period has elapsed. A new `shm_connect_allowed(state, conn)` guard in `event_loop.rs` controls this.

**When to use:** Only for `SimType::IRacing` and `SimType::LeMansUltimate`. F1 25 uses UDP only. AC has no anti-cheat concern.

**Trade-offs:** 5-second telemetry delay at session start. Acceptable because billing starts from `PlayableSignal` (game confirmed live), not from process spawn. The 5s is conservative — the key risk window is anti-cheat driver initialization which completes within the first 3-5 seconds of the game process.

**Example:**
```rust
// In event_loop.rs telemetry_interval branch:
let Some(ref mut adapter) = state.adapter else { continue };
if !adapter.is_connected() {
    if shm_connect_allowed(&state, &conn) {
        if adapter.connect().is_ok() {
            state.overlay.set_max_rpm(adapter.max_rpm());
        }
    }
    continue;
}

// Separate helper:
fn shm_connect_allowed(state: &AppState, conn: &ConnectionState) -> bool {
    if !state.safe_mode.active {
        return true; // unprotected sim — connect immediately
    }
    // Protected sim: require game live and 5s elapsed since game_check confirmed Running
    // conn.current_sim_type.is_some() && game_process running is already checked by adapter connect path
    // Use safe_mode.entered_at as proxy — game was launched when safe mode entered
    state.safe_mode.entered_at
        .map(|t| t.elapsed().as_secs() >= 5)
        .unwrap_or(false)
}
```

---

## Data Flow

### Safe Mode Entry Flow (LaunchGame)

```
Server → WS CoreToAgentMessage::LaunchGame { sim_type: F125, launch_args }
    ↓ ws_handler.rs handle_ws_message
conn.current_sim_type = Some(F125)          [existing]
if safe_mode::requires_safe_mode(F125) {    [NEW]
    state.safe_mode.enter(F125);
}
    ↓ (side effects, immediate)
kiosk:        kill-path suppressed this session
process_guard: kill_and_report suppressed (except CRITICAL binaries)
adapter:       connect() gated until 5s post-launch
    ↓
game_process::launch() → spawns F1_25.exe   [existing, unchanged]
    ↓ 5s later (event_loop telemetry interval)
shm_connect_allowed() returns true
adapter.connect() → opens UDP bind or shm handle
```

### Safe Mode Exit Flow (Game Death)

```
event_loop.rs game_check_interval (every 2s)
    → game_process.is_running() → false
    ↓ [existing exit_grace_timer logic]
exit_grace_timer armed (30s)
    ↓ on timer fire:
GameStatusUpdate::Off sent to server            [existing]
safe_mode.exit()                                [NEW — inserted here]
    ↓ (side effects, immediate)
kiosk:        kill-path restored
process_guard: kill_and_report restored
adapter:       already disconnected by BillingStopped path
```

### Belt-and-Suspenders Exit (BillingStopped / SessionEnded)

```
ws_handler.rs BillingStopped or SessionEnded arm
    → [existing: stop game, disconnect adapter, ffb zero, lock screen]
    → safe_mode.exit()   [NEW — added at end of these handlers]
```

These run in parallel with the exit_grace path. `exit()` is idempotent — calling it twice is safe.

### State Ownership Summary

```
AppState.safe_mode: SafeModeState
    Written (enter) by:   ws_handler.rs LaunchGame arm
    Written (exit) by:    event_loop.rs exit_grace fire
                          ws_handler.rs BillingStopped arm
                          ws_handler.rs SessionEnded arm
    Read by:              kiosk.rs (before kill-path)
                          process_guard.rs (before kill_and_report)
                          event_loop.rs (before adapter.connect())
```

---

## New vs Modified Components

### New Components

| Component | File | Justification |
|-----------|------|---------------|
| `SafeModeState` struct | `safe_mode.rs` | Centralises state transitions and classification. Single source of truth for which sims need protection. |
| `requires_safe_mode(sim)` fn | `safe_mode.rs` | Exhaustive `match` on `SimType` — compile error prevents silent omissions when new games are added in v13.0. |
| `[anti_cheat]` TOML section | `config.rs` | Allows per-pod override (Pod 8 canary testing with forced safe mode on AC to verify no regression). Fields: `enabled: bool`, `shm_defer_secs: u64`. |

### Modified Components

| Component | Change | Risk |
|-----------|--------|------|
| `app_state.rs` | Add `safe_mode: SafeModeState` field | LOW — additive, no existing code broken |
| `ws_handler.rs` LaunchGame arm | Insert `safe_mode.enter()` after `conn.current_sim_type` assignment, before `game_process::launch()` | LOW — non-blocking, no ordering issue |
| `ws_handler.rs` BillingStopped / SessionEnded | Insert `safe_mode.exit()` at end of each handler | LOW — belt-and-suspenders, idempotent |
| `event_loop.rs` exit_grace fire path | Insert `safe_mode.exit()` alongside existing `GameStatusUpdate::Off` emission | LOW — single line addition at correct lifecycle point |
| `event_loop.rs` telemetry interval adapter connect | Wrap `adapter.connect()` call with `shm_connect_allowed()` guard | LOW-MEDIUM — must preserve existing AC connect behaviour (AC returns `true` immediately) |
| `kiosk.rs` kill-path | Check `state.safe_mode.active` before kill in `enforce()` | MEDIUM — must not gate the game process itself (already excluded by existing self-exclusion) |
| `process_guard.rs` kill-path | Check `state.safe_mode.active` before `kill_and_report` | MEDIUM — CRITICAL binaries must bypass the gate |

---

## Integration Points

### Primary Integration Point: `ws_handler.rs` LaunchGame

This is where detection and safe mode entry happens. The existing code at line ~283 already has:
```rust
conn.current_sim_type = Some(launch_sim);  // existing
```

The new call inserts immediately after this, before any game launch logic:
```rust
if safe_mode::requires_safe_mode(launch_sim) {
    state.safe_mode.enter(launch_sim);
}
```

This is the single source of entry — nothing else calls `safe_mode.enter()`.

### Secondary Integration Point: `event_loop.rs` exit_grace fire

The existing exit_grace path already handles:
1. `emit GameStatusUpdate::Off`
2. clear `exit_grace_armed`

Safe mode exit inserts as step 3:
3. `state.safe_mode.exit()`

The `exit_grace_timer` fires 30 seconds after `is_running()` returns false — this is exactly when the anti-cheat driver has fully cleaned up after the game exit.

### Tertiary Integration Point: `event_loop.rs` telemetry branch, adapter.connect()

The existing pattern:
```rust
if !adapter.is_connected() {
    if adapter.connect().is_ok() { ... }
    continue;
}
```

Becomes:
```rust
if !adapter.is_connected() {
    if shm_connect_allowed(&state, &conn) && adapter.connect().is_ok() { ... }
    continue;
}
```

For non-protected sims (AC, Forza, F1 25), `shm_connect_allowed` returns `true` immediately — existing behaviour preserved.

### Subsystem Gate Placement: `kiosk.rs`

The kill-path in `kiosk.rs` runs inside `KioskManager::enforce()` when an unknown process exceeds `WARN_BEFORE_ACTION_COUNT`. The gate inserts before the kill call:
```rust
if app_state_safe_mode_active {
    tracing::info!(target: LOG_TARGET, process = %name, "Safe mode: skip kill");
    return;
}
```

Note: `KioskManager` does not currently hold a reference to `AppState` — it operates on passed parameters. The safe mode flag is best passed as a `bool` argument to `enforce()` from `event_loop.rs` where `AppState` is available.

### Subsystem Gate Placement: `process_guard.rs`

`process_guard::spawn()` runs as a detached `tokio::spawn` task. It currently receives only `ProcessGuardConfig`, `whitelist`, `tx`, and `machine_id`. To gate on safe mode, a `Arc<AtomicBool>` shared safe mode indicator should be passed to the spawn function — OR — process_guard can check a new shared atomic that `AppState` exposes, matching the pattern of `heartbeat_status` (which is already an `Arc<HeartbeatStatus>` with atomics).

Recommended: add `safe_mode_active: Arc<AtomicBool>` to the `process_guard::spawn()` signature, sourced from a new `AppState::safe_mode_flag: Arc<AtomicBool>` that is `store(true/false)` in parallel with `SafeModeState`.

---

## Anti-cheat System Classification

| Game | Anti-cheat | Kernel Level | Risk to rc-agent | Notes |
|------|------------|--------------|------------------|-------|
| F1 25 | **EA Javelin** (not EAC — confirmed) | YES | CRITICAL | Javelin scans external process memory access, monitors SetWindowsHookEx as injection vector. UDP telemetry (port 20777) is explicitly the official telemetry channel — safe to use. |
| iRacing | **Epic EOS** (migrated from EAC in May 2024) | YES | HIGH | iRacing SDK shared memory (`IRSDKMemMapFileName`) is the official telemetry API — used by iOverlay, RaceLab, Crew Chief without bans. Risk is in timing: opening handle during EOS driver init. Defer connect by 5s. |
| LMU | **Epic EOS** (rF2/LMU common engine) | YES | HIGH | rF2SharedMemoryMapPlugin is official. Same connect-timing caution as iRacing. |
| AC EVO | **Unknown** (Early Access, Kunos/505 Games) | UNKNOWN | HIGH by default | Treat as protected until Pod 8 canary confirms safe. May use EAC, Javelin, or Kunos custom. |
| Assetto Corsa (classic) | **None** | NO | SAFE | No kernel anti-cheat. SetWindowsHookEx, process scanning, and shared memory all safe. |
| AC Rally | **None confirmed** | NO | SAFE | Small Kunos EA title, no AC reported. |
| Forza Motorsport | Unknown | UNCLEAR | LOW | Xbox/Microsoft title — bans require online play. LAN/offline sessions unlikely to trigger detection. |
| Forza Horizon 5 | Unknown | UNCLEAR | LOW | Same reasoning as Forza Motorsport. |

**Key finding on F1 25:** EA Javelin is NOT Easy Anti-Cheat (EAC). The PROJECT.md currently says "EAC" — this is incorrect. Javelin is EA's own kernel-level anti-cheat. The risk profile is similar to EAC but the detection triggers differ. Javelin is known to block: external process memory reads, unsigned DLL injection, SetWindowsHookEx system-wide hooks. UDP telemetry on port 20777 is not flagged — it is the official developer-provided channel.

**Key finding on iRacing:** iRacing migrated from EAC to Epic EOS in May 2024. The SDK shared memory interface is officially supported and commercially used by dozens of overlay tools without bans. The risk is not the API itself but the timing of opening the handle.

---

## Shared Memory + Anti-cheat Interaction (Detailed)

### iRacing

`IracingAdapter::connect()` opens `IRSDKMemMapFileName` using `OpenFileMapping(FILE_MAP_READ, FALSE, "Local\\IRSDKMemMapFileName")`. This is the official iRacing SDK pattern. Commercial tools (iOverlay, RaceLab, Crew Chief) do this continuously without bans.

**Risk window:** The 5-10 seconds immediately after `iRacingSim64DX11.exe` spawns, while EOS is initializing and scanning the process environment. Opening a new shared memory handle during this window may appear suspicious in combination with other rc-agent behaviors (process scanning, keyboard hook).

**Mitigation:** Defer `IracingAdapter::connect()` until `shm_connect_allowed()` returns true (5s after safe mode entered). By that point, EOS initialization is complete and the handle open looks like a normal telemetry reader.

**Additional safety:** `IracingAdapter::disconnect()` closes the `MapViewOfFile` and `CloseHandle` handles. Verify this runs on game exit (it is called via `BillingStopped` handler: `if let Some(ref mut adp) = state.adapter { adp.disconnect(); }`). Confirmed via `ws_handler.rs` lines 239, 273.

### LMU / rFactor 2

`LmuAdapter` opens `$rFactor2SMMP_Scoring$` and `$rFactor2SMMP_Telemetry$`. These are exposed by the rF2SharedMemoryMapPlugin — a DLL that must be installed in the game's `Plugins/` folder. rc-agent does not install this DLL; that is an ops setup step per pod.

**Risk:** Same timing window as iRacing. Same 5-second defer mitigation applies.

**Note:** If the rF2 plugin is not installed, `OpenFileMapping` returns `NULL` and `LmuAdapter::connect()` returns an error. The adapter retries every 100ms via the existing `!adapter.is_connected()` path. This is benign.

### AC EVO

AC EVO shares memory via the `acpmf_*` named maps (same as classic AC). Whether Kunos has added kernel-level anti-cheat to the Early Access title is unconfirmed as of 2026-03-21. Treating it as protected (safe mode active, deferred connect) is the conservative default. The Pod 8 canary validation phase will confirm the actual risk level before any `requires_safe_mode(AssettoCorsaEvo)` change.

---

## Build Order (Phase Dependencies)

| Phase | Work | Depends On | Notes |
|-------|------|------------|-------|
| 1 | `safe_mode.rs` — `SafeModeState` struct + `requires_safe_mode()` table | Nothing | Standalone new module. Write tests for exhaustive SimType coverage. |
| 2 | `app_state.rs` — add `safe_mode: SafeModeState` + `safe_mode_flag: Arc<AtomicBool>` | Phase 1 | Additive field. `safe_mode_flag` is the shared atomic for process_guard. |
| 3 | `process_guard.rs` — accept `safe_mode_flag: Arc<AtomicBool>`, gate kill-path | Phase 2 | CRITICAL tier bypasses gate. |
| 4 | `kiosk.rs` — accept `safe_mode_active: bool` parameter in `enforce()`, gate kill-path | Phase 2 | Passed from event_loop which has AppState access. |
| 5 | `ws_handler.rs` — `safe_mode.enter()` in LaunchGame; `safe_mode.exit()` in BillingStopped/SessionEnded | Phase 2 | Primary entry/exit wiring. |
| 6 | `event_loop.rs` — `safe_mode.exit()` on exit_grace fire; `shm_connect_allowed()` guard | Phase 5 | Exit path wiring + adapter connect gate. |
| 7 | `config.rs` — add `[anti_cheat]` section | Phase 1 | Simple config extension. |
| 8 | Keyboard hook replacement — policy-based lockdown for protected games | After Phase 5-6 | Independent from 1-6 but safe_mode state available for per-game dispatch. Requires design work separate from the flag-gating above. |
| 9 | Code signing — procure cert, sign `rc-agent.exe` and `rc-sentry.exe` | Independent | Procurement + build pipeline task. Highest impact mitigation (unsigned binaries are flagged by most anti-cheat). Can run in parallel with Phases 1-8. |
| 10 | Pod 8 canary validation — per-game test sessions, anti-cheat matrix documentation | All phases complete | Pod 8 canary-first per standing policy. |

Phases 1-7 form the safe mode mechanism and should ship as one milestone phase. Phase 8 (keyboard hook replacement) is a separate phase requiring its own research (what policy-based alternative to use). Phase 9 (code signing) is a procurement task that should start immediately — certificate issuance takes 1-7 days.

---

## Anti-patterns to Avoid

### Anti-Pattern 1: Disabling All rc-agent Behaviour During Safe Mode

**What people do:** Treat safe mode as "minimal mode" — disable billing, lock screen, overlay, and WS heartbeat to "be safe."

**Why it's wrong:** Billing, lock screen, overlay (separate window, not DLL injection), and WebSocket heartbeats are not anti-cheat risks. They do not inspect game memory, inject DLLs, or install system-wide hooks. Disabling them breaks the core business logic while providing no protection benefit.

**Do this instead:** Gate only the two specific risky paths: (a) process kill in `kiosk.rs` and `process_guard.rs`, and (b) shared memory connect timing for iRacing and LMU. Everything else runs normally.

### Anti-Pattern 2: Permanent Removal of Keyboard Hook

**What people do:** Remove `SetWindowsHookEx WH_KEYBOARD_LL` entirely to eliminate the anti-cheat risk.

**Why it's wrong:** The hook provides kiosk lockdown for ALL games — it prevents customers from pressing Win key, Alt+F4, Alt+Tab, etc. during sessions. Removing it entirely weakens kiosk security for unprotected games (AC, Forza) where it is safe to use.

**Do this instead:** Suspend the hook when safe mode is active (protected game is running), restore it on safe mode exit. For protected games, use policy-based lockdown: Edge kiosk flags (`--kiosk`, `--kiosk-printing`), `SetForegroundWindow` enforcement loop, and Windows Group Policy to disable Win key — none of which use `SetWindowsHookEx`.

### Anti-Pattern 3: Re-entrant Safe Mode Without Idempotency

**What people do:** Call `safe_mode.enter()` in LaunchGame without checking if already active. If a second LaunchGame fires (crash recovery relaunch), safe mode enters twice but only exits once, leaving the pod permanently in safe mode after the session ends.

**Do this instead:** Make `enter()` and `exit()` idempotent. `enter()` when already active updates the game field but does not double-arm. `exit()` when already inactive is a no-op. See the `enter()` implementation in Pattern 1 above.

### Anti-Pattern 4: Racing Shared Memory Handles Against Anti-cheat Init

**What people do:** Call `adapter.connect()` immediately when the game process appears in `game_check_interval` (i.e., when `game_process.is_running()` first returns true).

**Why it's wrong:** The game process appearing does not mean EOS/Javelin has finished its own initialization scan. Opening a new file mapping handle during the first 3-5 seconds of game launch is the highest-risk window.

**Do this instead:** The 5-second defer in `shm_connect_allowed()`. For iRacing specifically, the adapter already has a natural delay — `IsOnTrack` must be true before meaningful telemetry exists, which takes 15-30 seconds from launch. The 5-second gate is conservative and still well within the natural connect window.

### Anti-Pattern 5: Holding Shared Memory Handles After Game Exit

**What people do:** Leave `IracingAdapter` or `LmuAdapter` in connected state after the billing session ends, relying on the next session's `disconnect()` call to clean up.

**Why it's wrong:** Some anti-cheat systems perform a cleanup scan after the game exits. An open `MapViewOfFile` handle into their memory region during that scan can trigger false positives in future sessions.

**Do this instead:** Confirm that `BillingStopped` and `SessionEnded` handlers call `adp.disconnect()` promptly (they do — lines 239, 273 in `ws_handler.rs`). Verify that `IracingAdapter::disconnect()` and `LmuAdapter::disconnect()` call both `UnmapViewOfFile` and `CloseHandle`. Add a test assertion.

---

## Scaling Considerations

Fixed 8-pod fleet. Traditional scaling does not apply. The relevant operational considerations:

| Concern | Impact | Mitigation |
|---------|--------|------------|
| `safe_mode.active` check in kiosk scan | Single bool read per 5s scan | Negligible |
| `safe_mode_flag.load()` in process_guard | Single atomic load per 60s scan | Negligible |
| 5s deferred shm connect | Telemetry missing for first 5s of protected game session | Acceptable — billing starts from PlayableSignal, not lap 1 |
| `requires_safe_mode()` match | Called once per game launch (LaunchGame WS message) | Negligible |
| Safe mode persisting through WS reconnect | AppState owns the field — reconnect loop never resets AppState | Correct by design |

---

## Sources

- Direct codebase inspection: `crates/rc-agent/src/app_state.rs` — all AppState fields, confirmed no existing safe_mode field
- Direct codebase inspection: `crates/rc-agent/src/ws_handler.rs` — LaunchGame dispatch, BillingStopped, SessionEnded handlers; `disconnect()` calls at lines 239, 273
- Direct codebase inspection: `crates/rc-agent/src/event_loop.rs` — ConnectionState, game_check_interval, exit_grace_timer, adapter.connect() call site
- Direct codebase inspection: `crates/rc-agent/src/game_process.rs` — all SimType process names, is_running(), stop()
- Direct codebase inspection: `crates/rc-agent/src/kiosk.rs` — WARN_BEFORE_ACTION_COUNT kill-path
- Direct codebase inspection: `crates/rc-agent/src/process_guard.rs` — spawn() signature, CRITICAL_BINARIES, kill_and_report path
- Direct codebase inspection: `crates/rc-agent/src/sims/iracing.rs` — IRSDKMemMapFileName, connect() implementation
- Direct codebase inspection: `crates/rc-agent/src/sims/lmu.rs` — rF2SharedMemoryMapPlugin handle names, connect() implementation
- Direct codebase inspection: `crates/rc-agent/src/config.rs` — AgentConfig structure, existing config sections
- [iRacing EAC to EOS migration (official support)](https://support.iracing.com/support/solutions/articles/31000173103-anticheat-not-installed-uninstalling-eac-and-installing-eos-) — confirms iRacing uses EOS not EAC since May 2024 (HIGH confidence, official source)
- [F1 25 PCGamingWiki](https://www.pcgamingwiki.com/wiki/F1_25) — confirms EA Javelin anti-cheat, not EAC (MEDIUM confidence)
- [EA Anti-Cheat overview](https://players.com.ua/en/ea-anticheat-a-beginner-s-guidea-brief-look-at-the-world-of-cheats-and-anti-cheats-using-f1-as-an-example/) — Javelin detection: process memory reads, external injection, DLL hash (MEDIUM confidence, third-party analysis)
- [iRacing SDK docs](https://sajax.github.io/irsdkdocs/) — confirms IRSDKMemMapFileName is official API (MEDIUM confidence)
- [SetWindowsHookExA MSDN](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowshookexa) — WH_KEYBOARD_LL is system-wide hook, monitored by anti-cheat as injection vector (HIGH confidence, official)
- PROJECT.md — v15.0 requirements, constraints, SimType inventory (HIGH confidence, source of truth)

---

*Architecture research for: v15.0 AntiCheat Compatibility — rc-agent safe mode integration*
*Researched: 2026-03-21 IST*
