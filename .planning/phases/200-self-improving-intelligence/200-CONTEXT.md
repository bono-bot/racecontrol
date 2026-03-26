# Phase 200: Self-Improving Intelligence - Context

**Gathered:** 2026-03-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Use accumulated launch_events data to compute combo reliability scores, warn staff about unreliable car/track/sim combos, suggest alternatives when reliability is low, display a launch matrix in admin dashboard API, and implement rolling-window self-tuning timeouts. Every launch makes the system smarter without manual threshold tuning.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — intelligence/analytics phase. Key decisions:
- Combo reliability score formula (success_rate over last N launches)
- Warning threshold (e.g., <70% success rate)
- Alternative suggestion algorithm (same game, different car/track with higher reliability)
- Rolling window size and self-tuning approach
- Admin API endpoint design for launch matrix
- Where to store/cache computed scores (in-memory vs per-query)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/racecontrol/src/metrics.rs` — launch_events table, query_dynamic_timeout(), query_best_recovery_action(), record_launch_event()
- `crates/racecontrol/src/game_launcher.rs` — GameLauncherImpl trait, launch_game() with dynamic timeout
- `crates/racecontrol/src/api/metrics.rs` — launch_stats_handler, billing_accuracy_handler
- `crates/racecontrol/src/api/routes.rs` — Axum route registration

### Established Patterns
- SQLite aggregate queries with GROUP BY for per-combo stats
- Axum handlers with State<Arc<AppState>> extraction
- JSON serialization via serde for API responses
- Dynamic timeout already uses median+stdev from historical data

### Integration Points
- launch_game() — check reliability before launch, warn if low
- POST /api/v1/games/launch — include reliability warning in response
- New API: GET /api/v1/metrics/launch-matrix — admin reliability grid
- metrics.rs — new query functions for combo scores and alternatives

</code_context>

<specifics>
## Specific Ideas

- 5 requirements: INTEL-01 through INTEL-05
- Combo reliability = success_count / total_count over rolling 30-day window
- Warning when reliability < 70%: include in launch response JSON
- Alternatives: suggest top 3 combos with same game + higher reliability
- Admin launch matrix: grid of car x track with success rate, avg time, total launches
- Self-tuning: dynamic timeout already adapts; extend to auto-adjust retry count based on combo reliability
- No manual threshold tuning — all derived from data

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
