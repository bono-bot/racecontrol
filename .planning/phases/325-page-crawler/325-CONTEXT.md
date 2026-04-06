# Phase 325: Page Crawler - Context

**Gathered:** 2026-04-06
**Status:** Ready for planning
**Mode:** Auto-generated (infrastructure phase — discuss skipped)

<domain>
## Phase Boundary

James can capture screenshots of every frontend page on demand, with proper authentication and structured output. The crawler visits all pages across web (:3200), admin (:3201), and kiosk (:3300), authenticates via saved Playwright storageState (staff PIN), saves screenshots to `tests/screenshots/{app}/{route}/{timestamp}.png`, and accepts flags to target specific apps or pages.

Requirements: CRAWL-01, CRAWL-02, CRAWL-03, CRAWL-04

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase. Use ROADMAP phase goal, success criteria, and codebase conventions to guide decisions.

Key context from research:
- 70+ pages across 3 apps: PWA (33 routes, :3300), Web (32 routes, :3200), Kiosk (10 routes, :3300)
- Staff PIN auth for web/kiosk/admin. Defer PWA customer OTP to future.
- Existing Playwright infrastructure: playwright.config.ts exists, 35+ E2E tests already in place
- Existing verify-pod-screen.js uses Playwright for pod screenshots — can reference patterns
- Route manifests from research: all routes mapped per app with auth requirements

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `playwright.config.ts` — main Playwright config at repo root
- `verify-pod-screen.js` — existing Playwright screenshot tool (pod screens)
- `visual-verify.js` — pixel-level visual analysis script
- `tests/e2e/playwright/` — 35+ existing E2E test files with auth patterns
- `e2e-regression/playwright.config.ts` — regression suite config
- Staff PIN auth: POST `/auth/validate-pin` → JWT → storageState

### Established Patterns
- E2E tests use `storageState` for auth persistence
- Tests target `:3200` (web/POS), `:3300` (kiosk)
- Serial execution (1 worker) to prevent conflicts
- HTML report + screenshot retention on failure

### Integration Points
- Page routes from Next.js app directories: `web/src/app/`, `kiosk/src/app/`, `pwa/src/app/`
- Auth endpoint: `POST /auth/validate-pin` (staff), `POST /customer/login` (customer OTP)
- Health endpoints for pre-flight: `/api/health` per app

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase. Refer to ROADMAP phase description and success criteria.

</specifics>

<deferred>
## Deferred Ideas

- PWA customer auth crawling (requires test OTP account) — deferred to EXT-01
- Cloud endpoint crawling (Bono VPS URLs) — deferred to EXT-02

</deferred>
