# Bug B — `minimize_background_windows` minimizes non-AC games

**Discovered:** 2026-04-12 attempt-02 canary trace
**Status:** ROOT CAUSE IDENTIFIED, NOT YET FIXED
**Severity:** HIGH — every non-AC game launch (F1 25, iRacing, LMU, ACE) will have its window minimized ~1-30 seconds after launch by the periodic event_loop tick

---

## How the bug surfaced

The canary fix (commit `29a4d8f1`) successfully fixed Bug 2 (F1 25 launch verifier). With the launch verifier no longer killing F1 25 launches at 180s, the launched F1 25 game window was visible long enough for the next downstream behavior to fire — `minimize_background_windows()` running on a periodic tick from the event_loop kiosk-enforcement path.

**Live evidence from this trace** (`runs/2026-04-12T04-13-17-IST-f1_25-attempt-02-canary/`):

Pod 4 log:
- `22:44:14.047 GAME-07: Game window confirmed for F125 (PID 20308)` — game launched
- `22:44:14.047 LAUNCH-FIX-3: Lock screen hidden via GAME-07 async signal` — lock screen out of the way
- `22:44:14.804 GameState::Loading emitted for F125`
- **`22:44:36.559 minimize_background_windows: Minimized: F1_25 (PID 20308)`** ← bug fires 22 seconds after launch

T+now screenshot captured at 04:15:53 IST shows Windows desktop visible (game minimized, lock screen still hidden via LAUNCH-FIX-3). Game ran in minimized state until 22:46:54 UTC when F1_25.exe exited code 1 (Bug C — separate issue, possibly memory pressure).

---

## Root cause

[`crates/rc-agent/src/ac_launcher.rs:2189`](../../../../../crates/rc-agent/src/ac_launcher.rs) — `pub fn minimize_background_windows()`

The PowerShell allowlist:
```powershell
$allowList = @(
    'acs', 'AssettoCorsa',                          # Game
    'msedge', 'msedgewebview2',                     # Overlay / Kiosk (Edge)
    'explorer',                                      # Shell (taskbar/desktop)
    'TextInputHost', 'ShellExperienceHost',          # System UI
    'SearchHost', 'StartMenuExperienceHost',         # System UI
    'SecurityHealthSystray', 'ctfmon',               # System tray
    'rc-agent',                                      # Our agent
    'Content Manager'                                # CM monitors game lifecycle
)
```

**Only AC game executables (`acs`, `AssettoCorsa`) are protected.** Any other game with a visible main window gets `ShowWindow(hWnd, 6)` (SW_MINIMIZE).

The author KNEW this could minimize games — comment at [ac_launcher.rs:2235-2236](../../../../../crates/rc-agent/src/ac_launcher.rs):
```rust
/// Bring the AC game window to the foreground so it's visible.
/// Must be called after minimize_background_windows() since that may minimize the game.
fn bring_game_to_foreground() {
```

But the recovery function `bring_game_to_foreground()` is also AC-specific:
```rust
for title in &["Assetto Corsa\0", "AC\0"] {
    let title_wide: Vec<u16> = title.encode_utf16().collect();
    let hwnd = winapi::um::winuser::FindWindowW(ptr::null(), title_wide.as_ptr());
    ...
}
// Fallback: use PowerShell to find acs.exe window
```

It only looks for window titles `"Assetto Corsa"`, `"AC"`, and the fallback only searches for `acs.exe`. **F1 25 is invisible to both functions.**

## Caller paths

1. **[event_loop.rs:1666-1672](../../../../../crates/rc-agent/src/event_loop.rs)** — periodic kiosk-enforcement tick:
   ```rust
   if state.kiosk_enabled && !state.kiosk.is_freedom_mode() {
       tokio::task::spawn_blocking(|| {
           ac_launcher::minimize_background_windows();
           // Native lock screen handles its own foreground via WM_TIMER in window.rs
           ac_launcher::ensure_conspit_link_running();
   ```
   This fires periodically while in kiosk mode. Every tick, F1 25 (or any non-AC game) gets minimized.

2. **[ws_handler.rs:312-313](../../../../../crates/rc-agent/src/ws_handler.rs)** — fires on `BillingStarted`:
   ```rust
   state.lock_screen.show_active_session(driver_name, allocated_seconds, allocated_seconds);
   tokio::task::spawn_blocking(|| ac_launcher::minimize_background_windows());
   ```
   This is intentional — when billing starts, clear background windows so the game has full screen. But again, only AC's window is protected.

## Why AC works and F1 25 doesn't

AC (Assetto Corsa) is the venue's primary sim and has had years of polish. The window-management code was written for AC. F1 25 was added later but the window-management didn't get generalized for it.

The bug pattern matches what the standing rule (`feedback_pod8_session_type_incident.md`) calls cross-boundary serialization issues — code paths designed for one specific value of an identifier that don't generalize to others.

## Proposed fix (NOT YET APPLIED)

Three options, in order of recommended:

### Option A — Add all configured game executables to the allowlist

Lookup-table approach. Trivially safe:
```rust
$allowList = @(
    'acs', 'AssettoCorsa',                          # Assetto Corsa
    'AssettoCorsaEvo',                              # ACE
    'F1_25',                                        # F1 25
    'iRacingSim64DX11',                             # iRacing
    'Le Mans Ultimate',                             # LMU (note: contains spaces, may need quoting)
    'msedge', 'msedgewebview2',
    ...
)
```

**Pros:** Smallest change. No state needed. Symmetric to existing AC entry.
**Cons:** Hardcoded list. New games require manual update. No way to handle non-Steam games or game updates that change exe name.

### Option B — Dynamic allowlist from currently-launched game

Pass the current game's process name into `minimize_background_windows()` from rc-agent state:
```rust
pub fn minimize_background_windows(extra_protect: Option<&[&str]>) {
    // Build allowlist with hardcoded base + caller-supplied extras
}
```

Caller (event_loop.rs / ws_handler.rs) reads the current game from state and passes its process name.

**Pros:** Self-updating. Works for any game in the TOML. Handles future games.
**Cons:** Touches more callers. Slight risk of stale state if game was killed but state wasn't updated.

### Option C — Skip minimize_background_windows entirely when a game is running

Wrap the call site with a check:
```rust
if state.current_game_state == GameState::Idle {
    minimize_background_windows();
}
```

**Pros:** Removes the entire class of "minimize game while it's running" bugs.
**Cons:** Background apps that get launched DURING gameplay (Steam updates, NVIDIA overlay, etc.) won't be cleaned up until the game exits.

## Recommendation

**Option A as a stop-gap, then Option B as the proper fix.** A is small enough to ship today (single PowerShell array edit, plus the same edit to `bring_game_to_foreground()` so it can find non-AC windows). B is a slightly larger refactor — defer to next session unless it's quick.

Critically: this fix should be a SEPARATE commit from the canary fix (`29a4d8f1`) so we can verify each layer independently:
1. Canary `29a4d8f1` — launch verifier accepts F1 25 — VERIFIED in this trace ✅
2. Bug B fix — game window stays visible after launch — TO BE VERIFIED
3. Bug C investigation — F1 25 actual crash — separate, lower priority (game-side or hardware)

## Decision pending

User said "save and commit" before proceeding further. This finding is documented but the fix is NOT yet applied. Awaiting user direction on whether to apply Option A, B, or defer.
