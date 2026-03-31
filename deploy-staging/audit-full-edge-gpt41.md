Here is an expert **Edge Case & Integration Audit** for **v29.0 Meshed Intelligence** (across the areas you listed):

---

### 1. EMPTY STATE (DB and Runtime)
---

#### a) Zero pods, empty DB, no telemetry, no employees, no events

**Findings:**

| P | File:Function | Description | Fix |
|---|---------------|-------------|-----|
| P1 | crates/racecontrol/src/data_collector.rs:collect_venue_snapshot | Snapshot reports hardcoded pod_count_online = 8 regardless of actual pods. Other fields (open_tasks, revenue, etc) default to 0 unless SQL errors. | Infer pod count dynamically from fleet health (not hardcoded). Handle DB connection error as a real empty state, not zero. Propagate Option or Result to API/collector. |
| P2 | main.rs (init order) | If DB is actually empty with no `employees`, `maintenance_events`, etc: All aggregations (sum/count) fallback to zero with `unwrap_or(0)`. No crash, but API returns empty dashboard (may look like system failure). | Add explicit "empty state initialized" logs. Consider in-memory "bootstrap" state so UI can show "System initializing" instead of zeros. |
| P3 | crates/racecontrol/src/data_collector.rs:RUL threshold check | RUL auto-task creation may insert "phantom" tasks with pod_id 0 if telemetry table is empty or pod_id parsing fails. | Validate pod_id for all inserts. Do not create default pod_id=0 tasks (already partly mitigated via filter). Log more prominently if metrics table is entirely empty. |

#### b) No telemetry rows

| P | File | Description | Fix |
|---|------|-------------|-----|
| P2 | data_collector.rs / telemetry_store.rs | Aggregates (min, max, avg) return Option<None>, ends up as None/0 in results. Downstream metrics use `unwrap_or(0)` so no crash. | For UX: Attach "no data since startup" label. WARN log if telemetry rows are missing for >1h. |

---

### 2. MIGRATION SAFETY

#### a) Table creation logic

- All new tables (`CREATE TABLE IF NOT EXISTS ...`) use safe creation, not `ALTER TABLE`.
- Some referential links (attendance -> employees) use explicit `FOREIGN KEY`.

**Findings:**

| P | File:Function | Description | Fix |
|---|---------------|-------------|-----|
| P1 | maintenance_store.rs:(all init_*) | If schema deploy is partial or out of order, the new features just silently start with empty tables; won't affect legacy data. | N/A (design is safe for additive tables). |
| P2 | telemetry_store.rs | If pre-existing table has *incompatible column types* (e.g. existing `maintenance_events` with mismatched columns in a different deployment), data loss risk may exist. | Bake in startup migration compatibility SQL (compare schema PRAGMA vs expected schema, abort/FATAL if columns do not match). |
| P2 | telemetry_store.rs / feedback_loop.rs | Indexes use `CREATE INDEX IF NOT EXISTS` (correct), but lack index *naming uniqueness* validation. | Ensure index names are unique per table to avoid eventual conflicts. |

---

### 3. CLOUD SYNC INTEROP

#### a) New sync payloads for maintenance/HR/analytics

| P | File:Function | Description | Fix |
|---|---------------|-------------|-----|
| P1 | cloud_sync.rs:sync_maintenance_data | Payloads now include recent `maintenance_events`, `employees` count, revenue, business metrics. If Bono VPS is running pre-v29 server — its `/sync/push` handler will ignore or error on these new fields. | Add versioning to payload (explicit "v29" identifier). On push handler, downcast/ignore unknown fields gracefully. Validate cloud response to fail safe, not silently drop. |
| P2 | cloud_sync.rs | Any fields not recognized by old binary are dropped — may lead to partial data loss until both sides upgraded. | Patch old server to "accept junk" in push payload (strict superset). Add monitoring for dropped fields/warnings. |

---

### 4. NETWORK PARTITION (resilience to API/service failure)

#### a) Server-pod WS drops

| P | File:Function | Description | Fix |
|---|---------------|-------------|-----|
| P2 | pod event loop (event_loop.rs) | Pod marks itself as disconnected/unavailable after N missed heartbeats (WS reconnect logic present). Initial drop disables kiosk after 60s. | Add "partition" state to pod_availability so dashboards differentiate network vs hard faults. |

#### b) Ollama unreachable

| P2 | predictive_maintenance.rs (if using Ollama for ML inferencing) | If Ollama UNREACHABLE, dependency timeouts may block background tasks that feed inputs (e.g. RUL predictions). | Add timeout & fallback for ML inferencing. On error: degrade feature, do not propagate panic. Emit error-level trace specifically for ML fallback. |

#### c) WhatsApp API down

| P3 | alert_engine.rs (spawn_alert_checker) | If WhatsApp failures (network or API quota), alert delivery errors are WARN-level, may retry indefinitely. No circuit breaker. | Add fail-fast/circuit breaker logic: after 5 repeated API failures, pause WhatsApp alerts for 10min, and log summary. |

---

### 5. WINDOWS SESSION (Session 0 - service context)

#### a) New code that breaks outside interactive desktop

| P2 | predictive_maintenance.rs / event loop | If hardware telemetry collection (especially something like GPU or process enumeration) is called from service/Session 0, DLLs may fail to load (“no interactive session”), telemetry is `None`. | Try-catch all hardware probes; if running as NT SERVICE, skip GUI-dependent calls. Emit warning that pod is running in non-interactive mode. |

---

### 6. STARTUP ORDER/RACE (main.rs)

| P | File:Function | Description | Fix |
|---|---------------|-------------|-----|
| P1 | main.rs (init order) | Multiple state fields (telemetry_writer_tx, rating_tx, etc.) are assigned via `Arc::get_mut` post-construct. If code before full assignment accesses state, fields may still be None. | Freeze all API handlers/spawns on `AppState::new()` until `init_done: true`. Only set to true after all critical fields are assigned. |
| P2 | main.rs | Some background tasks (aggregators) are started before feedback_loop/pricing tables created. If ran out of order, tables may not exist yet and aggregator crashes. | Move all task spawns *after* all table inits complete, or make all uses of DB robust to "table not found" with init-in-loop fallback. |

---

### 7. GRACEFUL DEGRADATION (background task failures)

| P | File:Function | Description | Fix |
|---|---------------|-------------|-----|
| P1 | All tokio background spawns | Most errors in background tasks (data retention, cloud sync, feedback table init, etc.) are caught and logged, but **if a panic occurs in critical path, may bring down main tokio runtime** (if not spawned in catch_unwind). | Wrap all `tokio::spawn` background tasks in `catch_unwind`, log all panics at FATAL and continue. |
| P2 | Any networked push loop | If cloud sync/WhatsApp delivery repeatedly fails, logs a warning, but main server keeps running. **Graceful.** | N/A (intended). Best practice is to auto-throttle or escalate after prolonged failures. |

---

### 8. ROLLBACK MISMATCH (server v29.0 + old pods)

#### a) New server features, old pods

| P | File:Function | Description | Fix |
|---|---------------|-------------|-----|
| P1 | event ingestion | Server expects extended `AgentMessage::ExtendedTelemetry`, old pod doesn't send new fields — server must default fields to Option/None. | Defensive deserialize all new fields with Option. If missing, continue. |
| P2 | pricing/maintenance feedback APIs | Server-side features become no-op for old pods (e.g. they never send/receive pricing proposals). | N/A (desired: features are only enabled on upgraded pods). |

---

### 9. CONCURRENT ACCESS (race on API modifications)

#### a) Multiple writes to same task/event

| P | File:Function | Description | Fix |
|---|---------------|-------------|-----|
| P1 | maintenance_store.rs | Lacks explicit row locking or version checking. If two admins update a maintenance_task in same second, *last write wins* (can overwrite fields). | Add optimistic concurrency (updated_at + version field) or prevent double-submissions in UI. Return error on version mismatch. |
| P2 | feedback_loop.rs | If two feedback records for same event, both can insert (id is PK, but otherwise no guard). | Consider upsert with conflict-check for id/primary keys. |

---

## High-Level Recommendations

- **Add logs for empty state**: Separate “empty” from “zero” in observability and user interface.
- **Enforce schema/version checks on cloud sync**: Prevent partial data loss during rolling upgrades.
- **Race-proof task/event updates**: Use optimistic concurrency when modifying tasks/events.
- **Gracefully degrade critical background tasks**: Never crash the whole server on background panic.
- **Startup dependencies**: All tables must be present before background/aggregator tasks run — refactor ordering in main.

*Audit performed across all code snippets and system integrations in v29.0 context. For any P1, patch before GA. Ask for targeted examples if you want field-level unsafe access detail!*

---

**Let me know if you want deep dives on a specific domain or code path.**