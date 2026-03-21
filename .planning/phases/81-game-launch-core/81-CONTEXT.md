# Phase 81: Game Launch Core - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Staff can launch any of 5 non-AC games (F1 25, iRacing, AC EVO, EA WRC, LMU) on any pod from the kiosk, customers can request games from the PWA, game status is visible across kiosk/fleet/spectator, and crash recovery works identically to AC. No per-game configuration (car/track/difficulty) -- pick and go.

</domain>

<decisions>
## Implementation Decisions

### TOML Launch Profiles
- All 5 games are installed via Steam -- use `steam://rungameid/{id}` launch method
- Games vary by pod -- `detect_installed_games()` already handles this via Steam appmanifest scanning
- Launch args/environment setup is TBD per game -- need testing on actual pods. Start with Steam launch only, add args if specific games require them
- `GamesConfig` in rc-agent config.rs already has fields for all games -- just need Steam app IDs populated in racecontrol.toml on each pod

### Crash Recovery
- Full crash recovery for non-AC games -- same as AC: detect, cleanup, auto-relaunch with backoff, alert staff after N failures
- Currently the non-AC crash branch in main.rs (~line 1715) just logs a warning -- needs to call `GameProcess::launch()` with cached config
- Claude's Discretion: Steam PID gap detection approach (process name scan via sysinfo is the existing pattern in `all_game_process_names()`)

### Kiosk / PWA Flow
- **Direct launch for non-AC games** -- no wizard. Click game icon on pod card, it launches immediately. No car/track/difficulty steps
- AC keeps its existing wizard flow (custom experience booking)
- **Game logos + names in kiosk** -- including Assetto Corsa. Visual game icons next to labels in selection UI
- **Customer PWA game menu** -- customer sees available games on their phone, taps one, staff gets a notification to confirm and launch
- Claude's Discretion: Game logo assets approach (static bundled PNGs vs CDN)

### Game State Reporting
- Existing GameState enum (Idle, Launching, Running, Crashed) is sufficient for non-AC games
- Fleet health API already reports `current_game` and `game_state` -- no schema changes needed
- Spectator view (venue TV) should show which game each pod is running as **text only** (not logos) -- e.g., "F1 25", "iRacing"

### Claude's Discretion
- Steam PID discovery strategy for process monitoring
- Game logo asset sourcing and bundling approach
- Exact crash recovery backoff parameters for non-AC games (can mirror AC's `EscalatingBackoff`)
- Whether to add EA WRC-specific config (telemetry JSON config file deployment) -- may be Phase 87 scope

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Game Launch Infrastructure
- `crates/rc-agent/src/game_process.rs` -- GameExeConfig struct, GameProcess::launch() with Steam URL and direct exe methods, orphan cleanup, PID persistence
- `crates/rc-agent/src/config.rs` -- GamesConfig struct with per-game GameExeConfig fields, detect_installed_games() with Steam appmanifest validation
- `crates/rc-agent/src/sims/mod.rs` -- SimAdapter trait definition (connect, read_telemetry, poll_lap_completed, session_info, disconnect)
- `crates/rc-agent/src/failure_monitor.rs` -- Crash detection, 90s launch timeout (CRASH-02), game_launch_elapsed tracking

### Server-Side Launch
- `crates/racecontrol/src/game_launcher.rs` -- launch_game() async handler, CoreToAgentMessage::LaunchGame dispatch, handle_game_state_update(), relaunch_game()
- `crates/racecontrol/src/billing.rs` -- BillingManager, compute_session_cost(), billing_rates integration

### Shared Types
- `crates/rc-common/src/types.rs` -- SimType enum (all 8 variants), GameState enum, GameLaunchInfo struct, PodStatus with current_game and installed_games fields

### Kiosk Frontend
- `kiosk/src/app/staff/page.tsx` -- Staff dashboard with onLaunchGame handler
- `kiosk/src/app/book/page.tsx` -- Booking wizard with game selection and installed_games filtering
- `kiosk/src/components/KioskPodCard.tsx` -- Pod card with game info display and launch button
- `kiosk/src/app/spectator/page.tsx` -- Spectator view with sim_type display

### Research
- `../game-launcher/.planning/research/ARCHITECTURE.md` -- Architecture analysis confirming trait-based adapter pattern
- `../game-launcher/.planning/research/PITFALLS.md` -- Steam PID gap, process tree cleanup, game update risks
- `../game-launcher/.planning/research/STACK.md` -- Zero new deps, winapi 0.3 + sysinfo for all needs

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `GameProcess::launch()`: Already handles Steam URL and direct exe launch -- just needs correct config
- `detect_installed_games()`: Scans Steam appmanifest + disk -- filtering already works
- `all_game_process_names()`: Has exe names for all games -- used by orphan cleanup
- `GameExeConfig`: Serde-deserializable from TOML with exe_path, working_dir, args, steam_app_id, use_steam
- `EscalatingBackoff`: Shared backoff logic in rc-common -- can be used for non-AC crash recovery

### Established Patterns
- AC launch: Content Manager URL scheme (`acmanager://`) via kiosk wizard
- Non-AC launch: `CoreToAgentMessage::LaunchGame { sim_type, launch_args }` -- same message type
- Crash detection: `failure_monitor.rs` polls game process status, triggers relaunch or alert
- State broadcast: `handle_game_state_update()` updates AppState and broadcasts via WebSocket

### Integration Points
- `main.rs` non-AC crash recovery branch (~line 1715): Currently logs warning, needs `GameProcess::launch()` call
- Kiosk pod card: `onLaunchGame` callback exists but triggers AC wizard -- needs direct launch path for non-AC
- Customer PWA: New game request endpoint needed on racecontrol API
- Spectator page: `gameName` display already resolves `sim_type` via `prettyName()` -- works out of the box

</code_context>

<specifics>
## Specific Ideas

- "Do the same for Assetto Corsa" -- game logos should include AC, not just the new games
- Direct launch (no wizard) for non-AC games -- click game icon, it launches. This is a deliberate UX simplification vs AC's deep config
- Customer PWA shows available games -- tapping sends a request that staff sees and confirms

</specifics>

<deferred>
## Deferred Ideas

None -- discussion stayed within phase scope

</deferred>

---

*Phase: 81-game-launch-core*
*Context gathered: 2026-03-21*
