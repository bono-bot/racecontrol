# Phase 8: Staff & PWA Integration - Research

**Researched:** 2026-03-14
**Domain:** Frontend wizard integration (Next.js/React) + Rust backend session_type wiring
**Confidence:** HIGH

## Summary

Phase 8 is a gap-filling integration phase, not greenfield work. The backend already supports all 5 session types (practice, hotlap, race, trackday, race_weekend) in the INI builder, catalog filtering, and validation. The kiosk SetupWizard already has a `session_type` step but only offers 3 options (practice, qualification, race). The kiosk GameConfigurator has a `mode` step (single/multi) but no session type at all. The PWA book/page.tsx has a `Mode` step (single/multi) but no session type concept.

The work consists of: (1) expanding kiosk session types from 3 to 5 with correct naming, (2) adding session type to GameConfigurator replacing the mode step, (3) replacing the PWA Mode step with a session type picker, (4) wiring `session_type` into launch_args on both customer and staff paths, (5) ensuring AI-line filtering hides unavailable session types, and (6) adding `session_type` to the backend `CustomBookingOptions` struct and `build_custom_launch_args()`.

**Primary recommendation:** Work front-to-back: update TypeScript types first, then kiosk wizards, then PWA wizard, then backend `CustomBookingOptions` + `build_custom_launch_args`, then verify the same `validate_launch_combo` path is hit for both.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **All 5 session types wired to both PWA and kiosk** -- Practice, Hotlap, Race vs AI, Track Day, Race Weekend
- **PWA: session type replaces the Mode step** -- "Mode (single/multi)" becomes a session type picker with the 5 types
- **Multiplayer stays as a separate entry point** -- not mixed into the session type list. Kept as its own button/card alongside the single-player session types.
- **Kiosk: both wizards updated** -- GameConfigurator (staff quick-launch from pod card) AND SetupWizard (full booking flow) both get all 5 session types
- **SetupWizard currently has 3 types** (practice, qualification, race) -- expand to all 5, replace "qualification" with "hotlap"
- **GameConfigurator currently has no session type** -- add session type step with all 5 types
- **AI-line filtering: hide unavailable types** -- Session types requiring AI (Race vs AI, Track Day) hidden when track has no AI data. Consistent with Phase 5: "invalid options hidden, not greyed out."
- **session_type goes inside launch_args JSON** -- alongside car, track, difficulty in the existing JSON payload. Both /games/launch (kiosk) and /customer/book (PWA) parse session_type from launch_args. Minimal API change.
- **Same backend validation** -- both staff and customer launches go through validate_launch_combo and the same INI builder pipeline
- **PWA and kiosk PIN must be the same to launch game** -- unified PIN validation path
- **Kiosk: optional customer PIN** -- staff can launch without PIN for walk-ins. If a customer is authenticated (QR/PIN), kiosk links the session to them. PWA always requires PIN (customer self-service).

### Claude's Discretion
- Session type card/button visual design in PWA and kiosk
- How to handle the wizard step flow when session type replaces Mode (step reindexing)
- Whether SetupWizard and GameConfigurator share a session type component or have independent implementations
- Loading/error states during the QR-to-drive flow
- How preset session_type maps into the updated wizard (presets already include session_type from Phase 7)

### Deferred Ideas (OUT OF SCOPE)
- Per-minute billing model (Rs 23.3/min first 30 min, Rs 15/min after 30 min) -- billing update, not Phase 8
- Multiplayer session type in session picker -- Phase 9 (Multiplayer Enhancement)
- Staff kiosk pin/unpin hero presets -- originally planned for Phase 8 but superseded by session type wiring priority
- AI grid size configuration UI (slider for AI count) -- could be part of Phase 8 if time permits, otherwise deferred
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| SESS-06 | Staff can configure any session type from kiosk for a pod | Kiosk GameConfigurator needs session_type step; SetupWizard needs expansion from 3->5 types; both must pass session_type in launch_args |
| CONT-03 | Staff can configure car/track/session from kiosk | GameConfigurator already has car/track steps; needs session_type step added and wired to launch_args JSON; validate_launch_combo already accepts session_type |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Next.js | 16.1.6 | Kiosk + PWA frontend framework | Already in use, both apps |
| React | 19.2.3 | UI components | Already in use |
| Rust/Axum | stable | Backend API server (rc-core) | Already in use |
| TypeScript | strict | Frontend type safety | Already in use |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| serde_json | (workspace) | JSON parsing in Rust | Backend launch_args manipulation |
| Tailwind CSS | (workspace) | Styling | All UI components |

### Alternatives Considered
None -- this phase uses only existing infrastructure. No new libraries needed.

## Architecture Patterns

### Current File Layout (relevant files)
```
racecontrol/
├── crates/rc-core/src/
│   ├── api/routes.rs            # launch_game(), customer_book_session()
│   └── catalog.rs               # validate_launch_combo(), build_custom_launch_args(), get_filtered_catalog()
├── crates/rc-agent/src/
│   └── ac_launcher.rs           # AcLaunchParams.session_type, INI builder
├── kiosk/src/
│   ├── components/
│   │   ├── GameConfigurator.tsx  # Staff quick-launch wizard (needs session_type)
│   │   └── SetupWizard.tsx      # Full booking wizard (3 types -> 5)
│   ├── hooks/
│   │   └── useSetupWizard.ts    # Wizard state + flow logic
│   └── lib/
│       └── types.ts             # SessionType, SetupStep, etc.
└── pwa/src/
    ├── app/book/page.tsx        # Customer booking wizard (Mode -> SessionType)
    └── lib/api.ts               # CustomBookingPayload type
```

### Pattern 1: Session Type Values
**What:** The canonical session type strings used across the entire codebase.
**Current state:** The backend (catalog.rs, ac_launcher.rs) uses: `"practice"`, `"hotlap"`, `"race"`, `"trackday"`, `"race_weekend"`.
**Kiosk types.ts currently:** `"practice" | "qualification" | "race"` -- needs to change to `"practice" | "hotlap" | "race" | "trackday" | "race_weekend"`.
**PWA currently:** No session_type state at all -- uses `mode` ("single" | "multi") instead.
```typescript
// Correct 5-value SessionType union (must match backend)
export type SessionType = "practice" | "hotlap" | "race" | "trackday" | "race_weekend";
```

### Pattern 2: Wizard Step Flow Replacement
**What:** PWA currently has 8 steps for single player with step 3 being "Mode" (single/multi). The decision is to replace "Mode" with "Session Type".
**Current PWA step flow:** Duration -> Game -> Mode -> Track -> Car -> Difficulty -> Transmission -> Confirm
**New PWA step flow:** Duration -> Game -> Session Type -> Track -> Car -> Difficulty -> Transmission -> Confirm
**Multiplayer:** Remains a separate entry point, not a session type. The decision says "kept as its own button/card alongside the single-player session types" -- meaning the session type picker should have the 5 types PLUS a separate multiplayer button/card that routes to the existing multi flow.

### Pattern 3: AI-Line Filtering in Session Type Picker
**What:** Session types requiring AI (Race vs AI, Track Day, Race Weekend) must be hidden when the selected track has no AI data.
**Challenge:** In the PWA, session type is selected BEFORE track. So the PWA cannot filter at selection time -- it must either (a) show all 5 and filter after track selection, or (b) pick session type after track. The kiosk SetupWizard also picks session type before track.
**Resolution:** Since the catalog already enriches each track with `available_session_types`, the filtering should happen at the TRACK step: after the user selects a session type, only show tracks that support it. Alternatively, re-validate after track selection and warn/redirect. The catalog's `available_session_types` per track is the data source.
**Recommendation:** Show all 5 session types at the session type step (user picks intent). At the track step, filter tracks to only those supporting the selected session type. This is consistent with Phase 5 pattern "invalid options hidden, not greyed out."

### Pattern 4: Launch Args Session Type Injection
**What:** `session_type` must be in the launch_args JSON for the INI builder to use it.
**Kiosk GameConfigurator:** Currently builds launch_args in `handleLaunch()` as inline JSON (line 117-128). Needs `session_type` added.
**Kiosk SetupWizard:** Uses `buildLaunchArgs()` from `useSetupWizard.ts` (line 160-192). Already includes `session_type: state.sessionType` (line 176). Just needs the type union expanded.
**PWA:** Builds `custom` payload in `handleBook()` (line 236-243). Currently sends `game_mode` but NO `session_type`. The `CustomBookingPayload` interface in `api.ts` also has no `session_type` field.
**Backend:** `CustomBookingOptions` struct (routes.rs line 4582-4591) has no `session_type` field. `build_custom_launch_args()` (catalog.rs line 715-722) has no `session_type` parameter. Both need it added.

### Anti-Patterns to Avoid
- **Renaming without checking downstream:** Changing "qualification" to "hotlap" in kiosk types must cascade to all places that reference "qualification" (SetupWizard.tsx line 215, 456).
- **Filtering session types by current track when track not yet selected:** The session type step comes before track in both wizards. Filter tracks by session type, not the other way around.
- **Forgetting GameConfigurator launch_args:** GameConfigurator builds its own JSON inline (not via `buildLaunchArgs()`). Easy to miss when only updating the hook.
- **Preset pre-fill losing session_type:** GameConfigurator's `selectPreset()` (line 103-113) doesn't set session_type because it doesn't exist yet. Must set it from `preset.session_type`.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Session type validation | Custom validation per endpoint | `catalog::validate_launch_combo()` | Already handles AI-line checks for all 5 types |
| Available session types per track | Manual per-track checks | `catalog::get_filtered_catalog()` → `available_session_types` | Already computed and served to frontends |
| INI builder session type mapping | New INI generation code | `ac_launcher.rs` existing INI builder | Already maps all 5 session types to AC server.cfg |
| Difficulty-to-aids mapping | New mapping tables | `catalog::build_custom_launch_args()` | Already handles easy/medium/hard to assist flags |

**Key insight:** The backend is already complete for all 5 session types. This phase is purely about wiring the frontends to send the correct `session_type` value through to the existing backend pipeline.

## Common Pitfalls

### Pitfall 1: PWA CustomBookingPayload Missing session_type
**What goes wrong:** PWA sends booking request without session_type, backend defaults to empty string, validate_launch_combo passes (empty session_type skips AI check), but INI builder defaults to practice.
**Why it happens:** `CustomBookingPayload` in `pwa/src/lib/api.ts` has no `session_type` field. `CustomBookingOptions` in `routes.rs` also lacks it.
**How to avoid:** Add `session_type` to both TypeScript interface AND Rust struct. Make it `Option<String>` in Rust for backward compat, default to "practice".
**Warning signs:** Customer selects "Race vs AI" in PWA but game launches in practice mode.

### Pitfall 2: GameConfigurator Step Flow After Adding Session Type
**What goes wrong:** Adding a new step changes the goBack() step array but the `handleLaunch` doesn't include the new state.
**Why it happens:** GameConfigurator uses a flat `ConfigStep` array for navigation and inline state variables, not a hook like SetupWizard.
**How to avoid:** Update the `ConfigStep` type union, the `goBack()` steps array, and `handleLaunch()` JSON builder all together.
**Warning signs:** Back button skips the session type step; review screen doesn't show session type.

### Pitfall 3: "qualification" -> "hotlap" Renaming in SetupWizard
**What goes wrong:** SetupWizard has "qualification" hardcoded in multiple places. Changing the type union without updating all references causes TypeScript errors or dead code.
**Why it happens:** `handleSelectSessionType` takes type `"practice" | "qualification" | "race"` (line 215). The session_type step renders 3 hardcoded entries (line 453-476).
**How to avoid:** Search for ALL occurrences of "qualification" in SetupWizard.tsx, useSetupWizard.ts, and types.ts. Replace with "hotlap" and add "trackday" and "race_weekend".
**Warning signs:** TypeScript compilation errors on `handleSelectSessionType` parameter type.

### Pitfall 4: Track Filtering Not Accounting for Session Type
**What goes wrong:** User selects "Race vs AI" session type, then sees tracks without AI lines in the track list. Selecting one leads to a backend rejection.
**Why it happens:** Track filtering in both wizards currently only filters by search/category, not by `available_session_types`.
**How to avoid:** After session type is selected, filter the track list to only show tracks where `available_session_types` includes the selected session type. The catalog already provides this data per track.
**Warning signs:** User gets "track has no AI lines" error after selecting track in Race mode.

### Pitfall 5: Multiplayer Entry Point Confusion
**What goes wrong:** Session type picker shows multiplayer as a session type, contradicting the locked decision that multiplayer is a separate entry point.
**Why it happens:** Easy to conflate "mode" replacement with keeping all mode functionality in session types.
**How to avoid:** Session type picker shows only the 5 single-player types. Multiplayer button/card sits alongside (not inside) the session type list. In PWA, this means the Mode step is split: the session type picker for 5 types + a separate "Race with Friends" card that enters multi flow.
**Warning signs:** Multiplayer appears as a 6th session type card.

## Code Examples

### Kiosk types.ts: Updated SessionType union
```typescript
// Replace old: "practice" | "qualification" | "race"
export type SessionType = "practice" | "hotlap" | "race" | "trackday" | "race_weekend";
```

### Session type display labels and descriptions (shared pattern)
```typescript
const SESSION_TYPES = [
  { type: "practice" as const, label: "Practice", desc: "Free driving, no AI, no timer", icon: "M13 10V3L4 14h7v7l9-11h-7z" },
  { type: "hotlap" as const, label: "Hotlap", desc: "Timed laps -- set the fastest time", icon: "M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" },
  { type: "race" as const, label: "Race vs AI", desc: "Full grid race against AI opponents", icon: "M3 21v-4m0 0V5a2 2 0 012-2h6.5l1 1H21l-3 6 3 6h-8.5l-1-1H5a2 2 0 00-2 2zm9-13.5V9" },
  { type: "trackday" as const, label: "Track Day", desc: "Open pit, mixed traffic", icon: "..." },
  { type: "race_weekend" as const, label: "Race Weekend", desc: "Practice, Qualify, then Race", icon: "..." },
] as const;
```

### GameConfigurator: Adding session_type to handleLaunch
```typescript
// In handleLaunch(), add session_type to the JSON
const launchArgs = JSON.stringify({
  car: car?.id || "",
  track: track?.id || "",
  driver: driverName,
  difficulty,
  transmission,
  ffb,
  game,
  game_mode: "single",
  session_type: sessionType, // NEW
  aids: preset?.aids || { abs: 1, tc: 1, stability: 1, autoclutch: 1, ideal_line: 1 },
  conditions: { damage: 0 },
});
```

### Backend: CustomBookingOptions with session_type
```rust
#[derive(Debug, Deserialize)]
struct CustomBookingOptions {
    game: String,
    game_mode: Option<String>,
    track: String,
    car: String,
    difficulty: String,
    transmission: String,
    #[serde(default = "default_ffb_preset")]
    ffb: String,
    #[serde(default)]
    session_type: Option<String>, // NEW -- defaults to None (backward compat)
}
```

### Track filtering by session type (useMemo pattern)
```typescript
const filteredTracks = useMemo(() => {
  if (!catalog) return [];
  let items = /* existing category/search filtering */;
  // Filter by session type availability
  if (sessionType) {
    items = items.filter((t) => {
      const available = (t as any).available_session_types as string[] | undefined;
      return !available || available.includes(sessionType);
    });
  }
  return items;
}, [catalog, trackCategory, trackSearch, sessionType]);
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| 3 session types (practice, qualification, race) in kiosk | 5 session types (practice, hotlap, race, trackday, race_weekend) | Phase 1 (backend) | Frontend must catch up |
| Mode step (single/multi) in PWA | Session type replaces Mode | Phase 8 decision | PWA step flow changes |
| No session_type in CustomBookingPayload | session_type added | Phase 8 | Backend struct update |
| GameConfigurator has mode step | GameConfigurator gets session_type step | Phase 8 decision | New step in staff quick-launch |

## Open Questions

1. **CatalogItem type lacks available_session_types**
   - What we know: Backend `get_filtered_catalog()` returns `available_session_types` per track, but the kiosk `CatalogItem` type and PWA `CatalogTrack` type don't include it.
   - What's unclear: Whether the frontends already receive this field and just don't type it, or if it's genuinely not in the response.
   - Recommendation: Check the actual API response. If the field is present, just add it to the TypeScript types. If not, verify `get_filtered_catalog` is being called (it is -- the catalog endpoint calls it).

2. **PWA multiplayer entry point after Mode removal**
   - What we know: Mode step currently has single/multi buttons. Decision says multiplayer stays as a separate entry point.
   - What's unclear: Where exactly the multiplayer button goes after Mode step is replaced.
   - Recommendation: Add a "Race with Friends" card alongside the 5 session type cards in the session type step, visually distinct (e.g., different border style). Tapping it enters the existing multi flow.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (Rust) + Next.js build (TypeScript) |
| Config file | Cargo.toml (workspace) |
| Quick run command | `cargo test -p rc-core -- catalog` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SESS-06 | Staff can configure any session type from kiosk | unit + build | `cargo test -p rc-core -- catalog` + `cd kiosk && npx tsc --noEmit` | Partial (catalog tests exist, TS build exists) |
| CONT-03 | Staff can configure car/track/session from kiosk | unit + build | `cargo test -p rc-core -- catalog` + `cd kiosk && npx tsc --noEmit` | Partial (catalog tests exist, TS build exists) |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-core -- catalog && cd kiosk && npx tsc --noEmit && cd ../pwa && npx tsc --noEmit`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core`
- **Phase gate:** Full suite green before /gsd:verify-work

### Wave 0 Gaps
- [ ] `validate_launch_combo` tests for `race_weekend` session type (currently only tests `race` and `trackday`)
- [ ] Test that `build_custom_launch_args` includes `session_type` when added as parameter
- [ ] TypeScript compilation check for both kiosk and pwa after type changes

## Sources

### Primary (HIGH confidence)
- **Codebase inspection** -- all findings verified by reading actual source files:
  - `kiosk/src/lib/types.ts` -- current SessionType = "practice" | "qualification" | "race"
  - `kiosk/src/components/GameConfigurator.tsx` -- ConfigStep type, handleLaunch JSON, selectPreset
  - `kiosk/src/components/SetupWizard.tsx` -- session_type step with 3 options, handleSelectSessionType
  - `kiosk/src/hooks/useSetupWizard.ts` -- SINGLE_FLOW, buildLaunchArgs (already has session_type)
  - `pwa/src/app/book/page.tsx` -- ModeStep, no session_type state, CustomBookingPayload usage
  - `pwa/src/lib/api.ts` -- CustomBookingPayload missing session_type
  - `crates/rc-core/src/catalog.rs` -- validate_launch_combo, build_custom_launch_args, get_filtered_catalog
  - `crates/rc-core/src/api/routes.rs` -- CustomBookingOptions struct, customer_book_session, launch_game
  - `crates/rc-agent/src/ac_launcher.rs` -- AcLaunchParams.session_type, all 5 types supported

### Secondary (MEDIUM confidence)
- None needed -- all evidence from codebase

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new libraries, all existing
- Architecture: HIGH -- all integration points verified in source code
- Pitfalls: HIGH -- identified from actual code gaps (missing fields, type mismatches)

**Research date:** 2026-03-14
**Valid until:** 2026-04-14 (stable codebase, gap-filling work)
