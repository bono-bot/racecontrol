---
phase: 24-crash-hang-launch-usb-bot-patterns
verified: 2026-03-16T12:30:00Z
status: passed
score: 7/7 must-haves verified
re_verification: false
---

# Phase 24: Crash/Hang/Launch/USB Bot Patterns Verification Report

**Phase Goal:** The bot autonomously handles game freeze, launch timeout, and USB wheelbase disconnect on any pod — without staff intervention
**Verified:** 2026-03-16T12:30:00Z
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Bot detects game freeze (UDP silent 30s + IsHungAppWindow) and kills/restarts game without staff intervention (CRASH-01) | VERIFIED | `failure_monitor.rs` polls every 5s; fires synthetic "Game frozen — IsHungAppWindow true + UDP silent 30s relaunch acs.exe" → `fix_frozen_game()`; `test_auto_fix_frozen_game_dispatches` + `test_freeze_detection_udp_silence_threshold` both pass |
| 2 | Bot detects launch timeout (game not running 90s after launch command) and kills Content Manager + retries (CRASH-02) | VERIFIED | `failure_monitor.rs` checks `launch_started_at.elapsed() > 90s && game_pid.is_none()`; fires "launch timeout — Content Manager hang kill cm process"; `fix_launch_timeout()` kills both "Content Manager.exe" and "acmanager.exe"; `test_fix_launch_timeout_kills_both_cm_names` passes |
| 3 | Bot zeros FFB torque before any game kill in teardown sequence (CRASH-03) | VERIFIED | `fix_frozen_game()` calls `FfbController::new(0x1209, 0xFFB0).zero_force()` before any `taskkill` calls; comment states "do NOT move the taskkill calls above this line"; `test_ffb_zero_before_kill_ordering` asserts detail contains "FFB" |
| 4 | Bot suppresses Windows error dialogs before any process kill (UI-01) | VERIFIED | `fix_kill_error_dialogs()` kills WerFault.exe, WerFaultSecure.exe, and msedge.exe; `fix_frozen_game()` also kills WerFault.exe + WerFaultSecure.exe before game taskkill; detail = "Suppressed WerFault.exe, WerFaultSecure.exe, msedge.exe"; `test_kill_error_dialogs_extended` passes |
| 5 | Bot polls for wheelbase USB reconnect and restarts FFB controller when device re-appears (USB-01) | VERIFIED | `failure_monitor.rs` detects `!prev_hid_connected && state.hid_connected && state.billing_active` transition; fires "Wheelbase usb reset required — HID reconnected VID:0x1209 PID:0xFFB0" → `fix_usb_reconnect()`; zeros FFB via `FfbController::new(0x1209, 0xFFB0).zero_force()`; all 3 USB tests pass |
| 6 | All 10 Phase 24 TDD tests pass GREEN | VERIFIED | `cargo test -p rc-agent-crate -- ai_debugger::tests::test_auto_fix_frozen_game_dispatches ...` → `10 passed; 0 failed` |
| 7 | failure_monitor is live spawned task wired into main.rs event loop | VERIFIED | `mod failure_monitor;` at line 6 of main.rs; `failure_monitor::spawn()` called at line 607; `tokio::sync::watch::channel` created at line 519; 13 `send_modify` update sites confirmed via grep |

**Score:** 7/7 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-agent/src/ai_debugger.rs` | fix_frozen_game, fix_launch_timeout, fix_usb_reconnect, extended fix_kill_error_dialogs, 3 new try_auto_fix arms, PodStateSnapshot with Default + 3 new fields | VERIFIED | All 4 functions implemented (no todo!), Pattern 3a/3b/3c arms present in correct order before Pattern 4, PodStateSnapshot has `#[derive(Default)]` and fields `last_udp_secs_ago`, `game_launch_elapsed_secs`, `hid_last_error` |
| `crates/rc-agent/src/failure_monitor.rs` | FailureMonitorState struct + spawn() + detection logic + 8 tests | VERIFIED | File exists (384 lines); FailureMonitorState has all 6 fields; spawn() present; is_game_process_hung() with CPU pre-filter + IsHungAppWindow; 8 tests all pass |
| `crates/rc-agent/src/main.rs` | failure_monitor module declaration + spawn call + watch channel + 13 state update sites + 3 new PodStateSnapshot fields | VERIFIED | mod failure_monitor at line 6; watch::channel at line 519; failure_monitor::spawn() at line 607; 13 send_modify sites confirmed; PodStateSnapshot in ai_result handler includes last_udp_secs_ago, game_launch_elapsed_secs, hid_last_error |
| `crates/rc-agent/src/driving_detector.rs` | last_udp_packet_elapsed_secs() public accessor + 2 tests | VERIFIED | pub fn last_udp_packet_elapsed_secs() at line 152; 2 tests pass (none_before_first_packet, some_after_udp_active) |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `try_auto_fix "game frozen" arm (Pattern 3a)` | `fix_frozen_game()` | `lower.contains("game frozen") \|\| (lower.contains("frozen") && lower.contains("relaunch"))` | WIRED | Pattern 3a confirmed before Pattern 4 (kill_stale_game); keyword specificity test passes |
| `fix_frozen_game` | `FfbController::zero_force()` | Direct call before taskkill, `use crate::ffb_controller::FfbController` import at line 7 | WIRED | zero_force() call at line 387; comment "do NOT move the taskkill calls above this line" enforces ordering |
| `failure_monitor::spawn()` | `ai_debugger::try_auto_fix()` | `tokio::task::spawn_blocking` at 3 call sites | WIRED | 4 spawn_blocking usages confirmed: USB reconnect, launch timeout, freeze detection, is_game_process_hung |
| `FailureMonitorState` | `main.rs watch::channel` | `tokio::sync::watch::channel(FailureMonitorState::default())` | WIRED | Channel created at line 519; receiver passed to failure_monitor::spawn() at line 609 |
| `driving_detector 100ms tick` | `failure_monitor_tx.send_modify` | `s.hid_connected + s.last_udp_secs_ago` update in detector_interval arm | WIRED | Site 1 at line 909; also updates last_udp_secs_ago via detector.last_udp_packet_elapsed_secs() |
| `LaunchGame handler` | `failure_monitor_state.launch_started_at` | `send_modify(|s| s.launch_started_at = Some(Instant::now()))` | WIRED | Site 2 at line 1458; cleared at Sites 3a/3b/3c/3d/3e |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| CRASH-01 | 24-01, 24-02, 24-03, 24-04 | Bot detects game freeze (UDP silent 30s + IsHungAppWindow) and kills/restarts without staff intervention | SATISFIED | fix_frozen_game() fully implemented; failure_monitor detects UDP silence >= 30s + is_game_process_hung(); synthetic string fires correctly; billing gate prevents replay abuse |
| CRASH-02 | 24-01, 24-02, 24-03, 24-04 | Bot detects launch timeout (game not running 90s after launch command) and kills CM + retries | SATISFIED | fix_launch_timeout() kills both "Content Manager.exe" and "acmanager.exe"; failure_monitor fires after 90s when game_pid.is_none(); launch_timeout_fired dedup prevents double-fire |
| CRASH-03 | 24-01, 24-02, 24-04 | Bot zeros FFB torque before any game kill in teardown sequence | SATISFIED | FFB zero_force() called before first taskkill in fix_frozen_game(); structural ordering enforced by code, validated by test_ffb_zero_before_kill_ordering checking detail contains "FFB" |
| UI-01 | 24-01, 24-02, 24-04 | Bot suppresses Windows error dialogs (WER, crash reporters) before any process kill | SATISFIED | fix_kill_error_dialogs() extended to WerFault.exe + WerFaultSecure.exe + msedge.exe; fix_frozen_game() also kills WerFault + WerFaultSecure before game kill; test_kill_error_dialogs_extended passes |
| USB-01 | 24-01, 24-02, 24-03, 24-04 | Bot polls for wheelbase USB reconnect (hidapi 5s scan, VID:0x1209 PID:0xFFB0) and restarts FFB controller | SATISFIED | failure_monitor detects hid_connected transition false→true; fires fix_usb_reconnect(); FFB reset via FfbController::new(0x1209, 0xFFB0); HardwareFailure AgentMessage sent on disconnect during billing |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| None | — | — | — | — |

No TODO, FIXME, PLACEHOLDER, todo!(), or unimplemented!() found in any Phase 24 modified files.

### Human Verification Required

#### 1. End-to-end freeze recovery on live pod

**Test:** Suspend acs.exe on Pod 8 (or let it freeze naturally), wait 30+ seconds, observe failure_monitor log output
**Expected:** "[failure-monitor] Game frozen: PID xxx — UDP silent Ns + IsHungAppWindow=true" followed by fix_frozen_game execution, acs.exe killed, FFB zero confirmed in logs
**Why human:** `is_game_process_hung()` uses real Windows APIs (sysinfo + EnumWindows + IsHungAppWindow) that cannot be tested in unit tests; actual 30s UDP silence requires a live game

#### 2. Launch timeout on live pod

**Test:** Issue a LaunchGame command to a pod, then kill Content Manager immediately so it never starts acs.exe, wait 90s
**Expected:** "[failure-monitor] Launch timeout: Ns elapsed, no game PID — killing Content Manager" in logs; Content Manager process list clear after fix
**Why human:** Requires real Content Manager process + real 90-second wait; unit tests only verify the state-machine logic

#### 3. USB reconnect FFB reset on live pod

**Test:** Unplug Conspit Ares wheelbase USB during an active session, wait 5s, replug it
**Expected:** HardwareFailure message sent to server on disconnect; "[failure-monitor] Wheelbase USB reconnect detected — firing FFB reset" on reconnect; FFB controller initializes to zero torque
**Why human:** Requires physical USB device + real HID enumeration via hidapi; hid_connected state flows from driving_detector HID poll which is hardware-dependent

### Gaps Summary

No gaps. All 5 requirements (CRASH-01, CRASH-02, CRASH-03, UI-01, USB-01) are fully satisfied:

- `fix_frozen_game()`, `fix_launch_timeout()`, `fix_usb_reconnect()`: all implemented with real logic, no stubs
- `fix_kill_error_dialogs()`: extended to WerFaultSecure.exe + msedge.exe (UI-01)
- `failure_monitor.rs`: 5s polling loop with 30s startup grace, all 3 detection conditions implemented, recovery_in_progress guard, HardwareFailure AgentMessage on USB disconnect
- `main.rs`: watch channel wired, spawn() called after self_monitor, 13 state update sites covering all 6 FailureMonitorState dimensions
- `driving_detector.rs`: last_udp_packet_elapsed_secs() accessor added with 2 tests
- All 18 Phase 24 tests (10 ai_debugger + 8 failure_monitor) pass GREEN
- 232 total tests compile and listed with 0 failures

---

_Verified: 2026-03-16T12:30:00Z_
_Verifier: Claude (gsd-verifier)_
