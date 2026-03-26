# Phase 200: Self-Improving Intelligence - Research

**Researched:** 2026-03-26
**Domain:** Rust/SQLite analytics — combo reliability scoring, warning injection, alternatives API, admin matrix
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
None — all implementation choices are Claude's discretion.

### Claude's Discretion
- Combo reliability score formula (success_rate over last N launches)
- Warning threshold (e.g., <70% success rate)
- Alternative suggestion algorithm (same game, different car/track with higher reliability)
- Rolling window size and self-tuning approach
- Admin API endpoint design for launch matrix
- Where to store/cache computed scores (in-memory vs per-query)

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| INTEL-01 | Combo reliability score computed from launch_events per (pod_id, sim_type, car, track) over 30-day rolling window — minimum 5 launches before scoring activates | New `query_combo_reliability()` in metrics.rs using GROUP BY on existing launch_events table |
| INTEL-02 | Warning injected into POST /api/v1/games/launch response when combo reliability < 70% (with minimum 5 launches) | Inject after billing gate in `launch_game()` in game_launcher.rs — query reliability, include `"warning"` field in HTTP response JSON |
| INTEL-03 | GET /api/v1/games/alternatives?game=&car=&track=&pod= returns top 3 combos with same sim, higher reliability (>90%), sorted by success_rate DESC; preference for same-car or same-track combos | New handler in api/metrics.rs or api/routes.rs, new route in routes.rs |
| INTEL-04 | GET /api/v1/admin/launch-matrix?game= returns per-pod grid with pod_id, total_launches, success_rate, avg_time_ms, top_3_failure_modes, flagged boolean (<70%) | New handler added to api/metrics.rs, new route in routes.rs admin section |
| INTEL-05 | Auto-tuning: dynamic timeout already adapts from historical data (Phase 197); extend to auto-adjust retry count — if combo reliability < 50%, increase max auto_relaunch_count from 2 to 3 | Inject into launch_game() after reliability check; pass adjusted retry cap into GameTracker |
</phase_requirements>

## Summary

Phase 200 is a pure server-side analytics layer built on top of the `launch_events` SQLite table established in Phase 195. The table schema is fully defined — `(id, pod_id, sim_type, car, track, session_type, timestamp, outcome, error_taxonomy, duration_to_playable_ms, error_details, launch_args_hash, attempt_number, created_at)` — with a composite index already on `(pod_id, sim_type, car, track)`. All intelligence is derived from GROUP BY aggregate queries on this single table; no new tables are required.

The implementation follows two parallel tracks: (1) new query functions added to `metrics.rs` that compute reliability scores, find alternatives, and aggregate per-pod stats, and (2) injection points in `game_launcher.rs` (warning at launch time) and `api/routes.rs` or `api/metrics.rs` (two new GET endpoints). The `launch_game()` function in `game_launcher.rs` is called through `handle_dashboard_command()` from the HTTP handler in `routes.rs`; the HTTP layer in `routes.rs` must be modified to expose the warning in its JSON response since `launch_game()` currently returns `Result<(), String>`.

The decision on where to compute reliability scores is: **per-query, no in-memory cache**. The existing patterns (`query_dynamic_timeout`, `query_best_recovery_action`) are all per-query with no caching, the SQLite WAL mode and indexes make these queries fast (<5ms on expected data volumes), and cached state introduces staleness bugs between launches.

**Primary recommendation:** Add `query_combo_reliability()` and `query_alternatives()` to `metrics.rs`, modify the HTTP `launch_game` handler in `routes.rs` to call reliability check and include warning in response, add two GET handlers to `api/metrics.rs`, and register both new routes in `routes.rs`.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| sqlx | existing | SQLite GROUP BY aggregate queries | Already in use for all existing metrics queries |
| serde / serde_json | existing | JSON serialization of response types | All existing API handlers use this pattern |
| axum | existing | HTTP handler registration | All routes follow `State<Arc<AppState>>` + `Query<Params>` pattern |
| chrono | existing | `datetime('now', '-30 days')` timestamp in SQL | Used in all existing rolling-window queries |

No new dependencies required. This phase uses only what is already in Cargo.toml.

## Architecture Patterns

### Recommended File Structure for Changes

```
crates/racecontrol/src/
├── metrics.rs                  # ADD: query_combo_reliability(), query_alternatives()
├── game_launcher.rs            # MODIFY: launch_game() - call reliability, adjust retry cap
├── api/
│   ├── metrics.rs              # ADD: alternatives_handler(), launch_matrix_handler()
│   └── routes.rs               # MODIFY: HTTP launch_game handler (expose warning in response)
│                               # MODIFY: register 2 new routes
└── db/mod.rs                   # ADD: CREATE TABLE combo_reliability (per success criteria INTEL-01)
```

### Pattern 1: Rolling-Window Aggregate Query (established in metrics.rs)

**What:** SQLite aggregate query with 30-day datetime filter and GROUP BY combo key.

**When to use:** All reliability score lookups — this mirrors `query_dynamic_timeout` and `query_best_recovery_action` exactly.

**Example (from existing `query_best_recovery_action`):**
```rust
// Source: crates/racecontrol/src/metrics.rs:268
let rows: Vec<(String, i64, i64)> = sqlx::query_as(
    "SELECT recovery_action_tried,
            COUNT(*) as total,
            SUM(CASE WHEN recovery_outcome='\"Success\"' THEN 1 ELSE 0 END) as successes
     FROM recovery_events
     WHERE pod_id = ? AND sim_type = ? AND failure_mode = ?
       AND created_at > datetime('now', '-30 days')
     GROUP BY recovery_action_tried
     ORDER BY (...) DESC
     LIMIT 1",
)
.bind(pod_id).bind(sim_type).bind(failure_mode)
.fetch_all(db).await.unwrap_or_default();
```

**New `query_combo_reliability()` will follow the same shape:**
```rust
// CRITICAL: outcome is stored as JSON-serialized enum: '"Success"' (with surrounding quotes)
// See metrics.rs:277 — CASE WHEN recovery_outcome='\"Success\"' — same pattern applies to outcome
pub async fn query_combo_reliability(
    db: &SqlitePool,
    pod_id: &str,
    sim_type: &str,
    car: Option<&str>,
    track: Option<&str>,
) -> Option<ComboReliability> {
    // Returns None when total_launches < 5 (minimum threshold — INTEL-01, INTEL-02 min-launches guard)
}
```

**Pitfall — the outcome serialization:** `LaunchOutcome::Success` serializes to `"Success"` (JSON string with quotes). The SQL CASE WHEN must match `'\"Success\"'` (the literal including the JSON quotes). This is already handled correctly in `query_best_recovery_action` and `launch_stats_handler` — copy that exact pattern.

### Pattern 2: HTTP Handler with Query Params (established in api/metrics.rs)

**What:** Axum handler extracting `Query<ParamsStruct>` + `State<Arc<AppState>>`, returning `Json<Value>`.

**Example (from existing `launch_stats_handler`):**
```rust
// Source: crates/racecontrol/src/api/metrics.rs:42
pub async fn launch_stats_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<LaunchStatsParams>,
) -> impl IntoResponse {
    // ...
    Json(serde_json::to_value(&response).unwrap_or_default())
}
```

Both `alternatives_handler` and `launch_matrix_handler` follow this exact pattern.

### Pattern 3: Warning Injection in HTTP Launch Handler

The HTTP `launch_game` handler in `routes.rs` line 3588 calls `game_launcher::handle_dashboard_command()` and returns `Json(json!({ "ok": true }))` on success. The internal `launch_game()` in `game_launcher.rs` returns `Result<(), String>` — it has no mechanism to return structured data back through the handler.

**Approach:** Modify the HTTP `launch_game` handler in `routes.rs` to call `metrics::query_combo_reliability()` directly (before dispatching to `handle_dashboard_command`), then include the warning in the HTTP response JSON. The internal game_launcher function does not need to change.

```rust
// In routes.rs, HTTP launch_game handler (around line 3641):
// 1. Parse pod_id, sim_type, car, track from body (already done for duration_minutes inject)
// 2. Call query_combo_reliability(&state.db, pod_id, sim_type, car, track).await
// 3. Build warning string if reliability < 0.70 and total >= 5
// 4. After handle_dashboard_command succeeds:
//    let mut resp = json!({ "ok": true });
//    if let Some(w) = warning { resp["warning"] = json!(w); }
//    Json(resp)
```

### Pattern 4: combo_reliability Table (INTEL-01 schema requirement)

The success criteria state: `SELECT * FROM combo_reliability WHERE game='assetto_corsa' AND pod='pod-8'` must show rows. This requires a **materialized `combo_reliability` table** updated after every launch.

**Two options:**
- Option A: Pure per-query (no table) — fast, but violates the explicit `SELECT * FROM combo_reliability` success criterion
- Option B: Materialized table updated after each `record_launch_event` call — satisfies success criterion exactly

**Use Option B.** Add a `combo_reliability` table to `db/mod.rs` migration and update it via `INSERT OR REPLACE` after each launch event is recorded.

**Schema:**
```sql
CREATE TABLE IF NOT EXISTS combo_reliability (
    combo_hash TEXT NOT NULL,       -- sha256 or simple concat key
    pod_id TEXT NOT NULL,
    sim_type TEXT NOT NULL,
    car TEXT,
    track TEXT,
    success_rate REAL NOT NULL,
    avg_time_to_track_ms REAL,
    p95_time_to_track_ms REAL,
    total_launches INTEGER NOT NULL,
    common_failure_modes TEXT,      -- JSON array of {mode, count}
    last_updated TEXT NOT NULL,
    PRIMARY KEY (pod_id, sim_type, car, track)
);
CREATE INDEX IF NOT EXISTS idx_combo_reliability_sim ON combo_reliability(sim_type);
```

**Update trigger:** After `record_launch_event()` completes, call `update_combo_reliability()` which recomputes the row from the last 30 days of launch_events and UPSERTs into combo_reliability. This keeps the table always fresh.

### Pattern 5: Alternatives Query — Same-Game Preference

**Algorithm:**
1. Query combo_reliability WHERE sim_type = target AND pod_id = target AND success_rate > 0.90 AND total_launches >= 5
2. Order by: (car = target_car OR track = target_track) DESC, success_rate DESC
3. LIMIT 3

This naturally surfaces same-car-different-track and same-track-different-car combos first, satisfying INTEL-03 ALTERNATIVES SIMILARITY criterion.

```sql
SELECT car, track, success_rate, avg_time_to_track_ms, total_launches
FROM combo_reliability
WHERE sim_type = ?
  AND pod_id = ?
  AND success_rate > 0.90
  AND total_launches >= 5
  AND NOT (car IS ? AND track IS ?)  -- exclude the problem combo itself
ORDER BY
  (CASE WHEN car = ? OR track = ? THEN 1 ELSE 0 END) DESC,
  success_rate DESC
LIMIT 3
```

### Pattern 6: Admin Launch Matrix

**Query:** One row per pod for a given game (sim_type). Aggregates across all car/track combos per pod.

```sql
SELECT
    pod_id,
    COUNT(*) as total_launches,
    (SUM(CASE WHEN outcome = '"Success"' THEN 1 ELSE 0 END) * 1.0 / COUNT(*)) as success_rate,
    AVG(CAST(duration_to_playable_ms AS REAL)) as avg_time_ms,
    (SUM(CASE WHEN outcome = '"Success"' THEN 1 ELSE 0 END) * 1.0 / COUNT(*)) < 0.70 as flagged
FROM launch_events
WHERE sim_type = ?
  AND created_at >= datetime('now', '-30 days')
GROUP BY pod_id
ORDER BY pod_id
```

For `top_3_failure_modes`: run a separate query per pod (or subquery) against the same `launch_events` table.

### Anti-Patterns to Avoid

- **In-memory reliability cache:** The existing code never caches query results — all metrics are fresh per-call. Adding a cache introduces a new bug surface (staleness, invalidation). Use per-query or the materialized table pattern instead.
- **Dynamic SQL string interpolation for binds:** The existing `launch_stats_handler` builds WHERE clauses dynamically using `format!()` with positional `?{n}` placeholders. Do not interpolate user values directly into SQL strings. Follow the existing bound-parameter approach.
- **Assume outcome = 'Success' (unquoted):** The enum serializes as JSON string: `"Success"` (with double-quotes). The SQL literal must be `'"Success"'` or `'\"Success\"'`. This is a known project pitfall already fixed in `query_best_recovery_action`.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Reliability score formula | Custom weighted decay algorithm | Simple `success_count / total_count` over 30-day window | Sufficient for staff warning; complexity adds no value at current data volumes |
| P95 calculation | Statistical library | Fetch sorted durations, index at 95th percentile (existing pattern in launch_stats_handler) | Already proven in codebase |
| Combo hash key | SHA256 of concatenated fields | SQLite composite PRIMARY KEY on (pod_id, sim_type, car, track) | SQL handles null handling naturally; hash doesn't |
| Alternatives ranking | ML similarity model | SQL ORDER BY with same-car/same-track preference | Deterministic, auditable, zero dependencies |

**Key insight:** All intelligence for this phase is aggregation over existing data. The complexity is in the SQL, not in application logic.

## Common Pitfalls

### Pitfall 1: outcome Enum Serialization Mismatch
**What goes wrong:** SQL CASE WHEN checks `outcome = 'Success'` (no quotes) and always returns 0 successes — reliability is always 0.0.
**Why it happens:** `LaunchOutcome::Success` serializes via serde_json as `"Success"` (a JSON string literal with surrounding double-quotes). SQLite stores it verbatim including the quotes.
**How to avoid:** Use `outcome = '"Success"'` in SQL (SQL single-quotes wrapping the JSON double-quoted string). See `query_best_recovery_action` and `launch_stats_handler` — both use this exact pattern.
**Warning signs:** success_rate always 0.0 despite confirmed successful launches in the table.

### Pitfall 2: Minimum Launch Guard Missing
**What goes wrong:** A combo with 1 launch and 0 successes (0% rate) triggers a warning on the very next launch attempt. This is noise, not signal.
**Why it happens:** The minimum threshold (5 launches per CONTEXT.md, INTEL-02) is not applied.
**How to avoid:** In `query_combo_reliability()`, return `None` when `total_launches < 5`. The warning injection code treats `None` as "no warning."
**Warning signs:** Warning fires for brand-new combos never launched before.

### Pitfall 3: launch_game() Return Type Cannot Carry Warning
**What goes wrong:** Developer adds warning to `game_launcher::launch_game()` return type, forcing changes to `relaunch_game()`, `handle_dashboard_command()`, and all test callsites.
**Why it happens:** The internal `launch_game()` in `game_launcher.rs` returns `Result<(), String>`. Adding warning data would require a new return type.
**How to avoid:** Compute the reliability warning in the HTTP handler in `routes.rs` (before dispatching to `handle_dashboard_command`), not inside the internal launcher function.
**Warning signs:** Cascading compile errors across `relaunch_game`, test helpers, and `handle_dashboard_command`.

### Pitfall 4: NULL Handling in Composite Key
**What goes wrong:** SQL WHERE `car = ?` with a NULL bind returns no rows (NULL != NULL in SQL).
**Why it happens:** Some launches have no car/track (sim-only modes). SQLite `car = NULL` is always false.
**How to avoid:** Use `(car = ? OR (car IS NULL AND ? IS NULL))` pattern, or use `IS` operator for nullable fields. The existing `query_dynamic_timeout` uses `(car = ? OR ? IS NULL)` — follow that exact pattern.
**Warning signs:** Combos with NULL car never match despite history existing.

### Pitfall 5: combo_reliability Table Not Updated After Crash Recovery Launches
**What goes wrong:** `relaunch_game()` calls `record_launch_event()` for crash recovery attempts, but the `update_combo_reliability()` call is only wired in the normal launch path.
**Why it happens:** The relaunch path in `game_launcher.rs` is separate from the HTTP launch path.
**How to avoid:** `update_combo_reliability()` must be called from within `record_launch_event()` itself (or immediately after it), not from the launch call site.
**Warning signs:** combo_reliability.total_launches does not match COUNT(*) from launch_events.

## Code Examples

### New `ComboReliability` struct in metrics.rs
```rust
// Following existing struct patterns in metrics.rs
#[derive(Debug, Clone, Serialize)]
pub struct ComboReliability {
    pub combo_hash: String,       // composite: pod_id:sim_type:car:track
    pub pod_id: String,
    pub sim_type: String,
    pub car: Option<String>,
    pub track: Option<String>,
    pub success_rate: f64,        // 0.0 - 1.0
    pub avg_time_to_track_ms: Option<f64>,
    pub p95_time_to_track_ms: Option<f64>,
    pub total_launches: i64,
    pub common_failure_modes: Vec<FailureMode>,  // reuse existing type from api/metrics.rs
    pub last_updated: String,
}
```

### Warning injection in HTTP launch_game handler (routes.rs)
```rust
// After parsing car/track from launch_args (already done for duration_minutes injection)
// and BEFORE returning the response:
let reliability_warning: Option<String> = {
    let car = args_parsed.get("car").and_then(|v| v.as_str());
    let track = args_parsed.get("track").and_then(|v| v.as_str());
    let rel = metrics::query_combo_reliability(&state.db, pod_id, sim_type_str, car, track).await;
    rel.filter(|r| r.success_rate < 0.70)
       .map(|r| format!(
           "This combination has a {:.0}% success rate on this pod ({}/{} launches)",
           r.success_rate * 100.0,
           (r.success_rate * r.total_launches as f64) as i64,
           r.total_launches
       ))
};

match game_launcher::handle_dashboard_command(&state, cmd).await {
    Ok(()) => {
        let mut resp = json!({ "ok": true });
        if let Some(w) = reliability_warning {
            resp["warning"] = json!(w);
        }
        Json(resp)
    }
    // ... error arms unchanged
}
```

### Auto-retry cap adjustment (INTEL-05) in game_launcher.rs
```rust
// In launch_game(), after querying dynamic_timeout:
let reliability = metrics::query_combo_reliability(
    &state.db, pod_id, &sim_type.to_string(),
    car_for_timeout.as_deref(), track_for_timeout.as_deref()
).await;
let max_relaunch_cap: u32 = match &reliability {
    Some(r) if r.success_rate < 0.50 && r.total_launches >= 5 => 3,
    _ => 2,
};
// Store max_relaunch_cap in GameTracker (new field)
let tracker = GameTracker {
    // ... existing fields
    max_auto_relaunch: max_relaunch_cap,
    // ...
};
```

### alternatives_handler pattern in api/metrics.rs
```rust
#[derive(Debug, Deserialize)]
pub struct AlternativesParams {
    pub game: String,
    pub car: Option<String>,
    pub track: Option<String>,
    pub pod: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AlternativeCombo {
    pub car: Option<String>,
    pub track: Option<String>,
    pub success_rate: f64,
    pub avg_time_ms: Option<f64>,
    pub total_launches: i64,
}

pub async fn alternatives_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<AlternativesParams>,
) -> impl IntoResponse {
    // Query combo_reliability table with similarity preference ORDER BY
    // Return Vec<AlternativeCombo> as JSON
}
```

### DB migration addition in db/mod.rs
```rust
// Add after recovery_events migration block (around line 406):
sqlx::query(
    "CREATE TABLE IF NOT EXISTS combo_reliability (
        pod_id TEXT NOT NULL,
        sim_type TEXT NOT NULL,
        car TEXT,
        track TEXT,
        success_rate REAL NOT NULL DEFAULT 0.0,
        avg_time_to_track_ms REAL,
        p95_time_to_track_ms REAL,
        total_launches INTEGER NOT NULL DEFAULT 0,
        common_failure_modes TEXT,
        last_updated TEXT NOT NULL,
        PRIMARY KEY (pod_id, sim_type, car, track)
    )",
)
.execute(pool)
.await?;
sqlx::query("CREATE INDEX IF NOT EXISTS idx_combo_rel_sim ON combo_reliability(sim_type)")
    .execute(pool).await?;
```

### New routes in routes.rs
```rust
// In api_router() around the existing /metrics/ routes (line 121):
.route("/metrics/launch-stats", get(metrics::launch_stats_handler))
.route("/metrics/billing-accuracy", get(metrics::billing_accuracy_handler))
.route("/games/alternatives", get(metrics::alternatives_handler))      // NEW INTEL-03
.route("/admin/launch-matrix", get(metrics::launch_matrix_handler))    // NEW INTEL-04
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| No reliability awareness | Rolling-window reliability score per combo | Phase 200 | Staff warned before unreliable launches |
| Fixed 2-attempt retry cap | Data-driven retry cap (2 or 3 based on reliability) | Phase 200 | Low-reliability combos get one extra auto-recovery attempt |
| Dynamic timeout only | Dynamic timeout + dynamic retry cap | Phase 200 | Two axes of self-tuning from same data source |

## Open Questions

1. **Alternatives when pod has no history**
   - What we know: `query_alternatives()` filtered to `pod_id = ?` will return empty for a new pod
   - What's unclear: Should alternatives fall back to fleet-wide data (any pod) if pod-specific results < 3?
   - Recommendation: Yes — two-pass query: first pod-specific, then fleet-wide if fewer than 3 results. Document in code.

2. **combo_reliability update performance at high launch frequency**
   - What we know: `update_combo_reliability()` re-queries 30 days of launch_events and UPSERTs. With 8 pods, this is low frequency.
   - What's unclear: At venue scale (maybe 40-80 launches/day across all pods), query time is negligible. But if launch storm occurs (RC stress test), 8 concurrent upserts could contend on WAL.
   - Recommendation: Acceptable for production. The WAL mode + busy_timeout=5000ms handles this. No action needed.

3. **GameTracker.max_auto_relaunch new field**
   - What we know: `GameTracker` struct in `game_launcher.rs` needs a new field to carry the data-driven retry cap.
   - What's unclear: Does adding a field break any test that constructs `GameTracker` directly?
   - Recommendation: Give `max_auto_relaunch` a default of 2 (matching existing hardcoded behavior). Search for all `GameTracker { ... }` construction sites in tests and add the field.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[tokio::test]` with `sqlite::memory:` |
| Config file | none — in-module `#[cfg(test)]` blocks |
| Quick run command | `cargo test -p racecontrol -- metrics 2>&1` |
| Full suite command | `cargo test -p racecontrol 2>&1` |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| INTEL-01 | combo_reliability table has row after launch, shows correct success_rate | unit | `cargo test -p racecontrol -- test_combo_reliability_upsert` | Wave 0 |
| INTEL-01 | success_rate = 0.40 for 4/10 launches | unit | `cargo test -p racecontrol -- test_combo_reliability_rate` | Wave 0 |
| INTEL-02 | warning present in launch response when reliability < 70% | unit | `cargo test -p racecontrol -- test_launch_reliability_warning` | Wave 0 |
| INTEL-02 | no warning when reliability >= 70% | unit | `cargo test -p racecontrol -- test_launch_no_warning_good_combo` | Wave 0 |
| INTEL-02 | no warning when total_launches < 5 | unit | `cargo test -p racecontrol -- test_launch_no_warning_below_minimum` | Wave 0 |
| INTEL-03 | alternatives returns top 3 by success_rate > 0.90 | unit | `cargo test -p racecontrol -- test_alternatives_top3` | Wave 0 |
| INTEL-03 | at least 1 alternative shares car or track with request | unit | `cargo test -p racecontrol -- test_alternatives_similarity` | Wave 0 |
| INTEL-04 | launch matrix returns per-pod rows with flagged=true when < 70% | unit | `cargo test -p racecontrol -- test_launch_matrix_flagged` | Wave 0 |
| INTEL-05 | max_relaunch_count = 3 when combo reliability < 50% | unit | `cargo test -p racecontrol -- test_retry_cap_low_reliability` | Wave 0 |
| INTEL-05 | max_relaunch_count = 2 when reliability >= 50% or insufficient data | unit | `cargo test -p racecontrol -- test_retry_cap_normal` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol -- metrics 2>&1`
- **Per wave merge:** `cargo test -p racecontrol 2>&1`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/racecontrol/src/metrics.rs` — `test_combo_reliability_upsert`, `test_combo_reliability_rate` (new test functions, add to existing `#[cfg(test)]` block)
- [ ] `crates/racecontrol/src/metrics.rs` — `test_alternatives_top3`, `test_alternatives_similarity`
- [ ] `crates/racecontrol/src/metrics.rs` — `test_retry_cap_low_reliability`, `test_retry_cap_normal`
- [ ] `crates/racecontrol/src/game_launcher.rs` — `test_launch_reliability_warning`, `test_launch_no_warning_good_combo`, `test_launch_no_warning_below_minimum` (extend existing `make_state()` test helper to include combo_reliability table)
- [ ] `crates/racecontrol/src/db/mod.rs` — add `combo_reliability` CREATE TABLE to migration (existing migration function, no new file needed)

## Sources

### Primary (HIGH confidence)
- Direct code reading: `crates/racecontrol/src/metrics.rs` — full file read, all query patterns verified
- Direct code reading: `crates/racecontrol/src/api/metrics.rs` — full file read, handler patterns verified
- Direct code reading: `crates/racecontrol/src/game_launcher.rs` — lines 1-360 read, launch_game() flow verified
- Direct code reading: `crates/racecontrol/src/db/mod.rs` — lines 315-410 read, migration patterns verified
- Direct code reading: `crates/racecontrol/src/api/routes.rs` — lines 121, 280-300, 3588-3650 read

### Secondary (MEDIUM confidence)
- CONTEXT.md: explicit success criteria specify exact SQL table names, column names, and query patterns — treated as HIGH for schema design
- ROADMAP.md lines 3198-3212: success criteria used to derive test map

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new dependencies; all libraries already present and in use
- Architecture patterns: HIGH — derived directly from existing working code in the same files
- Pitfalls: HIGH — outcome serialization pitfall is already documented in existing code comments; NULL handling pattern is already present in `query_dynamic_timeout`
- DB schema: HIGH — follows exact migration pattern from db/mod.rs recovery_events block

**Research date:** 2026-03-26
**Valid until:** 2026-04-25 (stable codebase — no fast-moving external dependencies)
