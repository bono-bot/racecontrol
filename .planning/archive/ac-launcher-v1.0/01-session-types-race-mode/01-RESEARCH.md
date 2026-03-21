# Phase 1: Session Types & Race Mode - Research

**Researched:** 2026-03-13
**Domain:** Assetto Corsa race.ini INI generation, session type configuration, AI opponent grid management
**Confidence:** HIGH

## Summary

Phase 1 extends the existing `write_race_ini()` function in `rc-agent/src/ac_launcher.rs` to support all five single-player session types: Practice (already working), Race vs AI, Hotlap, Track Day, and Race Weekend. The current implementation hardcodes TYPE=1 (Practice) with CARS=1 (solo) and a single `[SESSION_0]` block. The work involves: (1) adding new fields to `AcLaunchParams` for session type and AI configuration, (2) generating `[CAR_1]` through `[CAR_N]` sections for AI opponents, (3) generating multiple `[SESSION_N]` blocks for Race Weekend mode, and (4) propagating session type through the core-to-agent protocol.

The codebase is approximately 70% ready. The `SessionType` enum already exists in `rc-common/types.rs`, the server-side `generate_server_cfg_ini()` in `ac_server.rs` already demonstrates session block generation for Practice/Qualifying/Race, and the catalog provides 325 cars for AI selection. The primary gap is the client-side `write_race_ini()` which only generates Practice sessions with no AI opponents.

**Primary recommendation:** Extend `AcLaunchParams` with session_type, ai_cars (Vec of AI slot configs), and race_weekend fields. Refactor `write_race_ini()` to be a composable INI builder that generates the correct TYPE values, CAR sections, and SESSION blocks based on the session mode.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Customer chooses exact AI count via slider/dropdown in PWA
- Hard cap: **19 AI maximum** for single-player, **20 total slots** for multiplayer
- Default behavior: AI drives the **same car** as the player
- Full custom mode available: customer can assign a **specific car per AI slot**
- "Fill remaining with [car]" shortcut button for large grids
- AI driver names: **real-sounding randomized names** from a pool, shuffled each race
- Customer chooses their **starting grid position** from dropdown
- No minimum AI count -- customer can race alone (0 AI) if they want
- Race Weekend: **Auto-advance with 60-second timed break** between sessions (countdown screen showing next session name)
- Customer can **skip any session** (Practice, Qualify, or Race) -- skipped time is saved, not lost
- **One time pool** -- total session time divided by customer, not pre-split
- Qualifying grid positions **carry into the race** (best lap determines grid)
- **Time-based race** within the Weekend (runs until remaining pool time expires)
- Race vs AI (SESS-02) is a **standalone mode separate from Race Weekend**
- Standalone Race vs AI: **time-based** (runs for billing duration)
- Race start type: **standing start** (lights out)
- **Optional formation lap** -- customer can toggle on/off
- Practice and Hotlap: session runs for exactly the **billed duration** (30min or 60min)
- Track Day = Practice **with AI traffic** on track (open session, no competitive racing)
- Default **10-15 AI cars** for Track Day (medium traffic density)
- Track Day AI drives **mixed car classes** for realistic track day atmosphere
- Lap times **always shown** -- customer can ignore them if they want casual driving

### Claude's Discretion
- AI skin assignment strategy (random vs sequential)
- Exact AI name pool (50+ realistic driver names)
- Break screen visual design during Race Weekend transitions
- How track day AI car class distribution works (weighted random, even split, etc.)
- Formation lap implementation details

### Deferred Ideas (OUT OF SCOPE)
- Multiplayer session parameters (Phase 9)
- AI difficulty tiers mapping to AI_LEVEL (Phase 2)
- Content filtering to hide tracks without AI lines (Phase 5)
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| SESS-01 | Customer can select Practice mode (solo hot-lapping, no AI) from PWA | Already working -- TYPE=1, CARS=1 in current race.ini. No code changes needed for the INI itself; only needs session_type field in AcLaunchParams for protocol clarity. |
| SESS-02 | Customer can select Race vs AI mode with configurable grid size from PWA | Requires TYPE=3 session block, CARS=N in [RACE], [CAR_1]..[CAR_N] with AI=1 field, AI_LEVEL, STARTING_POSITION, and time-based race config (DURATION_MINUTES > 0, LAPS=0). |
| SESS-03 | Customer can select Hotlap mode (timed laps) from PWA | TYPE=4 (Hotlap) session block with CARS=1, single [SESSION_0]. Spawns near start line instead of pit. No AI. |
| SESS-04 | Customer can select Track Day mode (open pit, mixed traffic) from PWA | TYPE=1 (Practice) session block but with CARS=N, [CAR_1]..[CAR_N] for AI traffic. Mixed car classes from catalog. |
| SESS-05 | Customer can select Race Weekend mode (Practice -> Qualify -> Race sequence) from PWA | Multiple [SESSION_0]/[SESSION_1]/[SESSION_2] blocks with TYPE=1, TYPE=2, TYPE=3 respectively. Race Weekend is a multi-session sequence managed by AC's built-in session progression. |
| SESS-08 | Game launches with the exact preset/config selected -- no silent fallbacks | Validated by generating race.ini deterministically from AcLaunchParams. Test by parsing generated INI and verifying all fields match input. |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| rc-common | workspace | SessionType enum, AcLaunchParams types, protocol messages | Already exists, shared between rc-core and rc-agent |
| rc-agent | workspace | ac_launcher.rs -- race.ini generation and game launch | Where all write_race_ini() changes happen |
| serde/serde_json | workspace | JSON serialization of launch_args between core and agent | Already used for AcLaunchParams deserialization |
| rand | 0.8 | AI name shuffling, skin random selection, Track Day car class distribution | Needed for randomized AI name pool and skin assignment |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| rc-core catalog.rs | workspace | Car catalog (325 cars with categories) for Track Day mixed classes | When generating Track Day AI with mixed car classes |
| dirs-next | 2 | Cross-platform Documents path for race.ini | Already used in write_race_ini() |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| String formatting for INI | ini crate | String formatting is already the pattern in ac_launcher.rs; adding a crate adds dependency for minimal gain since INI writing is straightforward |
| rand for shuffling | deterministic seed | rand is simpler; deterministic seeds only needed if reproducibility is required (not a requirement) |

**Installation:**
```bash
# Only new dependency needed:
cargo add rand@0.8 -p rc-agent
```

## Architecture Patterns

### Recommended Project Structure
```
crates/rc-agent/src/
    ac_launcher.rs          # Extended: write_race_ini() with session types + AI cars
    mod.rs / main.rs        # LaunchGame handler already parses AcLaunchParams

crates/rc-common/src/
    types.rs                # Extended: no changes needed to SessionType enum
    protocol.rs             # No changes needed -- launch_args already passes JSON

crates/rc-core/src/
    catalog.rs              # Reference: car categories for Track Day mixed classes
    game_launcher.rs        # No changes needed -- forwards launch_args to agent
```

### Pattern 1: Composable INI Builder
**What:** Refactor `write_race_ini()` from a monolithic format! string into a composable builder that assembles sections based on session type.
**When to use:** Always -- the current single format! string cannot handle variable numbers of CAR sections and SESSION blocks.
**Example:**
```rust
// Pseudocode for the refactored approach
fn write_race_ini(params: &AcLaunchParams) -> Result<()> {
    let mut ini = String::new();

    // Fixed sections (always present)
    write_assists_section(&mut ini, params);
    write_autospawn_section(&mut ini);
    write_race_section(&mut ini, params);  // CARS=N, AI_LEVEL, etc.

    // Player car (always CAR_0)
    write_car_section(&mut ini, 0, &params.car, &params.skin, &params.driver, false);

    // AI cars (CAR_1 through CAR_N)
    for (i, ai_car) in params.ai_cars.iter().enumerate() {
        write_car_section(&mut ini, i + 1, &ai_car.model, &ai_car.skin, &ai_car.driver_name, true);
    }

    // Session blocks (single for most modes, multiple for Race Weekend)
    write_session_blocks(&mut ini, params);

    // Fixed trailing sections
    write_weather_section(&mut ini, params);
    write_dynamic_track_section(&mut ini);

    // Write to disk
    std::fs::write(&race_ini_path, ini)?;
    Ok(())
}
```

### Pattern 2: AcLaunchParams Extension
**What:** Add session configuration fields to the existing `AcLaunchParams` struct without breaking backward compatibility.
**When to use:** For all new session type data flowing from PWA/core to agent.
**Example:**
```rust
#[derive(Debug, Clone, Deserialize)]
pub struct AcLaunchParams {
    // Existing fields (unchanged)
    pub car: String,
    pub track: String,
    pub driver: String,
    // ... all existing fields ...

    // NEW: Session type configuration
    #[serde(default = "default_session_type")]
    pub session_type: String,  // "practice", "race", "hotlap", "trackday", "weekend"

    // NEW: AI opponent configuration
    #[serde(default)]
    pub ai_cars: Vec<AiCarSlot>,

    // NEW: Race-specific settings
    #[serde(default)]
    pub starting_position: u32,  // 1-indexed grid position (1 = pole)
    #[serde(default)]
    pub formation_lap: bool,

    // NEW: Race Weekend sub-session time allocation
    #[serde(default)]
    pub weekend_practice_minutes: u32,
    #[serde(default)]
    pub weekend_qualify_minutes: u32,
    // Race gets remaining time from the billing pool
}

#[derive(Debug, Clone, Deserialize)]
pub struct AiCarSlot {
    pub model: String,           // Car ID from catalog
    pub skin: String,            // Skin folder name
    pub driver_name: String,     // AI driver display name
    #[serde(default = "default_ai_level")]
    pub ai_level: u32,           // 0-100 (Phase 2 maps tiers to this)
}
```

### Pattern 3: AI Name Pool
**What:** A static pool of 60+ realistic driver names, shuffled per session using rand.
**When to use:** When generating AI car slots and the caller does not specify custom names.
**Example:**
```rust
const AI_DRIVER_NAMES: &[&str] = &[
    "Marco Rossi", "James Mitchell", "Carlos Mendes", "Yuki Tanaka",
    "Liam O'Brien", "Alessandro Bianchi", "Felix Weber", "Raj Patel",
    "Pierre Dubois", "Hans Mueller", "Takeshi Kimura", "David Chen",
    // ... 50+ more realistic international names ...
];

fn pick_ai_names(count: usize) -> Vec<String> {
    use rand::seq::SliceRandom;
    let mut rng = rand::thread_rng();
    let mut names: Vec<&str> = AI_DRIVER_NAMES.to_vec();
    names.shuffle(&mut rng);
    names.into_iter().take(count).map(|s| s.to_string()).collect()
}
```

### Anti-Patterns to Avoid
- **Separate race.ini per session type:** Do NOT create write_race_ini_race(), write_race_ini_hotlap(), etc. Use a single composable function with branching on session_type.
- **Hardcoding AI count:** Always respect the `ai_cars` Vec length. Never assume a fixed count.
- **Modifying SessionType enum:** The existing enum (Practice, Qualifying, Race, Hotlap) is for server-side sessions. Client-side "Track Day" and "Weekend" are composite modes that MAP to these types. Do not add TrackDay/Weekend to the enum.
- **Breaking AcLaunchParams backward compatibility:** All new fields must have `#[serde(default)]` so existing JSON payloads (from current PWA/kiosk) still deserialize correctly.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| AI name generation | Custom name generator | Static curated pool + rand shuffle | Names need to sound real and diverse; a generator produces nonsense |
| Skin discovery per car | Runtime filesystem scan | Fallback to sequential skin numbering (skin_00, skin_01) or "00_default" | Runtime scan would require pod filesystem access from agent; skins are pre-installed |
| Race Weekend session progression | Custom session state machine in rc-agent | AC's built-in multi-session progression via multiple [SESSION_N] blocks | AC natively transitions between sessions when you define SESSION_0, SESSION_1, SESSION_2 in race.ini |
| Formation lap | Custom implementation | CSP's built-in FORMATION_LAP=1 in server_cfg.ini or extra options | CSP handles formation lap rendering, pacing, and penalty logic |

**Key insight:** AC's single-player mode with `acs.exe` + `race.ini` natively supports multi-session weekends and AI grids. The work is INI generation, not game engine extension.

## Common Pitfalls

### Pitfall 1: CARS Count Mismatch
**What goes wrong:** `[RACE] CARS=N` doesn't match the number of `[CAR_X]` sections, causing "CANNOT HAVE 0 CARS IN RACE.INI" or silent failures.
**Why it happens:** CARS= must equal the TOTAL number of cars including the player. If you have 5 AI opponents, CARS=6 (player + 5 AI).
**How to avoid:** Calculate `CARS = 1 + ai_cars.len()` and assert this matches the generated CAR sections.
**Warning signs:** AC crashes on launch with no error dialog, or logs "cannot have 0 cars."

### Pitfall 2: TYPE Values for Session Blocks
**What goes wrong:** Using wrong TYPE integer values causes AC to load the wrong session mode.
**Why it happens:** AC's session TYPE values are not well-documented publicly. Based on codebase analysis and community patterns:
- TYPE=1 = Practice (confirmed in existing code)
- TYPE=2 = Qualifying
- TYPE=3 = Race
- TYPE=4 = Hotlap (spawn near start line)
- TYPE=5 = Drift (not needed)
**How to avoid:** Use constants, not magic numbers. Test each TYPE on a real pod.
**Warning signs:** Session loads but behavior doesn't match expectations (e.g., no qualifying timer, no race grid).

### Pitfall 3: AI Cars Without ai/ Folder on Track
**What goes wrong:** AI opponents don't move or crash into walls because the track has no AI racing line data.
**Why it happens:** AC AI requires pre-computed racing line data in the track's `ai/` subfolder. Some mod tracks don't include this.
**How to avoid:** Phase 5 (CONT-05) addresses this with content filtering. For Phase 1, document that AI-related session types may not work on all tracks and log warnings when AI is requested.
**Warning signs:** AI cars sit in pits or drive erratically.

### Pitfall 4: Skin Name Resolution
**What goes wrong:** AI cars appear with default livery or as invisible because skin name doesn't exist for that car.
**Why it happens:** Each car has its own set of installed skins. Specifying `skin: "skin_03"` on a car that only has `00_default` and `skin_01` will fail silently.
**How to avoid:** Use empty string for skin (AC picks a random installed skin) or always default to the empty string for AI cars. AC handles this gracefully.
**Warning signs:** All AI cars have identical appearance or are invisible.

### Pitfall 5: Race Weekend Time Pool Management
**What goes wrong:** Customer runs out of time in Practice, has no time left for Qualifying or Race.
**Why it happens:** AC runs each session for its DURATION_MINUTES; if Practice is set to the full billing duration, there's nothing left.
**How to avoid:** The Race Weekend time allocation must be managed by rc-agent BEFORE writing race.ini. Either: (a) pre-split time (e.g., 10 min practice, 10 min qualifying, 10 min race), or (b) let the customer control allocation. The CONTEXT.md says "one time pool" -- customer divides their own time.
**Warning signs:** Customer stuck in an earlier session with no race.

### Pitfall 6: STARTING_POSITION Indexing
**What goes wrong:** Customer always starts from pole or always last, regardless of their choice.
**Why it happens:** `STARTING_POSITION` in `[SESSION_0]` is 1-indexed. Setting it to 0 may cause undefined behavior.
**How to avoid:** Validate that `starting_position` is between 1 and `CARS` inclusive. Default to 1.
**Warning signs:** Grid position doesn't match what customer selected.

### Pitfall 7: Formation Lap in Single-Player
**What goes wrong:** Formation lap toggle does nothing or crashes the game.
**Why it happens:** Formation lap in vanilla AC single-player is not natively supported. It requires CSP (Custom Shaders Patch) server-side configuration (`FORMATION_LAP=1`, `FORMATION_TYPE=1`). For direct `acs.exe` single-player, this may need CSP's `extra_cfg.ini` or may not be available at all.
**How to avoid:** Research whether CSP's formation lap works in offline/single-player mode. If not, mark formation lap as server/multiplayer only and document for Phase 9. The CONTEXT.md says "optional formation lap" -- it's acceptable to defer if technically infeasible in single-player.
**Warning signs:** Game launches without formation lap despite being enabled, or crashes at race start.

## Code Examples

Verified patterns from the existing codebase:

### Current write_race_ini() Structure (Source: ac_launcher.rs:271-428)
```rust
// Current: Single format! string with hardcoded TYPE=1, CARS=1, no AI
// Key sections:
// [RACE] -- CARS=1, AI_LEVEL=100, MODEL={car}, TRACK={track}
// [CAR_0] -- Player car with SKIN, MODEL, DRIVER_NAME
// [SESSION_0] -- NAME=Practice, TYPE=1, DURATION_MINUTES={billing_duration}
```

### Server-Side Session Block Pattern (Source: ac_server.rs:258-293)
```rust
// Reference: How the server generates session blocks
// This is the pattern to follow for client-side multi-session
match session.session_type {
    SessionType::Practice => {
        // [PRACTICE] NAME=Practice TIME={duration} IS_OPEN=1
    }
    SessionType::Qualifying => {
        // [QUALIFY] NAME=Qualifying TIME={duration} IS_OPEN=1
    }
    SessionType::Race => {
        // [RACE] NAME=Race LAPS={laps} TIME={duration} IS_OPEN=1
    }
}
```

**Note:** Server-side uses named sections ([PRACTICE], [QUALIFY], [RACE]) while client-side race.ini uses numbered sections ([SESSION_0], [SESSION_1], [SESSION_2]) with a TYPE field. Both patterns coexist in AC.

### Entry List CAR Section Pattern (Source: ac_server.rs:330-367)
```rust
// Reference: How the server generates entry list [CAR_N] sections
// Client-side AI car sections follow the SAME format
// [CAR_0] MODEL={car} SKIN={skin} DRIVERNAME={name} GUID= BALLAST=0 RESTRICTOR=0
```

### Expected AI Car Section in race.ini
```ini
[CAR_1]
SETUP=
SKIN=              ; Empty = AC picks random installed skin
MODEL=ks_ferrari_488_gt3
MODEL_CONFIG=
BALLAST=0
RESTRICTOR=0
DRIVER_NAME=Marco Rossi
NATIONALITY=
NATION_CODE=
AI=1               ; CRITICAL: AI=1 marks this as an AI-controlled car
```

### Expected Multi-Session Race Weekend in race.ini
```ini
[SESSION_0]
NAME=Practice
DURATION_MINUTES=10
SPAWN_SET=PIT
TYPE=1
LAPS=0
STARTING_POSITION=1

[SESSION_1]
NAME=Qualifying
DURATION_MINUTES=10
SPAWN_SET=PIT
TYPE=2
LAPS=0
STARTING_POSITION=1

[SESSION_2]
NAME=Race
DURATION_MINUTES=10
SPAWN_SET=PIT
TYPE=3
LAPS=0
STARTING_POSITION=1
```

### Expected [RACE] Section for AI Grid
```ini
[RACE]
AI_LEVEL=100
CARS=6              ; Player + 5 AI
CONFIG_TRACK={track_config}
DRIFT_MODE=0
FIXED_SETUP=0
JUMP_START_PENALTY=0
MODEL={player_car}
MODEL_CONFIG=
PENALTIES=1
RACE_LAPS=0         ; 0 = time-based race
SKIN={player_skin}
TRACK={track}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Hardcoded Practice only | Configurable session types | This phase | All five session modes become available |
| CARS=1 (solo) | CARS=1..20 with AI grid | This phase | AI opponents supported |
| Single SESSION_0 | Multiple SESSION_N blocks | This phase | Race Weekend multi-session supported |
| No session_type in AcLaunchParams | session_type field added | This phase | Protocol carries session choice from PWA to agent |

**Not deprecated:**
- All existing AcLaunchParams fields remain valid
- Single-player acs.exe direct launch remains the method (no change)
- write_assists_ini() and set_ffb() are unchanged

## Open Questions

1. **Formation Lap in Single-Player AC**
   - What we know: CSP supports FORMATION_LAP=1 in server configuration. Single-player AC (acs.exe with race.ini) may or may not support this.
   - What's unclear: Whether CSP's formation lap feature works in offline mode or only on dedicated servers.
   - Recommendation: Implement the toggle in AcLaunchParams and race.ini. Test on Pod 8. If it doesn't work in single-player, mark it as multiplayer-only and update CONTEXT.md. LOW confidence this works in single-player.

2. **Hotlap TYPE=4 vs TYPE=1 Behavior**
   - What we know: Practice (TYPE=1) spawns in pit lane. Hotlap mode in AC's UI spawns near the start/finish line and shows ghost car.
   - What's unclear: Whether TYPE=4 is the correct value for hotlap in race.ini, or whether hotlap is a game mode flag elsewhere.
   - Recommendation: Test TYPE=4 on Pod 8. If it doesn't produce hotlap behavior, fall back to TYPE=1 with SPAWN_SET=START and note the difference. MEDIUM confidence on TYPE=4.

3. **Race Weekend Auto-Advance Timing**
   - What we know: The user wants 60-second breaks between sessions. AC's built-in multi-session naturally transitions between sessions.
   - What's unclear: Whether AC provides a configurable inter-session timer, or whether the transition is instant.
   - Recommendation: Test multi-session race.ini on Pod 8 to observe transition behavior. The break screen may need to be implemented as an overlay by rc-agent rather than relying on AC's native behavior. MEDIUM confidence on AC providing the 60s break natively.

4. **Track Day Mixed Car Classes**
   - What we know: Track Day should have mixed car classes. The catalog has categories (GT3, Supercars, JDM, F1 2025, etc.).
   - What's unclear: Whether AI can handle mixed car classes well on all tracks (performance differences may cause issues).
   - Recommendation: Use weighted random selection from 2-3 compatible categories. For v1, pick from the same broad performance class (e.g., GT3 + Supercars). Test on Pod 8.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (Rust built-in) |
| Config file | Cargo.toml workspace |
| Quick run command | `cargo test -p rc-agent -- --test-threads=1` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SESS-01 | Practice mode generates TYPE=1, CARS=1, no AI sections | unit | `cargo test -p rc-agent write_race_ini_practice -x` | Wave 0 |
| SESS-02 | Race vs AI generates TYPE=3, CARS=N, CAR_1..N with AI=1 | unit | `cargo test -p rc-agent write_race_ini_race_ai -x` | Wave 0 |
| SESS-03 | Hotlap mode generates TYPE=4, CARS=1 | unit | `cargo test -p rc-agent write_race_ini_hotlap -x` | Wave 0 |
| SESS-04 | Track Day generates TYPE=1, CARS=N, mixed AI car models | unit | `cargo test -p rc-agent write_race_ini_trackday -x` | Wave 0 |
| SESS-05 | Race Weekend generates SESSION_0/1/2 with TYPE=1/2/3 | unit | `cargo test -p rc-agent write_race_ini_weekend -x` | Wave 0 |
| SESS-08 | Generated INI matches input params exactly | unit | `cargo test -p rc-agent write_race_ini_no_fallback -x` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-agent -- --test-threads=1`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/rc-agent/src/ac_launcher.rs` -- add #[cfg(test)] mod with tests for each session type INI generation
- [ ] Test helper: function that parses generated INI string and returns a HashMap of sections for assertion
- [ ] `rand` dependency in rc-agent/Cargo.toml for AI name shuffling

## Sources

### Primary (HIGH confidence)
- **ac_launcher.rs** (rc-agent/src/ac_launcher.rs) -- Existing write_race_ini() implementation, lines 271-428. Verified current TYPE=1/CARS=1 hardcoding.
- **ac_server.rs** (rc-core/src/ac_server.rs) -- Server-side session block generation, lines 190-328. Verified SessionType mapping to INI blocks.
- **types.rs** (rc-common/src/types.rs) -- SessionType enum (Practice, Qualifying, Race, Hotlap), lines 85-92. Verified existing enum covers all needed types.
- **catalog.rs** (rc-core/src/catalog.rs) -- 325 cars with categories (GT3, Supercars, JDM, F1 2025, etc.). Verified for Track Day mixed class selection.
- **protocol.rs** (rc-common/src/protocol.rs) -- CoreToAgentMessage::LaunchGame with launch_args: Option<String>. Verified JSON pass-through works.
- **game_launcher.rs** (rc-core/src/game_launcher.rs) -- GameManager forwards LaunchGame to agent. No changes needed.

### Secondary (MEDIUM confidence)
- [OverTake.gg forum: Understanding session types](https://www.overtake.gg/threads/understanding-what-practice-hotlap-race-track-day-weekend-drift-and-drag-race-mean.269480/) -- Confirmed Practice=solo, Hotlap=near start line+ghost, Race=AI competition, Track Day=Practice+AI, Weekend=multi-session sequence.
- [Steam DIY Custom AI Races guide](https://steamcommunity.com/app/244210/discussions/0/648817377739220303/) -- Confirmed [CAR_N] sections with MODEL= for AI, CARS= count in [RACE].
- [Voltic Host formation lap guide](https://help.voltichost.com/hc/help/articles/1739485472-setting-up-rolling-starts-and-formation-laps-on-an-assetto-corsa-server) -- FORMATION_LAP=1, FORMATION_TYPE=1, server-side config.
- [AssettoCorsaMods.net race.ini errors](https://assettocorsamods.net/threads/tried-everything-but-keep-getting-cannot-have-0-cars-in-race-ini.2878/) -- Confirmed CARS count must match CAR_N sections.

### Tertiary (LOW confidence)
- Session TYPE integer values (TYPE=2 for Qualifying, TYPE=3 for Race, TYPE=4 for Hotlap) -- Inferred from community patterns and server-side code but not officially documented. **Needs Pod 8 validation.**
- Formation lap in single-player -- No confirmed source that CSP formation lap works with direct acs.exe offline. **Needs Pod 8 validation.**
- AI=1 field in CAR_N sections -- Community consensus but not found in official docs. **Needs Pod 8 validation.**

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- All libraries already in the project. Only rand is new.
- Architecture: HIGH -- Composable INI builder is a natural extension of existing string formatting pattern.
- INI format (Practice, Track Day): HIGH -- TYPE=1 is confirmed working in production.
- INI format (Race, Qualifying, Hotlap): MEDIUM -- TYPE=2/3 inferred from server code and community; TYPE=4 needs validation.
- AI car sections: MEDIUM -- [CAR_N] with AI=1 is consistent across community sources but needs Pod 8 testing.
- Formation lap: LOW -- May not work in single-player offline mode.
- Pitfalls: HIGH -- CARS count mismatch and skin resolution are well-documented community issues.

**Research date:** 2026-03-13
**Valid until:** 2026-04-13 (stable -- AC's race.ini format has not changed since 2019)
