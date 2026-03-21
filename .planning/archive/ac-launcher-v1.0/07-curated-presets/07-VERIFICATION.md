---
phase: 07-curated-presets
verified: 2026-03-14T02:30:00Z
status: passed
score: 7/7 must-haves verified
re_verification: false
human_verification:
  - test: "Open PWA /book page and verify preset landing screen appears with Staff Picks hero and category sections"
    expected: "3-4 featured presets in horizontal scroll with gradient backgrounds, then Race/Casual/Challenge sections below"
    why_human: "Visual rendering, gradient colors, layout responsiveness cannot be verified programmatically"
  - test: "Tap a preset in PWA and verify wizard jumps to Confirm step with correct pre-filled values"
    expected: "Track, car, and difficulty pre-filled matching the selected preset; customer can review and book"
    why_human: "End-to-end UI flow with state transitions requires visual verification"
  - test: "Open kiosk GameConfigurator and verify preset quick-pick section appears as first step"
    expected: "Staff Picks at top, all presets below, Custom Setup button at bottom; tapping preset jumps to review"
    why_human: "Kiosk UI rendering and flow requires on-site visual verification"
---

# Phase 7: Curated Presets Verification Report

**Phase Goal:** Customers can pick from popular pre-configured experiences for a fast path to driving
**Verified:** 2026-03-14T02:30:00Z
**Status:** passed
**Re-verification:** No -- initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Catalog API response includes a "presets" array with curated preset objects | VERIFIED | `get_catalog()` returns JSON with `"presets"` key (catalog.rs:316); test `catalog_includes_presets` asserts 13-15 entries and passes; 14 PresetEntry structs defined in PRESETS array (catalog.rs:513-685) |
| 2 | Each preset has all required fields (id, name, tagline, car_id, track_id, session_type, difficulty, category, duration_hint, featured) | VERIFIED | PresetEntry struct has all 10 fields (catalog.rs:500-511); `presets_to_json()` serializes all fields including resolved car_name/track_name (catalog.rs:695-710) |
| 3 | Presets include real-world popular AC combinations (Spa GT3, Nurburgring Hot Lap, Monza F1) | VERIFIED | gt3-spa-race (ks_ferrari_488_gt3 + spa), f1-monza (ferrari_sf25 + monza), nordschleife-tourist (ks_porsche_911_gt3_rs + ks_nordschleife), hotlap-monza-f1 (ks_ferrari_f2004 + monza) -- all verified against ALL_CAR_IDS/ALL_TRACK_IDS by passing tests `preset_car_ids_valid` and `preset_track_ids_valid` |
| 4 | PWA shows preset cards as first screen before the wizard with Staff Picks hero section | VERIFIED | `showPresets` state defaults to `true` (page.tsx:88); `if (showPresets)` renders preset landing screen (page.tsx:298-393); featured presets filtered and displayed in "Staff Picks" section with horizontal scroll (page.tsx:338-347); PresetCard component renders gradient backgrounds, track name, car name, tagline, duration badge (page.tsx:1444-1489) |
| 5 | Selecting a preset fills in all fields and lets customer launch with one tap | VERIFIED | PWA `selectPreset()` looks up car/track from catalog.all, sets track/car/difficulty, jumps to Confirm step (page.tsx:206-222); Kiosk `selectPreset()` looks up car/track from catalog, sets game/track/car/difficulty, jumps to review step (GameConfigurator.tsx:103-113) |
| 6 | Kiosk GameConfigurator shows preset quick-pick section as initial step | VERIFIED | ConfigStep type includes "presets" as first value (GameConfigurator.tsx:15); initial step set to "presets" (GameConfigurator.tsx:45); step === "presets" renders Staff Picks and All Presets grids with Custom Setup button (GameConfigurator.tsx:175-262) |
| 7 | Presets filtered by pod manifest -- only matching car+track appear; race/trackday excluded without AI lines | VERIFIED | `get_filtered_catalog()` filters presets by `car_ids.contains()` AND `track_ids.contains()` (catalog.rs:383-398); race/trackday presets check `has_ai` (catalog.rs:388-396); tests `filtered_catalog_filters_presets` and `preset_race_filtered_no_ai` both pass |

**Score:** 7/7 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-core/src/catalog.rs` | PresetEntry struct, PRESETS array, catalog integration | VERIFIED | Struct at line 500, 14-entry PRESETS array at line 513, presets_to_json helper at line 695, integrated into get_catalog (line 316) and get_filtered_catalog (line 413); 6 passing tests |
| `pwa/src/lib/api.ts` | PresetEntry TypeScript interface in ACCatalog | VERIFIED | PresetEntry interface with all 12 fields at lines 277-290; ACCatalog includes optional `presets?: PresetEntry[]` at line 296 |
| `kiosk/src/lib/types.ts` | PresetEntry TypeScript interface in AcCatalog | VERIFIED | PresetEntry interface with all 12 fields at lines 232-245; AcCatalog includes optional `presets?: PresetEntry[]` at line 260 |
| `pwa/src/app/book/page.tsx` | Preset landing screen with hero, categories, pre-fill logic | VERIFIED | showPresets state, selectPreset/startCustom handlers, Staff Picks hero section, category sections, "Build Your Own Experience" button, PresetCard component, "Back to Presets" link, eager catalog loading |
| `kiosk/src/components/GameConfigurator.tsx` | Preset quick-pick section in game configurator | VERIFIED | "presets" ConfigStep as initial step, featured/all grids, selectPreset handler, "Custom Setup" button, empty state handling |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| catalog.rs get_catalog() | PRESETS array | `presets_to_json(&all_presets)` in JSON response | WIRED | Line 316: `"presets": presets_to_json(&all_presets)` |
| catalog.rs get_filtered_catalog() | PRESETS + ContentManifest | Filters presets by car_ids/track_ids HashSets + AI line check | WIRED | Lines 383-413: filter logic and JSON inclusion |
| PWA page.tsx | ACCatalog.presets | `catalog?.presets` mapped to PresetCard components, tap calls selectPreset | WIRED | Lines 299-343: presets extracted from catalog, PresetCard renders each, onClick calls selectPreset |
| PWA selectPreset() | catalog.tracks.all / catalog.cars.all | Finds full track/car objects by preset.track_id/car_id | WIRED | Lines 208-209: `.find()` lookups with defensive error handling |
| Kiosk GameConfigurator | AcCatalog.presets | `catalog?.presets` mapped to quick-pick cards | WIRED | Lines 176-177: presets extracted from catalog, featured filtered, onClick calls selectPreset |
| Kiosk selectPreset() | catalog.tracks.all / catalog.cars.all | Finds track/car objects and sets wizard state | WIRED | Lines 105-107: `.find()` lookups, then state set and step changed to review |
| PWA api.ts acCatalog() | rc-core /customer/ac/catalog | fetchApi call returns ACCatalog with presets | WIRED | Line 581: `acCatalog: () => fetchApi<ACCatalog & { error?: string }>("/customer/ac/catalog")` |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| CONT-08 | 07-01, 07-02 | Curated popular presets available (e.g. "Spa GT3 Race", "Nurburgring Hot Lap", "Monza F1") | SATISFIED | 14 presets in PRESETS array including gt3-spa-race, hotlap-monza-f1, f1-monza, nordschleife-tourist; PWA and kiosk both display them; all car/track IDs validated by tests |
| CONT-09 | 07-01, 07-02 | Presets sourced from popular real-world AC community combinations | SATISFIED | Presets cover GT3 at Spa, F1 at Monza, Nordschleife tourist laps, AE86 on Mt. Haruna (Initial D), drift on Shuto Expressway, McLaren P1 at Monaco -- all well-known AC community favorites; categories (Race/Casual/Challenge) cover a range of driving experiences |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| pwa/src/app/book/page.tsx | 639 | "Coming soon" text | Info | Pre-existing label for disabled multiplayer mode -- not related to presets |
| kiosk/src/components/GameConfigurator.tsx | 279, 306 | "Coming Soon" text | Info | Pre-existing labels for disabled games/multiplayer -- not related to presets |

No blocker or warning anti-patterns found in preset-related code. All preset implementations are substantive with real logic, not stubs.

### Human Verification Required

### 1. PWA Preset Landing Screen Visual Check

**Test:** Navigate to /book page in PWA after login. Verify the preset landing screen appears as the first view.
**Expected:** Staff Picks hero section at top with 3-4 featured preset cards in horizontal scroll with category-gradient backgrounds (Race=red, Casual=blue, Challenge=purple). Category sections below with grid layout. "Build Your Own Experience" button prominently displayed at bottom.
**Why human:** Visual rendering, gradient colors, card layout, and responsive behavior cannot be verified programmatically.

### 2. PWA Preset Selection Flow

**Test:** Tap a featured preset (e.g. "GT3 at Spa-Francorchamps"). Verify the wizard jumps to the Confirm step with pre-filled values.
**Expected:** Track shows "Spa-Francorchamps", Car shows "Ferrari 488 GT3", Difficulty shows "Medium". User can review and proceed to book. "Back to Presets" link visible to return to preset screen.
**Why human:** End-to-end flow with state transitions and correct pre-fill requires visual verification.

### 3. Kiosk Preset Quick-Pick Flow

**Test:** Open kiosk GameConfigurator for a pod. Verify preset quick-pick section appears as the first step ("Quick Start" header).
**Expected:** Staff Picks cards at top in 2-column grid, remaining presets below in 3-column grid, "Custom Setup" button at bottom. Tapping a preset fills car/track/difficulty and jumps to Review step.
**Why human:** Kiosk rendering on-site with actual catalog data requires visual verification.

### Gaps Summary

No gaps found. All observable truths are verified with supporting evidence:

1. **Backend data model** -- PresetEntry struct with 10 fields, 14 curated presets in static array, presets_to_json serialization, integrated into both get_catalog() and get_filtered_catalog() with manifest-aware filtering. 6 TDD tests all passing.

2. **Frontend types** -- PresetEntry TypeScript interface defined identically in both PWA (api.ts) and kiosk (types.ts) with optional presets field on catalog types for backward compatibility.

3. **PWA UI** -- Preset landing screen shown first (showPresets=true default), Staff Picks hero with horizontal scroll of featured presets, categorized sections (Race/Casual/Challenge), PresetCard component with gradient backgrounds and duration badges, selectPreset handler pre-fills wizard and jumps to Confirm step, "Build Your Own Experience" equally prominent, "Back to Presets" navigation in wizard, eager catalog loading for immediate display.

4. **Kiosk UI** -- "presets" ConfigStep as initial step, featured and all preset grids with category-colored left borders, selectPreset jumps to review with pre-filled values, "Custom Setup" button advances to normal game selection, empty preset graceful handling.

5. **Real-world combinations** -- Presets cover popular AC community experiences: Spa GT3, Monza F1, Nordschleife, Shuto Expressway drift, AE86 touge, Monaco supercar cruise, Laguna Seca time attack.

6. **All commits verified** -- 83824cb (data model), 50b52ae (TS types), f505ac0 (PWA UI), 7df0871 (kiosk UI).

---

_Verified: 2026-03-14T02:30:00Z_
_Verifier: Claude (gsd-verifier)_
