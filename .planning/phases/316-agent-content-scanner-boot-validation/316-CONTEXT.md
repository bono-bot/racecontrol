# Phase 316: Agent Content Scanner & Boot Validation - Context

**Gathered:** 2026-04-03
**Status:** Ready for planning
**Mode:** Auto-generated (discuss skipped via autonomous mode)

<domain>
## Phase Boundary

rc-agent auto-detects all installed games (Steam + non-Steam) at boot and proactively validates AC combos against the filesystem before any customer session starts. Extends content_scanner.rs to scan Steam library via libraryfolders.vdf parsing + configured non-Steam paths. Adds boot-time AC preset validation (car folder, track folder, AI lines). Sends GameInventoryUpdate and ComboValidationResult WS messages to server. Includes 5-minute periodic rescan.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — discuss phase was skipped per autonomous mode. Use ROADMAP phase goal, success criteria, and codebase conventions to guide decisions.

Key constraints from research:
- Steam VDF parsing: use std::str::lines() pattern — no parser crate needed, just extract "path" keys from libraryfolders.vdf
- Non-Steam games: probe known exe paths (iRacing registry key, Forza via MS Store, etc.) — configurable in racecontrol.toml
- Boot validation must be async-decoupled from WS connect — use spawn_blocking for filesystem walks
- Wait for preset push from server before running combo validation (don't validate against empty list)
- Must follow boot resilience standing rule: periodic re-scan every 5 minutes, not single-fetch-at-boot
- ContentManifest backward compat required — new GameInventoryUpdate is ADDITIVE, don't break old agents
- Phase 315 added GameInventoryUpdate, ComboValidationResult types to rc-common — use those exact structs
- installed_games field exists in PodInfo (types.rs:104) but is unpopulated — populate it from scanner results

</decisions>

<code_context>
## Existing Code Insights

### Key Files
- `crates/rc-agent/src/content_scanner.rs` — existing AC scanner (cars/tracks/configs), ContentManifest
- `crates/rc-agent/src/steam_checks.rs` — existing Steam path detection (hardcoded C/D/E, known VDF gap)
- `crates/rc-agent/src/game_doctor.rs` — 12-point diagnostic (checks 9-11: car/track/config validation)
- `crates/rc-agent/src/main.rs` — agent startup, WS connect handler
- `crates/rc-common/src/types.rs` — GameInventoryUpdate, ComboValidationResult, InstalledGame, GameInventory structs (Phase 315)
- `crates/rc-common/src/protocol.rs` — AgentMessage::GameInventoryUpdate, AgentMessage::ComboValidationReport variants (Phase 315)
- `crates/rc-common/src/types.rs:104` — PodInfo.installed_games: Vec<SimType> (exists but unpopulated)

### Established Patterns
- content_scanner.rs scan_ac_content() → ContentManifest sent via WS on connect
- Boot resilience: spawn_periodic_refetch() pattern for 5-min re-fetch (used by allowlist, feature flags)
- Standing rule: no .unwrap() in production, no lock held across .await

### Integration Points
- Agent startup: content_scanner runs, sends ContentManifest — extend to also send GameInventoryUpdate
- WS reconnect: re-send inventory (same as ContentManifest pattern)
- PresetPush handler: after receiving presets, trigger combo validation
- PodInfo heartbeat: populate installed_games from scanner cache

</code_context>

<specifics>
## Specific Ideas

- Steam library scanning: parse C:\Program Files (x86)\Steam\steamapps\libraryfolders.vdf for all library paths, then check each for appmanifest_*.acf files matching known SimType app IDs
- Known Steam app IDs: AC=244210, AC Evo=TBD, F1 25=2488620 (anti-cheat wrapper: 3059520), iRacing=266410, LMU=TBD, Forza=TBD
- Non-Steam: check registry keys, Windows Store paths, known install locations
- Combo validation: for each AC preset, check content/cars/{car}/ exists, content/tracks/{track}/ exists, content/tracks/{track}/ai/ has files

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
