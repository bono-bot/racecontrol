---
phase: 325-page-crawler
plan: 01
subsystem: testing
tags: [playwright, screenshots, visual-regression, crawler, e2e]

# Dependency graph
requires: []
provides:
  - "Page crawler that visits all 84 frontend pages across 3 apps and captures screenshots"
  - "Route manifest (AppRoute interface, WEB_ROUTES, ADMIN_ROUTES, KIOSK_ROUTES)"
  - "Staff PIN auth via storageState with 1-hour caching"
  - "CLI filtering via CRAWL_APP, CRAWL_PAGE, CRAWL_PAGES env vars"
affects: [326-visual-regression, 328-ai-self-audit]

# Tech tracking
tech-stack:
  added: []
  patterns: ["storageState-based auth caching for Playwright crawlers", "structured screenshot output {app}/{route}/{timestamp}.png"]

key-files:
  created:
    - tests/page-crawler/routes.ts
    - tests/page-crawler/auth-setup.ts
    - tests/page-crawler/crawl.spec.ts
    - tests/page-crawler/playwright.config.ts
  modified:
    - .gitignore

key-decisions:
  - "Reused staff PIN auth (1234) via validate-pin endpoint rather than full browser login flow"
  - "ADMIN_ROUTES shares WEB_ROUTES definition since admin app serves the same web app on a different port"
  - "Screenshots directory added to .gitignore as generated output"

patterns-established:
  - "Route manifest pattern: typed AppRoute interface with getAllRoutes() helper for multi-app crawling"
  - "Auth caching: storageState JSON files in .auth/ with TTL-based reuse"

requirements-completed: [CRAWL-01, CRAWL-02, CRAWL-03, CRAWL-04]

# Metrics
duration: 5min
completed: 2026-04-06
---

# Phase 325 Plan 01: Page Crawler Summary

**Playwright-based page crawler covering 84 pages across web/admin/kiosk with staff PIN auth, structured screenshot output, and CLI-driven selective crawling**

## Performance

- **Duration:** 5 min
- **Started:** 2026-04-06T12:59:59Z
- **Completed:** 2026-04-06T13:05:00Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Route manifest with 37 web routes, 37 admin routes (shared), and 10 kiosk routes covering all known frontend pages
- Auth module that authenticates via staff PIN and caches storageState for 1 hour
- Page crawler Playwright spec that visits every page, captures full-page screenshots, and handles errors gracefully
- CLI filtering via CRAWL_APP, CRAWL_PAGE, and CRAWL_PAGES environment variables for targeted crawling

## Task Commits

Each task was committed atomically:

1. **Task 1: Create route manifest and auth setup module** - `9f8c2a86` (feat)
2. **Task 2: Create page crawler Playwright spec** - `47e89407` (feat)

## Files Created/Modified
- `tests/page-crawler/routes.ts` - Route manifest with AppRoute interface and 3 app route arrays
- `tests/page-crawler/auth-setup.ts` - Staff PIN auth via storageState with TTL caching
- `tests/page-crawler/crawl.spec.ts` - Main crawler spec with screenshot capture and CLI filtering
- `tests/page-crawler/playwright.config.ts` - Dedicated Playwright config with 3 projects (web, admin, kiosk)
- `.gitignore` - Added entries for auth cache and screenshots directories

## Decisions Made
- ADMIN_ROUTES is a shallow copy of WEB_ROUTES since both apps serve the same Next.js web app on different ports (3200 vs 3201)
- Staff PIN "1234" used for auth -- matches existing E2E test patterns in the repo
- Screenshots directory excluded from git as generated output -- future CI can archive these as artifacts

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added screenshots directory to .gitignore**
- **Found during:** Task 2
- **Issue:** Plan specified screenshot output to tests/screenshots/ but did not mention excluding it from git
- **Fix:** Added `tests/screenshots/` to .gitignore alongside the auth cache entry
- **Files modified:** .gitignore
- **Verification:** `grep tests/screenshots .gitignore` confirms entry exists
- **Committed in:** 47e89407 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 missing critical)
**Impact on plan:** Essential to prevent generated screenshot files from being committed. No scope creep.

## Issues Encountered
None

## Known Stubs
None -- all data sources are wired (routes are hardcoded manifests, auth uses live API endpoint).

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Page crawler is ready to run against live apps on the venue network
- Foundation for Phase 326 (visual regression) -- screenshots directory structure and route manifest are reusable
- Foundation for Phase 328 (AI self-audit) -- crawler provides the evidence capture mechanism

## Self-Check: PASSED

- All 5 files exist on disk
- Commit 9f8c2a86 found in git log
- Commit 47e89407 found in git log

---
*Phase: 325-page-crawler*
*Completed: 2026-04-06*
