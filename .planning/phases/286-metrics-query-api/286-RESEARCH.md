# Phase 286: Metrics Query API - Research

**Researched:** 2026-04-01
**Domain:** Rust/Axum REST API — SQLite time-series query with auto-resolution selection
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** Time-series query returns `{ metric: string, pod: Option<u32>, resolution: string, points: Vec<{ts: i64, value: f64}> }` — Unix timestamps in seconds, f64 values
- **D-02:** Snapshot endpoint returns `{ metrics: Vec<{name: string, pod: Option<u32>, value: f64, updated_at: i64}> }` — flat array, one entry per metric per pod
- **D-03:** Names endpoint returns `{ names: Vec<string> }` — simple list, no metadata
- **D-04:** Invalid metric name or empty time range returns 200 with empty `points: []` — dashboards should not break on missing data
- **D-05:** Invalid query parameters (bad date format, negative range) return 400 with `{ error: string }` JSON body
- **D-06:** All new metrics query endpoints are staff-only (behind existing auth middleware in routes.rs)
- **D-07:** Resolution auto-selection: raw samples for ranges <24h, hourly rollups for 24h-7d, daily rollups for >7d. Client can override with `?resolution=raw|hourly|daily` query param.

### Claude's Discretion

- Exact SQL query structure and indexing strategy
- Whether to add a metrics_query.rs module or extend existing metrics.rs
- Pagination strategy (if needed — likely not for 7-day windows at 1-min resolution = 10K points max)
- Response caching headers

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| QAPI-01 | GET /api/v1/metrics/query returns time-series data filtered by metric name and time range | Query `metrics_samples` (raw) or `metrics_rollups` (hourly/daily) depending on auto-resolution; WHERE clauses on metric_name + recorded_at/period_start |
| QAPI-02 | GET /api/v1/metrics/names returns list of all known metric names | SELECT DISTINCT metric_name FROM metrics_samples UNION SELECT DISTINCT metric_name FROM metrics_rollups |
| QAPI-03 | GET /api/v1/metrics/snapshot returns current (latest) value for all metrics | SELECT metric_name, pod_id, value, recorded_at GROUP BY metric_name, pod_id ORDER BY recorded_at DESC — one row per metric+pod |
| QAPI-04 | Query API supports per-pod filtering (e.g., ?pod=3) | Optional WHERE pod_id = ? clause; pod_id stored as TEXT (e.g. "pod-3"), query param is integer pod number |
| QAPI-05 | Query API auto-selects resolution (raw for <24h, hourly for <7d, daily for >7d) | Branch on `to_ts - from_ts` duration; override via `?resolution=raw|hourly|daily` param |
</phase_requirements>

## Summary

Phase 286 adds three REST endpoints under `/api/v1/metrics/` that expose the TSDB created in Phase 285. The foundation is already in place: `metrics_samples` and `metrics_rollups` tables exist in the DB (verified in `db/mod.rs` lines 3572-3607), and the handler pattern in `api/metrics.rs` is well-established (Query params, State, dynamic WHERE, sqlx binding, Json response).

The key technical challenge is the auto-resolution logic (QAPI-05): the handler must compute the time range from query params, select the correct table (`metrics_samples` vs `metrics_rollups` with the appropriate resolution filter), and return a uniform response shape regardless of which table was queried. The snapshot endpoint (QAPI-03) requires a `DISTINCT ON`-equivalent in SQLite, which means using `GROUP BY metric_name, pod_id` with `MAX(recorded_at)` and a self-join or subquery to retrieve the corresponding value.

**Primary recommendation:** Add a new `api/metrics_query.rs` module (do not extend the existing `metrics.rs` which is already 400+ lines). Register three routes in the staff_routes section of `routes.rs` immediately after the existing metrics routes at lines 523-525.

## Standard Stack

### Core (all already in Cargo.toml — no new dependencies)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| axum | existing | Handler routing, extractors | Project standard |
| sqlx | existing | Async SQLite queries with binding | Project standard |
| serde | existing | Deserialize query params, serialize response | Project standard |
| chrono | existing | Timestamp arithmetic for resolution selection | Project standard |

**No new dependencies required.** This phase is purely additive Rust code using the existing stack.

## Architecture Patterns

### Recommended Project Structure

New file only:
```
crates/racecontrol/src/api/
├── metrics.rs          # existing — DO NOT EXTEND (already 400+ lines)
└── metrics_query.rs    # NEW — three handlers for QAPI-01/02/03
```

Plus two edits:
- `crates/racecontrol/src/api/mod.rs` — `pub mod metrics_query;`
- `crates/racecontrol/src/api/routes.rs` — add 3 routes in `staff_routes()` after line 525

### Pattern 1: Query Params Struct with Validation

Follows the exact same shape as `LaunchStatsParams` in `metrics.rs`:

```rust
// Source: crates/racecontrol/src/api/metrics.rs (verified in codebase)
#[derive(Debug, Deserialize)]
pub struct MetricsQueryParams {
    pub metric: String,
    pub from: i64,        // Unix timestamp seconds
    pub to: i64,          // Unix timestamp seconds
    pub pod: Option<u32>, // pod number (1-8); maps to "pod-N" text in DB
    pub resolution: Option<String>, // "raw" | "hourly" | "daily" | absent = auto
}
```

Validation in handler: if `from >= to` or `to - from <= 0`, return 400 with `{"error": "..."}`.

### Pattern 2: Auto-Resolution Selection (QAPI-05)

```rust
// Source: D-07 decision from CONTEXT.md + DB schema from db/mod.rs
fn select_resolution(from: i64, to: i64, override_res: Option<&str>) -> &'static str {
    if let Some(r) = override_res {
        return match r { "raw" | "hourly" | "daily" => r, _ => "raw" };
    }
    let range_secs = to - from;
    let day = 86_400_i64;
    if range_secs < day {
        "raw"
    } else if range_secs < 7 * day {
        "hourly"
    } else {
        "daily"
    }
}
```

Then branch the SQL:
- `"raw"` → query `metrics_samples` WHERE `recorded_at` is between `datetime(from, 'unixepoch')` and `datetime(to, 'unixepoch')`
- `"hourly"` / `"daily"` → query `metrics_rollups` WHERE `resolution = ?` AND `period_start` is between the two timestamps

### Pattern 3: SQLite Timestamp Handling

The DB stores timestamps as TEXT in ISO 8601 format (`datetime('now')` = `"2026-04-01T10:00:00"`). Query params arrive as Unix epoch integers. Convert for WHERE clauses:

```sql
-- Convert Unix epoch param to SQLite datetime string for comparison
WHERE recorded_at >= datetime(?, 'unixepoch')
  AND recorded_at < datetime(?, 'unixepoch')
  AND metric_name = ?
```

Return `ts` as Unix epoch in response by converting back:
```sql
SELECT metric_name, pod_id, value,
       strftime('%s', recorded_at) AS ts_epoch
FROM metrics_samples
WHERE ...
```

`strftime('%s', ...)` returns TEXT in SQLite — cast to i64 in Rust via `sqlx::query_as::<_, (String, Option<String>, f64, i64)>`.

### Pattern 4: Snapshot Query — Latest Value per metric+pod

SQLite does not have `DISTINCT ON`. Use a subquery:

```sql
SELECT s.metric_name, s.pod_id, s.value,
       CAST(strftime('%s', s.recorded_at) AS INTEGER) AS updated_at
FROM metrics_samples s
INNER JOIN (
    SELECT metric_name, pod_id, MAX(recorded_at) AS max_ts
    FROM metrics_samples
    GROUP BY metric_name, pod_id
) latest ON s.metric_name = latest.metric_name
         AND COALESCE(s.pod_id, '') = COALESCE(latest.pod_id, '')
         AND s.recorded_at = latest.max_ts
ORDER BY s.metric_name, s.pod_id
```

This is O(n) with the existing `idx_metrics_samples_lookup` index on `(metric_name, recorded_at)`.

### Pattern 5: Names Endpoint — UNION across both tables

```sql
SELECT DISTINCT metric_name FROM metrics_samples
UNION
SELECT DISTINCT metric_name FROM metrics_rollups
ORDER BY metric_name
```

Returns all known metric names even if raw samples have been purged but rollups remain.

### Pattern 6: pod_id Format Mapping

DB stores pod_id as TEXT `"pod-1"` through `"pod-8"` (matches existing pattern in `combo_reliability`, `launch_events`). Query param `?pod=3` is `u32`. Map: `format!("pod-{}", pod_number)`.

### Pattern 7: Error Response (D-05)

```rust
// Source: consistent with existing pattern in metrics.rs
return (
    axum::http::StatusCode::BAD_REQUEST,
    Json(serde_json::json!({"error": "from must be less than to"})),
).into_response();
```

For empty results (D-04), return 200 with `points: []` — no special handling needed since `fetch_all` returns empty vec on no rows.

### Route Registration

Insert in `staff_routes()` in `routes.rs` after line 525 (existing metrics routes):

```rust
// Source: crates/racecontrol/src/api/routes.rs lines 523-525 (verified)
.route("/metrics/launch-stats", get(metrics::launch_stats_handler))
.route("/metrics/billing-accuracy", get(metrics::billing_accuracy_handler))
.route("/admin/launch-matrix", get(metrics::launch_matrix_handler))
// Phase 286: TSDB query API
.route("/metrics/query", get(metrics_query::query_handler))
.route("/metrics/names", get(metrics_query::names_handler))
.route("/metrics/snapshot", get(metrics_query::snapshot_handler))
```

**Route uniqueness check:** Run `grep -n 'metrics/query\|metrics/names\|metrics/snapshot' crates/racecontrol/src/api/routes.rs` before adding — must return 0 hits.

### Anti-Patterns to Avoid

- **Do NOT put new routes in `public_routes()`** — D-06 locks these as staff-only. Business intelligence data (revenue, session counts) must not be public.
- **Do NOT use `strftime('%s', ...)` without CAST to INTEGER** — SQLite returns TEXT from strftime; sqlx will fail to decode into `i64` without explicit CAST.
- **Do NOT use `unwrap()` on query results** — use `fetch_all(...).await.unwrap_or_default()` for lists, `?` operator for critical failures.
- **Do NOT extend `metrics.rs`** — it's already 400+ lines. A new module keeps concerns separated and avoids merge conflicts with Phase 287 dashboard work.
- **Do NOT hardcode the pod count (8)** — query from DB dynamically; pod count may change.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| SQLite "latest per group" | Custom Rust grouping | SQL self-join with MAX() subquery | All data stays in DB; no 10K row fetch into Rust memory |
| Timestamp conversion | chrono timezone math | SQLite `datetime(?, 'unixepoch')` and `strftime('%s', col)` | DB handles it natively; no chrono import needed in new module |
| Resolution threshold math | Floating point duration comparison | Integer seconds arithmetic (`to - from < 86400`) | Simpler, no precision issues |
| Response pagination | Cursor/offset logic | None needed — max points is 7d/1min = 10,080 raw samples | Within SQLite single-query budget; skip pagination complexity |

## DB Schema (Verified from db/mod.rs lines 3572-3607)

```sql
-- Raw samples (Phase 285)
CREATE TABLE metrics_samples (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    metric_name TEXT    NOT NULL,
    pod_id      TEXT,                    -- NULL = server-level metric; "pod-N" = pod metric
    value       REAL    NOT NULL,
    recorded_at TEXT    NOT NULL DEFAULT (datetime('now'))  -- ISO 8601 UTC
);
CREATE INDEX idx_metrics_samples_lookup ON metrics_samples(metric_name, recorded_at);

-- Rollups (Phase 285)
CREATE TABLE metrics_rollups (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    resolution   TEXT    NOT NULL CHECK(resolution IN ('hourly', 'daily')),
    metric_name  TEXT    NOT NULL,
    pod_id       TEXT,
    min_value    REAL    NOT NULL,
    max_value    REAL    NOT NULL,
    avg_value    REAL    NOT NULL,
    sample_count INTEGER NOT NULL,
    period_start TEXT    NOT NULL,       -- ISO 8601 UTC
    UNIQUE(resolution, metric_name, pod_id, period_start)
);
CREATE INDEX idx_metrics_rollups_lookup ON metrics_rollups(resolution, metric_name, period_start);
```

**Key observations:**
1. `pod_id` is nullable TEXT — `NULL` means server-level metric. Map response `pod: Option<u32>` by parsing "pod-N" back to N.
2. `recorded_at` and `period_start` are TEXT ISO 8601. Use `datetime(?, 'unixepoch')` for param binding and `strftime('%s', col)` for retrieval.
3. Rollups store `avg_value` — use this as the `value` field in query response for hourly/daily resolution.
4. The rollup table uses a single table with `resolution` discriminator, not separate `metrics_hourly`/`metrics_daily` tables. The query must bind `resolution = ?` to select the right tier.

## Common Pitfalls

### Pitfall 1: strftime Returns TEXT, Not INTEGER
**What goes wrong:** `sqlx::query_as::<_, (i64,)>("SELECT strftime('%s', col) ...")` panics at runtime with decode error.
**Why it happens:** SQLite's `strftime('%s', ...)` always returns TEXT. sqlx tries to decode into i64 and fails.
**How to avoid:** Use `CAST(strftime('%s', recorded_at) AS INTEGER)` in SQL, or bind as `(String,)` and parse in Rust.
**Warning signs:** Compile succeeds, test fails with "ColumnDecode" error.

### Pitfall 2: NULL pod_id in GROUP BY / COALESCE Required
**What goes wrong:** Snapshot query groups `NULL` pod_id as a single group — two server-level metrics with the same name and both `pod_id = NULL` appear as separate rows in the INNER JOIN if not handled.
**Why it happens:** In SQLite, `NULL = NULL` is false — the join condition `s.pod_id = latest.pod_id` fails when both are NULL.
**How to avoid:** Use `COALESCE(s.pod_id, '') = COALESCE(latest.pod_id, '')` in the join condition (verified pattern from `query_alternatives` in metrics.rs line 284).
**Warning signs:** Snapshot returns duplicate entries for server-level metrics.

### Pitfall 3: Duplicate Route Registration Causes Runtime Panic
**What goes wrong:** Server panics at startup with "Axum: route conflict" if any of the three new routes already exist elsewhere in routes.rs.
**Why it happens:** Axum validates route uniqueness at router build time (runtime, not compile time).
**How to avoid:** Run `grep -n 'metrics/query\|metrics/names\|metrics/snapshot' crates/racecontrol/src/api/routes.rs` before adding routes. Must return 0 hits. Also run `cargo test -p racecontrol` — `route_uniqueness_tests::no_duplicate_route_registrations` catches this.
**Warning signs:** Server exits immediately on startup with "called `Result::unwrap()` on an `Err`" containing route path.

### Pitfall 4: Resolution Boundary Off-by-One
**What goes wrong:** A 24h range query (`to - from = 86400`) returns hourly rollups but the user expected raw samples (or vice versa).
**Why it happens:** D-07 says "raw for <24h" — the `<` is strict. A range of exactly 86400 seconds should get hourly rollups.
**How to avoid:** Use strict `<` for raw threshold: `if range_secs < 86_400 { "raw" } else if range_secs < 7 * 86_400 { "hourly" } else { "daily" }`. Document this in a comment.

### Pitfall 5: pod_id String Format Mismatch
**What goes wrong:** `?pod=3` is parsed as `u32` but DB stores `"pod-3"`. Direct bind of `3` returns 0 rows.
**Why it happens:** The DB TEXT format "pod-N" is used consistently across all tables (launch_events, combo_reliability, metrics_samples).
**How to avoid:** Always convert: `let pod_id_str = format!("pod-{}", pod_number);` before binding.

## Code Examples

### Handler Skeleton (QAPI-01)

```rust
// Source: pattern from crates/racecontrol/src/api/metrics.rs (verified)
pub async fn query_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<MetricsQueryParams>,
) -> impl IntoResponse {
    // D-05: validate params
    if params.from >= params.to {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "from must be less than to"})),
        ).into_response();
    }

    let resolution = select_resolution(params.from, params.to, params.resolution.as_deref());
    let pod_id_str = params.pod.map(|n| format!("pod-{}", n));

    let points: Vec<TimePoint> = match resolution {
        "raw" => fetch_raw_samples(&state.db, &params.metric, params.from, params.to, pod_id_str.as_deref()).await,
        _ => fetch_rollup_samples(&state.db, &params.metric, resolution, params.from, params.to, pod_id_str.as_deref()).await,
    };

    let response = QueryResponse {
        metric: params.metric,
        pod: params.pod,
        resolution: resolution.to_string(),
        points,
    };
    Json(serde_json::to_value(&response).unwrap_or_default()).into_response()
}
```

### Raw Sample Fetch

```rust
// Uses datetime(?, 'unixepoch') for param conversion — no chrono needed
async fn fetch_raw_samples(
    db: &SqlitePool,
    metric: &str,
    from: i64,
    to: i64,
    pod_id: Option<&str>,
) -> Vec<TimePoint> {
    let mut q_str = "SELECT CAST(strftime('%s', recorded_at) AS INTEGER), value
                     FROM metrics_samples
                     WHERE metric_name = ?
                       AND recorded_at >= datetime(?, 'unixepoch')
                       AND recorded_at < datetime(?, 'unixepoch')".to_string();
    if pod_id.is_some() {
        q_str.push_str(" AND pod_id = ?");
    } else {
        q_str.push_str(" AND pod_id IS NULL");
    }
    q_str.push_str(" ORDER BY recorded_at ASC");

    let mut q = sqlx::query_as::<_, (i64, f64)>(&q_str)
        .bind(metric)
        .bind(from)
        .bind(to);
    if let Some(pid) = pod_id {
        q = q.bind(pid);
    }
    q.fetch_all(db).await
        .unwrap_or_default()
        .into_iter()
        .map(|(ts, value)| TimePoint { ts, value })
        .collect()
}
```

### Response Types

```rust
#[derive(Debug, Serialize)]
pub struct TimePoint {
    pub ts: i64,       // Unix epoch seconds
    pub value: f64,
}

#[derive(Debug, Serialize)]
pub struct QueryResponse {
    pub metric: String,
    pub pod: Option<u32>,
    pub resolution: String,
    pub points: Vec<TimePoint>,
}

#[derive(Debug, Serialize)]
pub struct SnapshotEntry {
    pub name: String,
    pub pod: Option<u32>,
    pub value: f64,
    pub updated_at: i64,   // Unix epoch seconds
}

#[derive(Debug, Serialize)]
pub struct SnapshotResponse {
    pub metrics: Vec<SnapshotEntry>,
}

#[derive(Debug, Serialize)]
pub struct NamesResponse {
    pub names: Vec<String>,
}
```

## Project Constraints (from CLAUDE.md)

- No `.unwrap()` in production Rust — use `?`, `.ok()`, or `.unwrap_or_default()`
- No `any` in TypeScript (not applicable here — Rust only)
- Static CRT already configured in `.cargo/config.toml` — no change needed
- Route uniqueness: verify with `grep` before adding routes; `route_uniqueness_tests::no_duplicate_route_registrations` test must pass
- Deploy: `touch crates/racecontrol/build.rs` before `cargo build --release` after committing
- Git config: `user.name="James Vowles"`, `user.email="james@racingpoint.in"`
- LOGBOOK.md: update on every commit

## Environment Availability

Step 2.6: SKIPPED — no external dependencies. This phase adds Rust code that queries existing SQLite tables using the existing sqlx pool. No new tools, services, or CLIs required.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust `#[tokio::test]` with `sqlx::sqlite::SqlitePoolOptions` in-memory DB |
| Config file | None — inline in test module |
| Quick run command | `cargo test -p racecontrol metrics_query` |
| Full suite command | `cargo test -p racecontrol` |

### Phase Requirements to Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| QAPI-01 | query returns time-series points for valid metric + range | unit | `cargo test -p racecontrol test_query_raw_samples` | No — Wave 0 |
| QAPI-01 | query returns empty points for unknown metric (D-04) | unit | `cargo test -p racecontrol test_query_unknown_metric_returns_empty` | No — Wave 0 |
| QAPI-01 | query returns 400 for invalid range (D-05) | unit | `cargo test -p racecontrol test_query_invalid_range_returns_400` | No — Wave 0 |
| QAPI-02 | names returns all distinct metric names | unit | `cargo test -p racecontrol test_names_distinct` | No — Wave 0 |
| QAPI-03 | snapshot returns latest value per metric+pod | unit | `cargo test -p racecontrol test_snapshot_latest_per_group` | No — Wave 0 |
| QAPI-04 | pod filter applied to query and snapshot | unit | `cargo test -p racecontrol test_pod_filter` | No — Wave 0 |
| QAPI-05 | auto-resolution selects raw for <24h | unit | `cargo test -p racecontrol test_resolution_raw` | No — Wave 0 |
| QAPI-05 | auto-resolution selects hourly for 24h-7d | unit | `cargo test -p racecontrol test_resolution_hourly` | No — Wave 0 |
| QAPI-05 | auto-resolution selects daily for >7d | unit | `cargo test -p racecontrol test_resolution_daily` | No — Wave 0 |
| QAPI-05 | resolution override param respected | unit | `cargo test -p racecontrol test_resolution_override` | No — Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test -p racecontrol metrics_query`
- **Per wave merge:** `cargo test -p racecontrol`
- **Phase gate:** Full suite green + route uniqueness test passes before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `crates/racecontrol/src/api/metrics_query.rs` — the new module with handlers AND inline `#[cfg(test)]` block covering all 10 test cases above
- [ ] `crates/racecontrol/src/api/mod.rs` — `pub mod metrics_query;` line
- [ ] No new test infrastructure needed — existing `sqlx::sqlite::SqlitePoolOptions::new().connect("sqlite::memory:")` pattern (from `metrics.rs` tests) works as-is

## Open Questions

1. **NULL pod_id vs server-level metrics in snapshot**
   - What we know: `pod_id IS NULL` means server-level metric (CPU, revenue, WS connections). When no `?pod=` filter is provided, snapshot should return both server-level (pod=null) and per-pod metrics.
   - What's unclear: Should the snapshot response include server-level metrics when `?pod=3` filter is applied? Logically no — but QAPI-04 says "return only that pod's data."
   - Recommendation: When `?pod=N` is specified, only return rows where `pod_id = 'pod-N'`. When no pod filter, return all rows including `pod_id IS NULL`. Document this in handler comments.

2. **Rollup `avg_value` as the response `value` field**
   - What we know: `metrics_rollups` stores `min_value`, `max_value`, `avg_value`. D-01 only specifies `value: f64` per point.
   - What's unclear: Should hourly/daily points return avg, min, or max?
   - Recommendation: Use `avg_value` as the canonical `value`. This is consistent with time-series dashboard convention (Phase 287 will likely need this). No need to expose min/max at this layer — that's a v2 enhancement.

## Sources

### Primary (HIGH confidence)
- Verified directly from codebase: `crates/racecontrol/src/db/mod.rs` lines 3572-3607 — exact table schema
- Verified directly from codebase: `crates/racecontrol/src/api/metrics.rs` — handler pattern, test infrastructure
- Verified directly from codebase: `crates/racecontrol/src/api/routes.rs` lines 297-525 — staff_routes structure, existing metrics routes
- Verified directly from codebase: `286-CONTEXT.md` — locked decisions D-01 through D-07

### Secondary (MEDIUM confidence)
- SQLite `strftime('%s', ...)` returns TEXT — standard SQLite behavior, confirmed by pattern in codebase using CAST

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new dependencies; existing stack verified in codebase
- Architecture: HIGH — DB schema verified, handler pattern verified, route locations verified
- Pitfalls: HIGH — strftime/TEXT issue and NULL pod_id COALESCE are verified patterns from existing codebase code

**Research date:** 2026-04-01
**Valid until:** 2026-05-01 (stable codebase, no fast-moving dependencies)
