**EDGE CASE & INTEGRATION AUDIT: v29.0 Meshed Intelligence**

---

### 1. EMPTY STATE
**P1: `maintenance_store::init*tables()` fails with empty DB on v29.0 launch**
- **File:** `crates/racecontrol/src/maintenance_store.rs:init_maintenance_tables()`
- **Description:** Migration code uses `CREATE TABLE IF NOT EXISTS` so empty DB is safe. But during fresh init, foreign key creation in `attendance_records` (`FOREIGN KEY (employee_id) REFERENCES employees(id)`) needs `employees` table to exist FIRST. Current init order in `main.rs` calls `init_maintenance_tables()` before `init_business_tables()` and `init_hr_tables()` which both have EMPLOYEES requirement. Tables referenced in FKs must be created in same transaction or before.

**P2: Anomaly scanner panics on zero pods**
- **File:** `crates/racecontrol/src/maintenance_engine.rs:scan_for_anomalies()`
- **Description:** Pattern matching `(1..=8).contains(&p)` fails when `current_pods_online()` returns empty list. Add guard clause: `if current_pods_online.is_empty() { tracing::warn!("No pods online for anomaly scan"); return }`

---

### 2. MIGRATION SAFETY
**P1: FK missing in maintenance_tasks**
- **File:** `crates/racecontrol/src/maintenance_store.rs:init_maintenance_tables()`
- **Description:** `maintenance_tasks.source_event_id` references `maintenance_events.id` but no `FOREIGN KEY` constraint. Add `FOREIGN KEY (source_event_id) REFERENCES maintenance_events(id) ON DELETE SET NULL`

**P2: `is_active` vs `status` inconsistency**
- **File:** `crates/racecontrol/src/maintenance_store.rs:init_hr_tables()`
- **Description:** `employees` table has `is_active INTEGER DEFAULT 1` while `maintenance_tasks` uses `status TEXT` with values like 'Open'. In query centers like `data_collector::collect_venue_snapshot()::staff_on_duty`, this mixed boolean/string model causes cast issues. Standardize to TEXT status with consistent values.

---

### 3. CLOUD SYNC
**P3: Extended sync fails silently on old Bono VPS**
- **File:** `crates/racecontrol/src/cloud_sync.rs:sync_maintenance_data()`
- **Description:** New v29.0 sync payloads (HR/staff counts, maintenance events) sent to older Bono VPS builds cause parsing panics in cloud receiver. No version handshake in existing `push_via_relay()`. Add `if config.cloud.min_v16_version < "29.0" { skip_extended_sync() }`

---

### 4. NETWORK PARTITION
**P2: WS drop triggers pod_decommission during self-healing**
- **File:** crates/racecontrol/src/self_healing.rs:HealingAction::MarkPodUnavailable**
- **Description:** When `Ollama` WS drops AND server-pod WS drops simultaneously, `self_healing` marks pod unavailable without checking `AcStatus`. Should require consecutive WS failures + `AcStatus::Off` before decrediting.

---

### 5. WINDOWS SESSION 0
**P1: `nvidia-smi` hangs in service context**
- **File:** `crates/pod_agent/src/main.rs:event_loop()`
- **Description:** New v29.0 GPU telemetry collection using `nvidia-smi` fails in Session 0 when run as Windows service (returns `C:\Program` is not recognized). Missing `gpu_query_tool.exe` wrapper for service mode.

---

### 6. STARTUP ORDER
**P1: `init_feedback_tables` before `init_maintenance_tables` breaks FK**
- **File:** `crates/racecontrol/src/main.rs`
- **Description:** `feedback_loop::init_feedback_tables()` creates `admin_overrides.recommendation_id` which references `maintenance_tasks.id`. Called before `init_maintenance_tables()` in `main.rs`. Swap init order.

**P3: `spawn_data_collector` delays pod readiness check**
- **File:** `crates/racecontrol/src/data_collector.rs:spawn_data_collector()`
- **Description:** 3-minute sleep before `collect_venue_snapshot` delays pod health checks. Reduce to 30s.

---

### 7. GRACEFUL DEGRADATION
**P3: Telemetry writer crash takes down pod agent**
- **File:** `crates/pod_agent/src/event_loop.rs:on_hardware_telemetry()`
- **Description:** `telemetry_writer_tx` panic during extended telem collection kills entire agent. Wrap in `if let Err(e) = telemetry_writer_tx.send().await { tracing::error!("Telemetry writer down: {}", e) }`

---

### 8. ROLLBACK
**P2: Old pods send `gpu_temp_celsius` as float to v29 server `hardware_telemetry`**
- **File:** `migrations/__init_maintenance_tables.sql`
- **Description:** `hardware_telemetry.gpu_temp_celsius` changed from `INTEGER` to `REAL` in v29.0. Old pods send integer varying by 10x magnitude. Need `ALTER TABLE ALTER COLUMN` migration script.

---

### 9. CONCURRENT ACCESS
**P2: `maintenance_tasks` status updated without transaction**
- **File:** `crates/racecontrol/src/maintenance_api.rs:update_task_status()`
- **Description:** API updates `maintenance_tasks.status` without `BEGIN TRANSACTION`. Two users marking task 'Completed' simultaneously creates phantom duplicates. Add `sqlx::query("BEGIN IMMEDIATE")` before update.

---

**Recommendations:** 
1. Add FK: `ALTER TABLE maintenance_tasks ADD FOREIGN KEY (source_event_id) REFERENCES maintenance_events(id)`
2. Wrap all `CREATE TABLE` calls in single transaction per domain
3. Add `min_v16_version` to cloud config struct
4. Ship `gpu_query_tool.exe` with Windows installer for service-mode GPUs
5. Reorder init in `main.rs`: feedback AFTER maintenance tables
6. Change `maintenance_tasks.status` TEXT → INTEGER with status codes

**Critical Fixes:**
1. Fix FK creation order (blocker)
2. Add GPU tool for Session 0
3. Thin cloud sync for old VPS