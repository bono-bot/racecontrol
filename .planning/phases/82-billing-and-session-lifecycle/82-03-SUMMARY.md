---
phase: 82-billing-and-session-lifecycle
plan: "03"
subsystem: ui
tags: [next.js, react, tailwind, billing, kiosk, typescript]

requires:
  - phase: 82-01
    provides: sim_type column in billing_rates DB table, API response includes sim_type

provides:
  - BillingRate TypeScript interface with sim_type field
  - Admin pricing page Game column with inline select editor
  - SIM_TYPE_LABELS mapping for all 9 sim types
  - Kiosk Loading state badge (amber, text-amber-400 bg-amber-400/10)
  - Loading count-up timer in M:SS format with amber monospace font
  - GameLogo shown during loading state
  - GameState union extended with "loading" literal

affects:
  - Any component using KioskPodState or GameState types
  - Any component rendering BillingRate data

tech-stack:
  added: []
  patterns:
    - "SIM_TYPE_LABELS Record<string, string> + SIM_TYPE_OPTIONS array for consistent sim_type rendering"
    - "count-up timer via useRef<number | null> + useState + useEffect with clearInterval on state exit"
    - "derivePodState loading branch before running — ordering matters for correct state resolution"

key-files:
  created: []
  modified:
    - web/src/lib/api.ts
    - web/src/app/billing/pricing/page.tsx
    - kiosk/src/lib/types.ts
    - kiosk/src/components/KioskPodCard.tsx

key-decisions:
  - "GameState union must include 'loading' for TypeScript to accept game_state === 'loading' comparisons"
  - "Loading label uses GAME_DISPLAY[sim_type].name (e.g. 'F1 25') not abbr — full name fits the badge context"
  - "Loading timer uses useRef to persist startTime across re-renders without resetting; null = not in loading"
  - "Both compact and full card variants get loading body section and border styling (amber-500/40)"

patterns-established:
  - "SIM_TYPE_LABELS + SIM_TYPE_OPTIONS pattern: define once at module level, reuse in both view and edit contexts"
  - "Amber (text-amber-400 bg-amber-400/10) is the 'pending billing' semantic color for loading and waiting states"

requirements-completed: [BILL-03, BILL-05]

duration: 8min
completed: 2026-03-21
---

# Phase 82 Plan 03: UI Updates — Per-Game Rates Column and Loading State Summary

**Game column in admin billing rates table (SIM_TYPE_LABELS + inline select editor) and kiosk Loading state badge with count-up timer (amber, M:SS, resets on transition to on_track)**

## Performance

- **Duration:** ~8 min
- **Started:** 2026-03-21T04:01:00Z
- **Completed:** 2026-03-21T04:09:46Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Admin pricing page: "Game" column between Threshold and Rate showing mapped labels (AC, F1 25, iRacing, etc.) or "All games" for null; inline edit replaces cell with select dropdown; Add Rate form includes game selector
- BillingRate TypeScript interface now includes `sim_type: string | null`
- Kiosk: Loading state added to KioskPodState and GameState unions; derivePodState returns "loading" before "running" check; StateLabel shows amber "Loading..." or "Loading F1 25..." badge
- Count-up timer in M:SS with amber monospace font; resets to zero when leaving loading state; shown in both compact and full card variants
- GameLogo visible during game_state === "loading"

## Task Commits

1. **Task 1: Admin pricing page — Game column with sim_type dropdown** - `4abf16e` (feat)
2. **Task 2: Kiosk Loading state badge with count-up timer** - `2bffdd2` (feat)

**Plan metadata:** (see final commit below)

## Files Created/Modified
- `web/src/lib/api.ts` - BillingRate interface: added `sim_type: string | null`
- `web/src/app/billing/pricing/page.tsx` - SIM_TYPE_LABELS, SIM_TYPE_OPTIONS, Game column, inline select, Add Rate form with game selector, Save Rate / Discard copy
- `kiosk/src/lib/types.ts` - KioskPodState union: added "loading"; GameState union: added "loading"
- `kiosk/src/components/KioskPodCard.tsx` - derivePodState loading branch, StateLabel loading entry, loadingElapsed state + useEffect timer, loading body section (both variants), amber border styling

## Decisions Made
- `GameState` union needed `"loading"` added — TypeScript refuses `game_state === "loading"` comparison otherwise (Rule 1 auto-fix)
- Loading label uses `GAME_DISPLAY[sim_type].name` (full name like "F1 25") since the badge shows the game being loaded
- `loadingStartRef.current = null` used as sentinel for "not in loading state"; timer only starts when `state === "loading"` and ref is null

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Added "loading" to GameState union**
- **Found during:** Task 2 (kiosk Loading state badge)
- **Issue:** `GameState = "idle" | "launching" | "running" | "stopping" | "error"` — TypeScript error on `gameInfo?.game_state === "loading"` comparison (no overlap)
- **Fix:** Added `"loading"` to GameState union in `kiosk/src/lib/types.ts`
- **Files modified:** kiosk/src/lib/types.ts
- **Verification:** `npx next build` passes with no TypeScript errors
- **Committed in:** 2bffdd2 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 - type mismatch)
**Impact on plan:** Necessary correctness fix — GameState must include the new game_state value the server now emits. No scope creep.

## Issues Encountered
- TypeScript rejected `game_state === "loading"` because `GameState` type predated the loading state added in Phase 82. Auto-fixed by extending the union.

## Next Phase Readiness
- Admin billing rates table shows per-game rate rows with Game column
- Kiosk pod card correctly shows Loading badge with timer during shader compilation/game loading phase
- Ready for Phase 82-04 if applicable, or downstream phases consuming billing session lifecycle

## Self-Check: PASSED

- FOUND: web/src/lib/api.ts
- FOUND: web/src/app/billing/pricing/page.tsx
- FOUND: kiosk/src/lib/types.ts
- FOUND: kiosk/src/components/KioskPodCard.tsx
- FOUND: 82-03-SUMMARY.md
- FOUND commit 4abf16e (Task 1)
- FOUND commit 2bffdd2 (Task 2)

---
*Phase: 82-billing-and-session-lifecycle*
*Completed: 2026-03-21*
