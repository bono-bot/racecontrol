---
phase: 328-ai-self-audit
plan: 01
subsystem: testing
tags: [playwright, screenshots, ai-audit, visual-verification, page-descriptions]

# Dependency graph
requires:
  - phase: 325-page-crawler
    provides: crawl.spec.ts screenshot capture, route manifest, auth-setup
  - phase: 326-visual-regression
    provides: mask-config.ts dynamic content selectors, visual.spec.ts CRITICAL_PAGES
provides:
  - 10 page description markdown files documenting expected layout/data/interactions/failures
  - self-audit.sh script for automated screenshot capture and audit prompt generation
  - audit-prompt.md structured review instructions for Claude
  - audit-report.md template for AI-generated findings
affects: [328-02, ai-self-audit, deploy-verify]

# Tech tracking
tech-stack:
  added: []
  patterns: [page-description-format, self-audit-workflow]

key-files:
  created:
    - tests/page-audit/descriptions/web-home.md
    - tests/page-audit/descriptions/web-fleet.md
    - tests/page-audit/descriptions/web-billing.md
    - tests/page-audit/descriptions/web-billing-history.md
    - tests/page-audit/descriptions/web-sessions.md
    - tests/page-audit/descriptions/web-drivers.md
    - tests/page-audit/descriptions/web-games.md
    - tests/page-audit/descriptions/kiosk-landing.md
    - tests/page-audit/descriptions/kiosk-staff.md
    - tests/page-audit/descriptions/kiosk-control.md
    - tests/page-audit/self-audit.sh
    - tests/page-audit/audit-prompt.md
    - tests/page-audit/audit-report.md
  modified: []

key-decisions:
  - "Page descriptions kept concise (20-30 lines) as AI reference docs, not exhaustive specs"
  - "Dynamic content selectors from mask-config.ts documented as expected-to-change in descriptions"
  - "Known failure modes from CLAUDE.md standing rules embedded in What Wrong Looks Like sections"

patterns-established:
  - "Page description format: header block + 4 sections (Layout, Data, Interactions, Wrong)"
  - "Self-audit workflow: crawl -> find screenshots -> generate prompt -> generate report template"

requirements-completed: [AUDIT-01, AUDIT-02, AUDIT-03]

# Metrics
duration: 4min
completed: 2026-04-06
---

# Phase 328 Plan 01: Page Descriptions and Self-Audit Script Summary

**10 page description files covering all critical web/kiosk pages plus self-audit.sh that orchestrates screenshot capture and generates structured AI review prompts**

## Performance

- **Duration:** 4 min
- **Started:** 2026-04-06T13:51:25Z
- **Completed:** 2026-04-06T13:55:15Z
- **Tasks:** 2
- **Files modified:** 13

## Accomplishments
- Created page description files for all 10 critical pages (7 web, 3 kiosk) documenting expected layout, data, interactions, and failure modes
- Built self-audit.sh script that runs the page crawler on critical pages and generates a structured audit-prompt.md for Claude to review
- Each description references dynamic content selectors from mask-config.ts so AI knows what changes are normal
- Known failure modes from CLAUDE.md standing rules (unstyled HTML, login redirect loops, WS disconnect) embedded in descriptions

## Task Commits

Each task was committed atomically:

1. **Task 1: Create page description files for all 10 critical pages** - `ed4f72aa` (feat)
2. **Task 2: Create self-audit shell script and anomaly report generation** - `bd67ad58` (feat)

## Files Created/Modified
- `tests/page-audit/descriptions/web-home.md` - Expected behavior for web dashboard home page
- `tests/page-audit/descriptions/web-fleet.md` - Expected behavior for fleet management page
- `tests/page-audit/descriptions/web-billing.md` - Expected behavior for billing page
- `tests/page-audit/descriptions/web-billing-history.md` - Expected behavior for billing history page
- `tests/page-audit/descriptions/web-sessions.md` - Expected behavior for sessions page
- `tests/page-audit/descriptions/web-drivers.md` - Expected behavior for drivers page
- `tests/page-audit/descriptions/web-games.md` - Expected behavior for games page
- `tests/page-audit/descriptions/kiosk-landing.md` - Expected behavior for kiosk landing page
- `tests/page-audit/descriptions/kiosk-staff.md` - Expected behavior for kiosk staff page
- `tests/page-audit/descriptions/kiosk-control.md` - Expected behavior for kiosk control page
- `tests/page-audit/self-audit.sh` - Shell script orchestrating crawler + prompt generation
- `tests/page-audit/audit-prompt.md` - Generated structured review instructions for Claude
- `tests/page-audit/audit-report.md` - Template for AI-generated audit findings

## Decisions Made
- Page descriptions kept concise (20-30 lines) as AI reference docs rather than exhaustive UI specs
- Dynamic content selectors from mask-config.ts documented in Expected Data sections so AI reviewers know what changes are normal vs anomalous
- Known failure modes from CLAUDE.md standing rules (unstyled HTML from static files 404, login redirect loops, WS disconnect indicators) embedded in "What Wrong Looks Like" sections

## Deviations from Plan

None - plan executed exactly as written.

## Known Stubs

None - all files are complete with real content. No placeholder data or TODO items.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Page descriptions ready for AI comparison against screenshots
- self-audit.sh ready for integration with Claude hooks (328-02)
- audit-prompt.md format designed for Claude Read tool consumption

## Self-Check: PASSED

All 13 files verified present. Both task commits (`ed4f72aa`, `bd67ad58`) verified in git log.

---
*Phase: 328-ai-self-audit*
*Completed: 2026-04-06*
