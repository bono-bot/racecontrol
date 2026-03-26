---
phase: 201-frontend-integration-type-sync
plan: "02"
subsystem: ui
tags: [kiosk, billing-states, typescript, react, reliability-warning]

dependency_graph:
  requires:
    - phase: 201-01
      provides: BillingSessionStatus 10-variant union in @racingpoint/types; AlternativeCombo type in @racingpoint/types
  provides:
    - Kiosk uses BillingSessionStatus from @racingpoint/types exclusively (local 6-variant type removed)
    - billing_session_changed handler treats cancelled_no_playable as terminal state
    - LiveSessionPanel shows Game Loading spinner for waiting_for_game (not countdown)
    - LiveSessionPanel shows amber Relaunching banner for paused_game_pause
    - LiveSessionPanel shows orange Disconnected banner for paused_disconnect
    - KioskPodCard.derivePodState maps waiting_for_game->loading, paused_game_pause/paused_disconnect->crashed
    - PodKioskView.deriveKioskState maps waiting_for_game->launching with blue spinner
    - SessionTimer countdown only runs when billing.status === "active"
    - Reliability warning banner in SetupWizard review step for combos < 70% success rate
    - Suggest Alternative modal with top-3 combo alternatives from GET /api/v1/games/alternatives
  affects:
    - kiosk billing display (any pod card or session panel using billing status)
    - SetupWizard game picker flow (review step now fetches reliability data)

tech_stack:
  added: []
  patterns:
    - Status-driven countdown guard — useEffect returns early unless billing.status === "active" (not just checking paused_manual)
    - Billing status takes priority over gameInfo.game_state in derivePodState — waiting_for_game mapped before game_state checks
    - Reliability warning fetched lazily on review step entry (not on every car/track selection)
    - Alternatives modal scoped to top-3 results from API with success_rate percentage display

key_files:
  created: []
  modified:
    - kiosk/src/lib/types.ts
    - kiosk/src/hooks/useKioskSocket.ts
    - kiosk/src/components/LiveSessionPanel.tsx
    - kiosk/src/components/KioskPodCard.tsx
    - kiosk/src/components/PodKioskView.tsx
    - kiosk/src/components/SessionTimer.tsx
    - kiosk/src/lib/api.ts
    - kiosk/src/components/SetupWizard.tsx

key-decisions:
  - "Local BillingStatus 6-variant type removed from types.ts; BillingSessionStatus imported into scope for RecentSession.status"
  - "TERMINAL_STATUSES const array used in billing_session_changed handler — cleaner than chained === checks; includes cancelled_no_playable as 4th terminal state"
  - "billing.status takes priority over gameInfo.game_state in derivePodState — waiting_for_game maps to loading before game_state checks to ensure correct state even before gameInfo arrives via WS"
  - "Reliability warning fetches only when entering review step with car+track — not on every selection to avoid unnecessary API calls; fetch failure silently skips warning (non-critical)"
  - "Alternatives modal shows top-3 combos from API; dismiss-only (no auto-selection) to keep staff in control of final combo choice"

requirements-completed:
  - KIOSK-01
  - KIOSK-02
  - KIOSK-03
  - KIOSK-04
  - KIOSK-05
  - SYNC-02

duration: 12min
completed: "2026-03-26"
---

# Phase 201 Plan 02: Kiosk Frontend Type Sync and UI States Summary

**Kiosk updated to 10-variant BillingSessionStatus, local type removed, game loading/crash-recovery/disconnect UI added, reliability warning with alternatives modal on review step.**

## Performance

- **Duration:** 12 min
- **Started:** 2026-03-26T11:00:00Z
- **Completed:** 2026-03-26T11:12:00Z
- **Tasks:** 2
- **Files modified:** 8

## Accomplishments

- Removed local 6-variant `BillingStatus` type from `kiosk/src/lib/types.ts`; `RecentSession.status` now uses `BillingSessionStatus` from `@racingpoint/types`
- `billing_session_changed` WS handler now treats `cancelled_no_playable` as terminal (session removed from billingTimers map)
- `LiveSessionPanel`, `KioskPodCard`, `PodKioskView`, and `SessionTimer` all correctly handle `waiting_for_game`, `paused_game_pause`, `paused_disconnect` with appropriate UI and paused countdown
- Reliability warning banner added to SetupWizard review step; shows success rate and "Suggest Alternative" button for combos below 70%

## Task Commits

1. **Task 1: Remove local types and update kiosk socket handler** - `86ae2a46` (feat)
2. **Task 2: Kiosk UI for loading, crash recovery, and reliability warning** - `0593b1b9` (feat)

## Files Created/Modified

- `kiosk/src/lib/types.ts` - Removed local `BillingStatus` type; `BillingSessionStatus` imported for `RecentSession.status`
- `kiosk/src/hooks/useKioskSocket.ts` - `TERMINAL_STATUSES` const array; `cancelled_no_playable` added as 4th terminal state
- `kiosk/src/components/LiveSessionPanel.tsx` - Countdown only runs on `active`; game loading spinner, amber relaunch, orange disconnect banners
- `kiosk/src/components/KioskPodCard.tsx` - `derivePodState` maps new billing states; countdown guard changed to `status !== 'active'`
- `kiosk/src/components/PodKioskView.tsx` - `deriveKioskState` maps `waiting_for_game`->launching; `LaunchingView` blue spinner; crash recovery + disconnect banners in `InSessionView`
- `kiosk/src/components/SessionTimer.tsx` - Countdown guard `status === 'active'` only; status label shows Game Loading/Relaunching/Disconnected
- `kiosk/src/lib/api.ts` - `getAlternatives()` endpoint added; `AlternativeCombo` imported from `@racingpoint/types`
- `kiosk/src/components/SetupWizard.tsx` - Reliability warning state; `useEffect` fetches alternatives on review step; amber banner + alternatives modal

## Decisions Made

- Local BillingStatus removed — imported BillingSessionStatus into scope via `import type` alongside the `export type {}` re-export block, since TypeScript requires a separate `import type` for local use
- TERMINAL_STATUSES const array pattern used in socket handler for readability and easy extension
- `billing.status === "waiting_for_game"` checked before `gameInfo?.game_state === "loading"` in `derivePodState` — billing status is more authoritative than game process state which may lag

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] BillingSessionStatus not in scope for local use in types.ts**
- **Found during:** Task 1 (TSC check after removing BillingStatus)
- **Issue:** `export type { BillingSessionStatus }` re-exports the type but does not bring it into local scope — `RecentSession.status: BillingSessionStatus` caused `TS2304: Cannot find name`
- **Fix:** Added `import type { GameState, BillingSessionStatus } from '@racingpoint/types'` — GameState was already imported, added BillingSessionStatus to same import
- **Files modified:** `kiosk/src/lib/types.ts`
- **Verification:** `tsc --noEmit` exits 0
- **Committed in:** `86ae2a46` (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Auto-fix was a one-line change required for TypeScript correctness. No scope creep.

## Issues Encountered

None beyond the auto-fixed TypeScript scope issue above.

## Next Phase Readiness

- Kiosk now correctly renders all 10 billing states with appropriate UI
- Reliability warning fetches from `GET /api/v1/games/alternatives` — this endpoint was defined in Phase 200-02; if not yet deployed, the warning silently skips (non-critical path)
- Phase 201-03 (web frontend) can proceed independently

---
*Phase: 201-frontend-integration-type-sync*
*Completed: 2026-03-26*
