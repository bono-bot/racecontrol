# Phase 286: Metrics Query API - Context

**Gathered:** 2026-04-01
**Status:** Ready for planning
**Mode:** Auto-generated (--auto mode, infrastructure API phase)

<domain>
## Phase Boundary

REST API endpoints for querying time-series metric data from the TSDB created in Phase 285. Five requirements: query by name+time range (QAPI-01), list metric names (QAPI-02), current snapshot (QAPI-03), per-pod filtering (QAPI-04), auto-resolution selection (QAPI-05). All endpoints under `/api/v1/metrics/` namespace alongside existing launch-stats and billing-accuracy endpoints.

</domain>

<decisions>
## Implementation Decisions

### Response Format
- **D-01:** Time-series query returns `{ metric: string, pod: Option<u32>, resolution: string, points: Vec<{ts: i64, value: f64}> }` — Unix timestamps in seconds, f64 values
- **D-02:** Snapshot endpoint returns `{ metrics: Vec<{name: string, pod: Option<u32>, value: f64, updated_at: i64}> }` — flat array, one entry per metric per pod
- **D-03:** Names endpoint returns `{ names: Vec<string> }` — simple list, no metadata

### Error Handling
- **D-04:** Invalid metric name or empty time range returns 200 with empty `points: []` — dashboards should not break on missing data
- **D-05:** Invalid query parameters (bad date format, negative range) return 400 with `{ error: string }` JSON body

### Authentication
- **D-06:** All new metrics query endpoints are staff-only (behind existing auth middleware in routes.rs) — metrics data reveals business intelligence (revenue, session counts)

### Auto-Resolution Logic
- **D-07:** Resolution auto-selection: raw samples for ranges <24h, hourly rollups for 24h-7d, daily rollups for >7d. Client can override with `?resolution=raw|hourly|daily` query param.

### Claude's Discretion
- Exact SQL query structure and indexing strategy
- Whether to add a metrics_query.rs module or extend existing metrics.rs
- Pagination strategy (if needed — likely not for 7-day windows at 1-min resolution = 10K points max)
- Response caching headers

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Existing API Patterns
- `crates/racecontrol/src/api/metrics.rs` — Existing metrics endpoints (launch-stats, billing-accuracy) — follow same handler structure
- `crates/racecontrol/src/api/routes.rs` — Route registration pattern, auth middleware placement (lines ~523-524 for existing metrics routes)

### Phase 285 Foundation
- `.planning/phases/285-metrics-ring-buffer/285-CONTEXT.md` — TSDB schema decisions, table names, established patterns
- `.planning/phases/285-metrics-ring-buffer/` — Plans for the TSDB that this API queries

### Requirements
- `.planning/REQUIREMENTS.md` — QAPI-01 through QAPI-05 definitions

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/racecontrol/src/api/metrics.rs` — Handler pattern with `Query<Params>`, `State<Arc<AppState>>`, dynamic WHERE clauses
- `crates/racecontrol/src/state.rs` — AppState with `db: SqlitePool` field
- `crates/racecontrol/src/api/routes.rs` — Staff-only route group (lines ~500+)

### Established Patterns
- All API handlers use `axum::extract::{Query, State}` with `Arc<AppState>`
- JSON responses via `axum::Json` wrapper
- Query params via `serde::Deserialize` structs
- Error responses use `impl IntoResponse` with status codes
- Existing metrics routes at `/api/v1/metrics/launch-stats` and `/api/v1/metrics/billing-accuracy`

### Integration Points
- New routes added to staff-only section of `routes.rs` under `/api/v1/metrics/` prefix
- Queries hit the `metrics_samples`, `metrics_hourly`, `metrics_daily` tables from Phase 285
- Snapshot reads latest value per metric from `metrics_samples` (or a maintained latest-value cache)

</code_context>

<specifics>
## Specific Ideas

No specific requirements beyond QAPI-01 through QAPI-05 — standard REST API implementation following existing patterns.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 286-metrics-query-api*
*Context gathered: 2026-04-01*
