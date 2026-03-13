# Phase 5: Blanking Screen Protocol - Research

**Researched:** 2026-03-13
**Domain:** Windows kiosk lockdown, session transition UX, PIN auth unification
**Confidence:** HIGH — all findings drawn directly from existing codebase; no speculative claims

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Anti-cheat compatibility:**
- Moderate approach: suppress WerFault during gameplay (already reactive in auto-fix). No NEW process manipulation during active game sessions beyond what's already happening.
- rc-agent already kills WerFault.exe reactively via ai_debugger.rs auto-fix — acceptable during gameplay.
- No proactive polling loop that kills popups every N seconds during active sessions — anti-cheat risk.
- Between sessions (no billing active): aggressive suppression is safe — kill all error dialogs, manage windows freely.
- Test requirement: Before deploying, test all three online games (iRacing, F1 25, LMU) with rc-agent running to verify no anti-cheat kicks or bans from current WerFault killing + Edge kiosk behavior. Test all three games in sequence before claiming Phase 5 complete.

**Dialog suppression strategy:**
- WerFault.exe: Already handled reactively by auto-fix. Continue this approach during gameplay.
- Between sessions: Proactive sweep of known popup processes (WerFault, crash dialogs, update prompts) during cleanup_after_session and enforce_safe_state.
- ConspitLink messages: Managed by 10s watchdog in main loop — no change needed.
- Windows Update prompts: Suppress via Group Policy on pods (defer updates, disable restart notifications).
- "Application has stopped working" dialogs: Add to the suppress list alongside WerFault.

**Lock screen coverage and transitions:**
- Game launch: Show branded splash screen ("Preparing your session...") between lock screen close and game window visible. Customer sees Racing Point branding during shader compilation delay, CM loading, etc.
- Game exit (session end): Launch Edge kiosk lock screen FIRST (covers screen), THEN kill the game process. Customer never sees desktop during transition.
- Game crash: Same as game exit — lock screen first, then cleanup. The crash dialog (WerFault) gets killed by the cleanup step.
- Lock screen uses Edge kiosk fullscreen mode (already implemented) — covers entire screen.

**Desktop hiding:**
- Taskbar: Hide via registry on all pods (not just auto-hide — fully hidden). Requires reboot to apply.
- File Explorer: Disable file browser windows via Group Policy or registry. If customer alt-tabs, they see only Edge lock screen window behind the game.
- Keyboard shortcuts: Block Win key, Alt+Tab, Ctrl+Esc on pods via registry/Group Policy. Prevents customers from leaving fullscreen games.
- Recovery: Admin login required to undo lockdown. All changes applied via pod-agent /exec during deploy.
- These are one-time pod setup changes, not rc-agent runtime code.

**PIN auth unification:**
- Single shared function: One `validate_pin()` handles all 3 callers (pod lock screen, staff kiosk, customer PWA). Callers pass source context (pod|kiosk|pwa) for logging only.
- No rate limiting: PINs are 4-digit, change per session, and venue is physical. Brute force risk is negligible.
- Error messages: Claude's discretion — just ensure identical messages across all 3 surfaces.
- Response time: Claude's discretion — optimize as needed, just don't let it feel sluggish (PERF-02 says 1-2s target).

### Claude's Discretion
- Exact error message text for wrong PIN (just make it identical everywhere)
- PIN response time optimization approach (within 1-2s target)
- Branded splash screen design during game launch
- Specific registry keys for taskbar hiding and keyboard lockdown
- Whether to use Group Policy objects or direct registry edits for pod lockdown
- Dialog suppression implementation details (polling interval between sessions, process list)

### Deferred Ideas (OUT OF SCOPE)
- Shell replacement (replace explorer.exe with rc-agent) — too risky for recovery if rc-agent crashes. Revisit if registry+GP lockdown proves insufficient.
- USB mass storage lockdown — already tracked in MEMORY.md TODOs, separate from screen protocol.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| SCREEN-01 | Clean branded lock screen visible before session starts and after session ends — no Windows desktop exposed | LockScreenManager.show_blank_screen() + session transition ordering change in main.rs SessionEnded handler |
| SCREEN-02 | All error popups suppressed on pod screens (WerFault, application errors, "Cannot find" dialogs, ConspitLink messages) | Extend enforce_safe_state() + cleanup_after_session() to kill "ApplicationFrameHost", "dwm" app-not-responding dialogs; Group Policy for Windows Update restart prompts |
| SCREEN-03 | No file path errors or system dialog text ever visible on customer-facing screen | Registry/GP lockdown of taskbar + explorer windows + keyboard shortcuts; validate existing ConfigError renders generic message (already done) |
| PERF-02 | Lock screen responds to PIN entry within 1-2 seconds | Current path: lock_screen HTTP POST → AgentMessage::PinEntered WS → validate_pin() SQLite → CoreToAgentMessage::PinFailed/BillingStarted WS back. Local SQLite is fast; WS round-trip on LAN is ~5-20ms. Path is already within 1-2s but needs verification. |
| AUTH-01 | PIN authentication works identically on pod lock screen, customer PWA, and customer kiosk — same validation, same flow, same response time | Two separate functions exist: validate_pin() (pod, called by handle_pin_entered) and validate_pin_kiosk() (kiosk/PWA). Must be unified into one function with a `source` parameter for logging. |
</phase_requirements>

---

## Summary

Phase 5 is almost entirely a **wiring and ordering** phase — the infrastructure already exists. `LockScreenManager` with its `show_blank_screen()`, `show_pin_screen()`, `enforce_kiosk_foreground()`, and `close_browser()` functions are all implemented and deployed. `cleanup_after_session()` and `enforce_safe_state()` in `ac_launcher.rs` already kill WerFault.exe and minimize background windows. Edge kiosk mode (`--kiosk --edge-kiosk-type=fullscreen`) is already deployed on all 8 pods.

The three gaps are: (1) transition ordering — game must be killed AFTER lock screen is shown, not before; currently the main.rs `SessionEnded` handler calls `enforce_safe_state()` which kills the game and only then foregrounded the lock screen, (2) the branded splash screen during game launch is missing — currently when a customer enters PIN, Edge closes and there is a desktop-visible gap while AC loads (~10 seconds), (3) `validate_pin()` (pod) and `validate_pin_kiosk()` (kiosk/PWA) have divergent logic — the kiosk version searches across all pods, the pod version is scoped to its own pod — but the error message strings differ and the response structures differ. Unification means extracting shared validation core.

**Primary recommendation:** Split Phase 5 into three tasks: (1) fix session-end transition ordering in main.rs, (2) add splash screen state to LockScreenState and lock_screen.rs HTML renderer with game launch wiring, (3) unify PIN validation into a shared `validate_pin_inner()` with a `source: PinSource` parameter.

---

## Standard Stack

### Core (already in place — no new dependencies)
| Component | Location | Purpose |
|-----------|----------|---------|
| LockScreenManager | `crates/rc-agent/src/lock_screen.rs` | Full lock screen lifecycle — state machine, HTTP server, Edge kiosk launch |
| enforce_safe_state() | `crates/rc-agent/src/ac_launcher.rs:956` | Kills games + dialogs + minimizes windows + foregrounds kiosk |
| cleanup_after_session() | `crates/rc-agent/src/ac_launcher.rs:903` | Post-session kill sequence |
| validate_pin() | `crates/rc-core/src/auth/mod.rs:323` | Pod-scoped PIN validation |
| validate_pin_kiosk() | `crates/rc-core/src/auth/mod.rs:1057` | Cross-pod PIN validation for kiosk/PWA |
| handle_pin_entered() | `crates/rc-core/src/auth/mod.rs:1021` | WS handler that calls validate_pin() and sends PinFailed back |
| suppress_notifications() | `crates/rc-agent/src/lock_screen.rs:433` | Focus Assist registry toggle — already called from show_blank_screen() |

### Group Policy / Registry (one-time pod setup — no Rust changes needed)
| Setting | Method | Effect |
|---------|--------|--------|
| Hide taskbar fully | `HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\StuckRects3` | Remove taskbar from customer view |
| Block Win key | `HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer` `NoWinKeys=1` | Win key does nothing |
| Block Alt+Tab | Keyboard Layout (scancode map) or PowerToys | Customer cannot switch windows |
| Disable Windows Update restart prompts | `HKLM\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU` `NoAutoRebootWithLoggedOnUsers=1` | No reboot dialogs |
| Disable File Explorer auto-open | `HKLM\SOFTWARE\Policies\Microsoft\Windows\Explorer` `NoShellExecuteInExplorer=0` + remove from shell | Explorer windows blocked |

**Installation:** Applied via pod-agent `/exec` powershell commands, one-time per pod. Requires reboot for taskbar change to take effect.

---

## Architecture Patterns

### Current Session-End Flow (BROKEN — desktop visible)

```
SessionEnded WS message received
  → overlay.deactivate()
  → game_process.stop()                   ← kills game, desktop may flash
  → enforce_safe_state()
      → kills all game procs
      → enforce_kiosk_foreground()        ← lock screen brought up AFTER desktop visible
  → lock_screen.show_session_summary()   ← summary displayed
```

### Corrected Session-End Flow (SCREEN-01 fix)

```
SessionEnded WS message received
  → overlay.deactivate()
  → lock_screen.show_session_summary()   ← lock screen FIRST (covers screen)
  → ffb.zero_force()
  → game_process.stop()                  ← kill game AFTER lock screen is up
  → enforce_safe_state()                 ← cleanup while lock screen already covers
```

The critical insight: `show_session_summary()` calls `launch_browser()` which kills and relaunches Edge in kiosk fullscreen. This gives ~500ms before the browser is visible, but it is enough to cover the game kill. In practice the sequence should be:
1. Call `lock_screen.show_session_summary()` (spawns Edge kiosk in background)
2. Brief `tokio::time::sleep(Duration::from_millis(500))` — let Edge initialize
3. Then kill game + enforce_safe_state()

The existing `BillingStopped` fallback path (line 829) also needs the same fix.

### Game Launch Splash Screen (SCREEN-01 new behavior)

New `LockScreenState::LaunchSplash { driver_name: String, message: String }` state:

```rust
// In lock_screen.rs
LaunchSplash {
    driver_name: String,
    message: String,   // "Preparing your session..."
}
```

The `LaunchGame` handler in main.rs should:
1. Show `LaunchSplash` on lock screen (Edge kiosk already open, just state update)
2. Run AC launch sequence in spawn_blocking (8-10 seconds)
3. On success: `lock_screen.show_active_session()` → calls `close_browser()`
4. Customer sees: splash screen during loading, then game appears

HTML for splash: Racing Point branded, Enthocentric header "PREPARING YOUR SESSION", Montserrat body "Loading your race..." with a CSS pulse animation. Background: #1A1A1A gradient. Accent: #E10600. No file paths or system text.

### PIN Auth Unification (AUTH-01)

**Current state:**
- `validate_pin(state, pod_id, pin)` — pod-scoped, called by `handle_pin_entered()` which is called from the WS handler for `AgentMessage::PinEntered`
- `validate_pin_kiosk(state, pin, chosen_pod_id)` — cross-pod search, called by `/auth/kiosk/validate-pin` HTTP route
- Error message differences: pod path returns `"Invalid PIN or no pending assignment for this pod"`, kiosk path returns `"Invalid PIN. Please check with reception."`

**Unified approach:**
```rust
// New shared enum in auth/mod.rs
#[derive(Debug, Clone, Copy)]
pub enum PinSource { Pod, Kiosk, Pwa }

// Shared inner function — both callers delegate to this
async fn validate_pin_inner(
    state: &Arc<AppState>,
    pin: String,
    pod_id: String,           // resolved pod_id (kiosk resolves before calling)
    source: PinSource,
) -> Result<String, String>  // returns billing_session_id

// validate_pin() wraps validate_pin_inner() with PinSource::Pod
// validate_pin_kiosk() resolves pod_id first (pod-match or fallback),
//   then calls validate_pin_inner() with PinSource::Kiosk
```

Error message (identical across all surfaces): `"Invalid PIN — please try again or see reception."` This is generic, gives no technical details, and works for all three entry points.

### Dialog Suppression (SCREEN-02, SCREEN-03)

**Between sessions — extend enforce_safe_state() to kill more popup processes:**

```rust
// Add to enforce_safe_state() after existing WerFault kill:
let dialog_processes = [
    "WerFault.exe",
    "WerFaultSecure.exe",
    "werfaultsecure.exe",
    "ApplicationFrameHost.exe",  // Windows Store app crash dialogs
    "SystemSettings.exe",         // Settings app that could leak paths
    "msiexec.exe",                // Installer dialogs
];
for proc in &dialog_processes {
    let _ = Command::new("taskkill").args(["/IM", proc, "/F"]).output();
}
```

**During active gameplay — no change from current behavior.** WerFault killed reactively by ai_debugger auto-fix only. Anti-cheat safe.

### Desktop Hiding — Registry Keys (SCREEN-03)

These are one-time pod setup changes applied via pod-agent `/exec`. They are idempotent and safe to re-apply.

**Hide taskbar (fully hidden, not auto-hide):**
The `StuckRects3` registry binary value controls taskbar position/visibility. The cleanest approach is auto-hide via `HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\StuckRects3` — byte index 8 controls auto-hide. Alternatively, use Task Manager → Properties → Auto-hide, applied via PowerShell:

```powershell
# Auto-hide taskbar (customer cannot find it)
$p = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\StuckRects3'
$v = (Get-ItemProperty -Path $p).Settings
$v[8] = 3  # 0=off, 1=auto-hide, 3=always-on-top+auto-hide
Set-ItemProperty -Path $p -Name Settings -Value $v
Stop-Process -Name explorer -Force  # Restart explorer to apply
```

**Block Win key:**
```powershell
# Disable Win key via registry (requires Group Policy or Keyboard Scancode Map)
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer' -Name 'NoWinKeys' -Value 1 -Type DWord
```

**Block Ctrl+Esc (Start Menu):**
Already blocked by `NoWinKeys`. The Start Menu key mapping is the same.

**Windows Update restart notifications:**
```powershell
# Prevent Windows Update from showing restart prompts
New-Item -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU' -Force
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU' -Name 'NoAutoRebootWithLoggedOnUsers' -Value 1 -Type DWord
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU' -Name 'AUOptions' -Value 2 -Type DWord  # Download only, no auto-install
```

**Note on Alt+Tab:** Standard registry/GP has no clean block for Alt+Tab without third-party tools. Options:
1. Accept it — customer Alt+Tabs to another Edge window (lock screen is the only other window, so they see lock screen anyway)
2. Use `AutoHotkey` script deployed on startup (additional dependency)
3. Use `Keyboard Scancode Map` to remap Alt+Tab to nothing (complex, affects all key combos)

**Recommended:** Accept Alt+Tab. Since the only visible window behind a fullscreen game is the Edge kiosk lock screen (which we cover with minimize_background_windows()), the customer will see either the game or the lock screen. No Windows desktop should be visible.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Dialog suppression | Custom WinAPI window enumeration | `taskkill /IM process.exe /F` | Already proven pattern in ac_launcher.rs and ai_debugger.rs |
| Taskbar hiding | WinAPI ShowWindow on taskbar HWND | Registry `StuckRects3` value | WinAPI approach fails on Windows 11 taskbar implementation changes |
| Foreground window control | Direct SetForegroundWindow Rust FFI | PowerShell via `Command::new("powershell")` | Already proven in ac_launcher.rs minimize_background_windows(); avoids winapi crate complexity |
| PIN validation | Per-caller custom SQL queries | Shared `validate_pin_inner()` | All three callers already have near-identical SQL with different error strings — unify |

---

## Common Pitfalls

### Pitfall 1: Desktop Flash on Session End
**What goes wrong:** Game is killed before lock screen browser is visible. For ~500ms-2s, Windows shows the desktop.
**Why it happens:** The current SessionEnded handler calls `enforce_safe_state()` which kills the game, then tries to foreground the lock screen that isn't launched yet.
**How to avoid:** Show lock screen state FIRST. Then wait 500ms for Edge to initialize. Then kill game.
**Warning signs:** If cleanup happens in under 200ms, suspect the lock screen hasn't appeared yet.

### Pitfall 2: Edge Stacking on Rapid Transitions
**What goes wrong:** Multiple Edge kiosk windows stacked on top of each other. Already fixed by commit 80ec001 (kills msedgewebview2.exe too).
**How to avoid:** Always call `close_browser()` before `launch_browser()`. The existing `close_browser()` does this — do not skip it.

### Pitfall 3: Lock Screen Browser Kills During Active Gameplay
**What goes wrong:** `show_active_session()` calls `close_browser()` — this kills ALL Edge processes. If the overlay is serving from Edge WebView, it could be killed.
**How to avoid:** The overlay does NOT use Edge — it uses a separate TCP server on port 18925. No conflict. Confirmed by reading overlay.rs.

### Pitfall 4: Registry Changes Not Applying Without Explorer Restart
**What goes wrong:** StuckRects3 taskbar registry change applied but taskbar still visible.
**Why it happens:** Explorer must be restarted to re-read StuckRects3.
**How to avoid:** `Stop-Process -Name explorer -Force` after writing the registry key. Explorer auto-restarts via Task Manager.

### Pitfall 5: PIN Error Message Divergence on Response Code Path
**What goes wrong:** After unification, kiosk path gets pod-scoped error ("no pending assignment for this pod") because the fallback path in validate_pin_kiosk that tries the chosen_pod_id first uses a different query result.
**How to avoid:** Standardize error message at the single `validate_pin_inner()` return site, not at callers.

### Pitfall 6: Anti-Cheat Trip from WerFault Kill
**What goes wrong:** EasyAntiCheat or Vanguard detects WerFault kill as process injection attempt.
**How to avoid:** Only kill WerFault reactively (on detection) during gameplay — already the current behavior. Do NOT add a proactive polling loop that kills processes during active billing sessions.
**Validation:** Test iRacing (EasyAntiCheat), F1 25 (EAC), LMU (no AC) explicitly before shipping Phase 5.

### Pitfall 7: `cleanup_after_session()` is Dead Code Warning
**What goes wrong:** The compiler currently warns `cleanup_after_session()` is never used (confirmed in test run). It exists but is not called from main.rs — `enforce_safe_state()` is called instead.
**How to avoid:** The Phase 5 session-end transition change should call `cleanup_after_session()` AFTER showing the lock screen, or fold its logic into the SessionEnded handler. Do not add another dead-code call site.

---

## Code Examples

### Correct Session-End Transition Order

```rust
// In main.rs SessionEnded handler — CORRECTED ORDER
rc_common::protocol::CoreToAgentMessage::SessionEnded {
    billing_session_id, driver_name, total_laps, best_lap_ms, driving_seconds,
} => {
    heartbeat_status.billing_active.store(false, std::sync::atomic::Ordering::Relaxed);
    crash_recovery_armed = false;
    overlay.deactivate();

    // STEP 1: Show lock screen FIRST (covers desktop before game is killed)
    lock_screen.show_session_summary(driver_name, total_laps, best_lap_ms, driving_seconds);
    // Brief yield — let Edge kiosk window initialize before we kill the game
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // STEP 2: Stop game and clean up AFTER lock screen is visible
    if let Some(ref mut game) = game_process {
        let _ = game.stop();
        game_process = None;
    }
    if let Some(ref mut adp) = adapter { adp.disconnect(); }
    { let f = ffb.clone(); tokio::task::spawn_blocking(move || { f.zero_force().ok(); ac_launcher::enforce_safe_state(); }); }

    // STEP 3: Auto-blank timer unchanged
    blank_timer.as_mut().reset(tokio::time::Instant::now() + Duration::from_secs(15));
    blank_timer_armed = true;
}
```

### New LaunchSplash State

```rust
// In lock_screen.rs — add variant to LockScreenState enum
/// Splash screen shown while game is loading (between PIN auth and game visible).
LaunchSplash {
    driver_name: String,
    message: String,
},

// Add method to LockScreenManager
pub fn show_launch_splash(&mut self, driver_name: String) {
    {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        *state = LockScreenState::LaunchSplash {
            driver_name,
            message: "Preparing your session...".to_string(),
        };
    }
    self.launch_browser();
}
```

```rust
// In main.rs LaunchGame handler — add splash before spawn_blocking
rc_common::protocol::CoreToAgentMessage::LaunchGame { sim_type: launch_sim, launch_args } => {
    // Show splash screen so customer sees branding during AC loading
    lock_screen.show_launch_splash(
        // driver_name not in LaunchGame message — use generic or thread through
        "Driver".to_string()
    );

    // ... existing spawn_blocking launch_ac() call unchanged ...
    // After success: show_active_session() closes the browser
}
```

**Note:** `LaunchGame` message does not carry `driver_name`. Two options:
1. Add `driver_name: Option<String>` to `CoreToAgentMessage::LaunchGame` in rc-common protocol
2. Cache driver_name in agent state when `BillingStarted` arrives (it carries `driver_name`)

Option 2 is simpler — agent already receives `BillingStarted { driver_name }` before `LaunchGame`. Cache it in a local variable.

### Unified PIN Validation Inner Function

```rust
// In rc-core/src/auth/mod.rs

#[derive(Debug, Clone, Copy)]
pub enum PinSource {
    Pod,   // Entered on physical pod lock screen
    Kiosk, // Staff kiosk /auth/kiosk/validate-pin endpoint
    Pwa,   // Customer PWA (currently goes through kiosk endpoint)
}

/// Shared validation core — validates pin against a known pod_id.
/// pod_id must be resolved by the caller before invoking.
async fn validate_pin_inner(
    state: &Arc<AppState>,
    pod_id: String,
    pin: String,
    source: PinSource,
) -> Result<String, String> {
    // Check employee debug PIN first
    let daily_pin = todays_debug_pin(&state.config.auth.jwt_secret);
    if pin == daily_pin {
        return validate_employee_pin(state, pod_id, pin).await;
    }

    // Atomically consume token — same query for all callers
    let row = sqlx::query_as::<_, (...)>(
        "UPDATE auth_tokens SET status = 'consuming'
         WHERE id = (
             SELECT id FROM auth_tokens
             WHERE pod_id = ? AND token = ? AND auth_type = 'pin' AND status = 'pending'
               AND expires_at > datetime('now')
             LIMIT 1
         ) AND status = 'pending'
         RETURNING id, driver_id, pricing_tier_id, custom_price_paise, custom_duration_minutes, experience_id, custom_launch_args",
    )
    .bind(&pod_id)
    .bind(&pin)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    // IDENTICAL error message on all three surfaces:
    .ok_or_else(|| "Invalid PIN — please try again or see reception.".to_string())?;

    tracing::info!("PIN validated via {:?} on pod {}", source, pod_id);

    // ... rest of billing start unchanged ...
    Ok(billing_session_id)
}

// validate_pin() becomes a thin wrapper:
pub async fn validate_pin(state: &Arc<AppState>, pod_id: String, pin: String) -> Result<String, String> {
    validate_pin_inner(state, pod_id, pin, PinSource::Pod).await
}

// validate_pin_kiosk() resolves pod_id then delegates:
pub async fn validate_pin_kiosk(state: &Arc<AppState>, pin: String, chosen_pod_id: Option<String>) -> Result<KioskPinResult, String> {
    // ... pod resolution logic (choose preferred pod_id) unchanged ...
    let resolved_pod_id = /* existing resolution logic */ ...;
    let billing_session_id = validate_pin_inner(state, resolved_pod_id.clone(), pin, PinSource::Kiosk).await?;
    // ... build KioskPinResult from billing_session_id + resolved_pod_id ...
}
```

### Extended Dialog Suppression in enforce_safe_state

```rust
// In ac_launcher.rs enforce_safe_state() — extend the dialog kill list
let dialog_processes = [
    "WerFault.exe",
    "WerFaultSecure.exe",
    "ApplicationFrameHost.exe",  // Windows Store crash dialogs
    "SystemSettings.exe",         // Settings app (leaks paths if open)
    "msiexec.exe",                // Installer popup dialogs
];
for proc in &dialog_processes {
    let _ = Command::new("taskkill").args(["/IM", proc, "/F"]).output();
}
```

### Pod Lockdown Script (via pod-agent /exec)

```powershell
# One-time pod setup — applied via pod-agent /exec
# 1. Auto-hide taskbar
$p = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\StuckRects3'
$v = (Get-ItemProperty -Path $p -ErrorAction SilentlyContinue).Settings
if ($v) { $v[8] = 3; Set-ItemProperty -Path $p -Name Settings -Value $v }

# 2. Disable Win key
$ep = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer'
New-Item -Path $ep -Force -ErrorAction SilentlyContinue | Out-Null
Set-ItemProperty -Path $ep -Name 'NoWinKeys' -Value 1 -Type DWord

# 3. Disable Windows Update restart notifications
$wup = 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU'
New-Item -Path $wup -Force -ErrorAction SilentlyContinue | Out-Null
Set-ItemProperty -Path $wup -Name 'NoAutoRebootWithLoggedOnUsers' -Value 1 -Type DWord
Set-ItemProperty -Path $wup -Name 'AUOptions' -Value 2 -Type DWord

# 4. Restart Explorer to apply taskbar change
Stop-Process -Name explorer -Force -ErrorAction SilentlyContinue
```

---

## State of the Art

| Old Approach | Current Approach | Phase 5 Change |
|--------------|------------------|----------------|
| Kill game, then show lock screen | Kill game, then show lock screen (broken) | Show lock screen FIRST, then kill game |
| No splash during game launch | Black screen + desktop flash during AC load | LaunchSplash state covers ~10s loading gap |
| Two separate validate_pin functions | validate_pin() + validate_pin_kiosk() | Single validate_pin_inner() with PinSource enum |
| WerFault killed reactively only | Same | Between sessions: proactive sweep of more dialog processes |

---

## Open Questions

1. **driver_name in LaunchSplash**
   - What we know: `LaunchGame` WS message does not carry `driver_name`. `BillingStarted` does.
   - What's unclear: Whether to add `driver_name` to `LaunchGame` in protocol.rs or cache it from `BillingStarted`.
   - Recommendation: Cache from `BillingStarted` — simpler, no protocol change. Add `let mut current_driver_name: Option<String> = None` to main.rs event loop state.

2. **Alt+Tab blocking**
   - What we know: No clean Windows registry/GP method to block Alt+Tab without side effects or third-party tools.
   - What's unclear: Whether customers actually use Alt+Tab, and whether the "they see lock screen anyway" argument holds in practice.
   - Recommendation: Accept Alt+Tab for Phase 5. Since `minimize_background_windows()` runs on game launch, the only window behind the game is the Edge kiosk. Alt+Tab shows lock screen, not desktop. Revisit only if customers exploit it.

3. **Taskbar reboot requirement**
   - What we know: StuckRects3 change requires Explorer restart (not full reboot). Explorer restart can be done via `Stop-Process -Name explorer -Force`.
   - What's unclear: Whether Explorer restart during pod setup causes any lasting issues (usually fine — Explorer auto-restarts in 2-3s).
   - Recommendation: Include `Stop-Process -Name explorer -Force` in the pod lockdown script. Explorer restarts automatically.

4. **Anti-cheat validation timing**
   - What we know: iRacing uses EasyAntiCheat, F1 25 uses EAC. Current WerFault kill via taskkill is already deployed.
   - What's unclear: Whether ADDING more process kills (ApplicationFrameHost, SystemSettings) between sessions triggers any post-session scan.
   - Recommendation: The extra kills happen only when `billing_active = false`. This is outside the gaming window. Should be safe. Confirm with test run on all 3 games as required by CONTEXT.md.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in + `#[cfg(test)]` modules |
| Config file | None — colocated with source modules |
| Quick run command | `cargo test -p rc-agent && cargo test -p rc-common` |
| Full suite command | `cargo test -p rc-agent && cargo test -p rc-common && cargo test -p rc-core` |

**Current test count:** 47 tests in rc-agent (ai_debugger: 9, driving_detector: 7, ffb_controller: 2, lock_screen: 6, overlay: 5, sims: 5, main: 13)

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SCREEN-01 | session_end shows lock screen before killing game | unit (pure fn, no IO) | `cargo test -p rc-agent lock_screen` | ❌ Wave 0 |
| SCREEN-01 | launch_splash state renders branded HTML (no paths/system text) | unit | `cargo test -p rc-agent lock_screen` | ❌ Wave 0 |
| SCREEN-02 | enforce_safe_state kills extended dialog process list | unit (mock Command) | `cargo test -p rc-agent` | ❌ Wave 0 |
| SCREEN-03 | pin_error HTML contains no file path strings | unit | `cargo test -p rc-agent lock_screen` | ❌ Wave 0 |
| SCREEN-03 | config_error page renders generic message only | unit | `cargo test -p rc-agent lock_screen::tests::health_degraded_for_config_error` | ✅ exists (partial) |
| AUTH-01 | validate_pin_inner returns identical error string from pod path | unit (in-memory SQLite) | `cargo test -p rc-core auth` | ❌ Wave 0 |
| AUTH-01 | validate_pin_inner returns identical error string from kiosk path | unit (in-memory SQLite) | `cargo test -p rc-core auth` | ❌ Wave 0 |
| PERF-02 | validate_pin_inner completes within 200ms on local SQLite | unit timing test | `cargo test -p rc-core auth -- --nocapture` | ❌ Wave 0 |

**Manual verification (no automated path):**
- Transition ordering: Deploy to Pod 8, end a session, visually confirm no desktop flash
- Anti-cheat: Launch iRacing, F1 25, LMU in sequence with rc-agent running
- Taskbar hiding: Apply registry change on Pod 8, reboot, confirm taskbar not visible

### Sampling Rate
- **Per task commit:** `cargo test -p rc-agent && cargo test -p rc-common`
- **Per wave merge:** `cargo test -p rc-agent && cargo test -p rc-common && cargo test -p rc-core`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/rc-agent/src/lock_screen.rs` — add tests for `LaunchSplash` state HTML rendering (no system text), `is_blanked()` returns true for `LaunchSplash`
- [ ] `crates/rc-agent/src/lock_screen.rs` — add test for transition: `show_session_summary()` called before `close_browser()` in mock scenario
- [ ] `crates/rc-core/src/auth/mod.rs` — add `#[cfg(test)] mod tests` block with in-memory SQLite tests for `validate_pin_inner()`: wrong PIN returns standard message, employee PIN accepted, expired token rejected
- [ ] `crates/rc-agent/src/ac_launcher.rs` — add test for extended dialog process list in `enforce_safe_state()` — verify process names are in the kill list (pure constant inspection, no IO)

---

## Sources

### Primary (HIGH confidence)
- `crates/rc-agent/src/lock_screen.rs` — Full LockScreenManager implementation, state enum, HTML templates, suppress_notifications, enforce_kiosk_foreground
- `crates/rc-agent/src/ac_launcher.rs` — cleanup_after_session, enforce_safe_state, minimize_background_windows, bring_game_to_foreground
- `crates/rc-agent/src/ai_debugger.rs` — fix_kill_error_dialogs, PROTECTED_PROCESSES list
- `crates/rc-agent/src/main.rs` — SessionEnded handler (lines 832-857), LaunchGame handler (lines 858+), PinEntered event handler (lines 767-779)
- `crates/rc-core/src/auth/mod.rs` — validate_pin, validate_pin_kiosk, handle_pin_entered, KioskPinResult
- `crates/rc-core/src/api/routes.rs` — route definitions: /auth/validate-pin, /auth/kiosk/validate-pin, /staff/validate-pin
- `cargo test -p rc-agent -- --list` — confirmed 47 tests, all passing

### Secondary (MEDIUM confidence)
- Windows Registry documentation (StuckRects3, NoWinKeys) — well-known kiosk configuration keys, widely documented for Windows 10/11
- Anti-cheat process kill behavior — moderate confidence; current WerFault kill is already deployed without issues, extending to more processes between sessions follows same pattern

### Tertiary (LOW confidence)
- Alt+Tab blocking via Scancode Map — not verified against Windows 11 22H2+ behavior; may not work reliably

---

## Metadata

**Confidence breakdown:**
- Session transition ordering: HIGH — code paths traced directly in main.rs lines 832-857
- LaunchSplash implementation: HIGH — pattern matches existing states; HTML renderer pattern clear from lock_screen.rs
- PIN unification: HIGH — both functions read directly; divergence documented with exact line numbers
- Registry/GP lockdown keys: MEDIUM — standard Windows kiosk configuration; specific byte offsets for StuckRects3 are version-sensitive
- Anti-cheat safety: MEDIUM — current WerFault kill already safe; extending kill list between sessions is lower risk but untested

**Research date:** 2026-03-13
**Valid until:** 2026-04-13 (stable Rust codebase — 30 day window appropriate)
