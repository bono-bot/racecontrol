# Phase 197: Launch Resilience & AC Hardening - Context

**Gathered:** 2026-03-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Add resilience to the game launcher: dynamic timeouts from launch_events history, pre-launch health checks on rc-agent, structured error taxonomy, auto-retry with clean state reset, Race Engineer atomic counter fix, and AC-specific improvements (polling waits, CM timeout, fresh PID on fallback). Launch failures should recover automatically in under 60 seconds.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — resilience/infrastructure phase. Key decisions:
- Dynamic timeout calculation strategy (median + 2*stdev from launch_events)
- Pre-launch health check implementation on rc-agent side
- Error taxonomy enum design
- Auto-retry mechanism (Race Engineer extension vs new system)
- Clean state reset: which processes to kill, what files to clean
- AC polling implementation (interval, max wait, backoff)
- WhatsApp alert format and triggering logic
- Whether to split into per-game launcher files or keep in game_launcher.rs

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/racecontrol/src/game_launcher.rs` — GameLauncherImpl trait from Phase 196, launch_game(), relaunch_game(), check_game_health()
- `crates/racecontrol/src/metrics.rs` — record_launch_event(), launch_events table from Phase 195
- `crates/rc-agent/src/game_manager.rs` — AC launch logic, Content Manager integration, process management
- `crates/rc-common/src/pod_id.rs` — normalize_pod_id()
- `crates/racecontrol/src/billing.rs` — WhatsApp notification patterns (Evolution API)

### Established Patterns
- GameLauncherImpl trait with validate_args(), make_launch_message(), cleanup_on_failure()
- tokio::spawn for background tasks (Stopping timeout pattern from Phase 196)
- tracing::warn!/error! for logging
- Feature flags checked before operations
- SQLite queries via sqlx::query()

### Integration Points
- launch_game() — add dynamic timeout, pre-launch checks, error taxonomy
- relaunch_game() — fix null args rejection, atomic counter
- check_game_health() — trigger Race Engineer on timeout
- rc-agent game_manager — AC polling waits, CM timeout, clean state reset
- WhatsApp via Evolution API for staff alerts
- launch_events table for historical data queries

</code_context>

<specifics>
## Specific Ideas

- 16 requirements: LAUNCH-08 through LAUNCH-19, AC-01 through AC-04
- Dynamic timeout: median(last 10 durations) + 2*stdev, per car/track/sim combo
- Default timeouts: AC=120s, F1=90s, iRacing=90s
- Pre-launch checks: no orphan game exe, disk > 1GB, no MAINTENANCE_MODE, no OTA_DEPLOYING
- Clean state reset: kill ALL 13 game exe names, delete game.pid, clear shared memory
- Auto-retry: 2 attempts max, same launch_args, then WhatsApp alert
- Error taxonomy: Timeout, ProcessCrash(exit_code), PreLaunchFailed(reason), AgentDisconnect, ContentManagerFailed, Unknown
- Game crash counter MUST be separate from pod health counter (no MAINTENANCE_MODE from game crashes)
- Race Engineer atomic: single write lock for counter increment + relaunch spawn
- AC polling: poll for acs.exe absence (max 5s), poll for AC window (max 30s)
- CM: 30s timeout, 5s progress logging, fresh PID on fallback to direct launch

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
