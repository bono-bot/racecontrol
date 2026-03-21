---
phase: 06-admin-panel-cloud-deploy
plan: 01
subsystem: infra
tags: [docker-compose, admin-panel, next.js, caddy, reverse-proxy]

# Dependency graph
requires:
  - phase: 02-api-pwa-cloud-deploy
    provides: compose.yml with working PWA service and Caddy proxy
provides:
  - Correct admin service Docker Compose config with build args matching Dockerfile ARGs
  - Server-side env vars for racecontrol API proxy via host.docker.internal
  - PORT=3300 override matching Caddy reverse_proxy and expose config
  - Caddy depends_on admin with service_healthy condition
affects: [06-02-PLAN, 07-dashboard-cloud-deploy]

# Tech tracking
tech-stack:
  added: []
  patterns: [host.docker.internal for container-to-host communication, PORT env override for Dockerfile default]

key-files:
  created: []
  modified: [cloud/compose.yml]

key-decisions:
  - "PORT=3300 set via environment override rather than changing Dockerfile (avoids breaking local dev where PORT=3000 is correct)"
  - "GATEWAY_URL points to host.docker.internal:8080 same as RC_URL since gateway routes through racecontrol on cloud"

patterns-established:
  - "Admin env pattern: build args for client-side NEXT_PUBLIC_* vars, environment for server-side runtime vars"

requirements-completed: [ADMIN-01, ADMIN-03, ADMIN-04, ADMIN-05, API-03]

# Metrics
duration: 3min
completed: 2026-03-22
---

# Phase 6 Plan 1: Admin Panel Cloud Deploy - Config Prep Summary

**Fixed compose.yml admin service with correct Dockerfile build args, server-side env vars for racecontrol proxy, PORT=3300 override, and Caddy dependency**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-21T22:15:45Z
- **Completed:** 2026-03-21T22:18:45Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments
- Fixed admin build args to match Dockerfile ARG names (NEXT_PUBLIC_RC_URL instead of wrong NEXT_PUBLIC_API_URL)
- Added NEXT_PUBLIC_GATEWAY_URL and NEXT_PUBLIC_GATEWAY_API_KEY build args for client-side analytics/bookings
- Added server-side environment vars (RC_URL, RACECONTROL_URL, GATEWAY_URL) pointing to host.docker.internal:8080
- Added PORT=3300 override so app listens on the port Caddy and expose config expect
- Added admin to Caddy depends_on so Caddy waits for admin healthcheck before starting

## Task Commits

Each task was committed atomically:

1. **Task 1: Fix admin service build args, env vars, and port in compose.yml** - `815674b` (feat)

## Files Created/Modified
- `cloud/compose.yml` - Fixed admin service build args, added environment section, added admin to Caddy depends_on

## Decisions Made
- PORT=3300 set via environment override rather than changing Dockerfile (avoids breaking local dev where PORT=3000 is correct)
- GATEWAY_URL points to host.docker.internal:8080 same as RC_URL since gateway routes through racecontrol on cloud

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- compose.yml is ready for Bono to pull and build on VPS (06-02-PLAN.md)
- Dashboard service (Phase 7) still has wrong NEXT_PUBLIC_API_URL but that is Phase 7 scope

---
*Phase: 06-admin-panel-cloud-deploy*
*Completed: 2026-03-22*
