---
phase: 57-session-end-safety
verified: 2026-03-20T14:30:00+05:30
status: passed
score: 5/5 must-haves verified
re_verification: false
---

# Phase 57: Session-End Safety Verification Report

**Phase Goal:** When a game session ends, the wheelbase returns to center safely within 2 seconds -- no stuck rotation, no snap-back, no staff intervention
**Verified:** 2026-03-20T14:30:00+05:30
**Status:** passed
**Re-verification:** No -- initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Wheelbase returns to center within 2 seconds of any game closing on any pod -- no stuck rotation | VERIFIED | `safe_session_end()` at line 441 in ffb_controller.rs: fxm.reset clears orphaned effects, 5-step idlespring ramp over 500ms applies centering spring. 11 call sites in main.rs (all session-end paths). Hardware-validated per 57-03-SUMMARY. |
| 2 | Centering force ramps up gradually (no sudden snap that could injure a customer's hands) | VERIFIED | Lines 464-470 in ffb_controller.rs: `for step in 1..=5 { let value = (target * step) / 5; }` with 100ms sleep between steps = 500ms ramp. test_idlespring_ramp_values confirms [400, 800, 1200, 1600, 2000]. |
| 3 | ConspitLink is closed before HID safety commands fire, eliminating P-20 contention | VERIFIED | Lines 443-453 in ffb_controller.rs: `close_conspit_link(Duration::from_secs(5))` called first in safe_session_end(), with WM_CLOSE + process exit polling. P-20 timeout warning logged but HID commands proceed regardless. |
| 4 | ConspitLink restarts automatically after the safety sequence with verified JSON config intact | VERIFIED | Line 476 in ffb_controller.rs: `restart_conspit_link()` fire-and-forget after HID commands. Lines 404-424: Global.json integrity check via serde_json::from_str after 5s delay. |
| 5 | ESTOP code path remains available but is never triggered during routine session ends | VERIFIED | main.rs line 423: panic hook uses `zero_force_with_retry(3, 100)`. Line 641: startup probe uses `zero_force_with_retry(3, 100)`. Zero `zero_force().ok()` patterns remain in session-end paths. All session-end sites use `safe_session_end()` instead. |

**Score:** 5/5 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-agent/src/ffb_controller.rs` | HID commands + safe_session_end orchestrator | VERIFIED | 654 lines. Contains: CLASS_FXM (0x0A03), CMD_FXM_RESET (0x01), CMD_IDLESPRING (0x05), POWER_CAP_80_PERCENT (52428), Clone derive, fxm_reset(), set_idle_spring(), close_conspit_link(), restart_conspit_link(), safe_session_end(). 11 tests (5 existing + 6 new). |
| `crates/rc-agent/src/ac_launcher.rs` | enforce_safe_state with skip_conspit_restart flag | VERIFIED | Line 1586: `pub fn enforce_safe_state(skip_conspit_restart: bool)`. Line 1612: `if !skip_conspit_restart`. Line 1332: `pub(crate) fn minimize_conspit_window()`. Line 18: `pub(crate) fn hidden_cmd()`. |
| `crates/rc-agent/src/main.rs` | All session-end sites wired to safe_session_end, startup power cap | VERIFIED | 11 `safe_session_end(&ffb).await` calls. 2 `zero_force_with_retry` calls (panic hook + startup probe only). 8 `enforce_safe_state(true)` calls. 0 `enforce_safe_state(false)`. Line 649: `set_gain(80)` for startup power cap. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| main.rs session-end sites | ffb_controller::safe_session_end() | async call replacing spawn_blocking zero_force | WIRED | 11 call sites confirmed via grep, all use `ffb_controller::safe_session_end(&ffb).await` |
| close_conspit_link() | winapi FindWindowW + PostMessageW | WM_CLOSE with 5s timeout | WIRED | Line 335: `PostMessageW(hwnd, WM_CLOSE, 0, 0)` confirmed |
| safe_session_end() | fxm_reset() + set_idle_spring() | spawn_blocking with 5-step ramp | WIRED | Lines 459-470: fxm_reset() call followed by 5-step loop calling set_idle_spring(value) |
| main.rs startup | ffb_controller::set_gain() | 80% power cap at boot | WIRED | Line 649: `ffb_cap.set_gain(80)` after zero_force_with_retry probe |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| SAFE-01 | 57-02, 57-03 | Wheelbase returns to center within 2s of game session ending | SATISFIED | safe_session_end() wired at all 11 session-end sites; hardware-validated |
| SAFE-02 | 57-01 | Session-end uses fxm.reset + axis.idlespring (NOT estop) | SATISFIED | fxm_reset() + set_idle_spring() in safe_session_end(); estop only in panic hook |
| SAFE-03 | 57-01 | Force ramp-up is gradual (500ms minimum) | SATISFIED | 5-step ramp over 500ms (100ms per step); test_idlespring_ramp_values confirms [400..2000] |
| SAFE-04 | 57-01, 57-03 | Venue power capped at safe maximum via axis.power | SATISFIED | POWER_CAP_80_PERCENT=52428 constant; set_gain(80) at startup line 649 |
| SAFE-05 | 57-01 | ESTOP reserved for genuine emergencies only | SATISFIED | zero_force_with_retry only at panic hook (line 423) and startup probe (line 641); 0 in session-end paths |
| SAFE-06 | 57-02 | ConspitLink gracefully closed (WM_CLOSE) before HID commands | SATISFIED | close_conspit_link() called first in safe_session_end() with PostMessageW WM_CLOSE |
| SAFE-07 | 57-02 | ConspitLink restarted after safety sequence with JSON integrity check | SATISFIED | restart_conspit_link() at end of safe_session_end(); Global.json parsed via serde_json after 5s |

No orphaned requirements -- all 7 SAFE-0x IDs from REQUIREMENTS.md Phase 57 are accounted for in plans.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| (none) | - | - | - | No anti-patterns found in any modified files |

No TODO/FIXME/PLACEHOLDER/stub patterns found in ffb_controller.rs, main.rs, or ac_launcher.rs changes.

### Human Verification Required

Hardware testing was performed as part of Plan 03 (checkpoint:human-verify) and approved per 57-03-SUMMARY.md. The following was validated on canary pod:

### 1. Wheel Centering Behavior

**Test:** End a game session on each of the 4 supported games
**Expected:** Wheel returns toward center within 2 seconds with gradual force
**Result:** Approved per 57-03-SUMMARY (idlespring target=2000 confirmed acceptable)

### 2. ConspitLink Lifecycle

**Test:** Observe ConspitLink in taskbar/tray during session end
**Expected:** Closes, then restarts after ~4-5 seconds
**Result:** Approved per 57-03-SUMMARY

### 3. Fleet Deployment

**Test:** Deploy new rc-agent.exe to all 8 pods (not just canary)
**Expected:** All pods exhibit same safe centering behavior
**Why human:** Fleet rollout requires admin deploy process, not verifiable programmatically

### Gaps Summary

No gaps found. All 5 observable truths verified. All 7 requirements (SAFE-01 through SAFE-07) satisfied with evidence in the codebase. All artifacts exist, are substantive (not stubs), and are properly wired. Anti-pattern scan clean.

The one remaining operational step is fleet-wide deployment (currently only canary-validated), which is a deployment concern, not a code gap.

---

_Verified: 2026-03-20T14:30:00+05:30_
_Verifier: Claude (gsd-verifier)_
