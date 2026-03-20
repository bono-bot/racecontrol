---
phase: 76-api-authentication-admin-protection
plan: 05
subsystem: ui
tags: [nextjs, jwt, auth, idle-timeout, pin-login, localstorage]

# Dependency graph
requires:
  - phase: 76-02
    provides: "POST /api/v1/auth/admin-login endpoint with argon2 PIN verification"
provides:
  - "PIN login page at /login with error handling"
  - "AuthGate client component wrapping all dashboard pages"
  - "useIdleTimeout hook with 15-minute auto-lock"
  - "JWT-bearing Authorization headers on all fetchApi calls"
  - "Auto-redirect to /login on 401 responses"
affects: [76-06-strict-enforcement]

# Tech tracking
tech-stack:
  added: []
  patterns: ["AuthGate wrapper pattern for client-side route protection", "useEffect + hydrated flag for SSR-safe localStorage access", "idle timeout via window event listeners with timer reset"]

key-files:
  created:
    - web/src/lib/auth.ts
    - web/src/app/login/page.tsx
    - web/src/components/AuthGate.tsx
    - web/src/hooks/useIdleTimeout.ts
  modified:
    - web/src/lib/api.ts
    - web/src/app/layout.tsx

key-decisions:
  - "JWT stored in localStorage with client-side expiry check (server validates on every request)"
  - "AuthGate skips redirect when pathname is /login to avoid redirect loop"
  - "401 responses from fetchApi auto-clear token and redirect to /login"

patterns-established:
  - "AuthGate wrapper: all pages gated by default, login page excluded via pathname check"
  - "useIdleTimeout hook: mousemove/keydown/mousedown/touchstart/scroll reset a setTimeout timer"

requirements-completed: [ADMIN-01, ADMIN-03]

# Metrics
duration: 8min
completed: 2026-03-20
---

# Phase 76 Plan 05: Dashboard Frontend PIN Gate Summary

**PIN login page, AuthGate route wrapper, and 15-minute idle timeout for the Next.js dashboard at :3200**

## Performance

- **Duration:** ~8 min
- **Started:** 2026-03-20T13:03:00Z
- **Completed:** 2026-03-20T13:11:00Z
- **Tasks:** 3 (2 auto + 1 human-verify)
- **Files modified:** 6

## Accomplishments
- Dashboard at :3200 now requires PIN authentication before any content is visible
- All API requests from dashboard carry JWT in Authorization: Bearer header
- 15-minute inactivity timeout auto-locks the dashboard and redirects to /login
- 401 responses from server auto-clear stale JWT and redirect to login
- No content flash before redirect (hydrated flag pattern prevents SSR mismatch)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create auth helpers, login page, AuthGate wrapper, and idle timeout hook** - `c615b3f` (feat)
2. **Task 2: Wire AuthGate into layout and add JWT to all API requests** - `43fe875` (feat)
3. **Task 3: Verify dashboard PIN gate and idle timeout** - human-verify checkpoint (approved)

## Files Created/Modified
- `web/src/lib/auth.ts` - JWT storage helpers: getToken, setToken, clearToken, isAuthenticated with expiry check
- `web/src/app/login/page.tsx` - PIN entry page with form, error handling, auto-redirect if already authenticated
- `web/src/components/AuthGate.tsx` - Client wrapper checking auth on mount, redirecting unauthenticated users to /login
- `web/src/hooks/useIdleTimeout.ts` - Hook clearing JWT after 15 min of no user interaction events
- `web/src/lib/api.ts` - Updated fetchApi to attach Authorization header and auto-redirect on 401
- `web/src/app/layout.tsx` - Wrapped children with AuthGate component

## Decisions Made
- JWT stored in localStorage with client-side expiry check -- server validates on every API request, client check only prevents stale UI
- AuthGate uses pathname check to skip redirect on /login page, avoiding infinite redirect loop
- 401 auto-redirect in fetchApi ensures expired JWTs during a shift are handled gracefully
- useIdleTimeout listens to 5 event types (mousemove, keydown, mousedown, touchstart, scroll) with passive listeners for performance

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Frontend PIN gate complete -- dashboard fully protected
- Ready for Plan 76-06 (switch permissive to strict JWT enforcement on backend)
- All 5 of 6 Phase 76 plans now complete

## Self-Check: PASSED

- All 6 source files verified present on disk
- Both task commits (c615b3f, 43fe875) verified in git history

---
*Phase: 76-api-authentication-admin-protection*
*Completed: 2026-03-20*
