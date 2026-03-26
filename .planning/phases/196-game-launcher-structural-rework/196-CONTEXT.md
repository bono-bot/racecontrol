# Phase 196: Game Launcher Structural Rework - Context

**Gathered:** 2026-03-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Decompose the monolithic launch_game() into per-game trait implementations (AcLauncher, F1Launcher, IRacingLauncher). Fix billing gate bugs (deferred sessions rejected, paused sessions allowed, TOCTOU races). Fix state machine gaps (Stopping timeout, disconnected agent detection, feature flag block propagation, externally-tracked games). Fix error propagation (invalid JSON bypass, silent broadcast failures).

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — structural refactoring phase. Key decisions:
- GameLauncher trait design (methods: launch(), validate_args(), cleanup())
- Per-game launcher struct placement (same file vs separate modules)
- Billing gate check ordering and TOCTOU mitigation strategy
- Stopping timeout implementation (tokio::spawn with sleep vs background task)
- How externally_tracked games are represented in GameTracker
- Whether to use enum dispatch or dynamic dispatch for trait
- Error type design for launch failures

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/racecontrol/src/game_launcher.rs` — monolithic launch_game(), handle_game_state_update(), relaunch_game(), stop_game()
- `crates/racecontrol/src/billing.rs` — billing session lifecycle, active_timers, waiting_for_game maps
- `crates/rc-common/src/pod_id.rs` — normalize_pod_id() from Phase 194
- `crates/racecontrol/src/metrics.rs` — record_launch_event() from Phase 195
- `crates/racecontrol/src/api/routes.rs` — existing Axum handlers

### Established Patterns
- Axum handlers with `State<Arc<AppState>>` extraction
- `sqlx::query()` with `.execute(&state.db)` for DB operations
- `tracing::error!` / `tracing::warn!` for logging
- HashMaps behind RwLock for shared state (active_games, active_timers, agent_senders)
- DashboardEvent broadcast via `dashboard_tx: broadcast::Sender<DashboardEvent>`

### Integration Points
- `launch_game()` called from POST /api/v1/games/launch
- `handle_game_state_update()` called from WebSocket agent messages
- `relaunch_game()` called from Race Engineer auto-recovery
- `stop_game()` called from POST /api/v1/games/stop and billing timeout
- Billing maps checked during launch: active_timers, waiting_for_game
- Feature flags checked via `is_feature_enabled()`

</code_context>

<specifics>
## Specific Ideas

- 13 requirements: LAUNCH-01 through LAUNCH-07, STATE-01 through STATE-06
- Trait-based architecture enables Phase 197 (per-game resilience) and Phase 198 (per-game PlayableSignal)
- Billing gate must check BOTH active_timers AND waiting_for_game (currently only checks active_timers)
- Stopping timeout: 30 seconds, then auto-transition to Error with dashboard broadcast
- Disconnected agent detection: immediate Error transition, not 120s timeout
- Feature flag block: agent sends explicit GameStateUpdate with Error state
- Externally tracked games: tracker with externally_tracked=true, launch_args=None

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
