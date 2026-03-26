# Phase 199: Crash Recovery - Context

**Gathered:** 2026-03-26
**Status:** Ready for planning

<domain>
## Phase Boundary

When a game crashes during launch or mid-session, perform full clean-slate reset and relaunch within 60 seconds total. Recovery actions informed by historical success data from launch_events. Customer session continues with minimal interruption. Safe mode and grace timer fixes.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — crash recovery phase. Key decisions:
- How to detect crash vs intentional stop (exit code analysis, process monitoring)
- Clean state reset sequence (kill processes, clear files, reset adapters)
- Recovery action selection from historical data
- Grace timer implementation for relaunch attempts
- Safe mode interaction with game crashes vs pod health crashes
- Auto-relaunch flow with preserved launch_args
- Staff alerting after exhausted retries

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/racecontrol/src/game_launcher.rs` — Race Engineer auto-retry (atomic, from Phase 197), relaunch_game(), externally_tracked field
- `crates/racecontrol/src/metrics.rs` — record_recovery_event(), recovery_events table, record_launch_event()
- `crates/rc-agent/src/game_process.rs` — clean_state_reset() from Phase 197, pre_launch_checks()
- `crates/racecontrol/src/billing.rs` — billing pause on crash (PausedGamePause from Phase 198)
- `crates/rc-common/src/types.rs` — ErrorTaxonomy from Phase 197

### Established Patterns
- Race Engineer: atomic counter increment, max 2 retries, WhatsApp alert on exhaustion
- clean_state_reset(): kills 13 game exe names, deletes game.pid, clears shared memory
- ErrorTaxonomy: ProcessCrash(exit_code), Timeout, PreLaunchFailed(reason), etc.
- record_recovery_event(): captures failure_mode, recovery_action_tried, outcome, duration_ms

### Integration Points
- handle_game_state_update() — crash detection triggers recovery flow
- Race Engineer relaunch — needs preserved launch_args, clean state, billing coordination
- recovery_events table — informs future recovery action selection
- Staff alert (WhatsApp) — after exhausted retries
- MAINTENANCE_MODE — must NOT be triggered by game crashes (separate counter)

</code_context>

<specifics>
## Specific Ideas

- 7 requirements: RECOVER-01 through RECOVER-07
- Under 60s total recovery SLA: detect crash + clean state + relaunch
- History-informed recovery: query recovery_events for success rates per failure_mode
- Preserved args: relaunch with exact same car/track/session (from GameTracker.launch_args)
- Grace timer: brief cooldown between crash detection and relaunch attempt
- Safe mode: game crash counter SEPARATE from pod health counter
- Clean state: full reset between attempts (no stale PIDs, no orphan processes)

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
