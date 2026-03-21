---
phase: 108-keyboard-hook-replacement
verified: 2026-03-21T16:30:00Z
status: human_needed
score: 4/5 must-haves verified
gaps:
human_verification:
  - test: "Deploy default rc-agent build to Pod 8 and test kiosk lockdown"
    expected: |
      1. rc-agent log shows "Kiosk: GPO set HKCU\Software\...\NoWinKeys = 1"
      2. rc-agent log shows "Kiosk: GPO set HKCU\Software\...\DisableTaskMgr = 1"
      3. Windows key press during kiosk does NOT open Start menu
      4. Ctrl+Shift+Esc during kiosk does NOT open Task Manager
      5. After kiosk deactivation: reg query for NoWinKeys returns ERROR (key deleted)
      6. After kiosk deactivation: Windows key opens Start menu normally
    why_human: "GPO registry effectiveness (does reg.exe run as the correct user context on Pod machines?) and behavioral kiosk lockdown require physical device testing. Cannot verify OS policy application or key intercept behavior programmatically."
---

# Phase 108: Keyboard Hook Replacement Verification Report

**Phase Goal:** The SetWindowsHookEx global keyboard hook installed by Phase 78 is fully removed from rc-agent source and permanently replaced by GPO registry key writes -- kiosk lockdown is equally effective without any hook, and no hook install/uninstall cycle is ever visible to a running anti-cheat driver

**Verified:** 2026-03-21T16:30:00Z (22:00 IST)
**Status:** human_needed
**Re-verification:** No -- initial verification

## Goal Achievement

### Observable Truths

| #  | Truth                                                                                              | Status       | Evidence                                                                                 |
|----|---------------------------------------------------------------------------------------------------|--------------|------------------------------------------------------------------------------------------|
| 1  | Default cargo build of rc-agent contains zero SetWindowsHookEx calls in compiled code            | VERIFIED  | `SetWindowsHookExW` at line 963 is inside `#[cfg(feature = "keyboard-hook")]` block; default build excludes it |
| 2  | GPO registry keys NoWinKeys=1 and DisableTaskMgr=1 are written on kiosk activate and removed on deactivate | VERIFIED  | `apply_gpo_lockdown()` called at line 467 (activate), `remove_gpo_lockdown()` at line 482 (deactivate); keys confirmed at kiosk.rs lines 997-999 and 1028-1029 |
| 3  | Hook code still exists in source behind keyboard-hook feature flag for emergency rollback         | VERIFIED  | `HOOK_HANDLE`, `keyboard_hook_proc`, `install_keyboard_hook`, `remove_keyboard_hook` all gated by `#[cfg(feature = "keyboard-hook")]` at lines 879, 882, 888, 894, 960, 978 |
| 4  | cargo build --release --bin rc-agent succeeds without keyboard-hook feature                       | VERIFIED  | Commit `2d20fac` message confirms both default and feature builds pass; SUMMARY documents build verification |
| 5  | Kiosk lockdown is equally effective (no Win key, no Task Manager) on real pod hardware            | HUMAN NEEDED | Cannot verify GPO registry enforcement or behavioral lockdown without physical Pod 8 test |

**Score:** 4/5 truths verified (1 requires human)

### Required Artifacts

| Artifact                              | Expected                                                        | Status       | Details                                                                                           |
|---------------------------------------|-----------------------------------------------------------------|--------------|---------------------------------------------------------------------------------------------------|
| `crates/rc-agent/Cargo.toml`         | `[features]` section with `keyboard-hook = []`                 | VERIFIED  | Lines 67-68: `[features]` and `keyboard-hook = []` confirmed present                            |
| `crates/rc-agent/src/kiosk.rs`       | `apply_gpo_lockdown` function, hook code gated behind feature  | VERIFIED  | `apply_gpo_lockdown` appears 4 times (definition at 991, call at 467, pub use at 1075, non-windows stub at 1086). `remove_gpo_lockdown` appears 4 times. 6 cfg(feature) gates on hook code. |

### Key Link Verification

| From                        | To                       | Via                                               | Status       | Details                                                                                    |
|-----------------------------|--------------------------|---------------------------------------------------|--------------|-------------------------------------------------------------------------------------------|
| `kiosk.rs activate()`       | `apply_gpo_lockdown()`   | Function call replacing `install_keyboard_hook()` | WIRED     | Line 467: `apply_gpo_lockdown();` inside `#[cfg(windows)]` block in `activate()`         |
| `kiosk.rs deactivate()`     | `remove_gpo_lockdown()`  | Function call replacing `remove_keyboard_hook()`  | WIRED     | Line 482: `remove_gpo_lockdown();` inside `#[cfg(windows)]` block in `deactivate()`      |
| `Cargo.toml [features]`     | `kiosk.rs #[cfg(feature = "keyboard-hook")]` | Cargo feature flag gates hook code | WIRED  | `keyboard-hook` defined in Cargo.toml lines 67-68; 6 matching `cfg(feature = "keyboard-hook")` gates in kiosk.rs |

### Requirements Coverage

| Requirement | Source Plan | Description                                                                                              | Status       | Evidence                                                                                        |
|-------------|-------------|----------------------------------------------------------------------------------------------------------|--------------|-------------------------------------------------------------------------------------------------|
| HARD-01     | 108-01-PLAN | SetWindowsHookEx keyboard hook fully removed and replaced with GPO registry keys (NoWinKeys, DisableTaskMgr) | SATISFIED | `SetWindowsHookExW` gated behind feature flag; `apply_gpo_lockdown()` is the default path; commit `2d20fac` confirms |
| VALID-03    | 108-01-PLAN | Kiosk lockdown remains effective (no Win key, no Alt+Tab, no Task Manager) after keyboard hook replacement | HUMAN NEEDED | Code paths exist and are wired. Behavioral effectiveness on pod hardware requires physical test (Task 2 in PLAN, deferred to morning) |

No orphaned requirements -- both HARD-01 and VALID-03 are mapped to Phase 108 in REQUIREMENTS.md (lines 73 and 80) and claimed in the plan frontmatter.

### Anti-Patterns Found

None. No TODO/FIXME/HACK/placeholder comments found in modified files (`crates/rc-agent/Cargo.toml`, `crates/rc-agent/src/kiosk.rs`). No stub implementations or empty handlers in the new GPO functions.

### Human Verification Required

#### 1. Pod 8 Canary: GPO Kiosk Lockdown Behavioral Test

**Test:** Deploy default `cargo build --release --bin rc-agent` (no `--features keyboard-hook`) to Pod 8 only. Activate kiosk mode. Run through the 10-step verification in the PLAN's Task 2.

**Expected:**
1. Log shows `Kiosk: GPO set HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer\NoWinKeys = 1`
2. Log shows `Kiosk: GPO set HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\System\DisableTaskMgr = 1`
3. Windows key press during kiosk session -- Start menu does NOT open
4. Ctrl+Shift+Esc during kiosk session -- Task Manager does NOT open
5. After kiosk deactivation: `reg query "HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer" /v NoWinKeys` returns ERROR (key deleted)
6. After kiosk deactivation: Windows key opens Start menu normally

**Why human:** GPO registry keys written to HKCU take effect only when Explorer reads them, which may require Explorer restart or may not apply in kiosk user context. Physical device test is the only way to confirm the keys enforce the expected behavioral lockdown. Also verifies `reg.exe` runs with correct permissions on pod machines (they run as a specific user account).

### Gaps Summary

No blocking code gaps. The only open item is the deferred Pod 8 physical canary test (Task 2 in PLAN). This was explicitly deferred because the venue pods are powered off for the night.

HARD-01 is fully satisfied by code evidence: the hook is gated, the GPO replacement is wired, and both builds (default and with feature) compile.

VALID-03 is pending the physical test. Once Pod 8 confirms lockdown behavior, both requirements are fully closed and Phase 108 is complete.

---

_Verified: 2026-03-21T16:30:00Z (22:00 IST)_
_Verifier: Claude (gsd-verifier)_
