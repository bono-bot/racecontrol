---
phase: 76-api-authentication-admin-protection
plan: 03
subsystem: auth
tags: [service-key, middleware, axum, subtle, constant-time, rc-agent]

# Dependency graph
requires: []
provides:
  - "require_service_key middleware on rc-agent protected routes"
  - "Permissive mode for safe rollout (empty/unset key = allow all)"
  - "Public /ping and /health endpoints (no auth required)"
affects: [76-04, 76-05, rc-agent-deployment]

# Tech tracking
tech-stack:
  added: [subtle 2.6]
  patterns: [public/protected router split, env-var-gated middleware, constant-time secret comparison]

key-files:
  created: []
  modified:
    - crates/rc-agent/Cargo.toml
    - crates/rc-agent/src/remote_ops.rs

key-decisions:
  - "subtle crate for constant-time comparison (100 lines, zero transitive deps) over ring or manual impl"
  - "Permissive mode when RCAGENT_SERVICE_KEY empty/unset -- safe rollout without breaking existing deployments"
  - "Router split: /ping and /health public, all other endpoints behind service key middleware"

patterns-established:
  - "Public/protected router split: health checks always accessible, operational endpoints gated"
  - "Env-var-gated middleware: empty var = permissive, set var = enforced"

requirements-completed: [AUTH-06]

# Metrics
duration: 8min
completed: 2026-03-20
---

# Phase 76 Plan 03: rc-agent Service Key Authentication Summary

**Service key middleware on rc-agent :8090 with constant-time comparison (subtle crate), permissive mode when RCAGENT_SERVICE_KEY unset, /ping and /health remain public**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-20T12:47:51Z
- **Completed:** 2026-03-20T12:55:56Z
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments
- require_service_key middleware with constant-time (ct_eq) secret comparison
- Router split in both start() and start_checked(): /ping and /health public, all other routes protected
- Permissive mode when RCAGENT_SERVICE_KEY is empty or unset (safe rollout)
- 7 new service key tests + 8 existing tests all pass (15/15)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add service key middleware (RED)** - `14f26c8` (test)
2. **Task 1: Add service key middleware (GREEN)** - `913527a` (feat)

## Files Created/Modified
- `crates/rc-agent/Cargo.toml` - Added subtle 2.6 dependency
- `crates/rc-agent/src/remote_ops.rs` - require_service_key middleware, public/protected router split, 7 new tests

## Decisions Made
- Used subtle crate for constant-time comparison (lightweight, zero deps) per research recommendation
- Kept existing connection_close_layer as outer layer on merged router
- Existing test_router() updated to include middleware (runs in permissive mode for backward compat)
- test_router_full() rebuilt with public/protected split matching production router

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required. RCAGENT_SERVICE_KEY env var is optional (permissive mode when unset).

## Next Phase Readiness
- rc-agent service key middleware ready
- Next: racecontrol must set X-Service-Key header when calling rc-agent endpoints (76-04 or later)
- Pods need RCAGENT_SERVICE_KEY env var in start-rcagent.bat when ready to enforce

---
*Phase: 76-api-authentication-admin-protection*
*Completed: 2026-03-20*
