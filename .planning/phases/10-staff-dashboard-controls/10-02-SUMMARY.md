---
phase: 10-staff-dashboard-controls
plan: 02
subsystem: ui
tags: [nextjs, react, typescript, kiosk, lockdown, pod-control]

# Dependency graph
requires:
  - phase: 10-staff-dashboard-controls/10-01
    provides: lockdown_pod + lockdown_all_pods + restart_all backend routes

provides:
  - kiosk /control page with 5 bulk action buttons (Wake All, Shutdown All, Restart All, Lock All, Unlock All)
  - per-pod lockdown toggle (padlock icon) in each pod's header bar
  - api.ts: lockdownPod(), lockdownAllPods(), restartAllPods() client functions
  - optimistic UI state for per-pod lockdown (lockedPods Set<string>)

affects: [phase-11-customer-experience]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Optimistic UI for toggle state using useState<Set<string>>
    - Bulk action confirmation dialogs for destructive operations (Restart All, Lock All)
    - Per-pod header buttons conditionally rendered based on isOnline status

key-files:
  created: []
  modified:
    - kiosk/src/lib/api.ts
    - kiosk/src/app/control/page.tsx

key-decisions:
  - "Optimistic UI for lockdown toggle — icon reflects last sent action without waiting for server roundtrip"
  - "Lock All updates optimistic state for all currently-online pods; Unlock All clears all"
  - "Unlock All is non-destructive — no confirmation dialog required"
  - "Lockdown toggle placed before Enable/Disable toggle in pod header button order"

patterns-established:
  - "Pattern: confirmation dialogs only on destructive bulk actions (Shutdown, Restart, Lock All)"
  - "Pattern: Unlock All / Wake All execute immediately — safety net actions need no friction"

requirements-completed: [PWR-04, PWR-05, PWR-06, KIOSK-01, KIOSK-02]

# Metrics
duration: 12min
completed: 2026-03-14
---

# Phase 10 Plan 02: Staff Dashboard Controls (Frontend Wiring) Summary

**Kiosk /control page wired to lockdown + restart-all backend: 5 bulk buttons, per-pod padlock toggle, optimistic UI — TypeScript clean, Next.js build passing**

## Performance

- **Duration:** ~12 min
- **Started:** 2026-03-14T00:00:00Z
- **Completed:** 2026-03-14T00:12:00Z
- **Tasks:** 1 auto complete, 1 checkpoint (human-verify pending)
- **Files modified:** 2

## Accomplishments
- Added `lockdownPod()`, `lockdownAllPods()`, `restartAllPods()` to api.ts alongside existing power functions
- Bulk action bar now shows 5 buttons: Wake All, Shutdown All, Restart All, Lock All, Unlock All
- Each online pod header now has a padlock icon toggle with optimistic state (bright orange = locked, dim = unlocked)
- Lock All and Restart All show confirmation dialogs; Unlock All executes immediately
- Kiosk Next.js app compiles with zero TypeScript errors and clean build

## Task Commits

Each task was committed atomically:

1. **Task 1: Add API client functions and UI controls to /control page** - `2b4f9f6` (feat)
2. **Task 2: Verify pod controls UI in /control page** - checkpoint:human-verify (pending user verification)

## Files Created/Modified
- `kiosk/src/lib/api.ts` - Added restartAllPods, lockdownPod, lockdownAllPods API functions
- `kiosk/src/app/control/page.tsx` - Added lockedPods state, 4 new handlers, 3 bulk buttons, per-pod lockdown toggle

## Decisions Made
- Optimistic UI for lockdown toggle: icon reflects last action sent to server, no roundtrip wait. Consistent with existing enable/disable toggle behaviour.
- Lock All optimistically marks all currently-online pods as locked in local state.
- Unlock All clears local state entirely with no confirmation — unlocking is always safe.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None — TypeScript type-check passed clean, Next.js build succeeded with no errors.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Task 2 (checkpoint:human-verify) requires staff to visually verify the /control page:
1. Start kiosk dev server: `cd kiosk && npm run dev`
2. Open http://localhost:3400/staff — log in with staff PIN
3. Navigate to /control page
4. Confirm 5 bulk buttons visible (Wake All, Shutdown All, Restart All, Lock All, Unlock All)
5. Confirm each online pod header has padlock toggle icon
6. Test Lock All (confirm dialog), Unlock All (no dialog), Restart All (confirm dialog)
7. Test per-pod padlock — icon toggles between locked (bright orange) and unlocked (dim)

Once visual verification passes, Phase 10 Plan 02 is complete.

---
*Phase: 10-staff-dashboard-controls*
*Completed: 2026-03-14*
