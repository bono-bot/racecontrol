# Feature Landscape

**Domain:** Commercial sim racing venue AC session management
**Researched:** 2026-03-13
**Overall Confidence:** HIGH (codebase-verified) / MEDIUM (AC internals from community sources)

## AC Technical Reference

Before categorizing features, here is the verified reference for all AC configuration parameters relevant to session management. This is the foundation that every feature maps onto.

### AC Session Types (Single-Player race.ini)

AC supports 7 single-player game modes, but only a subset are relevant for a commercial venue.

| Mode | Description | Has AI? | Venue-Relevant? |
|------|-------------|---------|-----------------|
| **Practice** | Solo on track, unlimited time | No | YES - warm-up, learning |
| **Hotlap** | Start near finish line, ghost car replay | No | YES - time attack competitions |
| **Race** | Grid start vs AI, no quali/practice | Yes | YES - core offering |
| **Track Day** | Open lapping with AI traffic | Yes | YES - casual customers |
| **Weekend** | Practice + Qualifying + Race sequence | Yes | YES - premium experience |
| **Drift** | Score-based drift points accumulation | No | MAYBE - niche appeal |
| **Drag Race** | Multiple drag strip runs vs AI | Yes | NO - requires drag strip tracks |

**Source:** [OverTake.gg session types guide](https://www.overtake.gg/threads/understanding-what-practice-hotlap-race-track-day-weekend-drift-and-drag-race-mean.269480/) - MEDIUM confidence

**race.ini SESSION_0 TYPE values** (from codebase + community knowledge):

| Value | Session Type |
|-------|-------------|
| 1 | Practice |
| 2 | Qualifying |
| 3 | Race |

**Note:** Hotlap, Drift, Track Day, Weekend, and Drag are game mode selections, not SESSION_0 TYPE values. They are selected via Content Manager or game UI, not via raw race.ini TYPE field. The race.ini [RACE] section has `DRIFT_MODE=0/1` to toggle drift scoring.

**Confidence:** MEDIUM - TYPE values confirmed in codebase (`TYPE=1` in all race.ini templates), TYPE 2 and 3 from community sources. No official Kunos documentation found for the complete enum.

### AC race.ini [RACE] Section Parameters

| Parameter | Range/Values | Current Codebase Default | Purpose |
|-----------|-------------|-------------------------|---------|
| `AI_LEVEL` | 0-100 (%) | 100 | AI speed/competence. 80% = already fast. 90-100% = competitive. |
| `CARS` | 1-N (limited by track pit count) | 1 | Total cars including player. CARS=1 means solo. |
| `RACE_LAPS` | 0 = unlimited, 1+ = lap count | 0 | Number of race laps. 0 defers to session duration. |
| `DRIFT_MODE` | 0 or 1 | 0 | Enable drift scoring mode |
| `FIXED_SETUP` | 0 or 1 | 0 | Lock car setup (prevents tuning) |
| `JUMP_START_PENALTY` | 0 or 1 | 0 | Penalize early start |
| `PENALTIES` | 0 or 1 | 1 | Enable racing penalties |
| `STARTING_POSITION` | 1-N | 1 | Player grid position |

**Confidence:** HIGH - verified in codebase (ac_launcher.rs lines 344-379, pod_ac_launch.py lines 78-113)

### AC Assist Parameters (race.ini [ASSISTS] + assists.ini)

| Parameter | Values | Default (Easy) | Purpose |
|-----------|--------|----------------|---------|
| `ABS` | 0 = off, 1 = on | 1 | Anti-lock braking |
| `TRACTION_CONTROL` | 0 = off, 1 = on | 1 | Traction control |
| `STABILITY` | 0 = off, 1 = on | 1 | Stability control (cuts throttle on slide) |
| `AUTO_CLUTCH` | 0 = off, 1 = on | 1 | Automatic clutch |
| `AUTO_SHIFTER` | 0 = manual, 1 = automatic | Depends on selection | Automatic/manual transmission |
| `IDEAL_LINE` | 0 = off, 1 = on | 1 (easy), 0 (medium+) | Racing line overlay |
| `DAMAGE` | 0-100 (%) | 0 (always) | Damage multiplier. **0 = no damage** (venue safety). |
| `VISUAL_DAMAGE` | 0 = off, 1 = on | 0 | Show cosmetic damage |
| `SLIPSTREAM` | 0 = off, 1 = on | 1 | Enable drafting physics |
| `TYRE_BLANKETS` | 0 = off, 1 = on | 1 | Pre-heated tyres (warm from start) |
| `AUTO_BLIP` | 0 = off, 1 = on | 1 | Rev-match on downshift |
| `FUEL_RATE` | 0-N (%) | 1 | Fuel consumption multiplier |

**Confidence:** HIGH - verified in codebase (ac_launcher.rs lines 279-291, 433-441)

### AC Server Configuration (server_cfg.ini) - Multiplayer

| Parameter | Values | Default | Purpose |
|-----------|--------|---------|---------|
| `ABS_ALLOWED` | 0 = none, 1 = factory only, 2 = forced on | 1 | Server-side ABS policy |
| `TC_ALLOWED` | 0 = none, 1 = factory only, 2 = forced on | 1 | Server-side TC policy |
| `STABILITY_ALLOWED` | 0 = off, 1 = on | 0 (default), 1 (venue) | Allow stability assist |
| `AUTOCLUTCH_ALLOWED` | 0 = off, 1 = on | 1 | Allow auto-clutch |
| `TYRE_BLANKETS_ALLOWED` | 0 = off, 1 = on | 1 | Allow pre-heated tyres |
| `FORCE_VIRTUAL_MIRROR` | 0 = off, 1 = on | 0 | Force rear-view mirror HUD |
| `DAMAGE_MULTIPLIER` | 0-100 (%) | 100 (default), 0 (venue) | Damage amount |
| `FUEL_RATE` | 0-N (%) | 100 | Fuel consumption |
| `TYRE_WEAR_RATE` | 0-N (%) | 100 | Tyre degradation |
| `PICKUP_MODE_ENABLED` | 0 or 1 | 1 | Allow join/leave mid-session |
| `MAX_CLIENTS` | 1-N | 16 | Max connected players |
| `LOOP_MODE` | 0 or 1 | 1 | Restart sessions automatically |
| `QUALIFY_MAX_WAIT_PERC` | 0-N (%) | 120 | Extra time to wait for qualifying |
| `RACE_OVER_TIME` | seconds | 60 | Extra time after winner finishes |
| `RESULT_SCREEN_TIME` | seconds | 5 | Time on results screen |

**Source:** [Kunos official server manual](https://www.assettocorsa.net/forum/index.php?faq/assetto-corsa-dedicated-server-manual.28/), [Elite Game Servers guide](https://www.elitegameservers.net/clientarea/knowledgebase/86/General-overview-The-basics-of-configuring-the-server.html)
**Confidence:** HIGH - verified in codebase (ac_server.rs lines 190-255) and multiple community sources

### AC FFB Configuration (controls.ini [FF])

| Parameter | Range | Presets | Purpose |
|-----------|-------|---------|---------|
| `GAIN` | 0-100 (%) | light=40, medium=70, strong=100 | Force feedback strength |

**Confidence:** HIGH - verified in codebase (ac_launcher.rs lines 217-258)

### AC Weather Presets

Available weather `GRAPHICS` values (from AC content):

| Value | Condition |
|-------|-----------|
| `1_heavy_fog` | Heavy fog |
| `2_light_fog` | Light fog |
| `3_clear` | Clear sky (default) |
| `4_mid_clear` | Partly clear |
| `5_light_clouds` | Light clouds |
| `6_mid_clouds` | Medium clouds |
| `7_heavy_clouds` | Heavy overcast |

**Confidence:** MEDIUM - from community sources and default in codebase

---

## Table Stakes

Features customers expect. Missing = product feels incomplete or customers leave.

### TS-1: One-Tap Game Launch

| Aspect | Detail |
|--------|--------|
| **Feature** | Customer selects car + track, game launches correctly with zero manual configuration |
| **Why Expected** | This is the core promise. Every sim racing venue does this. |
| **Complexity** | Low (already mostly built) |
| **AC Parameters** | race.ini write with car/track/skin, acs.exe launch, AUTOSPAWN=1 |
| **Status** | EXISTING - ac_launcher.rs handles full launch sequence |
| **Notes** | Current implementation writes race.ini, launches acs.exe, minimizes ConspitLink. Solid foundation. |

### TS-2: Billing Synced to Actual Driving

| Aspect | Detail |
|--------|--------|
| **Feature** | Timer starts when customer is on track, not during loading/DirectX init |
| **Why Expected** | Customers will not pay for loading screen time. This is a trust issue. |
| **Complexity** | Medium |
| **AC Parameters** | UDP telemetry port 9996, session_time_ms in TelemetryFrame, DrivingState detection |
| **Status** | PARTIAL - driving_detector.rs exists, billing timer exists, but sync to actual in-game session start needs work |
| **Notes** | DirectX init delay is a known issue (PROJECT.md line 60). Must detect first telemetry packet or first non-zero session_time_ms as session start. |

### TS-3: Safety Presets Always Enforced

| Aspect | Detail |
|--------|--------|
| **Feature** | Tyre Grip 100%, Damage 0%, Visual Damage 0% - non-negotiable |
| **Why Expected** | Hardware protection (wheelbases, monitors) and customer safety. Wrecked virtual car = confused customer. |
| **Complexity** | Low |
| **AC Parameters** | `DAMAGE=0`, `VISUAL_DAMAGE=0`, `SESSION_START=100`, `RANDOMNESS=0`, `SESSION_TRANSFER=100` |
| **Status** | EXISTING - hardcoded in ac_launcher.rs and catalog.rs |
| **Notes** | Also enforce in multiplayer: `DAMAGE_MULTIPLIER=0` in server_cfg.ini. Current default is 100 -- **needs fix**. |

### TS-4: Assist Presets (Difficulty Tiers)

| Aspect | Detail |
|--------|--------|
| **Feature** | Pre-built difficulty levels that map to assist combinations. No raw parameter tuning. |
| **Why Expected** | Customers do not know what ABS, TC, or stability control mean. They need "Easy / Medium / Hard". |
| **Complexity** | Low |
| **AC Parameters** | ABS, TC, STABILITY, AUTO_CLUTCH, IDEAL_LINE, AUTO_SHIFTER |
| **Status** | PARTIAL - catalog.rs has easy/medium/hard, PROJECT.md wants racing-themed names |

**Proposed Difficulty Tier Mapping:**

| Tier | ABS | TC | Stability | AutoClutch | IdealLine | AutoShifter | FFB | AI Level |
|------|-----|----|-----------|------------|-----------|-------------|-----|----------|
| **Rookie** | 1 | 1 | 1 | 1 | 1 | 1 (auto) | light (40) | 80 |
| **Amateur** | 1 | 1 | 0 | 1 | 0 | 1 (auto) | medium (70) | 85 |
| **Semi-Pro** | 1 | 1 | 0 | 1 | 0 | 0 (manual) | medium (70) | 90 |
| **Pro** | 1 | 0 | 0 | 0 | 0 | 0 (manual) | strong (100) | 95 |
| **Alien** | 0 | 0 | 0 | 0 | 0 | 0 (manual) | strong (100) | 100 |

**Notes:**
- Rookie = automatic everything + racing line. Perfect for first-timers. ~60% of customers.
- Amateur = remove stability + racing line. Customer has basic skills. ~25% of customers.
- Semi-Pro = manual transmission. Customer knows how to shift. ~10% of customers.
- Pro/Alien = for regulars and competitive racers. ~5% of customers.
- AI Level ranges from 80-100. Below 80 the AI is unrealistically slow and not fun.
- FFB gain scales with skill -- beginners get lighter feedback to avoid arm fatigue.

### TS-5: Car and Track Selection

| Aspect | Detail |
|--------|--------|
| **Feature** | Browse and select from available cars (325) and tracks (49) with categories and featured items |
| **Why Expected** | This is the content. Without selection, there is no experience. |
| **Complexity** | Low (already built) |
| **AC Parameters** | Car model IDs, track IDs, track configs |
| **Status** | EXISTING - catalog.rs with 36 featured tracks, 325 cars, 7 car categories, 5 track categories |
| **Notes** | Already has id_to_display_name() for unfeatured content. Categories: F1 2025, GT3, Supercars, Porsche, JDM, Classics, Other. |

### TS-6: Valid-Only Option Filtering

| Aspect | Detail |
|--------|--------|
| **Feature** | Only show options that actually work together. No invalid car/track/mode combos. |
| **Why Expected** | Invalid combos = crash or confusing error. Destroys customer trust. |
| **Complexity** | Medium |
| **AC Parameters** | Track pit count limits max cars, drift tracks need DRIFT_MODE=1, drag tracks need drag strip data |
| **Status** | NOT BUILT - PROJECT.md lists this as active requirement |
| **Notes** | Key validations needed: (1) Track pit count caps max AI opponents, (2) Not all tracks support all modes, (3) Some mod cars may crash on certain tracks. Need a compatibility matrix or runtime validation. |

### TS-7: Clean Session Lifecycle

| Aspect | Detail |
|--------|--------|
| **Feature** | Session starts cleanly, ends cleanly. Game killed, lock screen restored, pod ready for next customer. |
| **Why Expected** | Stale game state from previous customer = broken experience. |
| **Complexity** | Low (already built) |
| **AC Parameters** | Kill acs.exe + Content Manager.exe, minimize background windows, restore lock screen |
| **Status** | EXISTING - ac_launcher.rs cleanup_after_session() and enforce_safe_state() |
| **Notes** | Well-implemented. Kills all game processes, error dialogs, brings lock screen to foreground. |

### TS-8: Solo Race vs AI

| Aspect | Detail |
|--------|--------|
| **Feature** | Customer races against AI opponents on a grid, not just practice laps alone |
| **Why Expected** | Racing is the core appeal. Solo practice gets boring quickly. AI opponents = engagement. |
| **Complexity** | Medium |
| **AC Parameters** | `CARS=N` (total grid), `AI_LEVEL=80-100`, `RACE_LAPS=N` or `DURATION_MINUTES=N`, SESSION_0 `TYPE=3` |
| **Status** | NOT BUILT - Current race.ini always writes CARS=1 (solo) |
| **Notes** | Biggest gap in current system. Need to: (1) Add AI car blocks [CAR_1] through [CAR_N] to race.ini, (2) Set TYPE=3 for race session, (3) Cap AI count by track pit slots, (4) Use same car model for AI (simplest) or allow mixed grids. |

### TS-9: PWA Session Selection

| Aspect | Detail |
|--------|--------|
| **Feature** | Customer selects car, track, difficulty, session type from their phone (PWA after QR/PIN auth) |
| **Why Expected** | This is the customer interface. No PWA = staff must configure every session manually. |
| **Complexity** | Medium |
| **AC Parameters** | All parameters flow through to launch_args JSON |
| **Status** | PARTIAL - QR/PIN auth exists, custom experience booking exists, but session type selection not wired |
| **Notes** | PWA must show: car picker (categorized), track picker (categorized), difficulty tier (5 options), session type (Practice/Race). |

### TS-10: Staff Kiosk Configuration

| Aspect | Detail |
|--------|--------|
| **Feature** | Staff can configure and launch sessions from the kiosk dashboard |
| **Why Expected** | Staff need override capability. Walk-in customers need staff assistance. |
| **Complexity** | Medium |
| **AC Parameters** | Same as PWA but with more options (AI count, weather, time of day) |
| **Status** | PARTIAL - dashboard exists, AC server management exists |
| **Notes** | Staff kiosk should expose all parameters that PWA simplifies. Power-user interface. |

---

## Differentiators

Features that set the venue apart. Not expected, but create competitive advantage.

### D-1: Multi-Pod Multiplayer Racing

| Aspect | Detail |
|--------|--------|
| **Feature** | 2-8 customers race each other across pods on the same server |
| **Value Proposition** | "Race your friends" is the #1 group selling point. Most venues do solo-only. |
| **Complexity** | High |
| **AC Parameters** | AC dedicated server (acServer.exe), server_cfg.ini, entry_list.ini, Content Manager join via acmanager:// URI |
| **Status** | PARTIAL - ac_server.rs has full server lifecycle (start/stop/health), port allocator, preset system. CM join URI works. |
| **Notes** | Highest-value differentiator. Group bookings (birthdays, corporate events) drive 2-3x revenue per session. Architecture already supports it -- needs UX polish and billing coordination. Key challenge: all pods must join before race starts. Use PICKUP_MODE=1 for flexibility. |

### D-2: AI Grid Fill for Multiplayer

| Aspect | Detail |
|--------|--------|
| **Feature** | When 3 customers race on 8-pod capacity, AI fills remaining grid spots |
| **Value Proposition** | Empty grids feel hollow. AI traffic makes 2-3 player races feel like real events. |
| **Complexity** | Medium |
| **AC Parameters** | entry_list.ini AI slots, server-side AI configuration |
| **Status** | NOT BUILT - listed in PROJECT.md active requirements |
| **Notes** | AC dedicated server supports AI opponents natively. Need to configure entry_list.ini with AI slots alongside human slots. AI level should match the group's difficulty tier. |

### D-3: Race Weekend Mode (Multiplayer)

| Aspect | Detail |
|--------|--------|
| **Feature** | Full Practice -> Qualifying -> Race sequence for groups |
| **Value Proposition** | Premium experience for birthday parties, corporate events, league nights |
| **Complexity** | Medium |
| **AC Parameters** | Multiple SESSION blocks in server_cfg.ini: [PRACTICE] TIME=10, [QUALIFY] TIME=10, [RACE] LAPS=10 |
| **Status** | PARTIAL - AcLanSessionConfig already has sessions: Vec<AcSessionBlock> with Practice/Qualifying/Race defaults |
| **Notes** | Already modeled in types.rs. The default AcLanSessionConfig has a 3-session weekend (10min practice, 10min quali, 10-lap race). Just needs UX and billing integration for multi-session billing (sub-sessions). |

### D-4: Curated Experience Presets

| Aspect | Detail |
|--------|--------|
| **Feature** | Pre-built combos like "F1 at Monza", "GT3 at Spa", "JDM Street Racing" with perfect settings |
| **Value Proposition** | Reduces decision paralysis for new customers. One tap to drive. |
| **Complexity** | Low |
| **AC Parameters** | Pre-configured launch_args JSON with car/track/difficulty/weather combos |
| **Status** | PARTIAL - preset system exists in ac_server.rs (save/load/delete), needs customer-facing presets |

**Recommended Presets:**

| Preset Name | Car Category | Track | Difficulty | Appeal |
|-------------|-------------|-------|------------|--------|
| F1 Experience | Random F1 2025 | Random F1 circuit | Semi-Pro | F1 fans, headline experience |
| GT3 Battle | Random GT3 | Monza or Spa | Amateur | Approachable racing |
| Supercar Sunday | Random Supercar | Nordschleife | Rookie | Casual, wow-factor |
| Street Racer | Random JDM | Shuto/Haruna touge | Amateur | Initial D fans, youth appeal |
| Indian GP | Any F1 | Buddh International | Semi-Pro | Local pride, India circuit |
| First Timer | Ferrari 488 GT3 | Monza (short) | Rookie | Default for new customers |

### D-5: Hotlap Competition Mode

| Aspect | Detail |
|--------|--------|
| **Feature** | Timed hotlap sessions where customers compete for fastest lap on a leaderboard |
| **Value Proposition** | Drives repeat visits and social sharing. "Beat the record" is addictive. |
| **Complexity** | Medium |
| **AC Parameters** | Hotlap game mode (ghost car, standing start near finish), lap timing via UDP telemetry |
| **Status** | PARTIAL - LeaderboardEntry and Leaderboard types exist in rc-common, lap recording exists |
| **Notes** | Hotlap mode is perfect for walk-in "5 minute free trial" customers. Fixed car + fixed track = comparable times. Leaderboard displayed on venue screens. Weekly/monthly reset with prizes. |

### D-6: Live Mid-Session Adjustments

| Aspect | Detail |
|--------|--------|
| **Feature** | Change transmission (auto/manual), FFB strength, and assists without restarting the game |
| **Value Proposition** | Prevents session restarts when customer wants to try manual gearbox |
| **Complexity** | Low-Medium |
| **AC Parameters** | Write to race.ini + assists.ini while game running. Transmission change needs Ctrl+R or pit reset. |
| **Status** | PARTIAL - set_transmission() exists in ac_launcher.rs, set_ffb() exists |
| **Notes** | Transmission change already implemented (writes AUTO_SHIFTER to race.ini + assists.ini). FFB gain change implemented. ABS/TC/Stability changes need same pattern. Customer triggers from PWA; change takes effect on next pit stop or restart from pits. |

### D-7: Weather and Time of Day Presets

| Aspect | Detail |
|--------|--------|
| **Feature** | Staff can set weather (clear, cloudy, fog) and time of day (sun angle) for atmosphere |
| **Value Proposition** | Night racing at Singapore or sunset at Spa creates memorable, shareable experiences |
| **Complexity** | Low |
| **AC Parameters** | `SUN_ANGLE` (-80 to 80, where 16=afternoon, -80=night, 80=sunrise), `WEATHER.NAME` graphics preset |
| **Status** | PARTIAL - AcWeatherConfig exists in types.rs, SUN_ANGLE=16 hardcoded in race.ini |

**Proposed Time-of-Day Presets:**

| Preset | SUN_ANGLE | Mood |
|--------|-----------|------|
| Morning | 60 | Fresh, cool light |
| Afternoon | 16 | Default, bright |
| Golden Hour | -5 | Dramatic, warm |
| Night | -80 | Dark, headlights on (CSP required) |

**Notes:** Only staff should control weather/time (customer confusion risk). Night racing requires CSP weather FX.

### D-8: Session Split Billing (Multi-Session Packages)

| Aspect | Detail |
|--------|--------|
| **Feature** | 60-minute booking split into 3x20min sessions with different car/track combos |
| **Value Proposition** | Prevents boredom and lets customers try multiple experiences in one visit |
| **Complexity** | Medium |
| **AC Parameters** | Multiple sequential game launches within one billing session |
| **Status** | PARTIAL - BillingSessionInfo has split_count, split_duration_minutes, current_split_number fields |
| **Notes** | Data model exists. Need UX for pre-selecting 2-3 experiences at booking time, and auto-transition between sessions (kill game, launch next, billing continues). |

---

## Anti-Features

Features to explicitly NOT build. Each has a clear reason.

### AF-1: Raw Parameter Exposure to Customers

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| Showing ABS=0/1, TC=0/1, STABILITY=0/1 toggles to customers | Confusing, intimidating, leads to bad experiences (beginner turns off all assists, spins constantly) | Map everything to difficulty tiers (Rookie through Alien). One selection, all assists configured. |

### AF-2: Damage Enabled

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| Allowing any non-zero damage setting | Damaged virtual car = customer thinks they broke something. Mechanical failure mid-session ruins the experience. Wheel/bodywork damage reduces car performance, making the rest of the session unfun. | Always DAMAGE=0, VISUAL_DAMAGE=0, DAMAGE_MULTIPLIER=0 (server). No exceptions. |

### AF-3: Tyre Wear / Fuel Consumption for Casual Sessions

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| Enabling tyre degradation or fuel consumption for standard sessions | Running out of fuel or getting slow tyres mid-session confuses paying customers. They do not understand pit strategy. | FUEL_RATE=1 (minimal), TYRE_BLANKETS=1, TYRE_WEAR_RATE=0 for casual. Only enable for competitive events with experienced drivers. |

### AF-4: Dynamic Track Grip Variation

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| Allowing SESSION_START < 100 or RANDOMNESS > 0 | Inconsistent grip = customer spins "for no reason". Destroys confidence. | Always SESSION_START=100, RANDOMNESS=0, SESSION_TRANSFER=100. Track is always fully rubbered-in. |

### AF-5: Custom Car Setups for Customers

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| Letting customers modify car setups (suspension, aero, gearing) | Overwhelms 95% of customers. Wrong setup makes car undriveable. Support burden is massive. | FIXED_SETUP=0 but provide no UI for it. Advanced customers can access via in-game menu if they know how. Do not surface this. |

### AF-6: Online/Public Server Joining

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| Allowing customers to join random public AC servers | Security risk, inappropriate content, uncontrolled experience, billing impossible | All multiplayer is venue-controlled LAN servers only. REGISTER_TO_LOBBY=0 always. |

### AF-7: Replay Recording / Custom Liveries / Voice Chat

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| Building replay recording, custom livery selection, or inter-pod voice chat for v1 | Each is a large feature with limited impact on core value. Deferred per PROJECT.md scope. | v1 focus on launch + billing + multiplayer. These are v2+ features. |

### AF-8: Content Manager UI Exposure

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| Letting customers see or interact with Content Manager | CM is complex modding software. Customers will change settings, break things, access unauthorized content. | CSP gui.ini: FORCE_START=1 + HIDE_MAIN_MENU=1 (already configured). CM used only as launch mechanism for multiplayer join, never shown to customer. |

### AF-9: Drift Mode / Drag Racing (v1)

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| Building drift scoring or drag racing into v1 | Niche demand (~5% of customers at most). Drift requires specific tracks + specific cars. Drag requires drag strip tracks. Both need custom UX. | Defer to v2. Drift-interested customers can still drive touge tracks in Practice mode. |

---

## Feature Dependencies

```
TS-5 (Car/Track Selection) --> TS-6 (Valid-Only Filtering) --> TS-8 (Solo Race vs AI)
                                                                    |
                                                                    v
                                                              TS-1 (One-Tap Launch)
                                                                    |
                                                                    v
                                                              TS-2 (Billing Sync)
                                                                    |
                                                                    v
                                                              TS-7 (Clean Lifecycle)

TS-4 (Difficulty Tiers) --> TS-9 (PWA Selection) --> TS-1 (One-Tap Launch)
                                    |
                                    v
                              TS-10 (Staff Kiosk)

D-1 (Multi-Pod Multiplayer) --> D-2 (AI Grid Fill)
         |                            |
         v                            v
    D-3 (Race Weekend)          D-8 (Split Billing)

D-4 (Curated Presets)  [independent, can ship anytime]
D-5 (Hotlap Competition)  [depends on TS-2 billing sync + lap recording]
D-6 (Mid-Session Adjustments)  [depends on TS-1 launch working]
D-7 (Weather/Time Presets)  [independent, low effort]
```

**Critical path:** TS-5 -> TS-6 -> TS-4 -> TS-8 -> TS-1 -> TS-2 -> TS-7 is the core single-player flow.

D-1 (multiplayer) is the highest-value differentiator but can be built in parallel since ac_server.rs infrastructure already exists.

---

## MVP Recommendation

### Phase 1: Single-Player Complete (Immediate)

Build the complete single-player flow end-to-end:

1. **TS-8** Solo Race vs AI - Add AI car blocks to race.ini, TYPE=3 support, configurable grid size
2. **TS-4** Difficulty Tiers - Map Rookie/Amateur/Semi-Pro/Pro/Alien to AC parameters (assists + AI level + FFB)
3. **TS-6** Valid-Only Filtering - Prevent invalid combos (track pit count, mode support)
4. **TS-2** Billing Sync - Start timer on first telemetry, not on game launch
5. **TS-9** PWA Session Selection - Wire difficulty + session type + AI count into customer flow
6. **D-4** Curated Presets - 4-6 staff-created presets for quick selection

**Rationale:** This completes the customer journey for the most common case (solo customer, walk-in or booking). Every session today already works for practice; this phase adds racing with AI and proper billing. Biggest customer impact per effort.

### Phase 2: Multiplayer & Groups (High Value)

7. **D-1** Multi-Pod Multiplayer - UX for creating group races, coordinated launch
8. **D-2** AI Grid Fill - Fill empty server slots with AI
9. **D-3** Race Weekend Mode - Practice + Quali + Race sequence for groups
10. **D-8** Split Billing - Multi-session packages

**Rationale:** Group experiences drive 2-3x revenue per session. Birthday parties, corporate events, friend groups. The infrastructure (ac_server.rs, port allocator, presets) already exists; this phase adds the UX layer and billing coordination.

### Phase 3: Engagement & Retention

11. **D-5** Hotlap Competition - Leaderboard, weekly competitions
12. **D-6** Mid-Session Adjustments - PWA-driven assist/FFB changes
13. **D-7** Weather/Time Presets - Atmospheric variety

**Rationale:** These drive repeat visits and differentiate from competitors. Lower priority because they do not block core revenue generation.

### Defer to v2+

- Replay recording / sharing
- Custom liveries
- Voice chat between pods
- Full leaderboard/ranking system with progression
- Drift mode (score-based)
- Drag racing
- Tyre wear / fuel strategy (competitive events only)
- Championship/league management

---

## Sources

**Codebase (HIGH confidence):**
- `crates/rc-agent/src/ac_launcher.rs` - Full AC launch sequence, assists, FFB, race.ini generation
- `crates/rc-core/src/ac_server.rs` - Multiplayer server lifecycle, server_cfg.ini generation
- `crates/rc-core/src/catalog.rs` - Car/track catalog, difficulty presets
- `crates/rc-common/src/types.rs` - All type definitions including AcLanSessionConfig, AcAids, SessionType

**Official/Community (MEDIUM confidence):**
- [Kunos AC Dedicated Server Manual](https://www.assettocorsa.net/forum/index.php?faq/assetto-corsa-dedicated-server-manual.28/)
- [Elite Game Servers AC Configuration Guide](https://www.elitegameservers.net/clientarea/knowledgebase/86/General-overview-The-basics-of-configuring-the-server.html)
- [OverTake.gg Session Types Guide](https://www.overtake.gg/threads/understanding-what-practice-hotlap-race-track-day-weekend-drift-and-drag-race-mean.269480/)
- [OverTake.gg AI Configuration](https://www.overtake.gg/threads/ai-ini-configuration.149771/)
- [assetto-server-manager config_ini.go](https://github.com/JustaPenguin/assetto-server-manager/blob/master/config_ini.go)
- [GTPlanet AI Discussion](https://www.gtplanet.net/forum/threads/assetto-corsa-ai.350714/)

**Venue Industry (MEDIUM confidence):**
- [SimRacing.co.uk VMS V5.0 Features](https://www.simracing.co.uk/features.html)
- [SimStaff Event Guide 2025](https://simstaff.net/the-ultimate-guide-to-racing-simulator-experience-for-events-in-2025/)
- [Racing Unleashed Experience](https://www.racing-unleashed.com/experience)
