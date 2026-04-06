# Phase 326: Visual Regression Tests - Context

**Gathered:** 2026-04-06
**Status:** Ready for planning
**Mode:** Auto-generated (infrastructure phase — discuss skipped)

<domain>
## Phase Boundary

Frontend changes are automatically compared against known-good baselines, with dynamic content properly masked. Builds on Phase 325 page crawler infrastructure.

Requirements: VR-01, VR-02, VR-03, VR-04

Success criteria:
1. Critical pages have Playwright toHaveScreenshot() tests that fail on unexpected changes
2. Dynamic content (timestamps, counters, live metrics) is masked per-page so data changes don't trigger false failures
3. Baseline screenshots committed in git alongside test files, updateable via --update-snapshots
4. Before/after comparison workflow for frontend fixes

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — infrastructure phase. Key guidance:

- Use Playwright's built-in toHaveScreenshot() with pixelmatch — no external tools
- Use mask parameter with page.locator() for dynamic content
- Per-page mask config in a JSON/TS config file (not hardcoded in each test)
- Baselines go in tests/visual-regression/__screenshots__/ (Playwright default)
- Critical pages to test first: web / (dashboard), /fleet, /billing, /billing/history, /sessions, /drivers, /games; kiosk / (pod grid), /staff, /control
- Build on Phase 325 auth-setup.ts for storageState
- Before/after workflow: npm script or Playwright command that captures "before" screenshots, runs after fix, compares

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets (from Phase 325)
- `tests/page-crawler/routes.ts` — 84 routes across 3 apps with typed AppRoute interface
- `tests/page-crawler/auth-setup.ts` — staff PIN auth with storageState caching
- `tests/page-crawler/playwright.config.ts` — 3-project config (web/admin/kiosk)
- `tests/page-crawler/crawl.spec.ts` — page visiting logic with error handling

### Established Patterns
- Playwright toHaveScreenshot() uses pixelmatch internally
- mask option accepts Locator[] — hides dynamic content with colored boxes
- stylePath option injects CSS to disable animations before capture
- maxDiffPixels/maxDiffPixelRatio absorb minor rendering variance
- --update-snapshots flag regenerates baselines

### Integration Points
- Extends page-crawler config and auth setup
- Screenshots compared against git-committed baselines
- Same app URLs: web :3200, admin :3201, kiosk :3300

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
