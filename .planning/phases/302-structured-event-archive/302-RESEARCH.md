# Phase 302: Structured Event Archive - Research

**Researched:** 2026-04-01
**Domain:** Rust/Axum SQLite event persistence, JSONL export, SCP offload, filtered REST API
**Confidence:** HIGH — all findings sourced from live codebase inspection

## Summary

Phase 302 adds a structured events table (`system_events`) to the existing SQLite racecontrol.db, a nightly JSONL export task that archives the previous day's events, a 90-day purge that removes SQLite rows while leaving JSONL files untouched, SCP delivery to Bono VPS reusing the backup_pipeline.rs pattern exactly, and a filtered REST API at `GET /api/v1/events`.

The existing `pod_activity_log` table and `activity_log.rs` module handle pod-scoped operational logs with fire-and-forget inserts. Phase 302's events table is a **different, wider-scope table** covering system-level events across all sources — billing sessions, deploys, alerts, pod recovery — not just per-pod UI events. The implementation creates a new `event_archive.rs` module alongside the existing `activity_log.rs`. They serve different audiences: activity_log feeds the real-time dashboard; events feeds long-term archive and audit queries.

The backup_pipeline.rs SCP pattern (Steps A–E: mkdir, local SHA256, SCP with 120s timeout, remote sha256sum, update status) is the authoritative template. The nightly JSONL transfer reuses exactly this flow, differing only in the source file being a JSONL export rather than a .db file. The IST time-window gate, `last_remote_transfer: NaiveDate` deduplication, and `StrictHostKeyChecking=no BatchMode=yes ConnectTimeout=10` SSH flags must all carry over verbatim.

**Primary recommendation:** Create `event_archive.rs` as a new module. Use `append_event()` for writes (fire-and-forget tokio::spawn), `spawn()` for the background task, and a dedicated `EventArchiveConfig` in config.rs with serde defaults (no TOML change required at deploy).

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Extend existing activity_log.rs or create new event_archive.rs module
- Events table in SQLite (same WAL-mode DB)
- Daily JSONL export runs as a tokio task
- SCP to Bono VPS reuses backup_pipeline.rs SCP pattern from Phase 300
- REST API follows existing Axum route patterns
- 90-day purge runs as part of daily maintenance

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase.

### Deferred Ideas (OUT OF SCOPE)
None.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| EVENT-01 | All significant events written to SQLite events table with structured schema (type, source, pod, timestamp, payload) | New `system_events` table in db/mod.rs; `append_event()` called from billing.rs, deploy.rs, alert_engine.rs, pod_healer.rs |
| EVENT-02 | Daily JSONL export of events table for archival | `export_daily_jsonl()` in event_archive.rs; tokio task loop pattern from backup_pipeline.rs |
| EVENT-03 | SQLite events retained for 90 days, then purged (JSONL is permanent archive) | `purge_old_events()` DELETE WHERE timestamp < 90 days; runs in same tick as export |
| EVENT-04 | Nightly JSONL files shipped to Bono VPS via SCP | `transfer_jsonl_to_remote()` reusing backup_pipeline.rs Steps A–E verbatim; IST 02:00-03:59 window |
| EVENT-05 | Events queryable via REST API (GET /api/v1/events with filters: type, pod, date range) | `EventsQuery` struct with type/pod/from/to; dynamic query builder pattern from BillingListQuery |
</phase_requirements>

---

## Standard Stack

### Core (all already in Cargo.toml — zero new dependencies)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| sqlx | existing | SQLite CREATE TABLE, INSERT, SELECT, DELETE | Same pool as all other tables |
| chrono + chrono-tz | existing | IST timestamps, NaiveDate for deduplication | Used in backup_pipeline.rs verbatim |
| serde_json | existing | JSON payload serialization + JSONL line encoding | Used everywhere |
| tokio | existing | Background task spawn pattern | Same pattern as backup_pipeline::spawn |
| uuid | existing | Event IDs (`Uuid::new_v4().to_string()`) | Same as pod_activity_log |
| sha2 + hex | existing | SHA256 for remote checksum verification | Pulled from backup_pipeline.rs |
| axum | existing | REST route handler | Same as all existing routes |

### No new dependencies required
The entire phase is implementable with the existing dependency set. Confirmed by audit of backup_pipeline.rs (sha2, hex, chrono-tz all present) and event_archive use cases.

---

## Architecture Patterns

### Recommended Project Structure

```
crates/racecontrol/src/
├── event_archive.rs          # NEW: spawn(), append_event(), export_daily_jsonl(),
│                             #       transfer_jsonl_to_remote(), purge_old_events()
├── config.rs                 # ADD: EventArchiveConfig struct + serde defaults
├── db/mod.rs                 # ADD: system_events table + indexes at end of migrate()
├── lib.rs                    # ADD: pub mod event_archive;
├── main.rs                   # ADD: event_archive::spawn(state.clone());
└── api/routes.rs             # ADD: EventsQuery struct + get_events handler + route registration
```

### Pattern 1: DB Table Schema

The `system_events` table design follows the pod_activity_log schema style, with additions for event_type (machine-readable, indexed), pod (nullable — server-level events have no pod), and payload (JSON blob for extensible per-type data).

```rust
// In db/mod.rs, appended at end of migrate()
sqlx::query(
    "CREATE TABLE IF NOT EXISTS system_events (
        id TEXT PRIMARY KEY,
        event_type TEXT NOT NULL,
        source TEXT NOT NULL,
        pod TEXT,
        timestamp TEXT NOT NULL DEFAULT (datetime('now')),
        payload TEXT NOT NULL DEFAULT '{}'
    )",
)
.execute(pool)
.await?;

sqlx::query("CREATE INDEX IF NOT EXISTS idx_system_events_type ON system_events(event_type)")
    .execute(pool).await?;
sqlx::query("CREATE INDEX IF NOT EXISTS idx_system_events_pod ON system_events(pod)")
    .execute(pool).await?;
sqlx::query("CREATE INDEX IF NOT EXISTS idx_system_events_ts ON system_events(timestamp)")
    .execute(pool).await?;
```

**Column choices:**
- `event_type TEXT` — machine-readable category: `billing.session_started`, `billing.session_ended`, `deploy.started`, `deploy.completed`, `alert.fired`, `pod.recovery`, `game.launched`. Dot-namespaced for future prefix filtering.
- `source TEXT NOT NULL` — which subsystem wrote the event: `billing`, `deploy`, `alert_engine`, `pod_healer`, `game_launcher`
- `pod TEXT` — nullable because server-level events (deploy, backup alert) have no pod
- `payload TEXT NOT NULL DEFAULT '{}'` — JSON blob; schema is per event_type, not enforced at DB level

### Pattern 2: append_event() — fire-and-forget write

Follows activity_log.rs exactly: compute UUID + timestamp before spawn, clone all values into the async block.

```rust
// In event_archive.rs
pub fn append_event(
    db: &SqlitePool,
    event_type: &str,
    source: &str,
    pod: Option<&str>,
    payload: serde_json::Value,
) {
    let id = uuid::Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now()
        .with_timezone(&chrono_tz::Asia::Kolkata)
        .format("%Y-%m-%dT%H:%M:%S")
        .to_string();
    // Clone all string values before move into tokio::spawn
    let db = db.clone();
    let event_type = event_type.to_string();
    let source = source.to_string();
    let pod = pod.map(|s| s.to_string());
    let payload_str = payload.to_string();

    tokio::spawn(async move {
        let _ = sqlx::query(
            "INSERT INTO system_events (id, event_type, source, pod, timestamp, payload)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&event_type)
        .bind(&source)
        .bind(&pod)
        .bind(&timestamp)
        .bind(&payload_str)
        .execute(&db)
        .await;
    });
}
```

**Note:** Takes `&SqlitePool` not `&Arc<AppState>` to keep event_archive.rs independent of AppState. Callers already have access to `state.db`.

### Pattern 3: spawn() — background task

Follows backup_pipeline::spawn() exactly: check config.enabled, log start, loop with interval.tick().await.

```rust
pub fn spawn(state: Arc<AppState>) {
    if !state.config.event_archive.enabled {
        tracing::info!(target: LOG_TARGET, "event_archive disabled — skipping spawn");
        return;
    }

    tokio::spawn(async move {
        tracing::info!(target: LOG_TARGET, "event_archive task started");
        let mut interval = tokio::time::interval(Duration::from_secs(3600)); // hourly tick
        let mut last_remote_transfer: Option<chrono::NaiveDate> = None;

        loop {
            interval.tick().await;
            if let Err(e) = archive_tick(&state, &mut last_remote_transfer).await {
                tracing::error!(target: LOG_TARGET, "archive_tick error: {}", e);
            }
        }
    });
}
```

### Pattern 4: JSONL export

JSONL = one JSON object per line. Each line is a complete event row serialized as JSON. File named `events-YYYY-MM-DD.jsonl` for the **previous** calendar day (IST).

```rust
async fn export_daily_jsonl(db: &SqlitePool, archive_dir: &str) -> anyhow::Result<String> {
    use chrono::Datelike;
    let yesterday = (chrono::Utc::now().with_timezone(&chrono_tz::Asia::Kolkata)
        - chrono::Duration::days(1))
        .date_naive();
    let date_str = yesterday.format("%Y-%m-%d").to_string();
    let filename = format!("events-{}.jsonl", date_str);
    let filepath = format!("{}/{}", archive_dir, filename);

    // Idempotent: if file already exists, skip
    if std::path::Path::new(&filepath).exists() {
        return Ok(filename);
    }

    let rows: Vec<(String, String, String, Option<String>, String, String)> = sqlx::query_as(
        "SELECT id, event_type, source, pod, timestamp, payload
         FROM system_events
         WHERE date(timestamp) = ?
         ORDER BY timestamp ASC",
    )
    .bind(&date_str)
    .fetch_all(db)
    .await?;

    std::fs::create_dir_all(archive_dir)?;
    let mut lines = String::new();
    for (id, event_type, source, pod, timestamp, payload) in &rows {
        let obj = serde_json::json!({
            "id": id,
            "event_type": event_type,
            "source": source,
            "pod": pod,
            "timestamp": timestamp,
            "payload": serde_json::from_str::<serde_json::Value>(payload)
                .unwrap_or(serde_json::Value::String(payload.clone())),
        });
        lines.push_str(&obj.to_string());
        lines.push('\n');
    }
    std::fs::write(&filepath, lines)?;
    tracing::info!(target: LOG_TARGET, "JSONL export: {} ({} events)", filename, rows.len());
    Ok(filename)
}
```

### Pattern 5: SCP transfer — reuse backup_pipeline verbatim

The transfer logic is the same 5-step flow from backup_pipeline.rs Steps A–E. Key flags to copy exactly:
- `ssh -o StrictHostKeyChecking=no -o BatchMode=yes -o ConnectTimeout=10`
- `scp -o StrictHostKeyChecking=no -o BatchMode=yes -o ConnectTimeout=10`
- SCP timeout: `tokio::time::timeout(Duration::from_secs(120), ...)`
- Remote checksum: `sha256sum {remote_path}/{filename}`
- IST window: `ist_hour == 2 || ist_hour == 3`
- Deduplication: `last_remote_transfer: Option<NaiveDate>` — compare with today, skip if already transferred

**Remote path distinction:** Backup files go to `config.backup.remote_path` (e.g., `/root/racecontrol-backups`). JSONL event archives should go to a separate path: `config.event_archive.remote_path` (e.g., `/root/racecontrol-event-archive`). Keeps the two concerns separated on Bono VPS.

### Pattern 6: 90-day purge

Runs in the same `archive_tick()` as the export. Always run AFTER export (so the row still exists when exporting).

```rust
async fn purge_old_events(db: &SqlitePool) -> anyhow::Result<u64> {
    let result = sqlx::query(
        "DELETE FROM system_events WHERE timestamp < datetime('now', '-90 days')",
    )
    .execute(db)
    .await?;
    let deleted = result.rows_affected();
    if deleted > 0 {
        tracing::info!(target: LOG_TARGET, "Purged {} events older than 90 days", deleted);
    }
    Ok(deleted)
}
```

### Pattern 7: REST API — dynamic query builder

Follows BillingListQuery pattern: `WHERE 1=1` base, push_str conditionals, bind_values Vec.

```rust
#[derive(Deserialize)]
struct EventsQuery {
    event_type: Option<String>,
    pod: Option<String>,
    from: Option<String>,   // ISO date YYYY-MM-DD
    to: Option<String>,     // ISO date YYYY-MM-DD
    limit: Option<i64>,
}

async fn get_events(
    State(state): State<Arc<AppState>>,
    Query(q): Query<EventsQuery>,
) -> Json<Value> {
    let limit = q.limit.unwrap_or(200).min(1000);
    let mut query = String::from(
        "SELECT id, event_type, source, pod, timestamp, payload
         FROM system_events WHERE 1=1",
    );
    let mut bind_values: Vec<String> = Vec::new();

    if let Some(ref et) = q.event_type {
        if et.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.') {
            query.push_str(" AND event_type = ?");
            bind_values.push(et.clone());
        }
    }
    if let Some(ref pod) = q.pod {
        if pod.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            query.push_str(" AND pod = ?");
            bind_values.push(pod.clone());
        }
    }
    if let Some(ref from) = q.from {
        if from.len() == 10 && from.chars().all(|c| c.is_ascii_digit() || c == '-') {
            query.push_str(" AND date(timestamp) >= ?");
            bind_values.push(from.clone());
        }
    }
    if let Some(ref to) = q.to {
        if to.len() == 10 && to.chars().all(|c| c.is_ascii_digit() || c == '-') {
            query.push_str(" AND date(timestamp) <= ?");
            bind_values.push(to.clone());
        }
    }
    query.push_str(" ORDER BY timestamp DESC LIMIT ?");
    // ... bind all values + execute
}
```

**Route placement:** `GET /api/v1/events` goes in `staff_routes()` — same as `/debug/activity`. Event archive data (deploy history, alert history, recovery events) is internal operational data. Must NOT be in public_routes (security standing rule: MMA-P1 pattern).

### Pattern 8: EventArchiveConfig

Follows BackupConfig pattern with `serde(default = "...")` on all fields — zero TOML changes required at deploy.

```rust
#[derive(Clone, Debug, Deserialize)]
pub struct EventArchiveConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_archive_dir")]
    pub archive_dir: String,
    #[serde(default = "default_true")]
    pub remote_enabled: bool,
    #[serde(default = "default_remote_host")]
    pub remote_host: String,
    #[serde(default = "default_event_remote_path")]
    pub remote_path: String,
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
}

fn default_archive_dir() -> String { "./data/event-archive".to_string() }
fn default_event_remote_path() -> String { "/root/racecontrol-event-archive".to_string() }
fn default_retention_days() -> u32 { 90 }
```

Add `pub event_archive: EventArchiveConfig` to the `Config` struct with `#[serde(default)]`.

### Anti-Patterns to Avoid

- **Don't hold RwLock across .await in archive_tick:** Snapshot any state needed before the async export/SCP calls. The `state.db` is a pool (clone is cheap), not a lock.
- **Don't reuse backup_pipeline's `last_remote_transfer` variable:** Event archive and DB backup have separate transfer tracking. Each module owns its own `last_remote_transfer: Option<NaiveDate>`.
- **Don't put GET /api/v1/events in public_routes:** Standing rule (security): pod-internal diagnostic data must be staff-auth gated. Same reasoning as debug/activity move in MMA-P1.
- **Don't validate filter inputs by allowing arbitrary strings into SQL:** The BillingListQuery pattern uses character allowlists on all filter values before push_str. Copy this exactly.
- **Don't call `export_daily_jsonl` from `append_event`:** Export is time-gated (runs once per day at 01:00 IST); append is fire-and-forget per event. They are separate concerns.
- **Don't add event_archive.rs to activity_log.rs:** Keep them separate modules with different concerns. activity_log.rs is pod-scoped real-time; event_archive.rs is system-scoped archival.

---

## Significant Events to Capture (EVENT-01 instrumentation sites)

These are the call sites that need `append_event()` added alongside or replacing `log_pod_activity()`:

| Event Type | Source Module | Trigger Location | Payload Fields |
|------------|--------------|------------------|----------------|
| `billing.session_started` | billing.rs | line 2956 (near `log_pod_activity("Session Started")`) | driver_id, tier, allocated_seconds |
| `billing.session_ended` | billing.rs | line 3322 (Session Ended/Completed) | driver_id, driving_seconds, refund_paise |
| `billing.session_expired` | billing.rs | line 1450 | driver_id, driving_seconds |
| `deploy.started` | deploy.rs | line 422 | pod_id, binary_hash |
| `deploy.completed` | deploy.rs | line ~494 | pod_id, binary_hash, duration_secs |
| `deploy.failed` | deploy.rs | lines 454, 462, 521, 529 | pod_id, reason |
| `game.launched` | game_launcher.rs | line 358 | sim_type, car, track |
| `alert.fired` | alert_engine.rs or whatsapp_alerter.rs | where WhatsApp alerts are sent | alert_type, message |
| `pod.recovery` | pod_healer.rs | lines 351, 463, 487 | pod_id, action, before_state |
| `pod.online` | ws/mod.rs | line 196 | pod_number, conn_id |
| `pod.offline` | ws/mod.rs | line 903 | pod_id |

**Implementation note:** `append_event()` should be called FROM these existing modules — it's a thin call that takes `&state.db` and the required fields. It does NOT replace `log_pod_activity()` calls; both can coexist. The events table is a separate archive layer.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| SCP file transfer | Custom TCP file send | `tokio::process::Command::new("scp")` | Already proven in backup_pipeline.rs |
| Remote checksum | Custom protocol | `ssh ... sha256sum file` | Already proven pattern |
| IST time window check | Custom timezone logic | `chrono::Utc::now().with_timezone(&chrono_tz::Asia::Kolkata)` | Already in whatsapp_alerter.rs + backup_pipeline.rs |
| JSONL encoding | Custom serializer | `serde_json::json!(...).to_string() + "\n"` | Standard, no dependency needed |
| Query filtering | ORM/query builder | Dynamic `WHERE 1=1 + push_str` pattern | Proven in BillingListQuery, MaintenanceEventQuery |
| Idempotent JSONL writes | DB flag/lock | `std::path::Path::new(&filepath).exists()` check | Simple filesystem check, no DB transaction needed |

---

## Common Pitfalls

### Pitfall 1: JSONL export includes today's incomplete data
**What goes wrong:** If export runs at midnight the JSONL file includes partial data for the day still in progress.
**Why it happens:** `date(timestamp) = today` catches events from the current day.
**How to avoid:** Always export **yesterday** (`chrono::Utc::now() - Duration::days(1)`). The tick runs hourly — the nightly window (02:00-03:59 IST) means the export for yesterday runs after midnight when the day is complete.
**Warning signs:** JSONL file for today's date appearing before midnight.

### Pitfall 2: Duplicate JSONL files on server restart
**What goes wrong:** Server restarts during the 02:00 window, runs archive_tick again, tries to re-export and re-transfer.
**Why it happens:** `last_remote_transfer` is in-memory and lost on restart.
**How to avoid:** (1) JSONL file existence check before writing (`if filepath.exists() { return Ok(filename) }`). (2) Same `last_remote_transfer: Option<NaiveDate>` pattern as backup_pipeline — prevents duplicate SCP within the same window. On restart, the file-exists check prevents re-export; SCP still runs and idempotently transfers the existing file (sha256 will match).
**Warning signs:** JSONL files being rewritten with different line counts.

### Pitfall 3: 90-day purge runs before export
**What goes wrong:** Rows that should be in today's JSONL get deleted before export runs.
**Why it happens:** Wrong order in archive_tick.
**How to avoid:** In `archive_tick()`, ALWAYS run export first, purge second. The purge only deletes rows older than 90 days, so today's data is safe — but still enforce the order explicitly in code with a comment.
**Warning signs:** JSONL files with fewer rows than expected for old dates.

### Pitfall 4: SSH flags missing on SCP command
**What goes wrong:** SCP hangs indefinitely waiting for interactive host key prompt.
**Why it happens:** New SSH host not in known_hosts.
**How to avoid:** Always use `-o StrictHostKeyChecking=no -o BatchMode=yes -o ConnectTimeout=10` on both `ssh` and `scp` calls. Copy from backup_pipeline.rs verbatim — these flags are non-negotiable on an automated server.
**Warning signs:** archive_tick stalls indefinitely (no timeout hit if ConnectTimeout omitted).

### Pitfall 5: payload column stores raw string instead of parseable JSON
**What goes wrong:** API returns `"payload": "{\"key\":\"value\"}"` (double-encoded string) instead of `"payload": {"key": "value"}` (object).
**Why it happens:** `serde_json::to_string()` stores the string, but the HTTP response handler doesn't re-parse it.
**How to avoid:** In the GET /api/v1/events handler, always `serde_json::from_str::<Value>(&payload).unwrap_or(Value::String(payload))` before including in the response JSON. The JSONL export handler already does this (Pattern 4 above).
**Warning signs:** Frontend shows payload as a string literal instead of an object.

### Pitfall 6: Event archive config not added to Config struct default
**What goes wrong:** Server panics on startup: `missing field event_archive` if serde requires it.
**Why it happens:** Adding a field without `#[serde(default)]`.
**How to avoid:** Add `#[serde(default)]` to the `event_archive` field in `Config` struct. This matches BackupConfig pattern — BackupConfig was added with `#[serde(default)]` so no racecontrol.toml changes were needed at deploy.
**Warning signs:** Server fails to start on first deploy ("missing field 'event_archive'").

---

## Code Examples

### Full archive_tick() skeleton
```rust
// Source: backup_pipeline.rs pattern, adapted for event archive
async fn archive_tick(
    state: &Arc<AppState>,
    last_remote_transfer: &mut Option<chrono::NaiveDate>,
) -> anyhow::Result<()> {
    let archive_dir = state.config.event_archive.archive_dir.clone();

    // Step 1: Export yesterday's events to JSONL (idempotent)
    let filename = export_daily_jsonl(&state.db, &archive_dir).await?;

    // Step 2: Purge events older than retention_days (AFTER export)
    let retention = state.config.event_archive.retention_days as i64;
    purge_old_events(&state.db, retention).await?;

    // Step 3: SCP the JSONL file to Bono VPS (once per day, IST 02:00-03:59)
    if state.config.event_archive.remote_enabled {
        let filepath = format!("{}/{}", archive_dir, filename);
        if let Err(e) = transfer_jsonl_to_remote(state, &filepath, &filename, last_remote_transfer).await {
            tracing::error!(target: LOG_TARGET, "JSONL remote transfer failed: {}", e);
        }
    }

    Ok(())
}
```

### Route registration in staff_routes()
```rust
// In api/routes.rs staff_routes() — after existing event-related routes
.route("/events", get(get_events))
```

---

## Existing Table Disambiguation

**Important:** `db/mod.rs` already has a table named `events` (line 154 in the DB). This is the **hotlap/competition events** table (drivers competing in timed events). It is NOT a system event log. The new table for Phase 302 must be named `system_events` to avoid confusion and collision.

Also note `scheduler_events` (line 1549) — this is only for smart scheduler WoL/wake events. Not related.

**Summary of existing event-like tables:**
| Table | Purpose | Related to Phase 302? |
|-------|---------|----------------------|
| `events` | Hotlap competition events (drivers, tracks) | NO — different domain |
| `event_entries` | Driver entries for hotlap events | NO |
| `scheduler_events` | Pod wake/sleep scheduler log | NO |
| `pod_activity_log` | Per-pod real-time operational log | PARTIAL — activity_log.rs is the sister module |
| `billing_events` | Billing session state transitions | NO — internal billing FSM |
| `game_launch_events` | Game launch metric events | NO — metrics only |
| `recovery_events` | Pod recovery metric events | NO — metrics only |
| `system_events` | **NEW Phase 302** — system-wide archive | YES |

---

## Environment Availability

Step 2.6: SKIPPED — phase is pure code/config changes. SCP and SSH are already used by backup_pipeline.rs and confirmed working (Phase 300 shipped). No new external tools required.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + `#[tokio::test]` |
| Config file | none — inline in module under `#[cfg(test)]` |
| Quick run command | `cargo test -p racecontrol event_archive` |
| Full suite command | `cargo test -p racecontrol && cargo test -p rc-common && cargo test -p rc-agent` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| EVENT-01 | append_event() inserts a row to system_events | unit | `cargo test -p racecontrol event_archive::tests::append_event_inserts_row` | Wave 0 |
| EVENT-02 | export_daily_jsonl() creates JSONL with correct rows | unit | `cargo test -p racecontrol event_archive::tests::export_creates_jsonl` | Wave 0 |
| EVENT-02 | export_daily_jsonl() is idempotent (file exists = skip) | unit | `cargo test -p racecontrol event_archive::tests::export_is_idempotent` | Wave 0 |
| EVENT-03 | purge_old_events() deletes rows older than 90 days, keeps recent | unit | `cargo test -p racecontrol event_archive::tests::purge_deletes_old_keeps_recent` | Wave 0 |
| EVENT-03 | purge runs AFTER export in archive_tick (ordering) | unit | `cargo test -p racecontrol event_archive::tests::purge_after_export_order` | Wave 0 |
| EVENT-05 | GET /api/v1/events returns filtered events by type | integration | `cargo test -p racecontrol api::tests::events_filter_by_type` | Wave 0 |
| EVENT-05 | GET /api/v1/events rejects invalid filter values | unit | `cargo test -p racecontrol api::tests::events_filter_validation` | Wave 0 |

### Test patterns from backup_pipeline.rs (HIGH confidence, copy these)

backup_pipeline.rs has 9 unit tests (lines 625-871) following this pattern:
```rust
#[test]
fn test_name() {
    let tmp = tempfile::tempdir().expect("Failed to create temp dir");
    let dir = tmp.path();
    // create files, call function, assert
}
```

Tests use `tempfile::TempDir` for filesystem isolation. Same pattern should be used for event_archive JSONL tests.

For DB tests: use `sqlx::sqlite::SqlitePoolOptions` with an in-memory `:memory:` database in tokio::test blocks — same pattern used in billing/route tests.

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol event_archive`
- **Per wave merge:** `cargo test -p racecontrol && cargo test -p rc-common && cargo test -p rc-agent`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/racecontrol/src/event_archive.rs` — module does not exist yet; unit tests live here under `#[cfg(test)]`
- [ ] Route handler test for EVENT-05 — inline test at bottom of routes.rs (existing test pattern: lines 18288-18372 show inline route tests)

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| File copy for SQLite backup | VACUUM INTO (WAL-safe) | Phase 300 | Locked decision — use VACUUM INTO for DB backups |
| Direct SSH for remote transfer | SCP with SHA256 verify | Phase 300 | Template for JSONL SCP in this phase |
| Single-fetch-at-boot | Periodic re-fetch via tokio task | Phase 293/boot-resilience | Background task pattern required for any periodic work |

---

## Open Questions

1. **archive_tick frequency:** The backup pipeline ticks hourly. Should event_archive tick hourly or daily?
   - What we know: Hourly ticks are cheap (export is idempotent via file-exists check, purge is cheap DELETE, SCP only runs in 02:00-03:59 window).
   - Recommendation: Hourly tick (same as backup) — ensures SCP happens within the window even if server is briefly restarted. Idempotency guarantees make this safe.

2. **Which call sites to instrument for EVENT-01?**
   - What we know: The table above lists 11 primary call sites across billing.rs, deploy.rs, game_launcher.rs, alert_engine.rs, pod_healer.rs, ws/mod.rs.
   - Recommendation: Instrument the 6 highest-signal events for the initial phase (session_started, session_ended, deploy.started/completed/failed, pod.recovery). Game launches and pod online/offline are lower priority but easy to add.

3. **`pod` column type — TEXT vs INTEGER pod_number?**
   - What we know: pod_activity_log uses `pod_id TEXT` (internal UUID like "pod_uuid_123") and `pod_number INTEGER` separately. Some events (deploy, alert) are server-level with no pod at all.
   - Recommendation: Use `pod TEXT NULL` storing the human-readable pod identifier like `"pod_4"` or `"server"` — not the UUID. This is more useful for cross-archive queries without needing a pods table JOIN. Server-level events store NULL.

---

## Sources

### Primary (HIGH confidence)
- `crates/racecontrol/src/backup_pipeline.rs` — SCP pattern, IST window, SHA256 verify, NaiveDate deduplication, all code examples verified
- `crates/racecontrol/src/activity_log.rs` — fire-and-forget insert pattern
- `crates/racecontrol/src/db/mod.rs` — table creation patterns, index patterns, ALTER TABLE migration pattern, existing table disambiguation
- `crates/racecontrol/src/config.rs` (lines 933-984) — BackupConfig pattern with serde defaults
- `crates/racecontrol/src/api/routes.rs` (lines 4194-4230) — BillingListQuery dynamic filter pattern
- `crates/racecontrol/src/api/routes.rs` (lines 1513-1554) — MaintenanceEventQuery multi-filter pattern
- `crates/racecontrol/src/api/routes.rs` (lines 306, 95-96) — staff_routes auth placement for internal data
- `crates/racecontrol/src/lib.rs` — pub mod registration pattern
- `crates/racecontrol/src/main.rs` (lines 956-961) — spawn registration pattern
- `crates/racecontrol/src/whatsapp_alerter.rs` (line 41) — ist_now_string() helper

### Secondary (MEDIUM confidence)
- Multiple `log_pod_activity` call sites across billing.rs, deploy.rs, game_launcher.rs, pod_healer.rs, ws/mod.rs — confirmed instrumentation sites for EVENT-01

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — zero new dependencies, all libs already in Cargo.toml
- Architecture patterns: HIGH — all patterns copied from or modeled on Phase 300 (backup_pipeline.rs) which is in the same codebase
- Pitfalls: HIGH — derived from direct inspection of backup_pipeline.rs and existing query patterns
- Event instrumentation sites: HIGH — confirmed by grepping all log_pod_activity callers

**Research date:** 2026-04-01
**Valid until:** 2026-05-01 (stable codebase, no fast-moving external dependencies)
