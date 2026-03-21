---
phase: 07-curated-presets
plan: 01
subsystem: api
tags: [rust, serde, catalog, presets, typescript]

# Dependency graph
requires:
  - phase: 05-content-validation-filtering
    provides: ContentManifest car/track ID sets and get_filtered_catalog() infrastructure
provides:
  - PresetEntry struct with 10 fields in catalog.rs
  - PRESETS static array with 14 curated car/track/session combos
  - Preset filtering by pod manifest (car+track installed, AI line check)
  - PresetEntry TypeScript interface in PWA and kiosk
  - "presets" field in catalog API JSON responses
affects: [07-curated-presets, 08-staff-pwa-integration]

# Tech tracking
tech-stack:
  added: []
  patterns: [static preset array following FEATURED_CARS/FEATURED_TRACKS pattern, manifest-aware preset filtering]

key-files:
  created: []
  modified:
    - crates/rc-core/src/catalog.rs
    - pwa/src/lib/api.ts
    - kiosk/src/lib/types.ts

key-decisions:
  - "PresetEntry named distinctly from AcPresetSummary (multiplayer server presets) to avoid collision"
  - "4 presets marked featured=true (2 Race, 1 Casual, 1 Challenge) for Staff Picks hero section"
  - "presets field optional (?) in TypeScript for backward compatibility during rolling deploy"
  - "Race/trackday presets excluded when track has_ai=false, same pattern as validate_launch_combo"

patterns-established:
  - "Curated preset pattern: static array in catalog.rs, filtered same as cars/tracks by manifest"
  - "preset_car_name/preset_track_name helpers resolve display names from FEATURED_CARS/FEATURED_TRACKS"

requirements-completed: [CONT-08, CONT-09]

# Metrics
duration: 6min
completed: 2026-03-14
---

# Phase 7 Plan 01: Curated Presets Data Model Summary

**14 curated presets (Race/Casual/Challenge) with PresetEntry struct, manifest-aware filtering, and TypeScript types in both PWA and kiosk**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-14T01:42:28Z
- **Completed:** 2026-03-14T01:49:11Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- PresetEntry struct with 10 fields (id, name, tagline, car_id, track_id, session_type, difficulty, category, duration_hint, featured)
- 14 curated presets across 3 categories: Race (5), Casual (5), Challenge (4) -- all car/track IDs validated against static arrays
- get_catalog() and get_filtered_catalog() both return "presets" field in JSON response
- Manifest-aware preset filtering: car+track must be installed, race/trackday requires AI lines
- PresetEntry TypeScript interface added to both PWA (api.ts) and kiosk (types.ts) with optional presets field
- 6 new TDD tests pass; full test suite green (420 tests across 3 crates)

## Task Commits

Each task was committed atomically:

1. **Task 1: PresetEntry struct, PRESETS array, catalog integration, TDD tests** - `83824cb` (feat)
2. **Task 2: Update TypeScript types in PWA and kiosk** - `50b52ae` (feat)

## Files Created/Modified
- `crates/rc-core/src/catalog.rs` - PresetEntry struct, 14-entry PRESETS array, preset helpers, catalog integration, 6 tests
- `pwa/src/lib/api.ts` - PresetEntry interface + optional presets field on ACCatalog
- `kiosk/src/lib/types.ts` - PresetEntry interface + optional presets field on AcCatalog

## Decisions Made
- PresetEntry named distinctly from AcPresetSummary in types.rs (different concept: multiplayer server presets vs curated quick-start presets)
- 4 featured presets (gt3-spa-race, f1-monza, nordschleife-tourist, hotlap-monza-f1) selected for Staff Picks hero section
- TypeScript presets field marked optional (?) for backward compatibility during rolling deploy where old rc-core may not send presets yet
- Race/trackday presets filtered by has_ai using same pattern as validate_launch_combo and enrich_track_entry

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Catalog API returns presets in JSON response, ready for PWA and kiosk UI integration (Plan 02)
- TypeScript types ready in both frontends for consuming preset data
- Preset filtering tested against manifests with and without AI lines

---
*Phase: 07-curated-presets*
*Completed: 2026-03-14*
