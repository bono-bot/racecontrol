---
phase: 42-kiosk-source-prep-browser-smoke
plan: 01
subsystem: testing
tags: [playwright, data-testid, nextjs, tsx, kiosk, e2e]

# Dependency graph
requires:
  - phase: 41-test-foundation
    provides: Playwright 1.58.2 installed, playwright.config.ts configured, test directory structure
provides:
  - data-testid attributes on all wizard step containers in kiosk/src/app/book/page.tsx (customer booking flow)
  - data-testid attributes on all wizard step containers in kiosk/src/components/SetupWizard.tsx (staff wizard)
  - data-testid attributes on pod grid, pod cards, CTA, PIN modal, WS status in kiosk/src/app/page.tsx
affects:
  - 42-02 (smoke spec — references booking-phone-screen, pod-grid, book-session-btn)
  - 43 (wizard specs — references all step-* and interactive button testids)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "data-testid on step containers: data-testid='step-{step_name}' on root div of each {step === '...' && (...)} block"
    - "data-testid on dynamic buttons: data-testid={`{element-type}-{id}`} for buttons iterated over data arrays"
    - "data-testid on navigation: wizard-step-title, wizard-back-btn, cancel-btn, book-btn, launch-btn"

key-files:
  created: []
  modified:
    - kiosk/src/app/book/page.tsx
    - kiosk/src/components/SetupWizard.tsx
    - kiosk/src/app/page.tsx

key-decisions:
  - "data-testid naming is consistent between book/page.tsx and SetupWizard.tsx for shared step names (step-select-game, step-select-plan, etc.) so Phase 43 specs work against both paths"
  - "wizard-back-btn applied to both back button instances in SetupWizard.tsx (non-review footer and review footer) — both serve the same navigation purpose"
  - "pin-modal placed on PinModal component's outer fixed div (inside PinModal function), not on the conditional render site in CustomerLanding"

patterns-established:
  - "Step containers: always data-testid='step-{step_name}' on root div of conditional block"
  - "Dynamic item buttons: data-testid={`{type}-${item.id}`} where type matches element role"
  - "Screen-level containers: data-testid='booking-{phase}' for top-level phase divs (phone, otp, success, error, wizard)"

requirements-completed: [FOUND-06]

# Metrics
duration: 10min
completed: 2026-03-19
---

# Phase 42 Plan 01: Kiosk Source Prep — data-testid Summary

**97 data-testid attributes added across three kiosk TSX files: 49 in book/page.tsx (customer wizard), 43 in SetupWizard.tsx (staff wizard), 5 in page.tsx (landing page)**

## Performance

- **Duration:** 10 min
- **Started:** 2026-03-18T22:12:40Z
- **Completed:** 2026-03-18T22:22:44Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- Zero to 49 data-testid attributes in book/page.tsx — every phase screen (phone, otp, wizard, booking, success, error), every wizard step container, and every interactive button now has a testid
- Zero to 43 data-testid attributes in SetupWizard.tsx — all 13 staff wizard step containers plus driver registration inputs, dynamic item buttons (tier, game, split, experience, track, car), and settings buttons (difficulty, transmission, FFB)
- Zero to 5 data-testid attributes in page.tsx — pod-grid, pod-card-{number}, book-session-btn, pin-modal, ws-status
- TypeScript compilation passes with zero new errors after all changes

## Task Commits

Each task was committed atomically:

1. **Task 1: Add data-testid attributes to book/page.tsx (customer booking flow)** - `1441ca5` (feat)
2. **Task 2: Add data-testid attributes to SetupWizard.tsx and page.tsx** - `3fff854` (feat)

## Files Created/Modified

- `kiosk/src/app/book/page.tsx` - 49 data-testid attributes added to all wizard screens, steps, and buttons
- `kiosk/src/components/SetupWizard.tsx` - 43 data-testid attributes added to all 13 staff wizard steps and interactive elements
- `kiosk/src/app/page.tsx` - 5 data-testid attributes on pod grid, cards, CTA, PIN modal, connection status

## Decisions Made

- Naming is consistent between book/page.tsx and SetupWizard.tsx for shared step names — `step-select-game`, `step-select-plan`, etc. appear in both files with identical testids, so Phase 43 specs using `page.locator('[data-testid="step-select-game"]')` work against both customer and staff wizard paths
- wizard-back-btn applied to both back button instances in SetupWizard.tsx footer (the component has two conditional footer blocks: `!isFirstStep && step !== "review"` and `step === "review"`)
- pin-modal is on the outer fixed div inside the PinModal component function body (line ~524), not on the `{selectedPodId && <PinModal .../>}` call site

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## Next Phase Readiness

- Phase 42 Plan 02 (smoke spec) can now use `[data-testid="booking-phone-screen"]` as structural anchor for `/book` route check
- Phase 42 Plan 02 (smoke spec) can use `[data-testid="pod-grid"]` as structural anchor for `/` route check
- Phase 43 wizard specs can select any wizard element in both customer (book/page.tsx) and staff (SetupWizard.tsx) paths using consistent testid naming
- A Playwright locator like `page.locator('[data-testid="game-option-assetto_corsa"]')` will match in the live DOM when the select_game step is active

---
*Phase: 42-kiosk-source-prep-browser-smoke*
*Completed: 2026-03-19*
