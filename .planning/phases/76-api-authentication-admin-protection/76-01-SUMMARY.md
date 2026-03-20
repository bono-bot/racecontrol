---
phase: 76-api-authentication-admin-protection
plan: 01
subsystem: auth
tags: [jwt, axum-middleware, staff-auth, route-tiering, expand-migrate-contract]

# Dependency graph
requires:
  - phase: 75-security-audit-foundations
    provides: "Route inventory with tier classification (SECURITY-AUDIT.md), env var overrides for jwt_secret"
provides:
  - "StaffClaims struct for staff JWT tokens"
  - "require_staff_jwt strict middleware (returns 401)"
  - "require_staff_jwt_permissive middleware (logs warnings, allows through)"
  - "create_staff_jwt helper function for token generation"
  - "4-tier route split: public_routes, customer_routes, staff_routes, service_routes"
affects: [76-02, 76-03, dashboard, kiosk, rc-agent]

# Tech tracking
tech-stack:
  added: []
  patterns: ["from_fn_with_state for axum middleware with AppState", "expand-migrate-contract for auth rollout"]

key-files:
  created:
    - "crates/racecontrol/src/auth/middleware.rs"
  modified:
    - "crates/racecontrol/src/auth/mod.rs"
    - "crates/racecontrol/src/api/routes.rs"
    - "crates/racecontrol/src/api/mod.rs"
    - "crates/racecontrol/src/main.rs"

key-decisions:
  - "Permissive mode for initial deploy -- logs unauthenticated staff requests without rejecting (expand phase of expand-migrate-contract)"
  - "StaffClaims requires role=staff field -- customer JWTs (no role field) are automatically rejected by deserialization"
  - "extract_staff_claims helper shared between strict and permissive middleware to avoid code duplication"
  - "api_routes() signature changed to accept Arc<AppState> for from_fn_with_state"

patterns-established:
  - "Staff auth middleware pattern: from_fn_with_state(state, require_staff_jwt_permissive) on staff sub-router"
  - "Route tiering pattern: public_routes + customer_routes + staff_routes + service_routes merged into api_routes()"

requirements-completed: [AUTH-01, AUTH-02, AUTH-03, SESS-01]

# Metrics
duration: 10min
completed: 2026-03-20
---

# Phase 76 Plan 01: Staff JWT Middleware and Tiered Route Split Summary

**Staff JWT middleware with strict/permissive variants and 4-tier route split protecting 172+ staff routes via expand-migrate-contract pattern**

## Performance

- **Duration:** 10 min
- **Started:** 2026-03-20T12:48:07Z
- **Completed:** 2026-03-20T12:58:30Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- StaffClaims struct with role-based JWT validation (rejects customer tokens automatically)
- require_staff_jwt strict middleware + require_staff_jwt_permissive variant for safe rollout
- 269 routes split into 4 tiers: 24 public, 40 customer, 172+ staff (with middleware), 27 service
- 7 unit tests covering all middleware edge cases (no-auth, invalid, expired, valid, roundtrip, customer-rejected, wrong-role)
- Zero regressions: all 301 tests pass

## Task Commits

Each task was committed atomically:

1. **Task 1: Create auth middleware module with StaffClaims and require_staff_jwt** - `59508c5` (feat)
2. **Task 2: Split api_routes() into tiered sub-routers with staff middleware** - `46338cf` (feat)

## Files Created/Modified
- `crates/racecontrol/src/auth/middleware.rs` - StaffClaims struct, require_staff_jwt (strict), require_staff_jwt_permissive, create_staff_jwt, 7 unit tests
- `crates/racecontrol/src/auth/mod.rs` - Added pub mod middleware and re-exports
- `crates/racecontrol/src/api/routes.rs` - Split monolithic api_routes() into 4 tier functions
- `crates/racecontrol/src/api/mod.rs` - Updated api_routes() call to pass state
- `crates/racecontrol/src/main.rs` - Updated api_routes() call to pass state.clone()

## Decisions Made
- Permissive mode for initial deploy: staff routes log warnings for missing JWT but do not reject. This follows the expand-migrate-contract pattern from 76-RESEARCH.md. Strict enforcement comes after dashboard/kiosk/bots are updated to send staff JWTs.
- StaffClaims uses a separate `role` field rather than a nested claim -- simpler and explicit. Customer JWTs that lack the `role` field fail deserialization and are rejected.
- extract_staff_claims() is a shared helper used by both strict and permissive middleware, avoiding code duplication.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed additional call site in api/mod.rs**
- **Found during:** Task 2 (route split)
- **Issue:** api_routes() signature change from zero args to one arg broke a second call site in api/mod.rs (not mentioned in plan)
- **Fix:** Updated api/mod.rs line 10 to pass state.clone()
- **Files modified:** crates/racecontrol/src/api/mod.rs
- **Verification:** cargo build succeeds
- **Committed in:** 46338cf (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Trivial fix, no scope creep.

## Issues Encountered
- Test helper test_state() initially used block_on() inside tokio runtime, causing panic. Fixed by making test_state() async.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Staff JWT middleware is deployed in permissive mode -- ready for Phase B (dashboard/kiosk/bot JWT integration)
- create_staff_jwt() is available for generating staff tokens from login endpoints
- Strict mode (require_staff_jwt) is tested and ready to be swapped in when clients send JWTs

---
*Phase: 76-api-authentication-admin-protection*
*Completed: 2026-03-20*
