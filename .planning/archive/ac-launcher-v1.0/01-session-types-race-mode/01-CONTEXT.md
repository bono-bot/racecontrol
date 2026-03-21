# Phase 1: Session Types & Race Mode - Context

**Gathered:** 2026-03-13
**Status:** Ready for planning

<domain>
## Phase Boundary

Enable all single-player session types — Practice, Race vs AI, Hotlap, Track Day, and Race Weekend — by extending `write_race_ini()` to support multiple session types and AI car generation. This is single-player only; multiplayer is Phase 9. Content filtering is Phase 5. Difficulty tiers are Phase 2.

</domain>

<decisions>
## Implementation Decisions

### AI Opponent Configuration
- Customer chooses exact AI count via slider/dropdown in PWA
- Hard cap: **19 AI maximum** for single-player, **20 total slots** for multiplayer
- Sensible performance cap (not AC's ~50 limit) to protect pod frame rates
- Default behavior: AI drives the **same car** as the player
- Full custom mode available: customer can assign a **specific car per AI slot**
- "Fill remaining with [car]" shortcut button for large grids
- AI skins: Claude's discretion (random or sequential from installed skins)
- AI driver names: **real-sounding randomized names** from a pool, shuffled each race
- Customer chooses their **starting grid position** from dropdown
- No minimum AI count — customer can race alone (0 AI) if they want

### Race Weekend Flow
- **Auto-advance with 60-second timed break** between sessions (countdown screen showing next session name)
- Customer can **skip any session** (Practice, Qualify, or Race) — skipped time is saved, not lost
- **One time pool** — total session time divided by customer, not pre-split
- Qualifying grid positions **carry into the race** (best lap determines grid)
- **Time-based race** within the Weekend (runs until remaining pool time expires)
- Race vs AI (SESS-02) is a **standalone mode separate from Race Weekend** — customers don't need to do a full weekend to race

### Session Parameters
- Standalone Race vs AI: **time-based** (runs for billing duration)
- Race start type: **standing start** (lights out)
- **Optional formation lap** — customer can toggle on/off
- Practice and Hotlap: session runs for exactly the **billed duration** (30min or 60min)
- All single-player parameters; multiplayer may inherit or override in Phase 9

### Track Day Behavior
- Track Day = Practice **with AI traffic** on track (open session, no competitive racing)
- Default **10-15 AI cars** (medium traffic density)
- AI drives **mixed car classes** for realistic track day atmosphere
- Lap times **always shown** — customer can ignore them if they want casual driving
- AI count follows the same 19-max cap; customer can adjust if desired

### Claude's Discretion
- AI skin assignment strategy (random vs sequential)
- Exact AI name pool (50+ realistic driver names)
- Break screen visual design during Race Weekend transitions
- How track day AI car class distribution works (weighted random, even split, etc.)
- Formation lap implementation details

</decisions>

<specifics>
## Specific Ideas

- "Per-slot pick" for AI cars — customer can assign individual car models to each AI slot for full control
- "Fill remaining" shortcut to avoid tedium on large grids
- 60-second countdown between Race Weekend sessions for a breather
- Standalone Race vs AI is the primary "quick race" mode — Race Weekend is the premium immersive experience
- Track Day is the casual, low-pressure mode with mixed traffic — a "drive around and enjoy" option

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `SessionType` enum (rc-common/types.rs:85-92): Practice, Qualifying, Race, Hotlap — already defined, no changes needed
- `AcLaunchParams` struct (ac_launcher.rs:24-55): Base launch params with car, track, aids, conditions — extend with session_type and AI config
- `write_race_ini()` (ac_launcher.rs:271-428): Currently writes TYPE=1 Practice only — needs extension for TYPE=2/3 and multi-session blocks
- `generate_server_cfg_ini()` (ac_server.rs:190-328): Server-side already handles all session types — pattern to follow for race.ini
- `AcSessionBlock` struct: Demonstrates proper session type → INI block mapping
- Catalog (catalog.rs): 325 drivable cars, 50+ tracks — available for AI car selection

### Established Patterns
- INI file writing via string formatting in ac_launcher.rs
- `[CAR_0]` section with MODEL, SKIN, DRIVER_NAME fields — extend to [CAR_1] through [CAR_N] with AI=N field
- Session blocks: `[SESSION_0]` with NAME, DURATION_MINUTES, TYPE, LAPS — add [SESSION_1], [SESSION_2] for Race Weekend

### Integration Points
- `AcLaunchParams` is the entry point — all session config flows through this struct
- `GameLauncher` (game_launcher.rs) manages game lifecycle — calls write_race_ini() then launches acs.exe
- Booking flow: Kiosk/PWA → rc-core (auth/booking) → rc-agent (launch) — session type needs to flow through this chain

</code_context>

<deferred>
## Deferred Ideas

- Multiplayer session parameters (Phase 9) — may inherit single-player decisions or override
- AI difficulty tiers mapping to AI_LEVEL (Phase 2) — affects AI car blocks but not session structure
- Content filtering to hide tracks without AI lines (Phase 5) — affects what's selectable but not launch logic

</deferred>

---

*Phase: 01-session-types-race-mode*
*Context gathered: 2026-03-13*
