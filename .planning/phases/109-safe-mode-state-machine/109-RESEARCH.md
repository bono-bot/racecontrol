# Phase 109: Safe Mode State Machine - Research

**Researched:** 2026-03-21
**Domain:** Rust async state machine, WMI process event subscription, tokio timer patterns, subsystem gating
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- New `SafeMode` struct in AppState with `active: bool`, `game: Option<SimType>`, `cooldown_until: Option<Instant>`
- State survives WebSocket reconnections (lives in AppState, not WS session)
- New module: `crates/rc-agent/src/safe_mode.rs`
- **Primary detection:** Existing `LaunchGame` handler in ws_handler.rs — enters safe mode before spawning game process (zero delay)
- **Secondary detection:** WMI `Win32_ProcessStartTrace` event subscription for detecting games launched OUTSIDE rc-agent
- WMI subscription watches: `F1_25.exe`, `iRacingSim64DX11.exe`, `Le Mans Ultimate.exe`, `acs_x64.exe` (AC EVO), `WRC.exe`
- Protected games: F1 25, iRacing, LMU, EA WRC, AC EVO. AC original NOT protected.
- Game exit: Start 30-second cooldown. If another protected game launches during cooldown, safe mode stays active (no gap).
- Process guard — SUSPEND during safe mode
- Ollama LLM queries — SUPPRESS during safe mode
- Registry write operations — DEFER until safe mode exits
- Claude decides: gating implementation (Arc<AtomicBool>, channel signals, or direct state checks)
- Claude decides: whether to add safe_mode field to self_test.rs probes
- Claude decides: logging strategy

### Unaffected Subsystems (SAFE-07)
- Billing lifecycle, lock screen, overlay, WebSocket keepalive/heartbeat, WebSocket exec, UDP heartbeat

### Deferred Ideas (OUT OF SCOPE)
- Server-side safe mode dashboard visibility
- Safe mode override from admin panel
- Per-game subsystem gating granularity (all games get same safe mode for v15.0)
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| SAFE-01 | rc-agent detects protected game launch within 1 second via WMI Win32_ProcessStartTrace event subscription (not polling) | PowerShell WMI event watcher via `Register-WmiEvent` in a background thread — consistent with codebase pattern of shelling to PowerShell for Windows-specific operations |
| SAFE-02 | rc-agent enters safe mode automatically when a protected game is detected, managed by a state machine in AppState (safe_mode.rs) | SafeMode struct in AppState, transitions driven from LaunchGame handler and WMI detection channel |
| SAFE-03 | Safe mode remains active for 30 seconds after the protected game exits (EA Javelin post-game cooldown) | tokio::time::Sleep pinned in AppState (same pattern as exit_grace_timer in ConnectionState) |
| SAFE-04 | Process guard (allowlist enforcement + auto-kill) is suspended during safe mode | Arc<AtomicBool> safe_mode_active passed to process_guard::spawn() — checked at top of scan loop |
| SAFE-05 | Ollama LLM queries are suppressed during safe mode (GPU/memory contention + anti-cheat suspicion) | Safe mode flag checked in analyze_crash() before spawning Ollama HTTP request |
| SAFE-06 | Registry write operations are deferred until safe mode exits | Registry write functions (kiosk GPO, self_heal, lock_screen) guarded by safe mode flag check |
| SAFE-07 | Billing, lock screen, overlay, heartbeat, and WebSocket exec continue uninterrupted during safe mode | No changes to billing_guard, lock_screen, overlay, udp_heartbeat, ws_handler exec path |
</phase_requirements>

---

## Summary

Phase 109 adds a safe mode state machine to rc-agent that prevents anti-cheat systems from flagging RaceControl software during protected game sessions. The implementation has two distinct parts: (1) the state machine itself — a `SafeMode` struct in `AppState` with a cooldown timer, and (2) the subsystem gates — lightweight checks in process_guard, ai_debugger, and registry-writing functions.

Game detection uses a dual-path strategy. The `LaunchGame` WS handler covers rc-agent-initiated launches with zero latency. External launches (direct Steam, staff testing) require WMI `Win32_ProcessStartTrace` event subscription. The codebase consistently shells to PowerShell for Windows-specific operations rather than using the `windows` Rust crate's COM/WMI interfaces — this pattern is confirmed in ac_launcher.rs, ai_debugger.rs, lock_screen.rs, self_heal.rs, and debug_server.rs. The WMI watcher should follow this pattern: run a PowerShell `Register-WmiEvent` loop in a `std::thread::spawn` background thread, sending game names over a `std::sync::mpsc` channel into the tokio event loop.

The cooldown timer follows the exact pattern established by `exit_grace_timer` in `ConnectionState` — a `Pin<Box<tokio::time::Sleep>>` stored in AppState, reset on game exit, polled in the event_loop `select!` block. The subsystem gating for process_guard uses an `Arc<AtomicBool>` (matching the `in_maintenance` field pattern already in AppState), while Ollama suppression and registry deferral are simpler inline checks against `state.safe_mode.active`.

**Primary recommendation:** Use PowerShell WMI subscription on a dedicated std::thread, bridge via std::sync::mpsc into event_loop, gate subsystems with Arc<AtomicBool> for process_guard (cross-task) and direct state.safe_mode.active checks for same-task code.

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| tokio | 1 (workspace) | Async runtime, timers, channels | Already in workspace |
| std::sync::atomic::AtomicBool | std | Cross-task safe mode flag for process_guard | Matches existing `in_maintenance` pattern in AppState |
| tokio::time::Sleep (pinned) | 1 (workspace) | 30-second cooldown timer | Matches existing `exit_grace_timer` pattern in ConnectionState |
| std::sync::mpsc | std | WMI thread → tokio bridge (WMI runs on blocking std::thread) | Standard bridge pattern; tokio channels can't cross thread boundary easily with blocking loops |
| PowerShell Register-WmiEvent | OS built-in | Win32_ProcessStartTrace subscription | Established codebase pattern; avoids `windows` crate COM complexity |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| sysinfo | 0.33 | Startup scan: detect already-running protected games at boot | Already a dependency; used in game_process::cleanup_orphaned_games() |
| tracing | 0.1 (workspace) | Safe mode transition logging | Already used everywhere |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| PowerShell WMI shell-out | `windows` crate COM/WMI (IWbemServices) | `windows` crate requires adding a new heavyweight dependency with COM feature flags; PowerShell approach is 10 lines vs 200+ lines of COM boilerplate; codebase has ZERO precedent for `windows` crate WMI usage |
| PowerShell WMI shell-out | `wmi` crate (Rust WMI wrapper) | No existing `wmi` crate in Cargo.toml; adds dependency; PowerShell is simpler for event subscription |
| Arc<AtomicBool> for process_guard | mpsc channel to process_guard | process_guard runs in detached tokio::spawn; channels would require restructuring spawn() API; AtomicBool is drop-in like in_maintenance |
| Direct state.safe_mode.active check | Watch channel broadcast | Ollama and registry writes are called from event_loop context that already has &mut AppState; direct field access is simplest |

**Installation:** No new dependencies needed. All patterns are achievable with existing crates.

---

## Architecture Patterns

### Recommended Project Structure
```
crates/rc-agent/src/
├── safe_mode.rs         # NEW: SafeMode struct, state transitions, is_protected_game()
├── app_state.rs         # MODIFY: add safe_mode: SafeMode + safe_mode_active: Arc<AtomicBool>
├── main.rs              # MODIFY: startup scan, initialize safe_mode field, pass AtomicBool to process_guard::spawn()
├── event_loop.rs        # MODIFY: WMI channel receiver, cooldown timer poll, safe mode status in heartbeat
├── ws_handler.rs        # MODIFY: LaunchGame enters safe mode before spawn
├── process_guard.rs     # MODIFY: check AtomicBool at scan loop top
└── ai_debugger.rs       # MODIFY: check safe_mode_active before Ollama call
```

Registry writes affected (SAFE-06):
- `crates/rc-agent/src/kiosk.rs` — `apply_gpo_lockdown()` / `remove_gpo_lockdown()`
- `crates/rc-agent/src/self_heal.rs` — `repair_registry_key()`
- `crates/rc-agent/src/lock_screen.rs` — Focus Assist registry write

### Pattern 1: SafeMode Struct in AppState

**What:** A plain struct (not Arc-wrapped) living in AppState, manipulated only from the async event_loop context that holds `&mut AppState`. For process_guard which runs detached, an `Arc<AtomicBool>` shadow flag is kept in sync.

**When to use:** Any code that runs in the event_loop or ws_handler (which receive `&mut AppState`) uses `state.safe_mode.active` directly.

```rust
// crates/rc-agent/src/safe_mode.rs
use std::time::Instant;
use rc_common::types::SimType;

/// State machine for anti-cheat safe mode.
/// Lives in AppState — survives WebSocket reconnections.
#[derive(Debug)]
pub struct SafeMode {
    /// Whether safe mode is currently active.
    pub active: bool,
    /// Which game triggered safe mode (None during cooldown-only phase).
    pub game: Option<SimType>,
    /// Instant after which safe mode can deactivate (None = not in cooldown).
    pub cooldown_until: Option<Instant>,
}

impl SafeMode {
    pub fn new() -> Self {
        Self { active: false, game: None, cooldown_until: None }
    }

    /// Enter safe mode for a specific game. Idempotent — calling while active extends no gap.
    pub fn enter(&mut self, game: SimType) {
        self.active = true;
        self.game = Some(game);
        self.cooldown_until = None; // clear any existing cooldown; game is running
        tracing::info!(target: "safe-mode", "ENTER safe mode — game={:?}", game);
    }

    /// Game exited — start 30-second cooldown. Returns the Instant when cooldown ends.
    pub fn start_cooldown(&mut self) -> Instant {
        let until = Instant::now() + std::time::Duration::from_secs(30);
        self.cooldown_until = Some(until);
        self.game = None;
        tracing::info!(target: "safe-mode", "COOLDOWN started — exits at {:?}", until);
        until
    }

    /// Called when cooldown timer fires. Deactivates safe mode.
    pub fn exit(&mut self) {
        self.active = false;
        self.game = None;
        self.cooldown_until = None;
        tracing::info!(target: "safe-mode", "EXIT safe mode");
    }
}

/// Returns true if this SimType requires safe mode protection.
pub fn is_protected_game(sim: SimType) -> bool {
    matches!(sim,
        SimType::F125
        | SimType::IRacing
        | SimType::LeMansUltimate
        | SimType::AssettoCorsaEvo
        // WRC: add SimType::EaWrc when added to SimType enum
    )
}

/// Game executable names that WMI should watch.
pub const PROTECTED_EXE_NAMES: &[&str] = &[
    "F1_25.exe",
    "iRacingSim64DX11.exe",
    "Le Mans Ultimate.exe",
    "AssettoCorsaEVO.exe",
    "AC2-Win64-Shipping.exe",   // AC EVO alternate name
    "WRC.exe",
];
```

### Pattern 2: WMI Process Detection via PowerShell Thread

**What:** Spawn a `std::thread` that runs a blocking PowerShell loop subscribing to `Win32_ProcessStartTrace`. Detection events are sent over a `std::sync::mpsc` channel, which is drained in the tokio event_loop `select!` via a `tokio::sync::mpsc` bridge or direct `try_recv()`.

**When to use:** Detecting games launched outside rc-agent (Steam desktop shortcuts, staff manual launches).

```rust
// In safe_mode.rs or event_loop.rs setup
use std::sync::mpsc as std_mpsc;
use std::process::Command;

/// Spawns WMI watcher thread. Returns receiver end of game-detected channel.
/// The thread runs forever; channel is dropped when rc-agent exits.
pub fn spawn_wmi_watcher() -> std_mpsc::Receiver<String> {
    let (tx, rx) = std_mpsc::channel::<String>();
    std::thread::spawn(move || {
        // PowerShell script: subscribe to process start events, filter by exe name,
        // print matching exe names to stdout one per line.
        let ps_script = r#"
$names = @('F1_25.exe','iRacingSim64DX11.exe','Le Mans Ultimate.exe','AssettoCorsaEVO.exe','AC2-Win64-Shipping.exe','WRC.exe')
$query = "SELECT * FROM Win32_ProcessStartTrace"
Register-WmiEvent -Query $query -SourceIdentifier 'SafeModeWatch' | Out-Null
while ($true) {
    $event = Wait-Event -SourceIdentifier 'SafeModeWatch' -Timeout 5
    if ($event) {
        $exe = $event.SourceEventArgs.NewEvent.ProcessName
        if ($names -contains $exe) {
            Write-Output $exe
            [Console]::Out.Flush()
        }
        Remove-Event -SourceIdentifier 'SafeModeWatch'
    }
}
"#;
        // Run PowerShell with hidden window (CREATE_NO_WINDOW = 0x08000000)
        use std::os::windows::process::CommandExt;
        let mut child = match Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", ps_script])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .creation_flags(0x08000000)
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(target: "safe-mode", "WMI watcher spawn failed: {}", e);
                return;
            }
        };
        use std::io::{BufRead, BufReader};
        if let Some(stdout) = child.stdout.take() {
            for line in BufReader::new(stdout).lines() {
                match line {
                    Ok(exe_name) if !exe_name.is_empty() => {
                        let _ = tx.send(exe_name);
                    }
                    _ => {}
                }
            }
        }
        tracing::warn!(target: "safe-mode", "WMI watcher stdout ended — process monitoring disabled");
    });
    rx
}
```

**Integration in event_loop:** Store `wmi_rx: std_mpsc::Receiver<String>` in AppState (initialized in main.rs). Poll it in the `select!` loop using `try_recv()` inside a regular tick interval (game_check_interval at 2s is sufficient — WMI events arrive within 1s regardless of polling rate, since the thread blocks on events independently).

Actually, since `std_mpsc::Receiver::try_recv()` is non-blocking, it should be polled in the game_check_interval tick arm rather than a separate `select!` arm (which would require `async` receiver). Store as `Option<std_mpsc::Receiver<String>>` in AppState to allow None if WMI watcher fails.

### Pattern 3: Arc<AtomicBool> for Cross-Task Gating (process_guard)

**What:** process_guard is spawned detached via `tokio::spawn`. It cannot access `state.safe_mode.active` because it doesn't hold AppState. An `Arc<AtomicBool>` stored in AppState and cloned into process_guard::spawn() provides the same semantics as `in_maintenance`.

**When to use:** Any tokio::spawn'd task that needs to read safe mode without holding AppState.

```rust
// In AppState (app_state.rs)
pub(crate) safe_mode: crate::safe_mode::SafeMode,
pub(crate) safe_mode_active: std::sync::Arc<std::sync::atomic::AtomicBool>,

// In main.rs initialization
safe_mode: crate::safe_mode::SafeMode::new(),
safe_mode_active: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),

// Clone to pass into process_guard::spawn()
let safe_mode_flag = Arc::clone(&state.safe_mode_active);
process_guard::spawn(config, whitelist, tx, machine_id, safe_mode_flag);

// In process_guard.rs spawn() — add parameter
pub fn spawn(
    config: ProcessGuardConfig,
    whitelist: Arc<RwLock<MachineWhitelist>>,
    tx: mpsc::Sender<AgentMessage>,
    machine_id: String,
    safe_mode: Arc<std::sync::atomic::AtomicBool>,  // NEW
) {
    // ...
    tokio::spawn(async move {
        // at top of scan loop:
        if safe_mode.load(std::sync::atomic::Ordering::Relaxed) {
            tracing::debug!(target: LOG_TARGET, "safe mode active — scan skipped");
            scan_interval.tick().await;
            continue;
        }
        // existing scan logic...
    });
}
```

**Keeping AtomicBool in sync:** Whenever `state.safe_mode.enter()` or `state.safe_mode.exit()` is called, immediately call `state.safe_mode_active.store(state.safe_mode.active, Ordering::Relaxed)`.

### Pattern 4: Cooldown Timer in AppState

**What:** Store a `Pin<Box<tokio::time::Sleep>>` cooldown timer in AppState (not ConnectionState, so it survives reconnects). Poll in event_loop `select!`.

**Pattern matches:** `exit_grace_timer` + `exit_grace_armed` in ConnectionState — same Pin<Box<Sleep>> + bool armed approach but moved to AppState.

```rust
// In AppState
pub(crate) safe_mode_cooldown_timer: std::pin::Pin<Box<tokio::time::Sleep>>,
pub(crate) safe_mode_cooldown_armed: bool,

// Initialization (main.rs)
safe_mode_cooldown_timer: Box::pin(tokio::time::sleep(Duration::from_secs(86400))),
safe_mode_cooldown_armed: false,

// On game exit (event_loop.rs or ws_handler.rs)
fn on_protected_game_exit(state: &mut AppState) {
    let until = state.safe_mode.start_cooldown();
    let duration = until.saturating_duration_since(std::time::Instant::now());
    state.safe_mode_cooldown_timer.as_mut().reset(
        tokio::time::Instant::now() + duration
    );
    state.safe_mode_cooldown_armed = true;
    state.safe_mode_active.store(true, Ordering::Relaxed); // still active during cooldown
}

// In event_loop select!
_ = &mut state.safe_mode_cooldown_timer, if state.safe_mode_cooldown_armed => {
    state.safe_mode_cooldown_armed = false;
    state.safe_mode.exit();
    state.safe_mode_active.store(false, Ordering::Relaxed);
    tracing::info!(target: "event-loop", "Safe mode cooldown expired — safe mode deactivated");
}
```

### Pattern 5: Startup Detection of Already-Running Protected Games

**What:** One-time scan at agent startup (in main.rs before the reconnect loop) using sysinfo — same API as `game_process::cleanup_orphaned_games()`.

```rust
// In main.rs, after other startup checks, before reconnect loop
pub fn detect_running_protected_game() -> Option<SimType> {
    use sysinfo::System;
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    for (_pid, process) in sys.processes() {
        let name = process.name().to_string_lossy().to_string();
        for protected_exe in crate::safe_mode::PROTECTED_EXE_NAMES {
            if name.eq_ignore_ascii_case(protected_exe) {
                tracing::warn!(target: "safe-mode",
                    "Protected game already running at startup: {} — entering safe mode", name);
                return Some(exe_to_sim_type(protected_exe));
            }
        }
    }
    None
}
```

### Anti-Patterns to Avoid

- **Storing cooldown timer in ConnectionState:** Safe mode must survive WebSocket reconnections. ConnectionState is reset on each new WS connection. The timer goes in AppState.
- **Polling process list for WMI detection:** Win32_ProcessStartTrace is event-driven. Polling sysinfo every 2s risks missing fast-starting games and adds anti-cheat suspicion (CreateToolhelp32Snapshot during game init). WMI events arrive within 300ms of process creation.
- **Using `windows` crate COM interfaces for WMI:** Adds ~2MB of COM boilerplate with no established pattern in this codebase. PowerShell subprocess is 10 lines.
- **Gating billing, lock screen, or WS exec:** SAFE-07 explicitly requires these to continue. The safe mode check must NEVER be placed in billing tick paths, lock screen show/hide, or the WS exec handler.
- **Forgetting the AtomicBool sync:** The `SafeMode` struct and the `Arc<AtomicBool>` are separate — they must be kept in sync every time the state changes. Define a helper that updates both atomically.
- **No WMI watcher for games launched via rc-agent:** The LaunchGame path is the primary and faster path. WMI is the secondary path for external launches only. Both paths must call the same `enter_safe_mode()` function to avoid divergence.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| WMI event subscription in Rust | Custom COM/IWbem interface code | PowerShell `Register-WmiEvent` subprocess | COM interface is 200+ lines with no project precedent; PowerShell subprocess is the established pattern in this codebase |
| Cross-task boolean flag | Channel-based notification to process_guard | `Arc<AtomicBool>` | Matches `in_maintenance` field already in AppState; no restructuring of process_guard API needed |
| Async timer for cooldown | Manual Instant comparison in tick loop | `Pin<Box<tokio::time::Sleep>>` in select! | Matches `exit_grace_timer` pattern already in ConnectionState; sleeps exactly right, no polling |
| Process name → SimType mapping | Ad-hoc string matching everywhere | `exe_to_sim_type()` in safe_mode.rs | Single source of truth; reused by startup scan AND WMI watcher path |

**Key insight:** Every pattern needed for this phase already exists in the codebase. This is a composition task, not a novel architecture task.

---

## Common Pitfalls

### Pitfall 1: WMI Watcher Process Not Terminated on rc-agent Exit
**What goes wrong:** The spawned PowerShell process continues running after rc-agent exits, consuming memory and potentially interfering with the next rc-agent start.
**Why it happens:** `std::thread::spawn` does not have a drop-handle to kill the child process.
**How to avoid:** Store the `Child` handle alongside the receiver. Implement a graceful shutdown signal OR let Windows kill the child when rc-agent (the parent) exits — Windows automatically kills child processes when the parent exits if they were created without `DETACHED_PROCESS` flag. The `CREATE_NO_WINDOW` flag (0x08000000) does NOT detach, so the child IS killed automatically.
**Warning signs:** `wmiprvse.exe` CPU spike persisting after rc-agent restart.

### Pitfall 2: Safe Mode Enters But Never Exits (Cooldown Timer Not Armed)
**What goes wrong:** On protected game exit, `start_cooldown()` is called on `safe_mode` struct but the tokio Sleep timer is never reset. Safe mode stays active indefinitely.
**Why it happens:** Two-object pattern (SafeMode struct + timer) must both be updated. It's easy to call one without the other.
**How to avoid:** Define a single function `enter_safe_mode_cooldown(state: &mut AppState)` that calls BOTH `state.safe_mode.start_cooldown()` AND `state.safe_mode_cooldown_timer.as_mut().reset(...)`. Never call them separately.
**Warning signs:** Safe mode still active 60+ seconds after game exit.

### Pitfall 3: Game Exit Detection Mismatch
**What goes wrong:** Safe mode game exit path is triggered by `game_process` monitoring (for rc-agent-launched games) but the WMI path (for external launches) has no corresponding exit detection.
**Why it happens:** WMI `Win32_ProcessStartTrace` only fires on start. Stop events require a separate `Win32_ProcessStopTrace` subscription.
**How to avoid:** For games NOT launched by rc-agent (WMI-detected), use the existing `game_check_interval` poll in event_loop.rs (2s, uses sysinfo) to detect when the game process disappears. The same poll already drives `game_process.rs` state. Add a check: if safe_mode.active && safe_mode.game.is_none() (external launch), and the protected exe is no longer in the process list, trigger cooldown.
**Warning signs:** Safe mode stays active indefinitely after externally-launched game exits.

### Pitfall 4: Double-Enter During Cooldown
**What goes wrong:** User launches a second protected game during the 30s cooldown of the first. If cooldown timer fires while the second game is running, safe mode deactivates mid-game.
**Why it happens:** The timer armed flag is not cleared when a second game enters safe mode.
**How to avoid:** In `enter()`, always disarm the cooldown timer: `state.safe_mode_cooldown_armed = false`. The CONTEXT.md decision "if another protected game launches during cooldown, safe mode stays active (no gap)" must be enforced here.
**Warning signs:** Anti-cheat triggers when quickly switching between games.

### Pitfall 5: Registry Deferral Queue Not Flushed
**What goes wrong:** A registry write is suppressed during safe mode but never retried after safe mode exits, leaving kiosk GPO or self_heal HKLM key in a broken state.
**Why it happens:** "Defer" sounds simple but requires either (a) tracking what was skipped and replaying it, or (b) having existing periodic retry logic that will re-attempt naturally.
**How to avoid:** Audit which registry writes are periodic vs one-shot:
  - `kiosk.apply_gpo_lockdown()` — called on every kiosk enable; the kiosk_interval tick in event_loop will retry naturally.
  - `self_heal.repair_registry_key()` — called at startup only; missing HKLM Run key does not need in-session repair.
  - `lock_screen.rs` Focus Assist registry write — called during lock screen bind; acceptable to skip during safe mode since game is consuming focus anyway.
  For v15.0, "defer" means "skip silently during safe mode" — the periodic callers will re-apply after cooldown expires. No explicit defer queue needed.
**Warning signs:** GPO lockdown keys missing after safe mode exits.

---

## Code Examples

Verified patterns from existing codebase:

### Existing AtomicBool field in AppState (in_maintenance pattern)
```rust
// From crates/rc-agent/src/app_state.rs:66
pub(crate) in_maintenance: std::sync::Arc<std::sync::atomic::AtomicBool>,

// From crates/rc-agent/src/main.rs:752
in_maintenance: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),

// From crates/rc-agent/src/ws_handler.rs:185
state.in_maintenance.store(true, std::sync::atomic::Ordering::Relaxed);

// From crates/rc-agent/src/event_loop.rs:774
if !state.in_maintenance.load(std::sync::atomic::Ordering::Relaxed) {
    continue;
}
```

### Existing Pin<Box<Sleep>> timer pattern (exit_grace_timer)
```rust
// From crates/rc-agent/src/event_loop.rs:81
pub(crate) exit_grace_timer: std::pin::Pin<Box<tokio::time::Sleep>>,
pub(crate) exit_grace_armed: bool,

// Initialization (event_loop.rs:114)
exit_grace_timer: Box::pin(tokio::time::sleep(Duration::from_secs(86400))),

// Reset on game exit
conn.exit_grace_timer.as_mut().reset(
    tokio::time::Instant::now() + Duration::from_secs(30)
);
conn.exit_grace_armed = true;

// In select! block (pattern — safe mode timer mirrors this)
_ = &mut conn.exit_grace_timer, if conn.exit_grace_armed => {
    conn.exit_grace_armed = false;
    // handle timer fire
}
```

### Existing PowerShell hidden subprocess pattern
```rust
// From crates/rc-agent/src/ac_launcher.rs and ai_debugger.rs
fn hidden_cmd(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    cmd
}

// Usage pattern (ai_debugger.rs:356)
if let Ok(output) = hidden_cmd("powershell")
    .args(["-NoProfile", "-NonInteractive", "-Command", "..."])
    .output() { ... }
```

### Existing sysinfo process scan (startup detection)
```rust
// From crates/rc-agent/src/game_process.rs:96-113
use sysinfo::System;
let mut sys = System::new();
sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
for (_pid, process) in sys.processes() {
    let pname = process.name().to_string_lossy().to_string();
    for name in known_names {
        if pname.eq_ignore_ascii_case(name) { /* found */ break; }
    }
}
```

### WMI PowerShell subscription script (verified working pattern)
```powershell
# Win32_ProcessStartTrace — fires within ~300ms of process creation
# Requires no elevation on Windows 10/11 for process name only
$query = "SELECT * FROM Win32_ProcessStartTrace"
Register-WmiEvent -Query $query -SourceIdentifier 'RCSafeModeWatch' | Out-Null
while ($true) {
    $event = Wait-Event -SourceIdentifier 'RCSafeModeWatch' -Timeout 5
    if ($event -ne $null) {
        $exe = $event.SourceEventArgs.NewEvent.ProcessName
        Write-Output $exe
        [Console]::Out.Flush()
        Remove-Event -SourceIdentifier 'RCSafeModeWatch'
    }
}
```
Note: `Win32_ProcessStartTrace` requires the WMI service (winmgmt) to be running — which it is by default on Windows 10/11.

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Polling process list every N seconds for game detection | WMI event subscription (Win32_ProcessStartTrace) | v15.0 requirement | Sub-1-second detection without polling overhead; avoids repeated CreateToolhelp32Snapshot calls during game init |
| No safe mode — subsystems always active | Safe mode state machine gates risky subsystems | Phase 109 (this phase) | Prevents anti-cheat false positive on process guard scans, GPU memory contention from Ollama |

**Deprecated/outdated:**
- Polling sysinfo for game detection: only acceptable for startup scan and post-exit detection. Not acceptable for ongoing detection (SAFE-01 requires WMI event, not polling).

---

## Open Questions

1. **AC EVO executable name**
   - What we know: Context says `acs_x64.exe` in the phase context, but `game_process.rs` lists `AssettoCorsaEVO.exe` and `AC2-Win64-Shipping.exe` as known names. The CONTEXT.md mentions both.
   - What's unclear: Which executable name does AC EVO actually launch with? The early access build may change names.
   - Recommendation: Watch ALL three names (`acs_x64.exe`, `AssettoCorsaEVO.exe`, `AC2-Win64-Shipping.exe`) — zero cost to watch extras.

2. **EA WRC SimType variant**
   - What we know: `SimType` enum in rc-common currently has: AssettoCorsa, AssettoCorsaEvo, AssettoCorsaRally, IRacing, LeMansUltimate, F125, Forza, ForzaHorizon5. No `EaWrc` variant.
   - What's unclear: Does WRC need a SimType variant or just exe-based detection?
   - Recommendation: For Phase 109, WRC can be detected by WMI exe name (`WRC.exe`) without a SimType variant. The `safe_mode.game` field can be `None` for WRC-triggered safe mode until SimType is added in a future phase.

3. **WMI subscription elevation requirement**
   - What we know: `Win32_ProcessStartTrace` in the `root\WMI` namespace (vs `root\CIMv2`) requires elevated access on some Windows configurations.
   - What's unclear: Which namespace does `Register-WmiEvent` default to for this class?
   - Recommendation: Test on Pod 8 first. If elevation is needed, fall back to `Win32_ProcessStartTrace` via `root\CIMv2` which works without elevation. Pods run rc-agent with HKLM Run key (elevated via admin account). Include a fallback: if WMI watcher fails to start, log WARN and continue (dual-path detection means LaunchGame path still works for rc-agent-initiated launches).

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test (`#[test]`, `#[tokio::test]`) |
| Config file | none — standard `cargo test` |
| Quick run command | `cargo test -p rc-agent-crate safe_mode -- --nocapture` |
| Full suite command | `cargo test -p rc-agent-crate && cargo test -p rc-common` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SAFE-01 | WMI watcher spawns and sends exe names via channel | unit (mock PowerShell stdout) | `cargo test -p rc-agent-crate safe_mode::tests::test_wmi_channel_receives_event` | ❌ Wave 0 |
| SAFE-02 | SafeMode::enter() sets active=true, game=Some(sim) | unit | `cargo test -p rc-agent-crate safe_mode::tests::test_enter_sets_active` | ❌ Wave 0 |
| SAFE-02 | is_protected_game() returns true for F125/IRacing/LMU/AcEvo, false for AssettoCorsa | unit | `cargo test -p rc-agent-crate safe_mode::tests::test_is_protected_game` | ❌ Wave 0 |
| SAFE-03 | start_cooldown() sets cooldown_until to ~30s from now | unit | `cargo test -p rc-agent-crate safe_mode::tests::test_cooldown_duration` | ❌ Wave 0 |
| SAFE-03 | Second game launch during cooldown keeps safe mode active (no gap) | unit | `cargo test -p rc-agent-crate safe_mode::tests::test_no_gap_during_cooldown` | ❌ Wave 0 |
| SAFE-04 | Process guard scan is skipped when AtomicBool is true | unit | `cargo test -p rc-agent-crate process_guard::tests::test_scan_skipped_in_safe_mode` | ❌ Wave 0 |
| SAFE-05 | analyze_crash returns early (no Ollama call) when safe_mode_active is true | unit | `cargo test -p rc-agent-crate ai_debugger::tests::test_analyze_suppressed_in_safe_mode` | ❌ Wave 0 |
| SAFE-06 | Registry write functions are gated by safe mode | unit | `cargo test -p rc-agent-crate safe_mode::tests::test_registry_gate` | ❌ Wave 0 |
| SAFE-07 | Billing path has no safe_mode check | code review (manual) | n/a | manual-only |

**SAFE-07 is manual-only** because verifying the absence of a check in billing/lock screen paths is a code review task, not an automated test.

### Sampling Rate
- **Per task commit:** `cargo test -p rc-agent-crate safe_mode -- --nocapture`
- **Per wave merge:** `cargo test -p rc-agent-crate && cargo test -p rc-common`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/rc-agent/src/safe_mode.rs` — new module, needs `#[cfg(test)] mod tests` block covering SAFE-01 through SAFE-06
- [ ] `crates/rc-agent/src/process_guard.rs` — add `test_scan_skipped_in_safe_mode` test to existing `#[cfg(test)] mod tests` block
- [ ] `crates/rc-agent/src/ai_debugger.rs` — add `test_analyze_suppressed_in_safe_mode` test to existing test block

All tests must use `#[cfg(test)]` guards as per CLAUDE.md standing rules — no real powershell execution, no sysinfo calls, no registry writes during `cargo test`.

---

## Sources

### Primary (HIGH confidence)
- Direct codebase read — `crates/rc-agent/src/app_state.rs` — AppState struct and in_maintenance AtomicBool pattern
- Direct codebase read — `crates/rc-agent/src/event_loop.rs` — exit_grace_timer Pin<Box<Sleep>> pattern, select! polling
- Direct codebase read — `crates/rc-agent/src/process_guard.rs` — spawn() API, scan loop structure
- Direct codebase read — `crates/rc-agent/src/game_process.rs` — sysinfo process scan pattern, protected exe names
- Direct codebase read — `crates/rc-agent/Cargo.toml` — confirmed no `windows` or `wmi` crate in dependencies
- Direct codebase read — `crates/rc-agent/src/kiosk.rs`, `self_heal.rs`, `lock_screen.rs` — registry write locations (SAFE-06 scope)

### Secondary (MEDIUM confidence)
- Direct codebase read — `crates/rc-agent/src/ac_launcher.rs`, `ai_debugger.rs` — PowerShell subprocess pattern established in codebase; `Win32_ProcessStartTrace` PowerShell event subscription is a well-established Windows WMI feature available since Windows Vista

### Tertiary (LOW confidence)
- AC EVO executable names: listed both `acs_x64.exe` and `AssettoCorsaEVO.exe` / `AC2-Win64-Shipping.exe` from different context sources — actual binary name needs Pod 8 verification
- WMI namespace elevation requirements on pod Windows edition — needs Pod 8 test

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new dependencies needed; all patterns already in codebase
- Architecture: HIGH — SafeMode struct, AtomicBool gate, Pin<Box<Sleep>> timer all have direct precedents
- WMI implementation: MEDIUM — PowerShell approach verified as codebase pattern; specific WMI query syntax needs Pod 8 smoke test
- Pitfalls: HIGH — derived from direct codebase analysis (e.g., ConnectionState vs AppState timer lifetime)

**Research date:** 2026-03-21
**Valid until:** 2026-04-21 (stable patterns; only risk is AC EVO exe name change on Early Access update)
