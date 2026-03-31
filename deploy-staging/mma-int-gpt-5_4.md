Findings from the integration audit, focused on cross-phase wiring/dataflow/startup/schema/type issues visible in the provided code.

---

## 1) Maintenance analytics routes are mounted on the wrong auth tier
**Severity:** P1  
**File:line:** `crates/racecontrol/src/api/routes.rs:527-533` (inside `staff_routes`)

**What’s wrong:**  
The new v29.0 maintenance/analytics endpoints are added under `staff_routes(...)`, which is the authenticated staff write surface:

- `/maintenance/events`
- `/maintenance/summary`
- `/maintenance/tasks`
- `/maintenance/tasks/{id}`
- `/analytics/telemetry`
- `/analytics/trends`

But the comments above them say Phase 9 maintenance & analytics, and the kiosk maintenance gate endpoint was added under `public_routes()`. If kiosk/PWA/unauthenticated operational surfaces need visibility into maintenance/availability analytics, these endpoints are not reachable there. This is a wiring-tier mismatch: registered, but on the wrong router tier for likely consumers.

Also, these are mixed read/write endpoints; exposing all under staff may block read-side integrations while write-side is fine.

**Concrete fix:**  
Split routes by access tier:

- keep write endpoints in `staff_routes`:
  - `POST /maintenance/events`
  - `POST /maintenance/tasks`
  - `PATCH /maintenance/tasks/{id}`
- move read endpoints to the correct read tier (`public_routes` or a manager/staff read tier as intended):
  - `GET /maintenance/summary`
  - `GET /maintenance/events`
  - `GET /maintenance/tasks`
  - `GET /analytics/telemetry`
  - `GET /analytics/trends`

If these must remain staff-only, then wire the kiosk/PWA consumer to the already-public `/pods/{id}/availability` endpoint only, and remove any frontends expecting direct analytics access.

---

## 2) `analytics_telemetry` reads telemetry DB, but writer path is fire-and-forget and likely drops errors silently
**Severity:** P2  
**File:line:** `crates/racecontrol/src/ws/mod.rs` handler snippet, at `crate::telemetry_store::store_extended_telemetry(&state, pod_id, &agent_msg);`

**What’s wrong:**  
The WS handler stores incoming telemetry by calling `store_extended_telemetry(...)` without `await` and without handling any result. That strongly suggests one of two integration risks:

1. it enqueues to a background writer and can fail silently if `telemetry_writer_tx` is absent/closed/full;
2. it performs async work internally via spawn, making backpressure/error visibility disappear.

Meanwhile, `analytics_telemetry` reads from `state.telemetry_db`, and `data_collector::check_rul_thresholds` depends on telemetry aggregates. So the whole telemetry pipeline is:

agent → WS → telemetry_store → telemetry DB → analytics/RUL

If the WS side silently drops writes, downstream phases appear “wired” but consume empty data.

**Concrete fix:**  
Change the WS call site to explicit result handling. For example:

```rust
if let Err(e) = crate::telemetry_store::store_extended_telemetry(&state, &pod_id, &agent_msg).await {
    tracing::warn!(pod=%pod_id, error=%e, "failed to persist extended telemetry");
}
```

If `store_extended_telemetry` is intentionally sync and queue-based, make it return `Result<(), _>` and log enqueue failures here. Also add a startup assert/log if `telemetry_writer_tx.is_none()` when telemetry messages arrive.

---

## 3) RUL task creation depends on `telemetry_aggregates`, but no aggregate producer is wired in provided startup
**Severity:** P1  
**File:line:**  
- `crates/racecontrol/src/data_collector.rs:48-57`  
- `crates/racecontrol/src/main.rs:432-439` (Phase 35 spawn)

**What’s wrong:**  
`check_rul_thresholds()` queries:

```sql
SELECT pod_id, metric_name, avg_val FROM telemetry_aggregates
WHERE period_hours = 24 ...
```

But in the provided startup wiring, only these telemetry pieces are started:

- telemetry DB init
- telemetry writer
- telemetry maintenance scheduler
- anomaly scanner
- data collector

There is no visible aggregate builder/spawner producing `telemetry_aggregates`. If maintenance scheduler doesn’t do this, then Phase 35 is reading a table that is never populated. This is a classic data flow break: data collected into raw `hardware_telemetry`, but RUL reads from a different derived table with no visible producer.

**Concrete fix:**  
One of:

1. explicitly spawn the aggregate job in `main.rs` after telemetry DB init:
   ```rust
   racecontrol_crate::telemetry_store::spawn_aggregate_scheduler(telem_pool.clone());
   ```
2. or rewrite `check_rul_thresholds()` to query directly from `hardware_telemetry` with `AVG(...) GROUP BY pod_id` over the last 24h.

Also add a startup log confirming aggregate scheduler enabled.

---

## 4) `collect_venue_snapshot()` collects data but never persists or publishes it
**Severity:** P2  
**File:line:** `crates/racecontrol/src/data_collector.rs:21-46`, `:99-114`

**What’s wrong:**  
Phase 35 is described as “Unified data collector — cross-domain snapshots for AI consumption.” But the spawned loop does:

- call `collect_venue_snapshot(&pool).await`
- log a debug line
- discard the snapshot

So this phase is wired only to logs; no DB insert, no broadcast, no cloud sync handoff, no AI queue. That’s a pure collected-but-never-consumed flow break.

**Concrete fix:**  
Persist or publish snapshots. Example options:

- add `venue_snapshots` table and insert each snapshot;
- broadcast over an internal channel for AI consumers;
- push via `cloud_sync::sync_maintenance_data` or a new sync function;
- cache latest snapshot in `AppState`.

At minimum:

```rust
insert_venue_snapshot(&pool, &snapshot).await?;
```

and add corresponding schema init during startup.

---

## 5) Extended cloud sync function is never called
**Severity:** P2  
**File:line:** `crates/racecontrol/src/cloud_sync.rs:135-177`

**What’s wrong:**  
`sync_maintenance_data(...)` was added for Phase 33, but the provided `main.rs` only calls:

```rust
cloud_sync::spawn(state.clone());
```

and there is no evidence in the diff that `spawn()` invokes `sync_maintenance_data()`. As shown, this new function is dead integration code: implemented, registered nowhere.

**Concrete fix:**  
Wire it into the existing cloud sync loop inside `cloud_sync::spawn()`, e.g.:

```rust
if let Err(e) = sync_maintenance_data(&state.db, &cloud_url).await {
    tracing::warn!(target: "cloud-sync", error=%e, "extended maintenance sync failed");
}
```

If this is intentionally staged, guard behind a feature/config flag and log that it is disabled.

---

## 6) Business alerts are generated but not delivered to any channel
**Severity:** P2  
**File:line:** `crates/racecontrol/src/alert_engine.rs:81-97`

**What’s wrong:**  
`spawn_alert_checker()` runs every 30 minutes and calls `check_business_alerts(&pool).await;`, but then:

```rust
// TODO: wire alerts to whatsapp_alerter for WhatsApp channel
```

This is a direct wiring gap. Alerts are computed, logged, then dropped. Since `BusinessAlert.channel` includes `WhatsApp`, `Dashboard`, `Both`, the producer/consumer contract is unfinished.

**Concrete fix:**  
Inject `Arc<AppState>` or a sender into `spawn_alert_checker()` and route alerts:

- WhatsApp → existing `notification_outbox` / `whatsapp_alerter`
- Dashboard → `dashboard_tx`
- Both → both

For example:
```rust
pub fn spawn_alert_checker(state: Arc<AppState>) { ... }
```
and inside:
```rust
for alert in alerts {
    dispatch_business_alert(&state, alert).await;
}
```

---

## 7) Pricing bridge is initialized but never applied or exposed
**Severity:** P2  
**File:line:**  
- `crates/racecontrol/src/main.rs:278-283`  
- `crates/racecontrol/src/pricing_bridge.rs:48-76`

**What’s wrong:**  
Startup initializes `pricing_proposals` via `init_pricing_tables()`, but there is no:

- route to create/approve proposals,
- background task calling `apply_approved_pricing()`,
- integration with `dynamic_pricing`,
- actual write-through to billing config.

So the phase is schema-only. Worse, `apply_approved_pricing()` only marks rows `applied` and explicitly says actual price push is not implemented. This is a module wired into DB init but not into execution flow.

**Concrete fix:**  
Add all three missing links:

1. API routes for create/approve/list proposals.
2. Spawn periodic apply worker or call apply on admin approval.
3. Implement actual write-through into billing/rate-tier config and then refresh caches:
   ```rust
   billing::refresh_rate_tiers(&state).await;
   ```

If not ready, remove init from startup to avoid false sense of functionality.

---

## 8) Startup order risk: pod seeding inserts into `pods` table before any visible schema init for `pods`
**Severity:** P2  
**File:line:** `crates/racecontrol/src/main.rs:75-87`, `:299-302`

**What’s wrong:**  
`seed_pods_on_startup()` now writes to SQLite:

```sql
INSERT OR IGNORE INTO pods (...)
```

But in the provided `main.rs`, there is no explicit schema init call for the core app tables before this function, only `db::init_pool(...)`. If `db::init_pool` does not guarantee migrations/schema creation before returning, this startup path depends on implicit behavior. That is a startup-order coupling hazard.

**Concrete fix:**  
Make the dependency explicit:

- either ensure `db::init_pool()` runs full migrations and document it,
- or call a dedicated `db::init_schema(&pool).await?` before `seed_pods_on_startup()`.

Also log insert errors instead of ignoring them:
```rust
if let Err(e) = ...execute(&state.db).await {
    tracing::error!(pod=%pod.id, error=%e, "failed to seed pod into DB");
}
```

---

## 9) `seed_pods_on_startup()` can leave memory and DB out of sync
**Severity:** P2  
**File:line:** `crates/racecontrol/src/main.rs:35-57`, `:75-87`

**What’s wrong:**  
The function first seeds the in-memory `state.pods`, then separately inserts into DB with `let _ = ...execute(...).await;`. If DB inserts fail, memory has all 8 pods but DB may not. Kiosk/API consumers hitting DB vs in-memory WS consumers will diverge.

This is a cross-store consistency bug, especially since the comment says the kiosk queries DB directly.

**Concrete fix:**  
Treat DB as source of truth for seeding:

1. insert all pods into DB in a transaction,
2. query back / mirror into memory only after success.

Or at least log and reconcile failures. Example:
```rust
let mut tx = state.db.begin().await?;
...
tx.commit().await?;
```
then populate `state.pods`.

---

## 10) Duplicate/conflicting top-level route for `/cameras`
**Severity:** P2  
**File:line:** `crates/racecontrol/src/main.rs:560-561`, `:163-169`

**What’s wrong:**  
`/cameras` appears in `WEB_DASHBOARD_PATHS`, meaning it should proxy to the web dashboard on port 3200, but the main router also defines:

```rust
.route("/cameras", get(|| async { Redirect::permanent("/kiosk/fleet") }))
```

Because explicit routes take precedence over fallback proxying, `/cameras` will never reach the dashboard path listed in `WEB_DASHBOARD_PATHS`. That is a route conflict and likely wrong destination for staff users.

**Concrete fix:**  
Pick one canonical behavior:

- if `/cameras` belongs to web dashboard, remove the explicit redirect route;
- if it should redirect to kiosk fleet, remove `"/cameras"` from `WEB_DASHBOARD_PATHS`.

Do the same audit for `/book`, which is also in `WEB_DASHBOARD_PATHS` but explicitly redirected to `/kiosk/book`.

---

## 11) Another route conflict: `/book` in dashboard proxy list but explicitly redirected to kiosk
**Severity:** P2  
**File:line:** `crates/racecontrol/src/main.rs:166-169`, `:561-562`

**What’s wrong:**  
Same class as above. `WEB_DASHBOARD_PATHS` includes `"/book"`, but router hardcodes:

```rust
.route("/book", get(|| async { Redirect::permanent("/kiosk/book") }))
```

So the dashboard proxy list says web dashboard; actual route says kiosk. This will confuse future `/kiosk/<dashboard-path>` redirect logic too.

**Concrete fix:**  
Make `WEB_DASHBOARD_PATHS` match actual explicit routes. Remove one side of the mismatch.

---

## 12) `AlertChannel` serialization likely mismatches expected external/string consumers
**Severity:** P3  
**File:line:** `crates/racecontrol/src/alert_engine.rs:8`

**What’s wrong:**  
`AlertChannel` derives `Serialize` with default serde enum encoding, so JSON will be `"WhatsApp"`, `"Dashboard"`, `"Both"`. Elsewhere in the codebase, status/type fields are often persisted/queried as lowercase strings (`'pending'`, `'approved'`, `'completed'`, etc.). If dashboard/cloud/notification consumers expect lowercase or snake_case channel names, this is a type/serialization mismatch waiting to happen.

**Concrete fix:**  
Stabilize enum wire format:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertChannel { WhatsApp, Dashboard, Both }
```

Then use `whats_app` or rename variant to `Whatsapp` if needed consistently. Apply same policy to all newly introduced enums intended for JSON.

---

## 13) `PodAvailability` is serialized but no deserialize/wire-format contract is defined
**Severity:** P3  
**File:line:** `crates/racecontrol/src/self_healing.rs:38-44`

**What’s wrong:**  
`PodAvailability` is used as a state model for kiosk/PWA/POS and derives only `Serialize`. The public route `/pods/{id}/availability` likely returns this structure, but any consumer writing overrides or any future read-back from DB/cloud cannot deserialize it. Also enum variant names will serialize as `Available`, `Degraded`, etc., which may not match frontend expectations.

**Concrete fix:**  
Add stable serde contract now:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PodAvailability { ... }
```

This avoids future writer/reader mismatches.

---

## 14) Self-healing availability map exists but no startup wiring connects anomaly detection to it
**Severity:** P1  
**File:line:**  
- `crates/racecontrol/src/self_healing.rs:46-79`  
- `crates/racecontrol/src/main.rs` overall startup, no visible use

**What’s wrong:**  
Phase 27 introduces a thread-safe `PodAvailabilityMap` plus `recommend_action`, `apply_action`, `mark_available`. But in the provided startup there is no:

- map creation into `AppState`,
- spawn of a self-healing orchestrator,
- call from anomaly scanner / maintenance engine into `apply_action`.

So Phase 27 is effectively unwired. Meanwhile Phase 34 adds public `/pods/{id}/availability`, implying consumers expect real data. Unless `AppState` already carries this map and another hidden module wires it, the provided code shows a gap between anomaly detection and availability serving.

**Concrete fix:**  
Wire self-healing into state and startup:

1. add `pod_availability: PodAvailabilityMap` to `AppState`, initialized with `new_availability_map()`;
2. spawn an orchestrator consuming anomaly outputs;
3. update `/pods/{id}/availability` to read this shared map.

If anomalies already emit events elsewhere, bridge that stream here.

---

## 15) `check_business_alerts()` uses rough IST conversion that can misfire around half-hour boundaries
**Severity:** P3  
**File:line:** `crates/racecontrol/src/alert_engine.rs:38-39`

**What’s wrong:**  
Peak-hour detection uses:

```rust
let hour = (Utc::now().hour() + 5) % 24 + if Utc::now().minute() >= 30 { 1 } else { 0 };
```

This can produce `24` when UTC hour is 18 and minute >= 30, because modulo happens before adding the half-hour carry. Then `is_peak = hour >= 16 && hour < 22` breaks incorrectly. This is a cross-phase logic/data issue: alerts can be skipped or run in the wrong window.

**Concrete fix:**  
Use chrono timezone conversion or correct arithmetic:

```rust
let now_ist = Utc::now() + chrono::Duration::minutes(330);
let hour = now_ist.hour();
```

---

## 16) `feedback_loop` time filtering compares RFC3339 strings against SQLite `datetime('now')`-style strings
**Severity:** P2  
**File:line:**  
- `crates/racecontrol/src/feedback_loop.rs:52-58`  
- `:91-123`

**What’s wrong:**  
Rows are inserted with:

- `predicted_at` as RFC3339
- `created_at` defaulting to `datetime('now')` in SQLite (`YYYY-MM-DD HH:MM:SS`)

Queries then filter:

```sql
WHERE created_at > ?1
```

with `?1` bound to `Utc::now().to_rfc3339()`

Lexicographic comparison between SQLite datetime text and RFC3339 text is inconsistent due to separator/format differences (`' '` vs `'T'`, timezone suffix). This is a type/schema mismatch between writer and reader.

**Concrete fix:**  
Use one consistent format everywhere. Best options:

- store all timestamps as Unix epoch integers, or
- store all as SQLite `datetime(...)` text and compare with SQLite-generated bounds.

Minimal fix:
- insert `created_at` explicitly as RFC3339 too, or
- query with `datetime(created_at) > datetime(?1)` and bind ISO strings that SQLite parses.

---

## 17) `sync_maintenance_data()` likely queries wrong columns/types for `maintenance_events`
**Severity:** P2  
**File:line:** `crates/racecontrol/src/cloud_sync.rs:141-145`

**What’s wrong:**  
The query assumes `maintenance_events` has columns:

- `id`
- `event_type`
- `severity`
- `detected_at`

But route handlers use `crate::maintenance_models::MaintenanceEvent`, and elsewhere `severity` may be serialized enum text with capitalization. Since schema for `maintenance_events` isn’t shown here, this is a schema-coupling risk. Given other modules compare exact strings like `'Critical'`, any mismatch in stored case or column names breaks sync silently.

**Concrete fix:**  
Align all maintenance event SQL through `maintenance_store` rather than ad hoc SQL in `cloud_sync`:

```rust
let recent_events = maintenance_store::query_recent_events(pool, Duration::hours(1), 100).await?;
```

This centralizes schema knowledge and avoids column drift.

---

## 18) Multiple background tasks touch billing/session state without visible coordination
**Severity:** P2  
**File:line:** `crates/racecontrol/src/main.rs:349-430`

**What’s wrong:**  
These tasks run concurrently over billing/session data:

- 1s `billing::tick_all_timers`
- 5s `billing::sync_timers_to_db`
- staggered `billing::persist_timer_state`
- startup recovery/orphan detection
- 5 min background orphan detection
- reconciliation / expiry jobs

Without seeing billing internals, the integration pattern is high-risk: multiple spawned tasks mutate/read timer/session state and DB at overlapping cadences. Especially `tick_all_timers`, `sync_timers_to_db`, and `persist_timer_state` can race and persist partially updated elapsed values or stale statuses.

**Concrete fix:**  
Enforce a single writer pattern for in-memory timer/session state:

- one billing supervisor task owns mutable timer state,
- other tasks send commands over channels,
- DB persistence reads from a snapshot produced by that owner.

If current design uses internal locks, document them and coalesce DB writes to one persistence loop.

---

## 19) `server_ops::start()` is launched out-of-band and not tied to Axum lifecycle/state
**Severity:** P2  
**File:line:** `crates/racecontrol/src/main.rs:484`

**What’s wrong:**  
Main server builds one Axum router with shared `AppState`, but `server_ops::start()` starts a separate HTTP endpoint on `:8090` with no visible state injection, auth wiring, or shutdown coordination. That’s an integration/lifecycle gap:

- possible port conflict with other local services,
- auth tier may differ from main app,
- startup success/failure isn’t checked,
- no graceful shutdown linkage.

**Concrete fix:**  
Either:

1. nest server_ops into the main Axum router under authenticated routes, or
2. make `start()` return `JoinHandle<Result<...>>` / bind result, pass shared state/config/auth middleware, and log bind failure explicitly.

---

## 20) `analytics_telemetry` / telemetry storage likely schema-format mismatch on timestamps
**Severity:** P2  
**File:line:**  
- agent send: `crates/rc-agent/src/event_loop.rs:1350-1365`  
- API read: `crates/racecontrol/src/api/routes.rs:1566-1577`  
- collector aggregate read: `crates/racecontrol/src/data_collector.rs:52-56`

**What’s wrong:**  
Agent emits `collected_at` as RFC3339. API query filters telemetry using:

```sql
WHERE collected_at > ?1
```

with `cutoff` also RFC3339, which is okay only if storage preserves RFC3339 text consistently.

But `check_rul_thresholds()` compares aggregate `period_start > datetime('now', '-1 day')`, i.e. SQLite datetime text. If aggregate generation stores `period_start` in RFC3339 while comparing to SQLite datetime text, you get the same string-format bug as feedback_loop. There is already inconsistent timestamp usage across modules.

**Concrete fix:**  
Standardize telemetry timestamps:

- raw and aggregate tables should store integer epoch millis or ISO8601 consistently,
- all SQL comparisons should use the same format.

Audit telemetry_store schema and convert all `datetime('now')` defaults to explicit RFC3339 or epoch integer.

---

# Highest-priority fixes first
If you want the most important remediation order:

1. **P1:** Wire anomaly/availability/self-healing into actual state and routes.  
2. **P1:** Ensure `telemetry_aggregates` is actually produced, or make RUL read raw telemetry.  
3. **P1:** Put maintenance/analytics endpoints on the intended auth/read tier.  
4. **P2:** Fix route conflicts for `/cameras` and `/book`.  
5. **P2:** Call `sync_maintenance_data()` from cloud sync loop.  
6. **P2:** Deliver business alerts instead of just logging them.  
7. **P2:** Standardize timestamp storage/comparison formats.  
8. **P2:** Eliminate billing background-task write races.

If you want, I can do a second pass and turn these into a concise patch plan grouped by module (`main.rs`, `api/routes.rs`, `cloud_sync.rs`, `data_collector.rs`, etc.).