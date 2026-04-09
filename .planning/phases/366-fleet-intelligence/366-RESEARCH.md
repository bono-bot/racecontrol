# Phase 366: Fleet Intelligence — Research

**Phase:** 366
**Researched:** 2026-04-10
**Valid until:** 2026-05-10
**Confidence:** HIGH (all findings from direct codebase inspection)

---

## User Constraints (from CONTEXT.md — MANDATORY for planner)

### Locked Decisions
- **D-01:** Health score = weighted composite (40/30/20/10), compute-on-query, 7-day window.
- **D-02:** `null` score (not 0) when < 3 sessions in window. `insufficient_data: true`.
- **D-03:** New endpoint `GET /api/v1/fleet/intelligence` (staff JWT). Response schema documented in CONTEXT.md D-03.
- **D-04:** Upgrade `METRIC_POD_HEALTH_SCORE` in `metrics_producers.rs` to emit composite 0-100.
- **D-05:** Time-of-day: batch SQL on `/fleet/intelligence` call, 30-day window, 30%/3-sample threshold.
- **D-06:** No ML — pure SQL aggregation for Phase 366.
- **D-07:** Content drift: background tokio task polling every 60 minutes.
- **D-08:** Comparison = TOML (expected) vs rc-agent `/debug/content-dirs` (live disk). RESOLVED: endpoint exists.
- **D-09:** `ContentDriftDetected` WS event + `content_drift_events` table + WhatsApp for game_removed.
- **D-10:** `content_drift_events` table schema (id, pod_id, detected_at, game_key, delta_type, item, resolved_at, resolution_note).
- **D-11:** Concurrent session guard via `active_timers` check at billing start. Return HTTP 409.
- **D-12:** Guard applies to all start paths (all call `start_billing_session()`).
- **D-13:** Game launch guard via `active_games` check. Return HTTP 409.
- **D-14:** No DB query for concurrent guard. In-memory only.
- **D-15:** Health score NOT stored in new table — compute-on-query + TSDB (existing `metrics_samples`).
- **D-16:** `content_drift_events` is the only new table. Additive migration.
- **D-17:** Mixed real-time/batch per component (see D-17 in CONTEXT.md).
- **D-18:** NO write to `audit_known_issues`. Read-only cross-reference.
- **D-19:** `content_drift_events` replicates via Phase 301 cloud_data_sync_v2. Health scores do NOT sync.

### Claude's Discretion
- Component weight tuning (40/30/20/10) — researcher found no reason to change.
- Content drift poll interval (60 min) — can be made configurable via AppConfig if pattern exists.
- Time-of-day threshold (30%/3 samples) — confirmed reasonable.
- Feature flag kill switch for concurrent session guard (consistent with Phase 363 `phase363_session_audit`).

### Out of Scope (Deferred)
- Admin UI rendering of `/fleet/intelligence` — Phase 367.
- AI-tier-aware health targets — Phase 365.
- Historical backfill — not planned.
- rc-agent changes — server-only.

---

## Standard Stack

**Language/Runtime:** Rust 1.93.1 (stable-x86_64-pc-windows-msvc) — no change.
**Framework:** Axum (existing — `crates/racecontrol/`).
**DB:** SQLite via `sqlx` (async). Pattern: `sqlx::query("...")`.bind(x).fetch_optional(&state.db).
**Background tasks:** `tokio::spawn` + `tokio::time::interval`. Pattern: see `metrics_producers.rs` `spawn_metric_producers()` for exact loop structure.
**State access:** `AppState` via `Arc<AppState>`. All maps are `RwLock<HashMap<...>>`.
**Migrations:** `db/mod.rs` `migrate()` function. Use `let _ = sqlx::query("ALTER TABLE ... ADD COLUMN IF NOT EXISTS ...").execute(pool).await?;` for additive changes. New tables use `CREATE TABLE IF NOT EXISTS`.
**WS events:** rc_common protocol types — broadcast via `state.agent_senders`.

---

## Critical Codebase Findings

### GLD-F-01: Health Score

**billing_sessions columns (Phase 363 — already in code at db/mod.rs:3959-4021):**
```
billing_sessions.lap_count_flag TEXT DEFAULT 'UNVERIFIED'
billing_sessions.telemetry_coverage_pct REAL (nullable)
billing_sessions.suspect BOOLEAN NOT NULL DEFAULT 0
billing_sessions.suspect_reasons TEXT (JSON array, nullable)
billing_sessions.csv_fallback_received_at TEXT (nullable)
billing_sessions.lap_reject_grace_until TEXT (nullable)
```
NOTE: Phase 363 is code-complete but NOT deployed to production as of 2026-04-10. The schema migration IS in the code. When Phase 366 ships, Phase 363 must be deployed first OR Phase 366 ships in the same binary.

**FleetHealthStore fields available for composite score computation:**
- `crashes_last_hour: i32` — crash component (10 pts)
- `http_reachable: bool` — availability proxy
- `in_maintenance: bool` — affects availability
- `crash_loop: bool` — extreme crash signal
Located: `crates/racecontrol/src/fleet_health.rs`, stored in `AppState::pod_fleet_health: RwLock<HashMap<String, FleetHealthStore>>`.

**Config mismatch data:** No `config_mismatch_count` column in `billing_sessions` yet. The `ConfigMismatchDetected` WS event (Phase 362) fires but is NOT currently stored in the DB as a countable metric. **Planner implication:** The config mismatch component (20 pts) must query an existing table or log source. Options:
  a) Query `billing_events` table for events with `event_type = 'config_mismatch_detected'` — check if Phase 362 writes this.
  b) Default config mismatch rate to 0 (unknown = assume 0 mismatches) and add the column in this phase.
  
**Researcher finding:** Phase 362's `ConfigMismatchDetected` is emitted as a WS broadcast but NOT written to `billing_events` or any persistent table. The planner MUST add `billing_sessions.config_mismatch_detected BOOLEAN NOT NULL DEFAULT 0` (or similar) OR default the mismatch component to 0.0 (clean) when no data. Given Phase 366 is adding the intelligence layer, the right approach is to **default config mismatch component to 0 (unknown = no detected mismatch)** and document that Phase 365/367 can wire in the Phase 362 data retroactively. This avoids adding another WS handler for a column that Phase 362 should have added.

**METRIC_POD_HEALTH_SCORE already exists** in `metrics_tsdb.rs` (line 16: `pub const METRIC_POD_HEALTH_SCORE: &str = "pod_health_score";`) and is emitted by `metrics_producers.rs` as binary 0.0/1.0. The upgrade to composite 0-100 is in `spawn_metric_producers()` at the `// 3. Pod health scores` comment block (line ~68).

**No new route file needed** — add `fleet_intelligence_handler` to `fleet_health.rs` (natural home for fleet-wide queries) and register in `routes.rs` `staff_routes()` near line 94 (`/fleet/health`).

### GLD-F-02: Time-of-Day Analysis

**SQL pattern for hour-of-day aggregation in SQLite:**
```sql
SELECT 
    strftime('%H', started_at) as hour_of_day,
    pod_id,
    COUNT(*) as sample_count,
    SUM(CASE WHEN suspect = 1 THEN 1 ELSE 0 END) as failure_count,
    CAST(SUM(CASE WHEN suspect = 1 THEN 1 ELSE 0 END) AS REAL) / COUNT(*) as failure_rate
FROM billing_sessions
WHERE started_at >= datetime('now', '-30 days')
    AND status = 'completed'
    AND suspect IS NOT NULL
GROUP BY pod_id, strftime('%H', started_at)
HAVING COUNT(*) >= 3 AND failure_rate >= 0.30
ORDER BY pod_id, hour_of_day
```
This computes directly from `billing_sessions.suspect` (Phase 363 column). Time-of-day analysis is a sub-query within the `/fleet/intelligence` handler — no separate endpoint needed.

### GLD-F-03: Content Drift Detector

**CRITICAL FINDING — `GET /debug/content-dirs` EXISTS AND WORKS:**
Located at `crates/rc-agent/src/remote_ops.rs` line 208.
Route: `GET :8090/debug/content-dirs` (service-key protected).
Response type: `rc_common::inventory_types::ContentDirsResponse` containing:
```
ContentDirsResponse {
    games: Vec<GameDirs>
}
GameDirs {
    game_key: String,         // e.g. "assetto_corsa"
    cars_dir: String,          // filesystem path
    tracks_dir: String,        // filesystem path
    cars_on_disk: Vec<String>, // actual car IDs from filesystem scan
    tracks_on_disk: Vec<String>, // actual track IDs
    cars_enumerable: bool,
    tracks_enumerable: bool,
}
```
This means content drift comparison IS possible: TOML `[content.<game>].cars` and `.tracks` vs `cars_on_disk` / `tracks_on_disk` from the probe.

**racecontrol already calls rc-agent via HTTP for health probes** (see `fleet_health.rs` `start_probe_loop` — calls `:8090/health`). The same reqwest-based pattern can call `:8090/debug/content-dirs`.

**TOML comparison baseline:** `crates/racecontrol/src/api/pods.rs` provides `load_pod_inventory(pod_number, config_dir)` which reads the TOML and returns `PodInventory` with game inventories. The planner should use this function to get the expected state.

**Comparison logic:**
1. Call `load_pod_inventory(n, config_dir)` to get expected TOML inventory.
2. Call rc-agent `:8090/debug/content-dirs` to get live disk inventory.
3. Diff: for each game_key, compare expected cars/tracks vs disk cars/tracks.
4. If delta: insert into `content_drift_events`, emit WS `ContentDriftDetected`.

**Drift background task:** Pattern from `fleet_health.rs` `start_probe_loop()` (lines ~1100+) or `metrics_producers.rs` `spawn_metric_producers()`. Use `tokio::time::interval(Duration::from_secs(3600))` for 60-minute poll. Register in `main.rs` or `lib.rs` where other background tasks are spawned.

**rc-agent HTTP call pattern** (from `fleet_health.rs` HTTP probe):
```rust
let url = format!("http://{}:8090/debug/content-dirs", pod_ip);
let resp = reqwest::Client::new()
    .get(&url)
    .header("X-Service-Key", &sentry_service_key)
    .timeout(Duration::from_secs(10))
    .send()
    .await;
```
IP lookup: use `state.config.pods.pod_ips` HashMap (key = pod_id, value = IP string) — or `state.config.pods.get_pod_ip(pod_id)` — check how `start_probe_loop` resolves pod IPs.

**WS ContentDriftDetected broadcast:** Use `state.agent_senders` to broadcast to admin clients. JSON body per D-09.

**WhatsApp alert for game_removed:** Use `whatsapp_alerter::send_admin_alert()` (same pattern as `fleet_alert.rs`). Fire only when `delta_type == "game_removed"`.

### GLD-F-04: Concurrent Session Guard

**EXISTING BEHAVIOR (critical for planner):**

The concurrent guard ALREADY EXISTS in `routes.rs` at line 3536-3548 but returns HTTP 200 with an error JSON, NOT HTTP 409. The task is to upgrade the response code.

Current code (routes.rs ~3536):
```rust
{
    let timers = state.billing.active_timers.read().await;
    if timers.contains_key(pod_id.as_str()) {
        return Json(json!({ "error": format!("Pod {} already has an active billing session", pod_id) }));
    }
}
```
Must change return type from `Json<Value>` to `(StatusCode, Json<Value>)` (or use the `Response` extractor) and return `(StatusCode::CONFLICT, Json(json!({ "error": "pod_already_active", "active_session_id": "<id>", "pod_id": pod_id })))`.

**Active session ID lookup:** the `active_timers` map stores `BillingTimer` which has `pub session_id: String` (line 354+ in billing.rs). So the handler can extract: `timers.get(pod_id).map(|t| t.session_id.clone())`.

**Game launch concurrent guard ALREADY EXISTS** in `game_launcher.rs` at line 383-395:
```rust
if let Some(tracker) = games.get(pod_id) {
    if matches!(tracker.game_state, GameState::Launching | GameState::Running | GameState::Stopping) {
        return Err(format!("Pod {} already has a game active", pod_id));
    }
}
```
This surfaces at `routes.rs` launch_game handler (~line 5800) as:
```rust
Err(e) => Json(json!({ "ok": false, "error": e })),
```
Again, returns HTTP 200 with error. Must be changed to `(StatusCode::CONFLICT, Json(...))`.

**Handler return type changes required:**
- `routes.rs`: `billing_start` handler (find the async function that calls `start_billing_session` and does the BATOM-02 check)
- `routes.rs`: `launch_game` handler (change `Err(e) =>` arm of the `handle_dashboard_command` match)

**IMPORTANT — Route handler return type:** Both handlers currently return `Json<Value>` (no StatusCode). To return 409, they must switch to `(StatusCode, Json<Value>)` OR `impl IntoResponse`. Check if other handlers in the same file use a tuple return — the `lockdown_pod` handler at line 18926 does: `(axum::http::StatusCode, Json<Value>)`.

---

## Architecture Patterns

### New Module: `fleet_intelligence.rs`
Create `crates/racecontrol/src/fleet_intelligence.rs` containing:
- `compute_pod_health_score(pool, pod_id, fleet_health_store)` — pure async fn returning `PodHealthScore` struct
- `compute_fleet_intelligence(state)` — calls above per pod, returns full response
- `fleet_intelligence_handler(State(state): State<Arc<AppState>>) -> Json<Value>` — axum handler
- `spawn_content_drift_task(state)` — spawns background 60-minute polling loop

### New Module: `content_drift.rs` (or subsection of fleet_intelligence.rs)
- `check_pod_content_drift(state, pod_id, pod_number, config_dir)` — calls rc-agent + compares TOML
- `emit_drift_events(state, pod_id, drifts)` — inserts to `content_drift_events`, broadcasts WS, optional WhatsApp

### DB Migration (db/mod.rs)
Add `content_drift_events` table and optional `billing_sessions.config_mismatch_detected` column.

### Route Registration (api/routes.rs)
Add to `staff_routes()`:
```rust
.route("/fleet/intelligence", get(fleet_intelligence::fleet_intelligence_handler))
```

### Background Task Spawn (main.rs or lib.rs)
```rust
fleet_intelligence::spawn_content_drift_task(Arc::clone(&state));
```

---

## Common Pitfalls

1. **Pod IP resolution:** `start_probe_loop` in `fleet_health.rs` uses a specific IP resolution method — copy the exact pattern, don't invent a new one. If pod is offline, skip silently (same as health probe).

2. **TSDB emit value range:** The upgraded `METRIC_POD_HEALTH_SCORE` will emit 0-100 instead of 0/1. Verify `metrics_tsdb.rs` stores `f64` (it does — `value: f64`). The new range is compatible with existing storage.

3. **Phase 363 dependency:** `billing_sessions.suspect` and `telemetry_coverage_pct` are added by Phase 363's migration. If Phase 363 is not deployed, these columns won't exist. The `ALTER TABLE ADD COLUMN IF NOT EXISTS` pattern handles this gracefully (SQLite silently skips if column exists; Phase 363 adds them on its own migration). Phase 366's SQL queries MUST handle NULL values for these columns (use `COALESCE(suspect, 0)` and `COALESCE(telemetry_coverage_pct, 100)` to degrade gracefully).

4. **StatusCode return type change:** Changing `Json<Value>` to `(StatusCode, Json<Value>)` in a route handler requires updating the function signature AND the return type annotation. Axum derives `IntoResponse` for both, but the types are different. Double-check all early returns in the affected handlers.

5. **Content drift: rc-agent offline:** If `GET /debug/content-dirs` fails (pod offline, timeout), skip that pod's drift check silently and log at WARN level. Do NOT mark the pod as drifted just because it's offline.

6. **Content drift: first run baseline:** On first run, there's no previous snapshot. The task should record the current state as baseline without emitting drift events. Use the `content_drift_events` table as the delta store — only insert when delta is detected between current call and expected TOML, not between two consecutive calls.

7. **Cloud sync for content_drift_events:** Follow the exact pattern used for `fleet_solutions` or `fleet_incidents` in `cloud_sync.rs`. Researcher found cloud_sync pushes billing_sessions at line 689. The `content_drift_events` table should be added to the push payload in `cloud_sync.rs`.

---

## Don't Hand-Roll

- **HTTP client for rc-agent calls:** Use the existing `reqwest::Client` pattern from `fleet_health.rs`. Don't create a new HTTP client abstraction.
- **WS broadcast:** Use `state.agent_senders` broadcast pattern (existing in multiple handlers). Don't build a new channel.
- **Background task lifecycle:** Use the tokio `interval` loop pattern from `metrics_producers.rs`. Don't use `tokio::time::sleep` in a loop.
- **SQLite time functions:** Use SQLite's `strftime('%H', started_at)` for hour extraction and `datetime('now', '-7 days')` / `datetime('now', '-30 days')` for window filtering. The DB stores timestamps as RFC3339 strings (`TEXT`).

---

## Environment Availability

Step 2.6: All tools are in the existing codebase — no new external dependencies.

| Dependency | Required By | Available | Fallback |
|------------|------------|-----------|----------|
| `reqwest` | rc-agent HTTP probe | Already in Cargo.toml (fleet_health uses it) | N/A |
| `tokio::time::interval` | Background drift task | stdlib (tokio) | N/A |
| `sqlx` | DB queries | Already in Cargo.toml | N/A |
| `chrono` | Timestamp handling | Already in Cargo.toml | N/A |
| `serde_json` | WS event serialization | Already in Cargo.toml | N/A |

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `cargo test` + `#[tokio::test]` for async |
| Config file | `crates/racecontrol/Cargo.toml` |
| Quick run command | `cargo test -p racecontrol --lib 2>&1 \| tail -5` |
| Full suite command | `cargo test -p racecontrol 2>&1 \| tail -10` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| GLD-F-01 | Score returns 0-100 per pod | unit | `cargo test -p racecontrol fleet_intelligence 2>&1 \| tail -5` | ❌ Wave 0 |
| GLD-F-01 | insufficient_data=true when < 3 sessions | unit | same | ❌ Wave 0 |
| GLD-F-02 | time_patterns flagged_hours with >30% rate | unit | same | ❌ Wave 0 |
| GLD-F-03 | ContentDriftDetected event on inventory delta | unit | same | ❌ Wave 0 |
| GLD-F-04 | Concurrent billing start returns 409 | unit | `cargo test -p racecontrol concurrent_session 2>&1 \| tail -5` | ❌ Wave 0 |
| GLD-F-04 | Concurrent game launch returns 409 | unit | same | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol --lib 2>&1 | tail -5`
- **Per wave merge:** `cargo test -p racecontrol 2>&1 | tail -10`
- **Phase gate:** 891+ tests green (current count from State.md)

### Wave 0 Gaps
- [ ] `crates/racecontrol/src/fleet_intelligence.rs` — create module with tests
- [ ] `crates/racecontrol/src/content_drift.rs` — create module with tests (or inline in fleet_intelligence.rs)
- No new test infrastructure needed — existing `cargo test` + `#[tokio::test]` + `Arc<AppState>` test helpers

---

## Sources

### Primary (HIGH confidence — direct codebase inspection)
- `crates/racecontrol/src/fleet_health.rs` — FleetHealthStore, probe loop, active_games
- `crates/racecontrol/src/metrics_producers.rs` — METRIC_POD_HEALTH_SCORE, spawn_metric_producers pattern
- `crates/racecontrol/src/metrics_tsdb.rs` — MetricSample, record_sample
- `crates/racecontrol/src/billing.rs` — BillingTimer.session_id, start_billing_session, active_timers
- `crates/racecontrol/src/api/routes.rs` — BATOM-02 guard (line 3536), launch_game handler (line 5568)
- `crates/racecontrol/src/game_launcher.rs` — LIFE-04 guard (line 383), active_games map
- `crates/rc-agent/src/remote_ops.rs` — content_dirs_handler (line 1590), ContentDirsResponse
- `crates/rc-agent/src/content_scanner.rs` — content cache, car/track IDs
- `crates/racecontrol/src/db/mod.rs` — billing_sessions schema, Phase 363 migration (line 3959)
- `crates/racecontrol/src/fleet_kb.rs` — audit_known_issues (NOT used by Phase 366)
- `crates/racecontrol/src/cloud_sync.rs` — cloud sync billing_sessions push pattern (line 689)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — existing Rust/Axum/SQLite/tokio (unchanged)
- Architecture: HIGH — direct inspection of all relevant files
- Pitfalls: HIGH — identified from code patterns, not assumptions
- GLD-F-03 content drift endpoint: HIGH — `GET /debug/content-dirs` confirmed at rc-agent remote_ops.rs:208
- GLD-F-04 concurrent guard: HIGH — existing guard at routes.rs:3536 and game_launcher.rs:383 confirmed; only response code change needed

**Research date:** 2026-04-10
**Valid until:** 2026-05-10

## RESEARCH COMPLETE
