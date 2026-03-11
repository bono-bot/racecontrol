---
phase: 01-stability-sync-fixes
plan: 04
subsystem: api, billing, security
tags: [error-handling, unwrap, jwt, logging, rust]

requires:
  - phase: 01-stability-sync-fixes (plans 01-03)
    provides: wallet_transactions sync, PID cleanup, disconnect-pause billing
provides:
  - Hardened startup validation (JWT secret enforcement)
  - Panic-free error handling in routes, scheduler, pod_healer, websocket
  - Error visibility for silenced billing queries
affects: [billing, api, scheduler, pod_healer, security]

tech-stack:
  added: []
  patterns:
    - "map_err for logging before .ok() — preserves silent fallback but adds tracing"
    - "Pattern match extraction instead of unwrap after guard checks"
    - "expect() with descriptive message for compile-time-provable constants"

key-files:
  created: []
  modified:
    - crates/rc-core/src/main.rs
    - crates/rc-core/src/api/routes.rs
    - crates/rc-core/src/billing.rs
    - crates/rc-core/src/scheduler.rs
    - crates/rc-core/src/pod_healer.rs
    - crates/rc-core/src/ws/mod.rs

key-decisions:
  - "JWT secret: bail on startup instead of warn — production must set auth.jwt_secret"
  - "unwrap() replaced with pattern match or expect() with message, not unwrap_or"
  - ".ok() calls get map_err tracing before .ok() — keeps fallback behavior but logs errors"

patterns-established:
  - "map_err(|e| tracing::warn/error!(...)).ok() for silenced DB queries"
  - "Pattern match with extraction instead of guard + unwrap"

requirements-completed: []

duration: 4min
completed: 2026-03-11
---

# Phase 01 Plan 04: Error Handling Hardening Summary

**Reject default JWT secret at startup, replace 8 panic-risk unwrap() calls, add error logging to 7 silenced .ok() billing queries**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-11T03:55:09Z
- **Completed:** 2026-03-11T03:59:45Z
- **Tasks:** 4
- **Files modified:** 6

## Accomplishments
- rc-core now refuses to start with the default JWT secret (was only a warning)
- All panic-risk unwrap() calls in rc-core replaced with safe alternatives
- 7 critical billing DB queries now log errors instead of silently swallowing failures
- All 14 existing tests continue to pass

## Task Commits

Each task was committed atomically:

1. **Task 1: Reject startup with default JWT secret** - `5c9234c` (fix)
2. **Task 2: Replace unwrap() in routes.rs** - `31ef835` (fix)
3. **Task 3: Replace unwrap() in scheduler, pod_healer, ws** - `463966b` (fix)
4. **Task 4: Add error logging to silenced .ok() calls in billing** - `dbd37cd` (fix)

## Files Created/Modified
- `crates/rc-core/src/main.rs` - JWT secret startup validation (warn -> bail)
- `crates/rc-core/src/api/routes.rs` - Safe pattern match for share report and tournament registration
- `crates/rc-core/src/billing.rs` - Error logging on 7 silenced .ok() calls
- `crates/rc-core/src/scheduler.rs` - expect() with message for static time constants
- `crates/rc-core/src/pod_healer.rs` - match instead of unwrap on ping response
- `crates/rc-core/src/ws/mod.rs` - Debug format instead of unwrap on conn_id

## Decisions Made
- JWT secret: changed from warn to fatal error at startup. The production config already has a proper secret set, so this won't break existing deployments.
- Used `map_err(tracing::warn/error).ok()` pattern to preserve existing fallback behavior while adding visibility.
- Used `expect()` with descriptive message for `NaiveTime::from_hms_opt` calls since the values are compile-time constants that can never fail.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- P0 security (JWT secret) and P0 error handling (unwrap/ok) addressed
- Remaining P1 items: routes.rs splitting, integration tests, pod state race conditions
- Ready for next plan or phase transition

---
*Phase: 01-stability-sync-fixes*
*Completed: 2026-03-11*
