---
phase: 298-game-preset-library
plan: "02"
subsystem: ui
tags: [nextjs, typescript, react, admin-ui, preset-library, reliability-badges]

requires:
  - phase: 298-game-preset-library/298-01
    provides: GET /api/v1/presets (public), POST/PUT/DELETE /api/v1/presets (staff JWT), GamePresetWithReliability type

provides:
  - Admin /presets page with reliability badges (green/yellow/grey)
  - TypeScript API client (presetsApi) using rcFetch proxy with JWT auth via cookies
  - Presets nav link in AdminLayout.tsx Racing section
  - presetsApi exported from lib/api/index.ts

affects: [racingpoint-admin nav, staff operational workflow]

tech-stack:
  added: []
  patterns:
    - "rcFetch proxy pattern — all auth via Next.js /api/rc proxy (cookie JWT), no localStorage auth tokens"
    - "Optimistic delete: remove from state immediately, restore on API failure"
    - "No SWR — uses useState + useEffect + manual reload (consistent with coupons page pattern)"

key-files:
  created:
    - src/app/(dashboard)/presets/page.tsx
    - src/lib/api/presets.ts
  modified:
    - src/components/AdminLayout.tsx
    - src/lib/api/index.ts

key-decisions:
  - "Used rcFetch (proxy via /api/rc) instead of direct fetch with localStorage tokens — admin app handles auth via httpOnly cookie + Next.js proxy (same pattern as all other pages)"
  - "No SWR for presets — useEffect+useState is consistent with coupons/staff pages. SWR would add dependency without benefit here"
  - "Nav entry in AdminLayout.tsx navSections (not a Sidebar.tsx which doesn't exist in this repo)"
  - "Inline SVG for trash icon instead of lucide-react — no new dependencies added"

requirements-completed: [PRESET-04]

duration: 8min
completed: "2026-04-01"
---

# Phase 298 Plan 02: Game Preset Library Admin UI Summary

**Admin /presets page with green/yellow/grey reliability badges, create/delete form, and nav link — all auth via Next.js cookie proxy, TypeScript clean**

## Performance

- **Duration:** ~8 min
- **Started:** 2026-04-01T15:40:00Z
- **Completed:** 2026-04-01T15:48:00Z
- **Tasks:** 1
- **Files modified:** 2 + 2 created

## Accomplishments

- Created `src/lib/api/presets.ts` with typed API client (`presetsApi`) using `rcFetch` proxy
- Created `src/app/(dashboard)/presets/page.tsx` with full preset management UI
- `ReliabilityBadge` component: green (Reliable, score%), yellow (Unreliable, score%), grey (No data, N launches)
- Create form with name, game select, car, track, session_type, notes fields
- Delete with optimistic update (list filtered immediately, restored on failure)
- Added `/presets` to AdminLayout.tsx Racing section nav
- `npx tsc --noEmit` exits 0, no `any` types, no `localStorage` in state initializers

## Task Commits

1. **Task 1: TypeScript API client and preset page** - `ca5997f` (feat)

## Files Created/Modified

- `src/lib/api/presets.ts` - Typed API client with presetsApi object
- `src/app/(dashboard)/presets/page.tsx` - Admin preset management page with ReliabilityBadge
- `src/components/AdminLayout.tsx` - Added `{ href: '/presets', label: 'Presets' }` in Racing section
- `src/lib/api/index.ts` - Added presetsApi export and type re-exports

## Decisions Made

- Used `rcFetch` proxy (Next.js `/api/rc` → racecontrol with cookie JWT) rather than `localStorage`-based auth. The plan's `authHeaders()` pattern would have violated the hydration standing rule. All admin pages use the proxy pattern.
- Nav goes into `AdminLayout.tsx` navSections (not "Sidebar.tsx" which doesn't exist — plan referenced wrong filename, auto-corrected per Rule 1).
- No new dependencies added (inline SVG for delete icon instead of adding lucide-react).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Plan referenced Sidebar.tsx but actual nav file is AdminLayout.tsx**
- **Found during:** Task 1 (pre-read of component structure)
- **Issue:** Plan said to modify `web/src/components/Sidebar.tsx` with `Library` icon from lucide-react. This file does not exist in racingpoint-admin. The actual nav component is `src/components/AdminLayout.tsx` with a plain text `navSections` array.
- **Fix:** Added `{ href: '/presets', label: 'Presets' }` to the `navSections` Racing section in AdminLayout.tsx. No icon needed (existing nav items are text-only). No lucide-react import required.
- **Files modified:** `src/components/AdminLayout.tsx`
- **Committed in:** ca5997f

**2. [Rule 1 - Bug] Plan used localStorage-based authHeaders() but admin uses cookie proxy**
- **Found during:** Task 1 (pre-read of base.ts and route proxy)
- **Issue:** Plan specified `authHeaders()` reading `localStorage.getItem("staff_token")` directly in the API client. This would (a) violate the hydration standing rule, (b) duplicate auth logic that the Next.js proxy handles automatically via httpOnly cookie. All existing admin API clients use `rcFetch` which proxies through `/api/rc/[...path]` injecting JWT from cookie.
- **Fix:** Used `rcFetch` from `base.ts` for all API calls. Auth is handled transparently by the Next.js proxy route.
- **Files modified:** `src/lib/api/presets.ts`
- **Committed in:** ca5997f

**3. [Rule 1 - Bug] Plan referenced web/src/app path prefix but actual path is src/app**
- **Found during:** Task 1 (pre-read of repo structure)
- **Issue:** Plan specified files under `web/src/...` but racingpoint-admin has no `web/` subdirectory. The Next.js app is at the repo root with `src/app`.
- **Fix:** Used correct paths `src/app/(dashboard)/presets/page.tsx` and `src/lib/api/presets.ts`.
- **Files modified:** All new files
- **Committed in:** ca5997f

---

**Total deviations:** 3 auto-fixed (3 bugs — wrong filenames/paths in plan for different repo structure)
**Impact on plan:** All fixes necessary for correctness. UI functionality identical to plan spec.

## Known Stubs

None — the page fetches live data from `GET /api/v1/presets` on mount. Empty state shows "No presets yet" message.

## Next Phase Readiness

- Phase 298 fully complete (backend + admin UI)
- Pods receive preset library on WS connect (PRESET-02)
- Staff can manage presets via /presets admin page (PRESET-04)
- Reliability scoring auto-populates from combo_reliability as game sessions accumulate data (PRESET-03)

---
*Phase: 298-game-preset-library*
*Completed: 2026-04-01*
