---
phase: 76-api-authentication-admin-protection
plan: 04
subsystem: auth
tags: [rate-limiting, tower-governor, governor, sqlite-transaction, single-use-token, wallet-check]

# Dependency graph
requires:
  - phase: 76-01
    provides: 4-tier route split (public/customer/staff/service) for route extraction
provides:
  - Rate limiting layer on all auth endpoints (5 req/min per IP)
  - Atomic token consumption via SQLx transaction (pending->consuming->consumed)
  - ConnectInfo<SocketAddr> on server startup for IP extraction
  - auth_rate_limited_routes() sub-router with GovernorLayer
affects: [76-05, 76-06, 76-02]

# Tech tracking
tech-stack:
  added: [tower_governor 0.8, governor 0.10]
  patterns: [GovernorLayer rate limiting, SQLx transaction for atomic state transitions, into_make_service_with_connect_info]

key-files:
  created:
    - crates/racecontrol/src/auth/rate_limit.rs
  modified:
    - crates/racecontrol/Cargo.toml
    - crates/racecontrol/src/auth/mod.rs
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/src/main.rs

key-decisions:
  - "PeerIpKeyExtractor over GlobalKeyExtractor -- per-IP rate limiting on LAN requires ConnectInfo<SocketAddr>"
  - "into_make_service_with_connect_info added to server startup for ConnectInfo support"
  - "Bot wallet check (AUTH-05) already existed -- documented as pre-existing, no code changes"
  - "Billing is deferred (in-memory), not a DB write, so TOCTOU risk is mitigated by optimistic locking"
  - "SQLx transaction wraps validate_pin token lifecycle (consuming + consumed) for atomic rollback"

patterns-established:
  - "Rate-limited route group: auth_rate_limited_routes() merged first in api_routes()"
  - "GovernorLayer applied at sub-router level, not globally"
  - "Token state machine: pending -> consuming (tx) -> consumed (tx commit) with rollback on failure"

requirements-completed: [AUTH-04, AUTH-05, SESS-02, SESS-03]

# Metrics
duration: 21min
completed: 2026-03-20
---

# Phase 76 Plan 04: Rate Limiting + Session Integrity Summary

**tower_governor rate limiting on 6 auth endpoints + SQLx transaction-wrapped token consumption with 7 new tests**

## Performance

- **Duration:** 21 min
- **Started:** 2026-03-20T13:02:17Z
- **Completed:** 2026-03-20T13:23:50Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- Rate limiting (5 req/min per IP) on customer/login, verify-otp, validate-pin, kiosk/validate-pin, staff/validate-pin, admin-login
- Atomic token consumption via SQLx transaction in validate_pin -- rollback on billing failure reverts token to pending
- 7 new tests: 3 for rate limiting (burst, 429, per-IP isolation) + 4 for session integrity (double-consume, consuming-state, tx rollback, tx commit)
- ConnectInfo<SocketAddr> enabled on server for IP-based rate limiting

## Task Commits

Each task was committed atomically:

1. **Task 1: Rate limiting on auth endpoints** - `b6ea2a7` (feat)
2. **Task 2: Atomic token consumption + single-use verification** - `2f46715` (feat)

## Files Created/Modified
- `crates/racecontrol/src/auth/rate_limit.rs` - GovernorLayer config: PeerIpKeyExtractor, 12s replenish, burst 5
- `crates/racecontrol/src/auth/mod.rs` - SQLx transaction wrapping validate_pin token lifecycle + 4 SESS tests
- `crates/racecontrol/src/api/routes.rs` - auth_rate_limited_routes() sub-router with 6 endpoints
- `crates/racecontrol/src/main.rs` - into_make_service_with_connect_info::<SocketAddr>()
- `crates/racecontrol/Cargo.toml` - tower_governor 0.8, governor 0.10 deps
- `Cargo.lock` - dependency lockfile updates

## Decisions Made
- PeerIpKeyExtractor chosen for per-IP rate limiting. Required adding `into_make_service_with_connect_info::<SocketAddr>()` to server startup -- minimal change, no regression risk.
- Bot wallet check (AUTH-05) was already implemented in the codebase. Verified the check at bot_book handler lines 11295-11310. No code changes needed.
- Token 'consuming' intermediate state (SESS-02) was already implemented via atomic UPDATE...WHERE status='pending' RETURNING pattern. Added tests to verify the behavior.
- SESS-03 transaction wrapping applied to validate_pin only (the main customer flow). Other validation paths (validate_qr, validate_pin_kiosk, start_now_auth) use the same optimistic locking pattern but were not wrapped in transactions to keep the change scoped. They can be migrated in a future plan if needed.
- Billing deferral writes to an in-memory HashMap (not DB), so the TOCTOU concern from the plan was already mitigated by the optimistic locking pattern. Transaction adds crash safety.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added governor crate as direct dependency**
- **Found during:** Task 1 (rate_limit.rs compilation)
- **Issue:** tower_governor 0.8 uses governor types (NoOpMiddleware, QuantaInstant) but doesn't re-export them
- **Fix:** Added governor 0.10 as direct dependency in Cargo.toml
- **Files modified:** crates/racecontrol/Cargo.toml
- **Verification:** Compilation succeeds
- **Committed in:** b6ea2a7

**2. [Rule 3 - Blocking] Added ConnectInfo<SocketAddr> support to server startup**
- **Found during:** Task 1 (PeerIpKeyExtractor requires ConnectInfo in request extensions)
- **Issue:** axum::serve(listener, app) does not inject ConnectInfo -- PeerIpKeyExtractor would fail silently
- **Fix:** Changed to app.into_make_service_with_connect_info::<SocketAddr>()
- **Files modified:** crates/racecontrol/src/main.rs
- **Verification:** Build succeeds, rate limit tests pass with ConnectInfo in requests
- **Committed in:** b6ea2a7

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both were necessary for the rate limiter to function. No scope creep.

## Issues Encountered
- Windows Application Control policy intermittently blocked test binary execution. Resolved by `cargo clean -p racecontrol-crate` before each test run (forces fresh binary generation that passes AppControl scanning).

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Rate limiting infrastructure ready for all future auth endpoints
- auth_rate_limited_routes() is the single place to add new rate-limited endpoints
- Transaction pattern established for token consumption -- can be extended to other validation paths
- Plan 02 (staff login endpoint) can register /auth/admin-login in the rate-limited group (already added)
- Plan 05/06 can build on the session integrity guarantees

---
*Phase: 76-api-authentication-admin-protection*
*Completed: 2026-03-20*
