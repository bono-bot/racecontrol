---
phase: 261-design-system-foundation
plan: 03
subsystem: ui
tags: [motion, animation, jetbrains-mono, fonts, next-font]

requires:
  - phase: 261-01
    provides: shared tokens with --font-mono CSS variable referencing --font-jb-mono
  - phase: 261-02
    provides: shadcn/ui initialized in both apps
provides:
  - motion@12 animation library in both web and kiosk apps
  - JetBrains Mono font loading via next/font/google in web layout
  - Clean build verification proving Plans 01-03 are compatible
affects: [263-web-primitive-components, 264-web-dashboard-pages, 265-kiosk-pages]

tech-stack:
  added: [motion@12.38.0]
  patterns: [next/font/google multi-font loading, CSS variable wiring for monospace font]

key-files:
  modified:
    - web/package.json
    - kiosk/package.json
    - web/src/app/layout.tsx

key-decisions:
  - "motion@12 installed as runtime dependency (not devDependency) in both apps"
  - "JetBrains Mono variable --font-jb-mono applied to html element (root scope), not body"
  - "Font weights 400/500/700 chosen to match tabular numeral needs without loading unused weights"

patterns-established:
  - "Multi-font loading: each font gets its own CSS variable applied at html or body scope"
  - "Animation imports: use motion/react, never framer-motion"

requirements-completed: [DS-02, DS-06]

duration: 2min
completed: 2026-03-30
---

# Phase 261 Plan 03: Motion + JetBrains Mono Summary

**motion@12 animation library installed in both apps, JetBrains Mono font wired to web layout via next/font/google with --font-jb-mono CSS variable, both apps build clean**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-30T10:21:12Z
- **Completed:** 2026-03-30T10:23:06Z
- **Tasks:** 3 (2 implementation + 1 verification)
- **Files modified:** 4 (web/package.json, web/package-lock.json, kiosk/package.json, kiosk/package-lock.json, web/src/app/layout.tsx)

## Accomplishments
- motion@^12.38.0 added to runtime dependencies in both web and kiosk package.json files
- JetBrains Mono loaded via next/font/google in web/src/app/layout.tsx with CSS variable --font-jb-mono
- --font-jb-mono applied to html element, wiring to globals.css --font-mono variable from Plan 261-01
- Both apps (web + kiosk) build clean with zero TypeScript errors
- Phase 261 integration gate passed: all three plans' changes are compatible

## Task Commits

Each task was committed atomically:

1. **Task 1: Install motion@12 in both apps** - `455904bc` (feat)
2. **Task 2: Add JetBrains Mono to web layout.tsx** - `69345128` (feat)
3. **Task 3: Build verification** - verification-only, no commit needed

## Files Created/Modified
- `web/package.json` - Added motion@^12.38.0 to dependencies
- `web/package-lock.json` - Lock file updated for motion
- `kiosk/package.json` - Added motion@^12.38.0 to dependencies
- `kiosk/package-lock.json` - Lock file updated for motion
- `web/src/app/layout.tsx` - Added JetBrains_Mono import, font constant, and --font-jb-mono variable on html element

## Decisions Made
- None significant beyond plan specification. Font weights (400/500/700) chosen per plan to cover regular, medium, and bold for tabular numerals.

## Deviations from Plan
None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Known Stubs
None - no stubs or placeholders introduced.

## Next Phase Readiness
- Phase 261 (Design System Foundation) is now complete with all 3 plans done
- Shared tokens (Plan 01), shadcn/ui + Lucide (Plan 02), motion + fonts (Plan 03) all in place
- Ready for Phase 262 (Deploy Pipeline Hardening) or Phase 263 (Web Primitive Components)
- All five Phase 261 success criteria verified:
  1. shared tokens with rp-red-hover canonical, zero rp-red-light references
  2. shadcn initialized, tw-animate-css imported, no tailwindcss-animate
  3. motion in both package.json, no framer-motion
  4. JetBrains Mono in web layout with --font-jb-mono variable
  5. Both apps build clean

---
*Phase: 261-design-system-foundation*
*Completed: 2026-03-30*
