---
phase: 42-kiosk-source-prep-browser-smoke
plan: 02
subsystem: testing
tags: [playwright, smoke, e2e, cleanup-fixture, keyboard-nav, browser]

# Dependency graph
requires:
  - phase: 42-01
    provides: data-testid attributes on kiosk wizard components (walkin-btn, step-select-plan, step-select-game)
  - phase: 41-test-foundation
    provides: Playwright 1.58.2 installed, playwright.config.ts configured
provides:
  - Pre-test cleanup fixture stopping stale games and billing on pod-8 before every test
  - Kiosk route smoke tests for /, /book, /staff with pageerror capture and React boundary checks
  - Keyboard navigation test Tab+Enter through booking wizard
  - DOM snapshot attachment on test failure via testInfo.attach()
affects:
  - 42-03 (if exists — shares cleanup fixture via import from '../fixtures/cleanup')
  - 43 (wizard specs — inherit cleanup fixture and smoke patterns)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "test.extend<{ensureClean: void}>() with auto:true — auto-runs before every test in importing spec files"
    - "page.on('pageerror') attached in beforeEach (before goto) — captures hydration errors"
    - "testInfo.attach() with page.content() in afterEach on failure — HTML DOM snapshot in report"
    - "for...of loop over SMOKE_ROUTES array — parameterized smoke tests without test.each"

key-files:
  created:
    - tests/e2e/playwright/fixtures/cleanup.ts
    - tests/e2e/playwright/kiosk/smoke.spec.ts
  modified:
    - playwright.config.ts
    - .gitignore

key-decisions:
  - "jsErrors array scoped to module level (not testInfo metadata) — simpler than testInfo cast approach; workers:1 means no concurrency issue"
  - "DOM snapshot attachment uses 'dom-snapshot.html' name (with .html extension) — browser opens directly from report"
  - "Keyboard test accepts either advanced-to-game-step or stayed-on-plan-step as pass — only asserts Tab/Enter cause no JS errors (regression guard)"
  - "outputDir set to ./tests/e2e/results/ — collocates screenshots with test source, not at repo root test-results/"

# Metrics
duration: 1min
completed: 2026-03-18
---

# Phase 42 Plan 02: Browser Smoke Spec + Cleanup Fixture Summary

**Playwright cleanup fixture (auto-runs pre-test) + 4-test smoke suite covering 3 kiosk routes and keyboard navigation, with pageerror capture and DOM snapshot on failure**

## Performance

- **Duration:** 1 min
- **Started:** 2026-03-18T22:26:33Z
- **Completed:** 2026-03-18T22:28:12Z
- **Tasks:** 1
- **Files created/modified:** 4

## Accomplishments

- Created `tests/e2e/playwright/fixtures/cleanup.ts` — auto fixture using `test.extend` with `auto: true`; calls `GET /api/v1/games/active` and `GET /api/v1/billing/active` then stops any pod-8 stale sessions before every test; fully idempotent
- Created `tests/e2e/playwright/kiosk/smoke.spec.ts` — 4 tests: 3 parameterized route smokes (/, /book, /staff) asserting no pageerror, no React error boundary text, and visible structural content; 1 keyboard navigation test using Tab+Enter through booking wizard with data-testid selectors
- `pageerror` listener attached in `beforeEach` before any navigation — captures SSR hydration errors that would be missed if attached after `goto()`
- DOM snapshot attachment via `testInfo.attach('dom-snapshot.html', ...)` in `afterEach` on failure — captures HTML for React error boundary diagnosis
- Updated `playwright.config.ts` with `outputDir: './tests/e2e/results'` to colocate screenshots with test source
- Added `tests/e2e/results/` to `.gitignore`
- `npx playwright test --list` discovers all 4 tests without errors

## Task Commits

1. **Task 1: Create cleanup fixture and smoke spec with screenshot/DOM hooks** - `90ba39a` (feat)

## Files Created/Modified

- `tests/e2e/playwright/fixtures/cleanup.ts` — pre-test cleanup fixture; exports `test` and `expect` for spec consumption
- `tests/e2e/playwright/kiosk/smoke.spec.ts` — 3 route smoke tests + 1 keyboard nav test
- `playwright.config.ts` — added `outputDir: './tests/e2e/results'`
- `.gitignore` — added `tests/e2e/results/` to ignored paths

## Decisions Made

- `jsErrors` array scoped to module level (not stored on `testInfo` metadata via cast) — simpler and safe because `workers: 1` means no test concurrency
- Keyboard nav test uses a relaxed assertion (no JS errors) as the primary pass criterion — wizard step advancement is a bonus assertion (`if (advanced)`) since Tab behavior on first render may focus a non-tier element
- `dom-snapshot.html` attachment name includes `.html` extension so the HTML report opens it directly in browser

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered

None.

## Self-Check

- `tests/e2e/playwright/fixtures/cleanup.ts`: FOUND
- `tests/e2e/playwright/kiosk/smoke.spec.ts`: FOUND
- `playwright.config.ts` outputDir: FOUND
- Commit `90ba39a`: FOUND

## Self-Check: PASSED

## Next Phase Readiness

- Phase 43 wizard specs can import `{ test, expect }` from `'../fixtures/cleanup'` and receive automatic pre-test cleanup on pod-8
- The 4 smoke tests serve as regression anchors — if any kiosk route gains a JS error during development, the smoke suite catches it immediately
- Screenshot-on-failure (PNG via config) + DOM snapshot (HTML via afterEach) together provide full failure diagnosis artifacts

---
*Phase: 42-kiosk-source-prep-browser-smoke*
*Completed: 2026-03-18*
