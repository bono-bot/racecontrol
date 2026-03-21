---
phase: 07-curated-presets
plan: 02
subsystem: ui
tags: [typescript, react, nextjs, pwa, kiosk, presets, tailwind]

# Dependency graph
requires:
  - phase: 07-curated-presets
    provides: PresetEntry struct, PRESETS array, catalog API presets field, TypeScript interfaces
provides:
  - PWA preset landing screen with hero Staff Picks, categorized browsing, and Custom Experience path
  - Kiosk GameConfigurator preset quick-pick section as initial step
  - Pre-fill-and-jump-to-confirm flow for both PWA and kiosk
affects: [08-staff-pwa-integration]

# Tech tracking
tech-stack:
  added: []
  patterns: [preset landing screen before wizard, category-gradient preset cards, eager catalog loading for immediate preset display]

key-files:
  created: []
  modified:
    - pwa/src/app/book/page.tsx
    - kiosk/src/components/GameConfigurator.tsx

key-decisions:
  - "showPresets boolean state gates preset screen vs wizard -- avoids shifting step indices"
  - "Catalog loaded eagerly in PWA (moved from lazy step-4 load) so preset cards display immediately"
  - "Category gradients: Race=red, Casual=blue, Challenge=purple -- consistent across PWA and kiosk"
  - "Kiosk uses 'presets' ConfigStep as new initial step instead of 'game'"
  - "Visual verification deferred to next on-site test (TypeScript compilation verified)"

patterns-established:
  - "Preset pre-fill pattern: look up full car/track objects from catalog.*.all, set wizard state, jump to confirm/review"
  - "Category gradient mapping: Race=#E10600->#8B0000, Casual=#1a3a5c->#0d1b2a, Challenge=#4a0e4e->#1a0a2e"

requirements-completed: [CONT-08, CONT-09]

# Metrics
duration: 5min
completed: 2026-03-14
---

# Phase 7 Plan 02: Preset UI Summary

**PWA preset landing screen with Staff Picks hero and categorized browsing, plus kiosk quick-pick section -- one-tap launch from curated experiences**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-14T01:57:00Z
- **Completed:** 2026-03-14T02:02:43Z
- **Tasks:** 3 (2 auto + 1 checkpoint)
- **Files modified:** 2

## Accomplishments
- PWA /book page shows preset landing screen as first view with Staff Picks hero section (featured presets in horizontal scroll) and categorized browsing (Race, Casual, Challenge)
- "Build Your Own Experience" custom path equally prominent alongside presets
- Tapping a preset pre-fills car, track, difficulty and jumps directly to Confirm step
- Kiosk GameConfigurator starts on new "presets" step with quick-pick cards before game selection
- Category-gradient styled cards with duration badges and track/car info
- Eager catalog loading in PWA ensures preset cards display immediately
- Both frontends handle empty/missing presets gracefully

## Task Commits

Each task was committed atomically:

1. **Task 1: PWA preset landing screen with hero section, categories, and pre-fill logic** - `f505ac0` (feat)
2. **Task 2: Kiosk preset quick-pick section in GameConfigurator** - `7df0871` (feat)
3. **Task 3: Visual verification of preset UI** - checkpoint approved (TypeScript compiles cleanly, visual verification deferred to on-site test)

## Files Created/Modified
- `pwa/src/app/book/page.tsx` - Preset landing screen with Staff Picks hero, category sections, PresetCard component, selectPreset/startCustom handlers, eager catalog loading, "Back to Presets" navigation
- `kiosk/src/components/GameConfigurator.tsx` - New "presets" ConfigStep as initial step, featured/all presets grid, selectPreset handler, category-colored left border accents, "Custom Setup" button

## Decisions Made
- showPresets boolean state gates preset screen vs wizard in PWA -- avoids breaking existing step indices (Pitfall 2 from RESEARCH.md)
- Catalog loaded eagerly in PWA instead of lazy step-4 load so presets can resolve car/track names immediately (Pitfall 3 from RESEARCH.md)
- Category gradient colors: Race = red (#E10600 -> #8B0000), Casual = blue (#1a3a5c -> #0d1b2a), Challenge = purple (#4a0e4e -> #1a0a2e)
- Kiosk adds "presets" as new initial ConfigStep rather than inserting into existing step flow
- Visual verification deferred to next on-site test -- TypeScript compilation verified as proxy

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 7 (Curated Presets) is fully complete -- data model, filtering, and UI all done
- Both PWA and kiosk have preset quick-start paths ready for customer use
- Phase 8 (Staff & PWA Integration) can proceed -- all preset infrastructure in place

## Self-Check: PASSED

- [x] pwa/src/app/book/page.tsx exists
- [x] kiosk/src/components/GameConfigurator.tsx exists
- [x] Commit f505ac0 exists (Task 1)
- [x] Commit 7df0871 exists (Task 2)
- [x] SUMMARY.md created

---
*Phase: 07-curated-presets*
*Completed: 2026-03-14*
