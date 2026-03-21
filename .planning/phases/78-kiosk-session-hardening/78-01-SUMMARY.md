---
phase: 78-kiosk-session-hardening
plan: 01
subsystem: infra
tags: [kiosk, edge, keyboard-hook, registry, usb, security]

requires:
  - phase: 77-transport-security
    provides: "HTTPS transport for kiosk browser"
provides:
  - "Hardened Edge kiosk launch with 12 security flags (DevTools, extensions, file system, print blocked)"
  - "Enhanced keyboard hook blocking F12, Ctrl+Shift+I/J, Ctrl+L"
  - "pod-lockdown.ps1 with USB, accessibility, TaskMgr registry lockdown + undo"
affects: [78-kiosk-session-hardening]

tech-stack:
  added: []
  patterns: ["defense-in-depth: browser flags + keyboard hook for same escape vector"]

key-files:
  created: []
  modified:
    - crates/rc-agent/src/lock_screen.rs
    - crates/rc-agent/src/kiosk.rs
    - deploy/pod-lockdown.ps1

key-decisions:
  - "Both --disable-dev-tools and --disable-dev-tools-extension included for Edge/Chrome flag compatibility"
  - "Keyboard hook blocks kept under 10ms -- GetAsyncKeyState is fast kernel syscall, no tracing/IO inside callback"
  - "USBSTOR Start=4 disables USB mass storage only, HID devices (wheelbases, mice) unaffected"
  - "Accessibility Flags values (506/122/58) disable keyboard shortcuts only, not the features themselves"

patterns-established:
  - "Defense-in-depth: browser flags + keyboard hook for same escape vector (DevTools)"
  - "Registry lockdown with matching -Undo: every Set-ItemProperty has a revert in the Undo block"

requirements-completed: [KIOSK-01, KIOSK-02, KIOSK-03, KIOSK-04]

duration: 2min
completed: 2026-03-21
---

# Phase 78 Plan 01: Kiosk Session Hardening Summary

**Edge kiosk hardened with 12 security flags + keyboard hook blocks (F12/Ctrl+Shift+I/J/Ctrl+L) + pod-lockdown.ps1 USB/accessibility/TaskMgr registry lockdown**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-21T00:57:35Z
- **Completed:** 2026-03-21T00:59:48Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Edge kiosk browser launches with 12 additional security flags blocking DevTools, extensions, file system access, print preview, and new web contents
- Keyboard hook blocks 4 additional key combinations (F12, Ctrl+Shift+I, Ctrl+Shift+J, Ctrl+L) for defense-in-depth against DevTools access
- pod-lockdown.ps1 extended with USB mass storage disable, Sticky/Filter/Toggle Keys hotkey disable, and Task Manager disable -- all with matching -Undo support

## Task Commits

Each task was committed atomically:

1. **Task 1: Harden Edge kiosk flags and keyboard hook** - `d1e2048` (feat)
2. **Task 2: Extend pod-lockdown.ps1 with USB, accessibility, and TaskMgr registry** - `1a5bbe3` (feat)

## Files Created/Modified
- `crates/rc-agent/src/lock_screen.rs` - Added 12 security flags to Edge kiosk launch args
- `crates/rc-agent/src/kiosk.rs` - Added F12, Ctrl+Shift+I/J, Ctrl+L blocks to keyboard hook
- `deploy/pod-lockdown.ps1` - Added USB, accessibility, TaskMgr lockdown sections with undo support

## Decisions Made
- Both --disable-dev-tools and --disable-dev-tools-extension included for Edge/Chrome flag compatibility
- Keyboard hook blocks use GetAsyncKeyState (fast kernel syscall) -- no tracing or IO inside callback to stay under 10ms
- USBSTOR Start=4 disables USB mass storage only -- HID devices (wheelbases, mice, keyboards) unaffected
- Accessibility Flags values (506/122/58) disable keyboard shortcuts only, not the accessibility features themselves

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- rc-agent builds clean with hardened kiosk flags and keyboard hook
- pod-lockdown.ps1 ready for deployment to pods via /exec or manual install
- Phase 78 Plan 02 (session timeout) and Plan 03 can proceed

---
*Phase: 78-kiosk-session-hardening*
*Completed: 2026-03-21*
