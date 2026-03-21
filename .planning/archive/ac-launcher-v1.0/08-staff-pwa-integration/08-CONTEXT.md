# Phase 8: Staff & PWA Integration - Context

**Gathered:** 2026-03-14
**Status:** Ready for planning

<domain>
## Phase Boundary

Both customer (PWA/QR) and staff (kiosk) launch paths work end-to-end with the new session system. All 5 session types from Phase 1 (Practice, Hotlap, Race vs AI, Track Day, Race Weekend) are wired to both frontends. Session type selection uses Phase 5's AI-line filtering. Both launch paths converge on the same backend validation. Multiplayer orchestration is Phase 9.

</domain>

<decisions>
## Implementation Decisions

### Session Type Wiring
- **All 5 session types wired to both PWA and kiosk** — Practice, Hotlap, Race vs AI, Track Day, Race Weekend
- **PWA: session type replaces the Mode step** — "Mode (single/multi)" becomes a session type picker with the 5 types
- **Multiplayer stays as a separate entry point** — not mixed into the session type list. Kept as its own button/card alongside the single-player session types.
- **Kiosk: both wizards updated** — GameConfigurator (staff quick-launch from pod card) AND SetupWizard (full booking flow) both get all 5 session types
- **SetupWizard currently has 3 types** (practice, qualification, race) — expand to all 5, replace "qualification" with "hotlap"
- **GameConfigurator currently has no session type** — add session type step with all 5 types
- **AI-line filtering: hide unavailable types** — Session types requiring AI (Race vs AI, Track Day) hidden when track has no AI data. Consistent with Phase 5: "invalid options hidden, not greyed out."

### Launch Path Convergence
- **session_type goes inside launch_args JSON** — alongside car, track, difficulty in the existing JSON payload. Both /games/launch (kiosk) and /customer/book (PWA) parse session_type from launch_args. Minimal API change.
- **Same backend validation** — both staff and customer launches go through validate_launch_combo and the same INI builder pipeline
- **PWA and kiosk PIN must be the same to launch game** — unified PIN validation path

### PIN Authentication
- **Kiosk: optional customer PIN** — staff can launch without PIN for walk-ins. If a customer is authenticated (QR/PIN), kiosk links the session to them. PWA always requires PIN (customer self-service).

### Claude's Discretion
- Session type card/button visual design in PWA and kiosk
- How to handle the wizard step flow when session type replaces Mode (step reindexing)
- Whether SetupWizard and GameConfigurator share a session type component or have independent implementations
- Loading/error states during the QR-to-drive flow
- How preset session_type maps into the updated wizard (presets already include session_type from Phase 7)

</decisions>

<specifics>
## Specific Ideas

- The PWA wizard currently has 8 steps for single player and 9 for multi. Replacing Mode with session type keeps the step count the same.
- Kiosk GameConfigurator's "mode" step (single/multi) should be replaced with session type to match the PWA pattern.
- Phase 7 presets already include session_type — the preset pre-fill flow should work unchanged once session type is wired.
- The per-minute pricing model (Rs 23.3/min first 30 min, Rs 15/min after) was discussed but deferred — belongs in a billing update, not Phase 8.

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `GameConfigurator.tsx` (kiosk): Multi-step wizard with presets/game/mode/track/car/settings/review — add session type step, update mode step
- `SetupWizard.tsx` (kiosk): Full booking wizard with session_type step — expand from 3 to 5 types
- `useSetupWizard.ts` (kiosk): Hook managing wizard state including sessionType — update type union
- `book/page.tsx` (PWA): 8-step wizard with Mode step — replace Mode with session type
- `catalog.rs` (rc-core): validate_launch_combo + get_filtered_catalog — already supports session_type validation
- `build_custom_launch_args()` (catalog.rs): Builds launch JSON — already accepts session_type
- `customer_book_session()` (routes.rs): Customer booking endpoint — already validates against pod manifest
- `launch_game()` (routes.rs): Staff launch endpoint — needs session_type parsing from launch_args

### Established Patterns
- Wizard step navigation in both PWA and kiosk (step state + goBack/goNext)
- Phase 5 content filtering: tracks without AI hide Race/Track Day session types
- Phase 7 presets: PresetEntry.session_type pre-fills wizard
- PIN auth: /customer/book requires PIN header, /games/launch requires staff auth

### Integration Points
- PWA book/page.tsx: Replace ModeStep component with SessionTypeStep
- Kiosk GameConfigurator.tsx: Replace "mode" ConfigStep with "session_type"
- Kiosk SetupWizard.tsx: Expand session_type options from 3 to 5
- Kiosk types.ts: Update SessionType union to include all 5 types
- rc-core routes.rs launch_game(): Parse session_type from launch_args JSON
- Both frontends: Filter session types by track AI capability (catalog data)

</code_context>

<deferred>
## Deferred Ideas

- Per-minute billing model (Rs 23.3/min first 30 min, Rs 15/min after 30 min) — billing update, not Phase 8
- Multiplayer session type in session picker — Phase 9 (Multiplayer Enhancement)
- Staff kiosk pin/unpin hero presets — originally planned for Phase 8 but superseded by session type wiring priority
- AI grid size configuration UI (slider for AI count) — could be part of Phase 8 if time permits, otherwise deferred

</deferred>

---

*Phase: 08-staff-pwa-integration*
*Context gathered: 2026-03-14*
