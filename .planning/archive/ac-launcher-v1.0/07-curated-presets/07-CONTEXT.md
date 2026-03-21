# Phase 7: Curated Presets - Context

**Gathered:** 2026-03-14
**Status:** Ready for planning

<domain>
## Phase Boundary

Customers can pick from popular pre-configured experiences for a fast path to driving. 10-15 curated presets covering race, casual/tourist, and challenge categories, with a hero "Staff Picks" section at the top. Presets pre-fill the configurator (car, track, session type, recommended difficulty) — customer can tweak before launching. Presets are filtered by pod content manifest (Phase 5) so only launchable presets are shown. Both PWA and kiosk get preset quick-picks.

</domain>

<decisions>
## Implementation Decisions

### Preset Content & Selection
- **10-15 presets** at launch — broad variety covering F1 circuits, GT3, drift, tourist drives, hotlap challenges
- **Three experience categories:** Race-focused (F1/GT3 battles), Casual/tourist (scenic drives, drift, beginners), Challenge/record (hotlap attempts, time attack)
- **Organization:** Featured hero section ("Staff Picks") at top with 3-4 presets, then categories below for browsing
- **Naming:** Straightforward **Car + Track** format (e.g. "GT3 at Spa-Francorchamps"), not marketing names
- **Track thumbnails** as visual anchor on each preset card — image source is Claude's discretion (AC preview files, bundled assets, or hybrid)
- **1-2 line tagline** per preset describing the experience (e.g. "Race 10 GT3 cars through the legendary Belgian circuit")
- **Estimated session duration** shown as a badge (e.g. "~15 min", "~30 min")
- **Filtered by pod manifest** (Phase 5) — only show presets where car AND track are installed. Consistent with "only show what works" principle.

### Presentation in PWA
- **Presets are the first screen** when booking — "Quick Pick" section with hero presets, categories below
- **"Custom Experience" is an equal button** alongside presets — both paths feel equally valid, not hidden
- **Tapping a preset opens the pre-filled configurator** — customer can tweak difficulty, AI count, etc. before launching
- **Hero section layout:** Claude's discretion for card design (horizontal scroll, stacked, etc.)

### What a Preset Includes
- **Pre-filled fields:** Car + Track + Session type + Recommended difficulty tier
- **Derived fields:** AI count derived from session type defaults (not preset-specified)
- **Customer-tweakable:** Difficulty, AI count, assists, FFB — all modifiable in the pre-filled configurator
- **Track layout:** Preset specifies track ID only — if multiple layouts exist, customer picks layout in configurator
- **Description:** 1-2 line tagline per preset
- **Duration hint:** Estimated session time badge

### Preset Management
- **Hardcoded in Rust** — static array in catalog.rs, alongside existing FEATURED_CARS/FEATURED_TRACKS pattern
- **`featured: bool` field** on each preset — defaults hardcoded now, kiosk toggle deferred to Phase 8
- **Same catalog endpoint** — add a "presets" field to get_catalog()/get_filtered_catalog() response. One API call.
- **Both PWA and kiosk** get preset quick-picks — staff uses them for walk-in customers too

### Claude's Discretion
- Track thumbnail image source (AC preview files, bundled assets, or hybrid approach)
- Hero section card layout and visual design
- Exact preset list (which 10-15 car/track/session combos) — based on popular AC community combinations
- How preset filtering integrates with existing get_filtered_catalog() function
- Kiosk preset UI placement (within existing GameConfigurator or new section)

</decisions>

<specifics>
## Specific Ideas

- Presets should feel like a "menu" at a restaurant — customers scan, pick something appealing, and go. The full configurator is the "build your own" option.
- The existing FEATURED_TRACKS (26 tracks) and FEATURED_CARS (190+ cars) in catalog.rs are the reference pool — presets combine specific pairings from this pool.
- PWA booking page already has a "Featured / Show All" toggle and multi-step wizard — presets add a quick path that bypasses the first few steps.
- Estimated session duration helps customers match presets to their remaining credits/time.

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `FEATURED_TRACKS` / `FEATURED_CARS` (catalog.rs): Static curated arrays — preset data model follows the same pattern
- `get_catalog()` / `get_filtered_catalog()` (catalog.rs): Existing catalog API — extend with "presets" field
- `build_custom_launch_args()` (catalog.rs): Builds launch JSON from car/track/difficulty — presets pre-fill these args
- `DIFFICULTY_PRESETS` (PWA book/page.tsx, kiosk GameConfigurator.tsx): Client-side difficulty presets — reference for how presets display
- `GameConfigurator` (kiosk): Multi-step wizard with track/car/difficulty selection — preset tap should jump into this with fields pre-filled

### Established Patterns
- Static arrays in Rust for curated content (FEATURED_TRACKS, FEATURED_CARS, ALL_CAR_IDS, ALL_TRACK_IDS)
- Catalog JSON response structure: `{ tracks: { featured, all }, cars: { featured, all }, categories }` — add `presets` at same level
- Per-pod content filtering via ContentManifest (Phase 5) — preset filtering reuses the same car_ids/track_ids check
- PWA "Featured / Show All" toggle — presets extend this concept to full experiences, not just individual cars/tracks

### Integration Points
- catalog.rs: Add PRESETS array + include in get_catalog()/get_filtered_catalog()
- PWA book/page.tsx: New preset selection screen as first step, existing configurator as "Custom Experience"
- Kiosk GameConfigurator.tsx: Add preset quick-pick section (before step 1 of wizard)
- PWA api.ts / kiosk api.ts: Parse presets from existing catalog response

</code_context>

<deferred>
## Deferred Ideas

- Staff kiosk UI to pin/unpin hero presets — Phase 8 (Staff & PWA Integration)
- Customer-created or saved favorite presets — future feature
- Preset usage analytics (which presets are most popular) — future feature
- Seasonal/themed preset rotations — ops concern, not code

</deferred>

---

*Phase: 07-curated-presets*
*Context gathered: 2026-03-14*
