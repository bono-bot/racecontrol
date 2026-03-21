---
phase: 109-safe-mode-state-machine
verified: 2026-03-21T10:45:00+05:30
status: gaps_found
score: 6/7 must-haves verified
gaps:
  - truth: "Cooldown timer fires in event_loop select! block 30 seconds after game exit, deactivating safe mode"
    status: partial
    reason: "Cooldown fires correctly for LaunchGame-initiated games (game_process exit path). For externally-launched games detected only via WMI, the exit detection path is a deliberate stub returning false — safe mode persists indefinitely until rc-agent restart when a protected game was launched externally and not via LaunchGame. Covers the primary use case (staff using kiosk to launch) but not external/Steam launches."
    artifacts:
      - path: "crates/rc-agent/src/event_loop.rs"
        issue: "Lines 364-373: external game exit cooldown path has stub body — `false // conservative: don't trigger cooldown from this path`. The still_running bool is computed but immediately discarded (`let _ = still_running`). WMI-detected games (game=Some(sim)) never trigger start_cooldown() on exit."
    missing:
      - "Implement sysinfo process scan inside the still_running check to determine if any PROTECTED_EXE_NAMES process is still alive. If not found, call state.safe_mode.start_cooldown() and arm the timer. This mirrors the detect_running_protected_game() pattern already in safe_mode.rs."
human_verification:
  - test: "Verify <1 second detection for externally launched F1 25"
    expected: "WMI fires within 1 second of F1_25.exe process start; safe_mode.active becomes true; process_guard scan stops"
    why_human: "WMI Win32_ProcessStartTrace latency cannot be measured by code inspection alone — requires launching F1 25 and observing logs"
  - test: "Verify cooldown holds process guard suspended for full 30 seconds after game exits"
    expected: "After F1 25 exits, process guard debug logs show 'scan skipped' for ~30 seconds, then resume"
    why_human: "Timer accuracy and AtomicBool sync across threads requires runtime observation"
---

# Phase 109: Safe Mode State Machine — Verification Report

**Phase Goal:** rc-agent automatically enters a defined safe mode within 1 second of a protected game launching, disables all risky subsystems (process guard, Ollama queries, registry writes) for the duration of the game plus a 30-second cooldown, and defaults to safe mode at startup if a protected game is already running — billing, lock screen, and WebSocket exec are unaffected throughout

**Verified:** 2026-03-21T10:45:00+05:30 (IST)
**Status:** gaps_found
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|---------|
| 1 | SafeMode struct exists with active/game/cooldown_until fields and enter/start_cooldown/exit transitions | VERIFIED | `safe_mode.rs` lines 32-95: `pub struct SafeMode { active, game, cooldown_until }` with all 3 methods implemented and tested |
| 2 | WMI channel is polled in event_loop and triggers safe mode for externally launched games | VERIFIED | `event_loop.rs` lines 335-357: `wmi_rx.try_recv()` inside `game_check_interval` tick arm, handles both SimType and WRC (no SimType) cases |
| 3 | Cooldown timer fires in event_loop select! block 30 seconds after game exit, deactivating safe mode | PARTIAL | Timer select! arm (lines 880-885) is correct. LaunchGame path (lines 474-483) triggers cooldown on GameProcess exit. External WMI game exit path (lines 364-373) is a stub returning `false` — safe mode persists if game was launched outside rc-agent |
| 4 | Process guard scan loop skips entirely when safe_mode_active AtomicBool is true | VERIFIED | `process_guard.rs` line 74: `if safe_mode.load(Relaxed) { continue; }` at top of scan loop |
| 5 | Ollama analyze_crash returns early without HTTP call when safe mode is active | VERIFIED | `event_loop.rs` lines 441-444: call-site guard checks `state.safe_mode.active` before spawning `analyze_crash` task |
| 6 | Registry write functions skip during safe mode | VERIFIED | `kiosk.rs` lines 478, 498: `safe_mode_active.load(Relaxed)` before `apply_gpo_lockdown()`/`remove_gpo_lockdown()`. `lock_screen.rs` line 506: same guard before `suppress_notifications()` (Focus Assist). `self_heal.rs`: startup-only, no gate needed (confirmed line 239 main.rs) |
| 7 | Billing, lock screen, overlay, heartbeat, and WS exec have NO safe mode checks | VERIFIED | `billing_guard.rs`: 0 refs. `udp_heartbeat.rs`: 0 refs. `overlay.rs`: 0 refs. `ws_handler.rs`: safe_mode refs only in LaunchGame handler (lines 290-295), not in exec paths. Lock screen show/hide unaffected (safe_mode_active only gates the Focus Assist registry write, not bind/show/hide) |

**Score:** 6/7 truths verified (1 partial — external game exit cooldown)

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-agent/src/safe_mode.rs` | SafeMode struct, WMI watcher, startup detection, 21 tests | VERIFIED | 419 lines. All public functions present: `new()`, `enter()`, `start_cooldown()`, `exit()`, `is_protected_game()`, `PROTECTED_EXE_NAMES`, `exe_to_sim_type()`, `spawn_wmi_watcher()`, `detect_running_protected_game()`. 21 tests confirmed. |
| `crates/rc-agent/src/app_state.rs` | 5 safe_mode fields in AppState | VERIFIED | Lines 70-79: `safe_mode`, `safe_mode_active`, `wmi_rx`, `safe_mode_cooldown_timer`, `safe_mode_cooldown_armed` all present |
| `crates/rc-agent/src/main.rs` | mod declaration, initialization, startup scan, WMI spawn | VERIFIED | Line 20: `mod safe_mode;`. Lines 754-758: all 5 fields initialized. Lines 766-773: startup scan + WMI spawn. Lines 777-778: `wire_safe_mode()` called for kiosk and lock_screen. Line 786: `safe_mode_active` passed to `process_guard::spawn()`. |
| `crates/rc-agent/src/event_loop.rs` | WMI polling, cooldown timer, game exit trigger | PARTIAL | WMI polling: VERIFIED (lines 335-357). Cooldown timer select! arm: VERIFIED (lines 880-885). LaunchGame exit cooldown: VERIFIED (lines 474-483). External game exit cooldown: STUB (lines 364-373). |
| `crates/rc-agent/src/ws_handler.rs` | Safe mode entry in LaunchGame before spawn | VERIFIED | Lines 287-295: safe mode entered BEFORE any game spawn code (`if launch_sim == SimType::AssettoCorsa` check comes after) |
| `crates/rc-agent/src/process_guard.rs` | Arc<AtomicBool> parameter + scan skip | VERIFIED | Lines 37-43: 5th param `safe_mode: Arc<AtomicBool>`. Lines 74-77: `if safe_mode.load(Relaxed) { continue; }` |
| `crates/rc-agent/src/kiosk.rs` | wire_safe_mode() + GPO registry gate | VERIFIED | Lines 436, 456, 462-463: `safe_mode_active` field + `wire_safe_mode()`. Lines 478, 498: registry write gates |
| `crates/rc-agent/src/lock_screen.rs` | wire_safe_mode() + Focus Assist registry gate | VERIFIED | Lines 160-161, 173, 179-180: `safe_mode_active` field + `wire_safe_mode()`. Line 506: Focus Assist gate |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `main.rs` | `safe_mode.rs` | `mod safe_mode` + `SafeMode::new()` + `spawn_wmi_watcher()` + `detect_running_protected_game()` | VERIFIED | All 4 usages confirmed at lines 20, 754, 766, 773 |
| `app_state.rs` | `safe_mode.rs` | `use crate::safe_mode` + `safe_mode::SafeMode` field type | VERIFIED | Line 3: `use crate::safe_mode;`. Line 70: `safe_mode: safe_mode::SafeMode` |
| `ws_handler.rs` | `safe_mode.rs` | `state.safe_mode.enter()` in LaunchGame handler | VERIFIED | Lines 290-291: `is_protected_game(launch_sim)` + `state.safe_mode.enter(launch_sim)` |
| `event_loop.rs` | `safe_mode.rs` | `wmi_rx.try_recv()` + `exe_to_sim_type()` + `start_cooldown()` + `exit()` | PARTIAL | WMI poll + enter + cooldown select! arm all wired. External exit cooldown path (still_running check) is stub. |
| `process_guard.rs` | `AppState.safe_mode_active` | `Arc<AtomicBool>` in spawn(), `.load(Relaxed)` in scan loop | VERIFIED | Lines 42, 74 |
| `event_loop.rs` | `AppState.safe_mode_cooldown_timer` | select! arm polls when armed, `safe_mode.exit()` on fire | VERIFIED | Lines 880-885: timer arm correctly calls `exit()` and stores `false` |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|---------|
| SAFE-01 | 109-01, 109-02 | rc-agent detects protected game launch within 1 second via WMI Win32_ProcessStartTrace | SATISFIED | `spawn_wmi_watcher()` subscribes to `Win32_ProcessStartTrace`. Primary path: LaunchGame handler enters immediately (zero delay). WMI polled every `game_check_interval` tick. |
| SAFE-02 | 109-01, 109-02 | rc-agent enters safe mode automatically when a protected game is detected, managed by a state machine in AppState | SATISFIED | `SafeMode` struct in `AppState` with enter/exit transitions. Both LaunchGame and WMI paths call `state.safe_mode.enter()`. `safe_mode_active` AtomicBool kept in sync. |
| SAFE-03 | 109-01, 109-02 | Safe mode remains active for 30 seconds after the protected game exits | PARTIAL | 30-second cooldown implemented and wired for games launched via LaunchGame handler. External WMI-detected game exit does not trigger cooldown (stub at `event_loop.rs:370`). Primary real-world path (staff using kiosk) is covered. |
| SAFE-04 | 109-02 | Process guard suspended during safe mode | SATISFIED | `process_guard.rs` line 74: `continue` skips entire scan iteration when `safe_mode_active` is true |
| SAFE-05 | 109-02 | Ollama LLM queries suppressed during safe mode | SATISFIED | `event_loop.rs` lines 441-444: `analyze_crash` spawn guarded by `state.safe_mode.active` check |
| SAFE-06 | 109-02 | Registry write operations deferred until safe mode exits | SATISFIED | `kiosk.rs` GPO lockdown/unlock gated. `lock_screen.rs` Focus Assist gated. `self_heal.rs` startup-only (no gate needed). |
| SAFE-07 | 109-02 | Billing, lock screen, overlay, heartbeat, and WS exec continue uninterrupted | SATISFIED | `billing_guard.rs`: 0 safe_mode refs. `udp_heartbeat.rs`: 0 refs. `overlay.rs`: 0 refs. WS exec path has no safe_mode checks. Lock screen bind/show/hide unaffected. |

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `event_loop.rs` | 366-372 | Stub: `false // conservative: don't trigger cooldown from this path` with `let _ = still_running` | Warning | External WMI-detected games (externally/Steam launched) stay in safe mode indefinitely after exit. Primary kiosk launch path unaffected. Not a build blocker — system is operational. |

---

### Human Verification Required

#### 1. WMI Sub-Second Detection Latency

**Test:** On a pod, open powershell and manually start `F1_25.exe` (or rename a small .exe to that name). Watch rc-agent logs.
**Expected:** Within 1 second, logs show `"WMI watcher: protected process started"` and `"Safe mode ENTER"`. Process guard logs show `"safe mode active — scan skipped"` on next tick.
**Why human:** WMI event latency under load, PowerShell startup time, and named pipe buffering cannot be verified by static analysis.

#### 2. Cooldown Duration Accuracy

**Test:** Launch a protected game via kiosk (LaunchGame path). Close the game. Observe rc-agent logs for 35 seconds.
**Expected:** `"Protected game exited — 30s safe mode cooldown started"` appears at game exit. Approximately 30 seconds later: `"Safe mode cooldown expired — safe mode DEACTIVATED"`. Process guard resumes scanning.
**Why human:** tokio timer accuracy and the Instant-based cooldown calculation need runtime validation.

#### 3. Startup Default Safe Mode

**Test:** Kill rc-agent while a protected game is running. Restart rc-agent. Check logs before the reconnect loop starts.
**Expected:** `"Protected game already running at startup — safe mode ACTIVE"` appears during initialization, before WebSocket connection.
**Why human:** Requires coordinating process kill and re-launch timing.

---

### Gaps Summary

One partial gap found in SAFE-03: the 30-second exit cooldown is fully implemented for games launched via the LaunchGame WebSocket handler (the primary path for all kiosk-initiated sessions). The external game exit cooldown path — used when a protected game was launched outside rc-agent control (direct Steam launch, staff testing) — has a stub implementation that never fires. A comment in the code explicitly marks this as "conservative."

**Practical impact:** In the primary operational scenario (staff uses kiosk to start game), safe mode entry and exit work correctly. The gap only affects edge cases where a protected game is launched externally. In that case, safe mode stays active until rc-agent restarts, which is the safe failure direction (process guard stays off rather than potentially firing during an active anti-cheat session).

The fix is straightforward: replace the stub with a sysinfo process scan checking `PROTECTED_EXE_NAMES`, calling `start_cooldown()` if none found. The `detect_running_protected_game()` function in `safe_mode.rs` already provides this logic and can be reused.

---

### Commit Verification

All 4 phase commits confirmed in git history:

| Commit | Plan | Description |
|--------|------|-------------|
| `0921d5c` | 109-01 Task 1 | feat: create safe_mode.rs module with SafeMode struct and WMI watcher |
| `ebe1020` | 109-01 Task 2 | feat: integrate safe_mode into AppState and main.rs initialization |
| `0913e82` | 109-02 Task 1 | feat: wire safe mode into event_loop and ws_handler |
| `705b07d` | 109-02 Task 2 | feat: gate process_guard, kiosk, and lock_screen during safe mode |

---

_Verified: 2026-03-21T10:45:00+05:30 (IST)_
_Verifier: Claude (gsd-verifier)_
