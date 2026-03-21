---
phase: 02-api-pwa-cloud-deploy
plan: 01
subsystem: infra
tags: [docker, compose, pwa, manifest, dockerfile]

requires:
  - phase: 01-cloud-infrastructure
    provides: Docker Compose base config, Caddyfile routing, VPS deployment
provides:
  - Correct NEXT_PUBLIC_API_URL with /api/v1 suffix in compose.yml
  - NEXT_PUBLIC_IS_CLOUD build arg in compose.yml and Dockerfile
  - Caddy service starts without waiting for admin/dashboard
  - PWA manifest with explicit scope for installability
affects: [02-02-PLAN, phase-6-admin-deploy, phase-7-dashboard-deploy]

tech-stack:
  added: []
  patterns:
    - "Build args flow: compose.yml args -> Dockerfile ARG/ENV -> Next.js process.env at build time"

key-files:
  created: []
  modified:
    - cloud/compose.yml
    - pwa/Dockerfile
    - pwa/public/manifest.json

key-decisions:
  - "NEXT_PUBLIC_IS_CLOUD defaults to false in Dockerfile so local builds are unaffected"
  - "Admin/dashboard service blocks kept in compose.yml for future phases, only Caddy depends_on changed"

patterns-established:
  - "NEXT_PUBLIC env vars: add ARG with default + ENV in builder stage, pass via compose build args"

requirements-completed: [PWA-01, PWA-02, PWA-05, API-01]

duration: 3min
completed: 2026-03-22
---

# Phase 2 Plan 01: Config Prep Summary

**Fixed compose.yml PWA build args (API URL with /api/v1, IS_CLOUD flag), removed premature Caddy dependencies, added Dockerfile build arg, verified manifest scope**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-22T03:14:03+05:30
- **Completed:** 2026-03-22T03:17:00+05:30
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Fixed NEXT_PUBLIC_API_URL to include /api/v1 suffix matching api.ts fallback pattern
- Added NEXT_PUBLIC_IS_CLOUD build arg to compose.yml and Dockerfile for cloud-specific features
- Removed admin/dashboard from Caddy depends_on so Docker Compose starts without those services
- Added explicit scope "/" to PWA manifest for installability

## Task Commits

Each task was committed atomically:

1. **Task 1: Fix compose.yml -- PWA build args and Caddy dependencies** - `514c38a` (feat)
2. **Task 2: Add NEXT_PUBLIC_IS_CLOUD to Dockerfile and verify manifest** - `3b09541` (feat)

## Files Created/Modified
- `cloud/compose.yml` - Fixed API URL suffix, added IS_CLOUD build arg, removed premature Caddy deps
- `pwa/Dockerfile` - Added ARG/ENV for NEXT_PUBLIC_IS_CLOUD with false default
- `pwa/public/manifest.json` - Added scope "/" for PWA installability

## Decisions Made
- NEXT_PUBLIC_IS_CLOUD defaults to false in Dockerfile so local `docker build` is unaffected
- Admin/dashboard service blocks preserved in compose.yml for Phase 6/7; only Caddy depends_on trimmed

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- compose.yml, Dockerfile, and manifest.json are ready for Bono to pull and build on VPS
- Plan 02-02 will coordinate with Bono for actual VPS deployment and verification

---
*Phase: 02-api-pwa-cloud-deploy*
*Completed: 2026-03-22*
