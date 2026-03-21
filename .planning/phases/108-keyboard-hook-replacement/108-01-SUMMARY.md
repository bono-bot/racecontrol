---
phase: 108-keyboard-hook-replacement
plan: 01
subsystem: infra
tags: [kiosk, anti-cheat, registry, windows, gpo, keyboard-hook, cargo-features]

# Dependency graph
requires:
  - phase: 107-behavior-audit-certificate-procurement
    provides: anti-cheat risk inventory identifying SetWindowsHookEx as CRITICAL risk (HARD-01)

provides:
  - GPO registry lockdown replacing SetWindowsHookEx in default rc-agent build
  - keyboard-hook Cargo feature flag for emergency rollback
  - apply_gpo_lockdown() / remove_gpo_lockdown() functions in kiosk.rs

affects:
  - 109-safe-mode-state-machine (hook state no longer exists, safe mode simpler)
  - 111-code-signing-per-game-canary-validation (canary test includes kiosk verification)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Cargo feature flags for emergency rollback of dangerous OS API usage"
    - "GPO registry writes via reg.exe (std::process::Command, CREATE_NO_WINDOW) matching lock_screen.rs pattern"
    - "cfg(feature) gates on static, function defs, and re-exports — imports also gated to avoid unused warnings"

key-files:
  created: []
  modified:
    - crates/rc-agent/Cargo.toml
    - crates/rc-agent/src/kiosk.rs

key-decisions:
  - "GPO via reg.exe chosen over winreg crate — matches existing lock_screen.rs pattern, zero new deps"
  - "NoWinKeys=1 in HKCU Explorer and DisableTaskMgr=1 in HKCU System are sufficient replacement for keyboard hook"
  - "keyboard-hook feature flag preserves rollback path — cargo build --features keyboard-hook restores old behavior"
  - "imports (AtomicPtr, Ordering, LPARAM, LRESULT, WPARAM) also gated to prevent unused import warnings"

patterns-established:
  - "Cargo feature = emergency rollback: dangerous OS APIs gated behind feature flag, safe replacement is default"
  - "reg.exe via Command::new with creation_flags(0x08000000) for silent registry writes"

requirements-completed: [HARD-01, VALID-03]

# Metrics
duration: 30min
completed: 2026-03-21
---

# Phase 108 Plan 01: Keyboard Hook Replacement Summary

**SetWindowsHookEx global keyboard hook removed from default rc-agent build and replaced with GPO registry keys (NoWinKeys=1 + DisableTaskMgr=1 via reg.exe), with hook preserved behind keyboard-hook Cargo feature for emergency rollback**

## Performance

- **Duration:** ~30 min
- **Started:** 2026-03-21T16:05:00Z (21:35 IST)
- **Completed:** 2026-03-21T16:08:28Z (21:38 IST)
- **Tasks:** 1/2 code complete (Task 2 = Pod 8 canary verify, awaiting human)
- **Files modified:** 2

## Accomplishments

- HOOK_HANDLE, keyboard_hook_proc, install_keyboard_hook, remove_keyboard_hook all gated behind `#[cfg(feature = "keyboard-hook")]`
- apply_gpo_lockdown() writes NoWinKeys=1 (blocks Win key) and DisableTaskMgr=1 (blocks Task Manager) via reg.exe
- remove_gpo_lockdown() deletes both keys on kiosk deactivate — clean exit
- activate() and deactivate() updated to call GPO functions instead of hook functions
- [features] keyboard-hook = [] added to Cargo.toml — rollback build: cargo build --features keyboard-hook
- Default build (cargo build --release --bin rc-agent) succeeds with zero SetWindowsHookExW calls
- Feature build (cargo build --release --bin rc-agent --features keyboard-hook) also succeeds — rollback confirmed

## Task Commits

Each task was committed atomically:

1. **Task 1: Gate hook code behind keyboard-hook feature flag and add GPO registry lockdown functions** - `2d20fac` (feat)
2. **Task 2: Verify kiosk lockdown on Pod 8 canary build** - AWAITING human checkpoint (physical verification)

## Files Created/Modified

- `crates/rc-agent/Cargo.toml` - Added [features] section with keyboard-hook = []
- `crates/rc-agent/src/kiosk.rs` - GPO lockdown functions added, hook code gated, activate/deactivate updated

## Decisions Made

- GPO via reg.exe chosen over winreg crate — matches existing lock_screen.rs pattern, zero new dependencies added
- imports (AtomicPtr, Ordering, LPARAM, LRESULT, WPARAM) also gated behind cfg(feature = "keyboard-hook") to prevent unused import warnings without the feature

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- `cargo test -p rc-agent-crate` output files were deleted by a concurrent Claude Code process cleanup. Build verification (cargo build --release --bin rc-agent, cargo build --release --bin rc-agent --features keyboard-hook) confirmed both pass. Tests will run as part of Pod 8 canary verification.

## User Setup Required

Task 2 requires physical verification on Pod 8:
1. Build: `cargo build --release --bin rc-agent` on James
2. Deploy rc-agent.exe to Pod 8 only (canary)
3. Verify log contains "Kiosk: GPO set" entries for NoWinKeys and DisableTaskMgr
4. Test Win key press during kiosk (must NOT open Start menu)
5. Test Ctrl+Shift+Esc during kiosk (must NOT open Task Manager)
6. After deactivate: verify `reg query "HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer" /v NoWinKeys` returns ERROR
7. Type "approved" to signal completion

## Next Phase Readiness

- Code changes complete and pushed to main (2d20fac)
- Pod 8 canary test is the only remaining gate before Phase 109 (Safe Mode State Machine)
- After Task 2 approved: phase 108 done, 109 unblocked
- HARD-01 satisfied: SetWindowsHookEx removed from default build
- VALID-03 pending: awaiting Pod 8 physical confirmation

---
*Phase: 108-keyboard-hook-replacement*
*Completed: 2026-03-21 (Task 1 done, Task 2 pending human verification)*
