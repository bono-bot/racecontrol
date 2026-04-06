---
phase: 327-enforcement-deploy-integration
plan: 02
subsystem: infra
tags: [bash, deploy, playwright, verification, visual-regression]

requires:
  - phase: 325-page-crawler-foundation
    provides: Page crawler Playwright config and route manifest
  - phase: 326-visual-regression-baseline
    provides: Visual regression Playwright config and baseline screenshots
provides:
  - deploy-verify.sh standalone post-deploy verification script
  - deploy-nextjs.sh frontend deploy with automated verification
affects: [deploy-pipeline, ci-cd, frontend-deploy]

tech-stack:
  added: []
  patterns: [post-deploy-verification, build-hash-table, deploy-verify-integration]

key-files:
  created:
    - scripts/deploy-verify.sh
    - scripts/deploy-nextjs.sh
  modified: []

key-decisions:
  - "Hash mismatches are informational warnings, not deploy failures -- cloud may be intentionally behind"
  - "Admin shares web codebase, no separate admin build step needed"
  - "deploy-verify.sh is standalone-callable for flexibility beyond deploy-nextjs.sh"

patterns-established:
  - "Post-deploy verification pattern: hash table + crawler + VR as composable pipeline"
  - "Verification flags (--skip-crawler, --skip-vr) for flexible verification scopes"

requirements-completed: [DEPLOY-01, DEPLOY-02, DEPLOY-03]

duration: 5min
completed: 2026-04-06
---

# Phase 327 Plan 02: Deploy Verification Summary

**Standalone deploy-verify.sh with build hash table, page crawler, and visual regression checks -- integrated into deploy-nextjs.sh frontend deploy flow**

## Performance

- **Duration:** 5 min
- **Started:** 2026-04-06T13:29:40Z
- **Completed:** 2026-04-06T13:35:00Z
- **Tasks:** 2
- **Files created:** 2

## Accomplishments

- Created deploy-verify.sh (235 lines) with 3-section verification: build hash table, page crawler, visual regression
- Created deploy-nextjs.sh (210 lines) wrapping frontend build + deploy-verify.sh call
- Both scripts follow deploy-server.sh conventions (colors, helpers, set -euo pipefail)
- Hash table covers all 5 targets: Server, Web, Admin, Kiosk, Cloud with color-coded status

## Task Commits

Each task was committed atomically:

1. **Task 1: Create deploy-verify.sh standalone verification script** - `135e88d3` (feat)
2. **Task 2: Create deploy-nextjs.sh with post-deploy verification** - `786f9f63` (feat)

## Files Created/Modified

- `scripts/deploy-verify.sh` - Standalone post-deploy verification: hash table + crawler + VR
- `scripts/deploy-nextjs.sh` - Frontend deploy script calling deploy-verify.sh after build

## Decisions Made

- Hash mismatches in the verification table are warnings (yellow), not failures -- cloud may be intentionally behind venue
- Admin app shares the web codebase, so `--app web` builds both web (:3200) and admin (:3201)
- HTTP 302/307 treated as OK for frontend health checks (auth redirects are expected)
- Visual regression with missing baselines treated as success with a note (first-run scenario)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- deploy-verify.sh and deploy-nextjs.sh ready for use
- Both scripts pass `bash -n` syntax validation
- Visual regression baselines from Phase 326 are in place
- Page crawler from Phase 325 is integrated via Playwright config

---
*Phase: 327-enforcement-deploy-integration*
*Completed: 2026-04-06*
