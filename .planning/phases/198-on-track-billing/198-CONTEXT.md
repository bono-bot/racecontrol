# Phase 198: On-Track Billing - Context

**Gathered:** 2026-03-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Rework billing to start ONLY when the customer's car is on-track and controllable. Implement per-game PlayableSignal detection (AC: UDP telemetry, F1 25: UDP port 20777, iRacing: shared memory). Add "Loading..." / WaitingForGame dashboard state. Pause billing on crash, resume on successful relaunch. Add configurable timeouts for PlayableSignal detection. Fix multiplayer billing to use PlayableSignal instead of immediate start.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — billing accuracy phase. Key decisions:
- PlayableSignal detection implementation per game (UDP parsing, shared memory reading)
- WaitingForGame state representation in dashboard events
- Billing pause/resume mechanism on crash/relaunch
- Configurable timeout values and where to store them
- How to handle PlayableSignal timeout (fallback to timer-based billing?)
- Whether PlayableSignal runs on server or agent side
- Dashboard state machine for Loading... → Playing transitions

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/racecontrol/src/billing.rs` — billing session lifecycle, active_timers, waiting_for_game, defer_billing_start
- `crates/racecontrol/src/game_launcher.rs` — GameLauncherImpl trait, handle_game_state_update(), GameTracker
- `crates/racecontrol/src/metrics.rs` — record_billing_accuracy_event() with delta_ms
- `crates/rc-agent/src/ac_launcher.rs` — AC-specific launch logic
- `crates/rc-common/src/types.rs` — GameLaunchInfo, GameState enum, DashboardEvent

### Established Patterns
- GameState enum: Idle, Launching, Running, Stopping, Error
- BillingSessionStatus: Active, PausedManual, PausedDisconnect, PausedGamePause
- DashboardEvent::GameStateChanged for real-time updates
- waiting_for_game HashMap for deferred billing start
- UDP telemetry ports: 9996 (AC), 20777 (F1), 5300 (Forza), 6789 (iRacing)

### Integration Points
- handle_game_state_update() — detect PlayableSignal and trigger billing start
- billing.rs defer_billing_start() — already defers, needs to wait for PlayableSignal
- DashboardEvent — needs WaitingForGame/Loading state
- rc-agent game state reporting — needs PlayableSignal detection

</code_context>

<specifics>
## Specific Ideas

- 12 requirements: BILL-01 through BILL-12
- PlayableSignal per game: AC uses UDP telemetry (car speed > 0 or lap count > 0), F1 25 uses UDP motion data, iRacing uses shared memory
- WaitingForGame state: between Launching and Running, shows "Loading..." on dashboard
- Billing pause on crash: use existing PausedGamePause status
- Billing resume on relaunch: detect Running after PausedGamePause
- Configurable timeouts: PlayableSignal timeout per game (default AC=180s, F1=120s, iRacing=120s)
- If PlayableSignal times out: start billing anyway with warning log (don't block indefinitely)
- Multiplayer: PlayableSignal for first player, then start billing for all

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
