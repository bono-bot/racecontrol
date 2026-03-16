---
phase: 05-blanking-screen-protocol
plan: 02
subsystem: auth
tags: [rust, axum, sqlite, powershell, kiosk, anti-cheat]

# Dependency graph
requires:
  - phase: 05-blanking-screen-protocol
    provides: LaunchSplash state, SCREEN-01/SCREEN-02 lock screen ordering, enforce_safe_state() with DIALOG_PROCESSES
provides:
  - PinSource enum (Pod/Kiosk/Pwa) with standardized INVALID_PIN_MESSAGE constant
  - Unified PIN error surface across pod lock screen, kiosk, and PWA
  - deploy/pod-lockdown.ps1 for one-time SCREEN-03 kiosk hardening
  - Anti-cheat verification: iRacing, F1 25, LMU all confirmed safe with rc-agent active
affects: [phase-06, any future auth changes, pod setup procedures]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - PinSource enum used for logging-only differentiation — validation is identical across all entry points
    - INVALID_PIN_MESSAGE const as single source of truth for user-facing error text
    - PowerShell registry-based kiosk lockdown with idempotent apply and -Undo recovery

key-files:
  created:
    - deploy/pod-lockdown.ps1
  modified:
    - crates/racecontrol/src/auth/mod.rs

key-decisions:
  - "INVALID_PIN_MESSAGE uses em dash (U+2014): 'Invalid PIN — please try again or see reception.' — same across all 3 entry points (pod WS, kiosk HTTP, PWA HTTP)"
  - "PinSource enum is for logging only — validation SQL and post-validation billing logic are identical; no branching on source in business logic"
  - "Alt+Tab NOT blocked in pod-lockdown.ps1 — per CONTEXT.md decision, customer sees lock screen behind game which is acceptable"
  - "pod-lockdown.ps1 is a deploy artifact, not rc-agent runtime code — runs once per pod via pod-agent /exec or pendrive"
  - "Anti-cheat gate approved: iRacing, F1 25, LMU all passed testing with Phase 5 rc-agent active"

patterns-established:
  - "Error message constants: user-facing strings live in pub(crate) const to prevent message drift across code paths"
  - "PinSource logging pattern: log source at validation time with tracing::info!, keep business logic source-agnostic"

requirements-completed: [AUTH-01, PERF-02, SCREEN-03]

# Metrics
duration: ~35min
completed: 2026-03-13
---

# Phase 5 Plan 02: PIN Unification, Pod Lockdown, Anti-Cheat Gate Summary

**Unified PIN error message across all 3 auth surfaces (pod/kiosk/PWA) via PinSource enum + INVALID_PIN_MESSAGE const, plus idempotent pod-lockdown.ps1 for taskbar/Win-key/WU suppression, verified anti-cheat safe on iRacing, F1 25, LMU**

## Performance

- **Duration:** ~35 min
- **Started:** 2026-03-13T03:34:00Z
- **Completed:** 2026-03-13T09:10:00Z
- **Tasks:** 3 (2 auto + 1 human-verify checkpoint)
- **Files modified:** 2

## Accomplishments
- AUTH-01 closed: validate_pin() and validate_pin_kiosk() now return identical error text via INVALID_PIN_MESSAGE const — no more split "Invalid PIN or no pending assignment for this pod" vs "Invalid PIN. Please check with reception."
- SCREEN-03 closed: deploy/pod-lockdown.ps1 covers taskbar hide (StuckRects3), Win key block (NoWinKeys), Windows Update restart suppression (NoAutoRebootWithLoggedOnUsers), with -Undo recovery for admin use
- PERF-02 verified: SQLite PIN query timing tested via pin_validation_timing_proxy — well within 200ms threshold on local DB
- Anti-cheat gate passed: iRacing, F1 25, and LMU all run without anti-cheat kicks or bans with Phase 5 rc-agent active
- Full test suite green: 210 tests across 3 crates (55 rc-common, 52 rc-agent, 103 racecontrol)

## Task Commits

Each task was committed atomically:

1. **Task 1: Unify PIN validation with shared validate_pin_inner() and PinSource enum** - `c370cdc` (feat)
2. **Task 2: Create pod lockdown PowerShell script for SCREEN-03** - `c76e634` (feat)
3. **Task 3: Anti-cheat compatibility gate** - (checkpoint:human-verify, no code commit — human approved)

## Files Created/Modified
- `crates/racecontrol/src/auth/mod.rs` - Added PinSource enum (Pod/Kiosk/Pwa), INVALID_PIN_MESSAGE const, standardized both error paths, added tracing with source, added 3 tests
- `deploy/pod-lockdown.ps1` - New idempotent kiosk lockdown script: taskbar auto-hide, Win key block, WU suppression, -Undo flag, Explorer restart

## Decisions Made
- INVALID_PIN_MESSAGE uses em dash (U+2014) not a double dash — consistent with existing render_pin_page text
- PinSource is logging-only — zero branching in business logic on source; all paths hit identical SQL and billing flow
- Alt+Tab not blocked in pod-lockdown.ps1 — per CONTEXT.md locked decision; lock screen is visible behind game, acceptable UX
- pod-lockdown.ps1 is a one-time deploy artifact, not rc-agent runtime — keeps agent binary clean, lockdown survives agent restarts

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
**Pod lockdown not yet deployed to pods.** When ready to apply SCREEN-03 hardening:
```
# Deploy via pod-agent /exec (write JSON payload, then POST):
# Download: http://192.168.31.27:9998/pod-lockdown.ps1
# Run: powershell -ExecutionPolicy Bypass -File C:\RacingPoint\pod-lockdown.ps1
# To revert: powershell -ExecutionPolicy Bypass -File C:\RacingPoint\pod-lockdown.ps1 -Undo
```
Copy pod-lockdown.ps1 to deploy-staging/ and deploy to all 8 pods when convenient.

## Next Phase Readiness
- Phase 5 (Blanking Screen Protocol) is complete: SCREEN-01, SCREEN-02, SCREEN-03, AUTH-01, PERF-02 all closed
- All 5 requirements from Phase 5 delivered across Plans 01 and 02
- Pod lockdown script ready for deployment to all 8 pods at next maintenance window
- No blockers for any future phase

---
*Phase: 05-blanking-screen-protocol*
*Completed: 2026-03-13*
