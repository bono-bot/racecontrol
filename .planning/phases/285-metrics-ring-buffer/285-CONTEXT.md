# Phase 285: Metrics Ring Buffer - Context

**Gathered:** 2026-04-01
**Status:** Ready for planning
**Mode:** Auto-generated (infrastructure phase — discuss skipped)

<domain>
## Phase Boundary

Server persistently stores time-series metric data with automatic rollups and bounded storage. New `metrics_tsdb` SQLite tables alongside existing DB. Captures CPU, GPU temp, FPS, billing revenue, WS connections, pod health score, game sessions at 1-min resolution. Hourly/daily rollups. 7-day raw retention, 90-day rollup retention. Async ingestion that never blocks the main event loop.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase. Key patterns to follow:
- Use existing `db::init_pool` / `migrate()` pattern for table creation (CREATE TABLE IF NOT EXISTS in db/mod.rs)
- SQLite WAL mode already enabled — extend the existing pool, don't create a separate DB file
- Use `tokio::spawn` with mpsc channel for async batched writes (pattern used in fleet_kb.rs)
- Rollup computation via scheduled tokio task (pattern used in business_aggregator.rs)
- Purge via DELETE WHERE timestamp < threshold (standard SQLite pattern)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/racecontrol/src/db/mod.rs` — SQLite pool init, WAL mode, migration pattern
- `crates/racecontrol/src/metrics.rs` — existing launch event recording (different domain, reusable pattern)
- `crates/racecontrol/src/api/metrics.rs` — existing metrics API (launch stats, billing accuracy)
- `crates/racecontrol/src/alert_engine.rs` — existing business alert engine with WhatsApp channel
- `crates/racecontrol/src/business_aggregator.rs` — daily business metrics aggregation
- `crates/racecontrol/src/telemetry_store.rs` — telemetry persistence pattern

### Established Patterns
- All tables use `CREATE TABLE IF NOT EXISTS` in db/mod.rs `migrate()` function
- SQLite WAL mode with busy_timeout=5000ms, synchronous=NORMAL
- sqlx for all DB operations (no raw rusqlite)
- chrono::Utc for timestamps
- tokio::spawn for background tasks with mpsc channels
- serde Serialize/Deserialize on all data structs

### Integration Points
- New `metrics_tsdb.rs` module added to `crates/racecontrol/src/`
- Tables added to `db/mod.rs` `migrate()` function
- Background ingestion task spawned from `main.rs`
- Metrics data fed from: WS pod status updates, billing events, fleet health loop

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase. Refer to ROADMAP phase description and success criteria.

</specifics>

<deferred>
## Deferred Ideas

None — discuss phase skipped.

</deferred>
