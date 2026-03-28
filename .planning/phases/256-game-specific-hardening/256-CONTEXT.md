# Phase 256: Game-Specific Hardening - Context

**Gathered:** 2026-03-29
**Status:** Ready for planning
**Mode:** Auto-generated (infrastructure phase, discuss skipped)

<domain>
## Phase Boundary

Each supported game launches reliably with correct process monitoring and content verification. Steam pre-launch checks, process name corrections, Forza session enforcer, AC EVO config adapter, iRacing subscription check, DLC verification.

Requirements: GAME-01 (Steam check), GAME-02 (process names), GAME-03 (Forza enforcer), GAME-04 (AC EVO adapter), GAME-05 (iRacing check), GAME-06 (DLC check), GAME-07 (Steam dialog detection), GAME-08 (non-AC crash detection)

Depends on: Phase 253 (FSM hardened before per-game launch guards added)

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
Key guidance from MMA audit (Nemotron game-specific analysis):

- GAME-01: Before Steam URL launch, check if Steam.exe is running via sysinfo process scan. If not running, attempt to start it, wait 10s, recheck. If still not running, reject launch with error.
- GAME-02: Audit actual exe names on pods. Known: acs.exe (AC), F1_25.exe or F1_2025.exe (F1 25), iRacingSim64DX11.exe (iRacing), LMU.exe or LeMansUltimate.exe (LMU), ForzaHorizon5.exe (FH5). Update ALL_GAME_PROCESS_NAMES in game_process.rs.
- GAME-03: For Forza Horizon 5 (open-world, no session concept): agent spawns a timer task. At duration_minutes - 1 min, send a warning overlay. At duration_minutes, taskkill the game process.
- GAME-04: AC EVO uses Unreal engine. race.ini format doesn't apply. Create a minimal adapter that writes GameUserSettings.ini or passes config via command-line args.
- GAME-05: iRacing subscription check is hard to automate (requires web scraping or API). Simpler: check if iRacing launches successfully within 30s. If it shows a login/subscription error dialog, detect via window title and reject.
- GAME-06: Before launch, check Steam appmanifest for installed DLC. If car/track content ID is not in installed content, reject with clear error.
- GAME-07: After Steam URL launch, poll for actual game window (not Steam overlay). If only Steam windows visible after 60s, report launch failure.
- GAME-08: For non-AC games, use process exit monitoring as primary crash detection. Poll game PID every 5s. If PID gone + no graceful exit marker, treat as crash.

</decisions>

<code_context>
## Existing Code Insights

### Key Files (rc-agent crate)
- `crates/rc-agent/src/game_process.rs` — ALL_GAME_PROCESS_NAMES, cleanup, PID management
- `crates/rc-agent/src/sims/` — per-game adapter modules (f1_25.rs, iracing.rs, lmu.rs, etc.)
- `crates/rc-agent/src/ac_launcher.rs` — AC-specific launch sequence
- `crates/rc-agent/src/ws_handler.rs` — LaunchGame handler, pre-launch checks
- `crates/rc-agent/src/config.rs` — detect_installed_games(), steam_app_id

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
