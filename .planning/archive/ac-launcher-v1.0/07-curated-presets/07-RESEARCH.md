# Phase 7: Curated Presets - Research

**Researched:** 2026-03-14
**Domain:** Static preset data model + PWA/kiosk booking UX integration
**Confidence:** HIGH

## Summary

Phase 7 adds 10-15 curated "quick start" presets to the existing AC booking flow. Each preset is a specific car + track + session type + recommended difficulty combination. The work is primarily a data modeling task (Rust static array) plus UI additions to both PWA and kiosk. There is no new infrastructure needed -- presets extend the existing catalog system that already has established patterns for static arrays (`FEATURED_TRACKS`, `FEATURED_CARS`) and filtered catalog responses.

The main integration points are: (1) a new `PRESETS` static array in `catalog.rs` following the exact same pattern as `FEATURED_TRACKS`/`FEATURED_CARS`, (2) adding a `presets` field to the `get_catalog()` and `get_filtered_catalog()` JSON responses, and (3) new UI in both PWA `book/page.tsx` and kiosk `GameConfigurator.tsx` to display presets and pre-fill the existing configurator workflow. The preset filtering against pod content manifests reuses the Phase 5 infrastructure (`ContentManifest` car/track ID sets).

**Primary recommendation:** Follow the established static-array-in-Rust pattern exactly. Add `PRESETS` array, extend `get_catalog()`/`get_filtered_catalog()` to include a `presets` field, then build PWA and kiosk UI as a "quick path" that pre-fills existing wizard state and drops the customer into the configurator review step.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **10-15 presets** at launch -- broad variety covering F1 circuits, GT3, drift, tourist drives, hotlap challenges
- **Three experience categories:** Race-focused (F1/GT3 battles), Casual/tourist (scenic drives, drift, beginners), Challenge/record (hotlap attempts, time attack)
- **Organization:** Featured hero section ("Staff Picks") at top with 3-4 presets, then categories below for browsing
- **Naming:** Straightforward **Car + Track** format (e.g. "GT3 at Spa-Francorchamps"), not marketing names
- **Track thumbnails** as visual anchor on each preset card
- **1-2 line tagline** per preset describing the experience
- **Estimated session duration** shown as a badge (e.g. "~15 min", "~30 min")
- **Filtered by pod manifest** (Phase 5) -- only show presets where car AND track are installed
- **Presets are the first screen** when booking -- "Quick Pick" section with hero presets, categories below
- **"Custom Experience" is an equal button** alongside presets -- both paths feel equally valid, not hidden
- **Tapping a preset opens the pre-filled configurator** -- customer can tweak difficulty, AI count, etc. before launching
- **Pre-filled fields:** Car + Track + Session type + Recommended difficulty tier
- **Derived fields:** AI count derived from session type defaults (not preset-specified)
- **Customer-tweakable:** Difficulty, AI count, assists, FFB -- all modifiable in the pre-filled configurator
- **Track layout:** Preset specifies track ID only -- if multiple layouts exist, customer picks layout in configurator
- **Hardcoded in Rust** -- static array in catalog.rs, alongside existing FEATURED_CARS/FEATURED_TRACKS pattern
- **`featured: bool` field** on each preset -- defaults hardcoded now, kiosk toggle deferred to Phase 8
- **Same catalog endpoint** -- add a "presets" field to get_catalog()/get_filtered_catalog() response. One API call.
- **Both PWA and kiosk** get preset quick-picks -- staff uses them for walk-in customers too

### Claude's Discretion
- Track thumbnail image source (AC preview files, bundled assets, or hybrid approach)
- Hero section card layout and visual design
- Exact preset list (which 10-15 car/track/session combos) -- based on popular AC community combinations
- How preset filtering integrates with existing get_filtered_catalog() function
- Kiosk preset UI placement (within existing GameConfigurator or new section)

### Deferred Ideas (OUT OF SCOPE)
- Staff kiosk UI to pin/unpin hero presets -- Phase 8 (Staff & PWA Integration)
- Customer-created or saved favorite presets -- future feature
- Preset usage analytics (which presets are most popular) -- future feature
- Seasonal/themed preset rotations -- ops concern, not code
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| CONT-08 | Curated popular presets available (e.g. "Spa GT3 Race", "Nurburgring Hot Lap", "Monza F1") | Static `PRESETS` array in catalog.rs with PresetEntry struct; `get_catalog()`/`get_filtered_catalog()` extended with `presets` field; PWA and kiosk UI show preset cards; tapping pre-fills configurator |
| CONT-09 | Presets sourced from popular real-world AC community combinations | Preset list curated from popular AC community combos based on featured cars/tracks already in catalog; verified against ALL_CAR_IDS and ALL_TRACK_IDS to ensure all preset car/track IDs exist |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Rust (serde, serde_json) | current | Serialize preset data in catalog JSON | Already used throughout catalog.rs |
| Next.js (React) | 14+ | PWA and kiosk frontends | Already the framework for both apps |
| Tailwind CSS | current | Styling preset cards | Already used in both PWA and kiosk |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| None | -- | -- | No new dependencies needed |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Static Rust array | Database table | DB adds runtime complexity; static array matches existing pattern and is faster. Kiosk toggle for `featured` deferred to Phase 8 |
| Bundled track images | External image CDN | CDN adds network dependency; bundled static assets work offline (venue may lose internet) |

**Installation:**
```bash
# No new packages needed -- all infrastructure already exists
```

## Architecture Patterns

### Recommended Project Structure
```
crates/rc-core/src/
  catalog.rs                    # Add PRESETS array + PresetEntry struct + filtering

pwa/src/
  app/book/page.tsx             # Add preset selection as initial screen before wizard
  lib/api.ts                    # Add PresetEntry type to ACCatalog interface

kiosk/src/
  components/GameConfigurator.tsx  # Add preset quick-pick section before step 1
  lib/types.ts                     # Add PresetEntry type to AcCatalog interface
```

### Pattern 1: Static Curated Array (Rust)
**What:** Define `PRESETS` as a `const` static array of `PresetEntry` structs, exactly like `FEATURED_TRACKS` and `FEATURED_CARS`.
**When to use:** Always -- this is the locked decision.
**Example:**
```rust
// Source: Existing pattern in catalog.rs (FEATURED_TRACKS, FEATURED_CARS)
#[derive(Debug, Clone, Serialize)]
pub struct PresetEntry {
    pub id: &'static str,           // unique slug: "gt3-spa-race"
    pub name: &'static str,         // "GT3 at Spa-Francorchamps"
    pub tagline: &'static str,      // "Race 10 GT3 cars through Eau Rouge"
    pub car_id: &'static str,       // must exist in ALL_CAR_IDS
    pub track_id: &'static str,     // must exist in ALL_TRACK_IDS
    pub session_type: &'static str, // "race", "practice", "hotlap", "trackday"
    pub difficulty: &'static str,   // "easy", "medium", "hard"
    pub category: &'static str,     // "Race", "Casual", "Challenge"
    pub duration_hint: &'static str,// "~15 min", "~30 min"
    pub featured: bool,             // true = show in hero/Staff Picks section
}

const PRESETS: &[PresetEntry] = &[
    PresetEntry {
        id: "gt3-spa-race",
        name: "GT3 at Spa-Francorchamps",
        tagline: "Race 10 GT3 cars through the legendary Belgian circuit",
        car_id: "ks_ferrari_488_gt3",
        track_id: "spa",
        session_type: "race",
        difficulty: "medium",
        category: "Race",
        duration_hint: "~20 min",
        featured: true,
    },
    // ... 10-15 total presets
];
```

### Pattern 2: Catalog Response Extension
**What:** Add `presets` field at the same level as `tracks`/`cars`/`categories` in the catalog JSON.
**When to use:** In both `get_catalog()` and `get_filtered_catalog()`.
**Example:**
```rust
// Source: Existing get_catalog() pattern in catalog.rs
json!({
    "tracks": { "featured": ..., "all": ... },
    "cars": { "featured": ..., "all": ... },
    "categories": { ... },
    "presets": presets_json,  // NEW: array of preset objects
})
```

### Pattern 3: Preset Filtering Against Manifest
**What:** In `get_filtered_catalog()`, filter presets to only those where both `car_id` AND `track_id` exist in the pod's `ContentManifest`.
**When to use:** When manifest is `Some` (pod-specific catalog).
**Example:**
```rust
// Source: Existing filtering pattern in get_filtered_catalog()
let presets: Vec<Value> = PRESETS
    .iter()
    .filter(|p| car_ids.contains(p.car_id) && track_ids.contains(p.track_id))
    .map(|p| json!({
        "id": p.id,
        "name": p.name,
        "tagline": p.tagline,
        "car_id": p.car_id,
        "car_name": find_car_name(p.car_id),
        "track_id": p.track_id,
        "track_name": find_track_name(p.track_id),
        "session_type": p.session_type,
        "difficulty": p.difficulty,
        "category": p.category,
        "duration_hint": p.duration_hint,
        "featured": p.featured,
    }))
    .collect();
```

### Pattern 4: PWA Preset-to-Configurator Flow
**What:** When customer taps a preset, pre-fill the wizard state (car, track, difficulty, mode) and jump to the confirmation/review step, skipping manual selection steps. Customer can still go back and tweak.
**When to use:** In PWA `book/page.tsx` after the new preset selection screen.
**Example:**
```typescript
// Source: Existing wizard state in BookWizard
function selectPreset(preset: PresetEntry) {
  // Find the catalog items to get full objects
  const presetTrack = catalog?.tracks.all.find(t => t.id === preset.track_id);
  const presetCar = catalog?.cars.all.find(c => c.id === preset.car_id);
  if (presetTrack) setTrack(presetTrack);
  if (presetCar) setCar(presetCar);
  setDifficulty(preset.difficulty as "easy" | "medium" | "hard");
  // Jump to Confirm step (skip Track/Car/Difficulty/Transmission manual selection)
  setStep(confirmStepIndex);
}
```

### Anti-Patterns to Avoid
- **Separate API endpoint for presets:** Do NOT create a new `/presets` endpoint. The decision is to include presets in the existing catalog response. One API call, one cache.
- **Dynamic presets from database:** Do NOT store presets in the database. The decision is static Rust array. DB-backed presets are deferred.
- **Referencing car/track IDs not in ALL_CAR_IDS/ALL_TRACK_IDS:** Every preset MUST reference IDs that exist in the static arrays. Add a compile-time or test-time validation.
- **Hiding "Custom Experience":** The custom path MUST remain equally prominent alongside presets. Not a small link, but an equal-size button/card.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Track thumbnail images | Image generation or scraping | Static bundled placeholder images or AC preview file paths | AC has preview images in track folders; use these or simple colored placeholders |
| Preset filtering | Custom filter logic | Reuse the existing `car_ids`/`track_ids` HashSets from `get_filtered_catalog()` | Already built in Phase 5, battle-tested |
| Preset ID validation | Runtime validation | Unit test that cross-references PRESETS car/track IDs against ALL_CAR_IDS/ALL_TRACK_IDS | Catch invalid IDs at compile/test time, not runtime |
| Category organization | Custom sorting | Simple `.filter(p => p.category === cat)` in frontend | Categories are just strings, no complex logic needed |

**Key insight:** This phase is a thin layer on top of existing infrastructure. The catalog system, manifest filtering, and booking wizard are all built. Presets add static data and UI, not new systems.

## Common Pitfalls

### Pitfall 1: Preset Car/Track IDs Not in Static Arrays
**What goes wrong:** A preset references `car_id: "ferrari_f40"` but the actual ID in `ALL_CAR_IDS` is `"ferrari_f40"` (correct) or `"ks_ferrari_f40"` (mismatch). Preset silently disappears from filtered catalog.
**Why it happens:** AC car/track folder names have inconsistent prefixes (`ks_`, `ddm_`, etc.).
**How to avoid:** Write a unit test that iterates all PRESETS and asserts each `car_id` exists in `ALL_CAR_IDS` and each `track_id` exists in `ALL_TRACK_IDS`.
**Warning signs:** Preset count in API response is less than expected.

### Pitfall 2: PWA Step Index Drift
**What goes wrong:** The PWA wizard uses numeric step indices (1-8). Adding a preset selection screen before Step 1 shifts all indices, breaking the existing wizard flow.
**Why it happens:** The current `step` state is a number, and content is mapped via `stepLabels[step - 1]`.
**How to avoid:** Two safe approaches: (A) Add presets as a "step 0" or landing screen BEFORE the wizard starts, using a boolean flag like `showPresets: true` that gates entry to the wizard. (B) Insert a new step label at position 0 and shift all steps up by 1. Approach A is cleaner -- the preset screen is conceptually separate from the wizard.
**Warning signs:** Steps show wrong content, step progress bar off by one.

### Pitfall 3: Preset Pre-Fill Not Finding Catalog Items
**What goes wrong:** Customer taps preset, but `catalog?.tracks.all.find(t => t.id === preset.track_id)` returns `undefined` because the catalog hasn't loaded yet, or the preset references an ID filtered out by the manifest.
**Why it happens:** Catalog is lazy-loaded when the Track step is reached. If presets are shown before catalog loads, the lookup fails.
**How to avoid:** Load catalog eagerly when presets are displayed (not lazily on Track step). The catalog is small (JSON) and loads fast. Also, presets are already filtered server-side, so this shouldn't happen -- but add a defensive check.
**Warning signs:** Preset tap does nothing, or fills in null car/track.

### Pitfall 4: Kiosk and PWA Preset Type Mismatch
**What goes wrong:** PWA `api.ts` and kiosk `types.ts` define different TypeScript interfaces for presets, causing one to break when the Rust struct changes.
**Why it happens:** Two separate TypeScript codebases with manually-maintained types.
**How to avoid:** Define the `PresetEntry` interface identically in both `pwa/src/lib/api.ts` (as part of `ACCatalog`) and `kiosk/src/lib/types.ts` (as part of `AcCatalog`). Use the same field names.
**Warning signs:** One frontend works, the other doesn't parse preset data.

### Pitfall 5: AcPresetSummary Name Collision
**What goes wrong:** The codebase already has `AcPresetSummary` in `types.rs` -- this is for AC multiplayer **server** presets (LAN race configs stored in DB), NOT curated experience presets.
**Why it happens:** Similar naming, different concepts.
**How to avoid:** Name the new struct `CuratedPresetEntry` or `ExperiencePreset` -- NOT `AcPreset` or `PresetSummary`. Or keep it simple: `PresetEntry` in `catalog.rs` (not exported to types.rs).
**Warning signs:** Import confusion between server presets and curated presets.

## Code Examples

Verified patterns from the existing codebase:

### Existing Static Array Pattern (catalog.rs)
```rust
// Source: catalog.rs lines 28-69 (FEATURED_TRACKS)
const FEATURED_TRACKS: &[TrackEntry] = &[
    TrackEntry { id: "spa", name: "Spa-Francorchamps", category: "F1 Circuits", country: "Belgium" },
    TrackEntry { id: "monza", name: "Monza", category: "F1 Circuits", country: "Italy" },
    // ...
];
```

### Existing Catalog JSON Response (catalog.rs)
```rust
// Source: catalog.rs lines 301-314 (get_catalog)
json!({
    "tracks": { "featured": featured_tracks, "all": all_tracks },
    "cars": { "featured": featured_cars, "all": all_cars },
    "categories": {
        "tracks": ["F1 Circuits", "Real Circuits", "Indian Circuits", "Street / Touge", "Other"],
        "cars": ["F1 2025", "GT3", "Supercars", "Porsche", "JDM", "Classics", "Other"],
    }
})
```

### Existing Manifest Filtering (catalog.rs)
```rust
// Source: catalog.rs lines 330-331 (get_filtered_catalog)
let car_ids: HashSet<&str> = manifest.cars.iter().map(|c| c.id.as_str()).collect();
let track_ids: HashSet<&str> = manifest.tracks.iter().map(|t| t.id.as_str()).collect();
// Then: .filter(|c| car_ids.contains(c.id))
```

### Existing PWA Catalog Types (api.ts)
```typescript
// Source: pwa/src/lib/api.ts lines 277-281
export interface ACCatalog {
  tracks: { featured: CatalogTrack[]; all: CatalogTrack[] };
  cars: { featured: CatalogCar[]; all: CatalogCar[] };
  categories: { tracks: string[]; cars: string[] };
}
```

### Existing Kiosk Catalog Types (types.ts)
```typescript
// Source: kiosk/src/lib/types.ts lines 232-244
export interface AcCatalog {
  tracks: { featured: CatalogItem[]; all: CatalogItem[] };
  cars: { featured: CatalogItem[]; all: CatalogItem[] };
  categories: { tracks: string[]; cars: string[] };
}
```

### Existing Catalog API Call (routes.rs)
```rust
// Source: routes.rs lines 4567-4577
async fn customer_ac_catalog(
    State(state): State<Arc<AppState>>,
    Query(query): Query<CatalogQuery>,
) -> Json<Value> {
    let manifest = if let Some(ref pod_id) = query.pod_id {
        state.pod_manifests.read().await.get(pod_id).cloned()
    } else { None };
    Json(catalog::get_filtered_catalog(manifest.as_ref()))
}
```

## Recommended Preset List

Based on the cars/tracks available in `ALL_CAR_IDS` and `FEATURED_TRACKS`/`FEATURED_CARS`, here are recommended presets covering the three categories. All car and track IDs verified against existing static arrays.

### Race Category (4-5 presets)
| Preset | Car ID | Track ID | Session | Difficulty |
|--------|--------|----------|---------|------------|
| GT3 at Spa | `ks_ferrari_488_gt3` | `spa` | race | medium |
| F1 2025 at Monza | `ferrari_sf25` | `monza` | race | hard |
| GT3 at Silverstone | `ks_mercedes_amg_gt3` | `ks_silverstone` | race | medium |
| F1 2025 at Bahrain | `red_bull_rb21` | `bahrain` | race | hard |
| Supercars at Red Bull Ring | `ks_lamborghini_aventador_sv` | `ks_red_bull_ring` | race | easy |

### Casual Category (4-5 presets)
| Preset | Car ID | Track ID | Session | Difficulty |
|--------|--------|----------|---------|------------|
| Nordschleife Tourist | `ks_porsche_911_gt3_rs` | `ks_nordschleife` | practice | easy |
| JDM Touge Run | `ks_toyota_ae86` | `haruna` | practice | easy |
| Supercar at Monaco | `ks_mclaren_p1` | `monaco` | practice | easy |
| Drift at Shuto | `ks_toyota_supra_mkiv` | `shuto_revival_project_beta` | practice | easy |
| Indian Circuit Drive | `ks_ferrari_488_gtb` | `madras_international_circuit` | practice | easy |

### Challenge Category (3-4 presets)
| Preset | Car ID | Track ID | Session | Difficulty |
|--------|--------|----------|---------|------------|
| Hotlap: Monza F1 | `ks_ferrari_f2004` | `monza` | hotlap | hard |
| Hotlap: Nurburgring GT3 | `ks_porsche_911_gt3_r_2016` | `ks_nurburgring` | hotlap | medium |
| Time Attack: Laguna Seca | `ks_mazda_mx5_cup` | `ks_laguna_seca` | hotlap | medium |
| Nordschleife Challenge | `cky_porsche992_gt3rs_2023` | `ks_nordschleife` | hotlap | hard |

**Featured (Staff Picks):** GT3 at Spa, Nordschleife Tourist, F1 2025 at Monza, Hotlap: Monza F1 (3-4 hero presets)

## Track Thumbnail Strategy

**Recommendation: Colored placeholder cards with track name overlay (Phase 7), real images deferred.**

Options investigated:
1. **AC preview files** -- AC stores preview images in `content/tracks/{track_id}/preview.png`. These exist on pods but are NOT available on rc-core (the server doesn't have AC installed). Would require the agent to serve images, adding complexity.
2. **Bundled static images** -- Ship 10-15 track images as static assets in the Next.js `public/` folder. Requires sourcing/licensing images. Adds ~2-5MB to the build.
3. **Colored gradient placeholders** -- Use the track category to assign a gradient color (e.g., F1 Circuits = red, Real Circuits = blue, Street = purple). Show track name prominently. No images needed.
4. **Hybrid** -- Placeholders now, real images when available.

**Decision: Use approach 3 (colored placeholders) for Phase 7.** This keeps the phase focused on functionality, not asset sourcing. Track thumbnails can be added in a polish pass. The preset card design should use category-derived gradient backgrounds with the track name and car name as the visual anchors.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| No presets (manual selection only) | Static curated presets in catalog.rs | Phase 7 (now) | Fast-path booking for customers |
| Lazy catalog loading (on Track step) | Eager catalog loading (on preset display) | Phase 7 (now) | Presets need catalog to resolve car/track names |

**No deprecated patterns affected.** This is purely additive.

## Open Questions

1. **Session type validation for presets with AI-requiring modes**
   - What we know: Some presets specify `session_type: "race"` which requires AI lines on the track. The manifest filtering checks car+track presence but does NOT check AI availability.
   - What's unclear: Should presets with `session_type: "race"` be filtered out if the track's `has_ai` is false?
   - Recommendation: YES -- add an additional filter in `get_filtered_catalog()` that checks `has_ai` for race/trackday presets, consistent with how `available_session_types` works for individual tracks.

2. **PWA flow when catalog has no matching presets**
   - What we know: If a pod has very limited content, all presets might be filtered out.
   - What's unclear: Should we show "No presets available" or skip directly to Custom Experience?
   - Recommendation: Show "No matching presets for your rig" with a prominent "Custom Experience" button. This is edge case -- all 8 pods have similar content.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (Rust), no frontend test framework |
| Config file | Cargo.toml (workspace) |
| Quick run command | `cargo test -p rc-core --lib -- catalog` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CONT-08 | Presets appear in catalog response | unit | `cargo test -p rc-core --lib -- catalog::tests::catalog_includes_presets -x` | Wave 0 |
| CONT-08 | Presets filtered by manifest | unit | `cargo test -p rc-core --lib -- catalog::tests::filtered_catalog_filters_presets -x` | Wave 0 |
| CONT-08 | Featured presets marked correctly | unit | `cargo test -p rc-core --lib -- catalog::tests::presets_featured_flag -x` | Wave 0 |
| CONT-09 | All preset car IDs exist in ALL_CAR_IDS | unit | `cargo test -p rc-core --lib -- catalog::tests::preset_car_ids_valid -x` | Wave 0 |
| CONT-09 | All preset track IDs exist in ALL_TRACK_IDS | unit | `cargo test -p rc-core --lib -- catalog::tests::preset_track_ids_valid -x` | Wave 0 |
| CONT-08 | Race presets filtered when track has no AI | unit | `cargo test -p rc-core --lib -- catalog::tests::preset_race_filtered_no_ai -x` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-core --lib -- catalog`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `catalog::tests::catalog_includes_presets` -- verify presets field in get_catalog() response
- [ ] `catalog::tests::filtered_catalog_filters_presets` -- verify preset filtering by manifest
- [ ] `catalog::tests::preset_car_ids_valid` -- cross-reference preset car_ids against ALL_CAR_IDS
- [ ] `catalog::tests::preset_track_ids_valid` -- cross-reference preset track_ids against ALL_TRACK_IDS
- [ ] `catalog::tests::presets_featured_flag` -- verify featured presets count and flag
- [ ] `catalog::tests::preset_race_filtered_no_ai` -- race/trackday presets hidden when track lacks AI

*(All tests are new additions to existing `catalog::tests` module -- no new test files or framework setup needed)*

## Sources

### Primary (HIGH confidence)
- **catalog.rs** (rc-core) -- Direct source code review of FEATURED_TRACKS, FEATURED_CARS, get_catalog(), get_filtered_catalog(), validate_launch_combo()
- **types.rs** (rc-common) -- Direct source code review of ContentManifest, CarManifestEntry, TrackManifestEntry, AcPresetSummary (LAN server presets, different concept)
- **routes.rs** (rc-core) -- Direct source code review of customer_ac_catalog handler (lines 4562-4577)
- **api.ts** (PWA) -- Direct source code review of ACCatalog interface and acCatalog() call
- **types.ts** (kiosk) -- Direct source code review of AcCatalog and CatalogItem interfaces
- **GameConfigurator.tsx** (kiosk) -- Direct source code review of existing wizard flow
- **book/page.tsx** (PWA) -- Direct source code review of BookWizard multi-step flow

### Secondary (MEDIUM confidence)
- **ALL_CAR_IDS / ALL_TRACK_IDS** -- Static arrays verified from Pod 8 filesystem scan. Preset car/track IDs validated against these.

### Tertiary (LOW confidence)
- **Preset list curation** -- Based on general Assetto Corsa community knowledge (popular car/track combos). Should be reviewed by staff for local customer preferences.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new dependencies, reuses existing patterns
- Architecture: HIGH -- follows established FEATURED_TRACKS/FEATURED_CARS pattern exactly
- Pitfalls: HIGH -- derived from direct code review of existing integration points
- Preset list: MEDIUM -- based on AC community knowledge, may need adjustment for local customer preferences

**Research date:** 2026-03-14
**Valid until:** 2026-04-14 (stable -- static data, no external dependencies)
