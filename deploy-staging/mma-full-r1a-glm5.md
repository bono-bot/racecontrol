# FULL AUDIT REPORT: v29.0 Meshed Intelligence
## Racing Point eSports — 8-Pod Racing Simulator Venue

**Auditor**: Senior Rust/Systems Security  
**Scope**: Bundles A (maintenance_models, maintenance_store, anomaly_engine, self_healing, feedback_loop, business_alerts, escalation, data_collector, ollama_client)  
**Date**: [Audit Timestamp]

---

# P1 CRITICAL FINDINGS

## P1-01: MONEY LOADED AS f64 — RULE VIOLATION
**File**: `business_alerts.rs:check_business_alerts()`  
**Severity**: P1 CRITICAL  
**Description**: All monetary values (`revenue_gaming_paise`, `expense_maintenance_paise`) are loaded as `f64` via `sqlx::query_scalar`, violating the mandatory "All money MUST be integer paise (i64), NEVER f64" rule. This causes silent precision loss in financial comparisons.

```rust
// VIOLATION — current code
let today_rev: f64 = sqlx::query_scalar(
    "SELECT COALESCE(revenue_gaming_paise + revenue_cafe_paise, 0) FROM daily_business_metrics WHERE date = ?1"
).bind(&today).fetch_one(pool).await.unwrap_or(0.0);

// FIX — use i64 for all monetary values
let today_rev: i64 = sqlx::query_scalar(
    "SELECT COALESCE(revenue_gaming_paise + revenue_cafe_paise, 0) FROM daily_business_metrics WHERE date = ?1"
).bind(&today).fetch_one(pool).await.unwrap_or(0);

let avg_rev: i64 = sqlx::query_scalar(
    "SELECT COALESCE(AVG(revenue_gaming_paise + revenue_cafe_paise), 0) FROM daily_business_metrics WHERE date >= date('now', '-7 days')"
).fetch_one(pool).await.unwrap_or(0);

// Comparison must use integer arithmetic
if avg_rev > 0 && today_rev * 10 < avg_rev * 7 {  // today_rev < avg_rev * 0.7 without f64
    let drop_pct = ((avg_rev - today_rev) * 100) / avg_rev;
    alerts.push(BusinessAlert {
        alert_type: "RevenueDropAlert".into(),
        severity: "High".into(),
        message: format!(
            "Revenue today ₹{:.2} is {}% below 7-day average ₹{:.2}",
            today_rev as f64 / 100.0,
            drop_pct,
            avg_rev as f64 / 100.0
        ),
        channel: AlertChannel::Both,
        timestamp: Utc::now().to_rfc3339(),
        value: today_rev as f64,
        threshold: avg_rev as f64 * 0.7,
    });
}
```

**Same violation exists for**: `maint_cost`, `month_rev` — all must be `i64`.

---

## P1-02: INTEGER CASTS USE `as` INSTEAD OF `try_from`
**File**: `maintenance_store.rs` — multiple locations  
**Severity**: P1 CRITICAL  
**Description**: Systematic violation of "All integer casts from DB MUST use try_from, not `as`". The `as` operator silently truncates/wraps on overflow.

```rust
// VIOLATION — insert_event()
.bind(event.pod_id.map(|p| p as i64))
.bind(event.customers_affected.map(|c| c as i64))
.bind(event.downtime_minutes.map(|d| d as i64))

// VIOLATION — insert_task()
.bind(task.pod_id.map(|p| p as i64))
.bind(task.priority as i64)

// VIOLATION — query_events(), query_tasks()
.bind(limit as i64)

// VIOLATION — upsert_daily_metrics()
.bind(metrics.sessions_count as i64)

// VIOLATION — insert_employee()
.bind(employee.is_active as i64)

// FIX — use try_from with proper error handling
fn u8_to_i64(v: u8) -> i64 {
    i64::from(v)
}

fn u32_to_i64(v: u32) -> anyhow::Result<i64> {
    i64::try_from(v).context("u32 overflow converting to i64")
}

// In insert_event:
.bind(event.pod_id.map(u8_to_i64))
.bind(event.customers_affected.and_then(|c| u32_to_i64(c).ok()))
.bind(event.downtime_minutes.and_then(|d| u32_to_i64(d).ok()))

// In insert_task:
.bind(task.pod_id.map(u8_to_i64))
.bind(i64::from(task.priority))

// In query functions:
.bind(i64::try_from(limit).context("limit overflow")?)

// In upsert_daily_metrics:
.bind(i64::from(metrics.sessions_count))

// In insert_employee:
.bind(if employee.is_active { 1i64 } else { 0i64 })
```

---

## P1-03: SILENT DATA CORRUPTION IN ROW CONVERSIONS
**File**: `maintenance_store.rs:row_to_event()`, `row_to_task()`  
**Severity**: P1 CRITICAL  
**Description**: Invalid database values are silently clamped/replaced instead of returning errors. This hides data integrity problems and produces incorrect records.

```rust
// VIOLATION — pod_id 255 becomes 8, negative values become 1
pod_id: row.pod_id.and_then(|p| {
    u8::try_from(p.clamp(0, 255)).ok().map(|v| v.clamp(1, 8))
}),

// VIOLATION — negative customers_affected becomes u32::MAX
customers_affected: row.customers_affected.map(|c| {
    u32::try_from(c.max(0)).unwrap_or(u32::MAX)
}),

// VIOLATION — invalid priority defaults to 50
priority: u8::try_from(row.priority.clamp(0, 100)).unwrap_or(50),

// FIX — return errors for invalid data
fn row_to_event(row: EventRow) -> anyhow::Result<MaintenanceEvent> {
    let pod_id = match row.pod_id {
        Some(p) => {
            let val = u8::try_from(p)
                .map_err(|_| anyhow::anyhow!("pod_id {} out of u8 range", p))?;
            if val < 1 || val > 8 {
                anyhow::bail!("pod_id {} out of valid range 1-8", val);
            }
            Some(val)
        }
        None => None,
    };

    let customers_affected = match row.customers_affected {
        Some(c) => {
            if c < 0 {
                anyhow::bail!("customers_affected is negative: {}", c);
            }
            Some(u32::try_from(c)?)
        }
        None => None,
    };

    let downtime_minutes = match row.downtime_minutes {
        Some(d) => {
            if d < 0 {
                anyhow::bail!("downtime_minutes is negative: {}", d);
            }
            Some(u32::try_from(d)?)
        }
        None => None,
    };

    // ... rest of function with proper error returns
    Ok(MaintenanceEvent {
        pod_id,
        customers_affected,
        downtime_minutes,
        // ...
    })
}

// In row_to_task:
let priority = u8::try_from(row.priority)
    .map_err(|_| anyhow::anyhow!("priority {} out of u8 range", row.priority))?;
if priority > 100 {
    anyhow::bail!("priority {} exceeds maximum 100", priority);
}
```

---

## P1-04: DATE PARSE FALLBACK CORRUPTS DATA
**File**: `maintenance_store.rs:row_to_event()`, `row_to_task()`  
**Severity**: P1 CRITICAL  
**Description**: When `detected_at` or `created_at` fails to parse, the code falls back to `Utc::now()`. This silently corrupts event timing data, making MTTR calculations and temporal queries meaningless.

```rust
// VIOLATION — uses Utc::now() on parse failure
let detected_at = match row.detected_at_str.as_deref() {
    Some(s) => match DateTime::parse_from_rfc3339(s) {
        Ok(d) => d.with_timezone(&Utc),
        Err(e) => {
            tracing::warn!(...);
            Utc::now()  // WRONG — corrupts event timing
        }
    },
    None => {
        tracing::warn!(...);
        Utc::now()  // WRONG — corrupts event timing
    }
};

// FIX — return error instead of corrupting data
let detected_at = match row.detected_at_str.as_deref() {
    Some(s) => DateTime::parse_from_rfc3339(s)
        .map_err(|e| anyhow::anyhow!("detected_at parse failed for {}: '{}' — {}", row.id, s, e))?
        .with_timezone(&Utc),
    None => anyhow::bail!("detected_at is NULL for event {}", row.id),
};

let resolved_at = match row.resolved_at_str.as_deref() {
    Some(s) => Some(
        DateTime::parse_from_rfc3339(s)
            .map_err(|e| anyhow::anyhow!("resolved_at parse failed for {}: '{}'", row.id, s, e))?
            .with_timezone(&Utc)
    ),
    None => None,
};
```

---

## P1-05: RACE CONDITION IN AUTO-ASSIGN TASK
**File**: `maintenance_store.rs:auto_assign_task()`  
**Severity**: P1 CRITICAL  
**Description**: The assignment logic reads task data, queries employee loads, then updates — all without a transaction. Two concurrent calls can assign the same task to different employees, or both select the "least loaded" employee.

```rust
// VIOLATION — no transaction, race condition between read and write
pub async fn auto_assign_task(pool: &SqlitePool, task_id: &str) -> anyhow::Result<Option<String>> {
    let component = sqlx::query_scalar(...).await?;  // READ
    let employees = sqlx::query_as(...).await?;       // READ
    for (emp_id, _, _) in &employees {
        let load: i64 = sqlx::query_scalar(...).await?;  // READ (N times!)
        if load < best_load { best_id = Some(emp_id.clone()); }
    }
    sqlx::query("UPDATE ... SET assigned_to = ?1").await?;  // WRITE — stale data!
}

// FIX — use SQLite transaction with row locking
pub async fn auto_assign_task(pool: &SqlitePool, task_id: &str) -> anyhow::Result<Option<String>> {
    let mut tx = pool.begin().await?;

    // Verify task exists and is unassigned (with row lock hint)
    let component: Option<String> = sqlx::query_scalar(
        "SELECT component FROM maintenance_tasks WHERE id = ?1 AND assigned_to IS NULL"
    )
    .bind(task_id)
    .fetch_optional(&mut *tx)
    .await?;

    let component = match component {
        Some(c) => c,
        None => {
            tx.rollback().await?;
            return Ok(None); // Task doesn't exist or already assigned
        }
    };

    // Get employees with their task counts in a single query
    let candidates = sqlx::query_as::<_, (String, String, String, i64)>(
        "SELECT e.id, e.name, e.skills, 
                COALESCE(t.cnt, 0) AS open_task_count
         FROM employees e
         LEFT JOIN (
             SELECT assigned_to, COUNT(*) as cnt
             FROM maintenance_tasks
             WHERE status NOT IN ('Completed', 'Failed', 'Cancelled')
             GROUP BY assigned_to
         ) t ON t.assigned_to = e.id
         WHERE e.is_active = 1 
           AND (e.role = 'Technician' OR e.role = 'Manager')
         ORDER BY open_task_count ASC
         LIMIT 20"
    )
    .fetch_all(&mut *tx)
    .await?;

    let component_lower = component.to_lowercase().replace('"', "");
    let mut best_id: Option<String> = None;

    for (emp_id, _, skills_json, load) in &candidates {
        let skills: Vec<String> = serde_json::from_str(skills_json).unwrap_or_default();
        let has_skill = skills.iter().any(|s| {
            s.to_lowercase().contains(&component_lower) || s.to_lowercase() == "general"
        });

        if has_skill || skills.is_empty() {
            best_id = Some(emp_id.clone());
            break; // First match has lowest load due to ORDER BY
        }
    }

    if let Some(ref emp_id) = best_id {
        // Update within transaction — guaranteed atomic
        let result = sqlx::query(
            "UPDATE maintenance_tasks SET assigned_to = ?1, status = 'Assigned' WHERE id = ?2 AND assigned_to IS NULL"
        )
        .bind(emp_id)
        .bind(task_id)
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() == 0 {
            // Concurrent assignment won — rollback and return None
            tx.rollback().await?;
            return Ok(None);
        }

        tx.commit().await?;
        tracing::info!(target: "maint-store", task_id, employee_id = %emp_id, "Task auto-assigned");
    } else {
        tx.rollback().await?;
    }

    Ok(best_id)
}
```

---

## P1-06: QUERY FILTERS AFTER LIMIT — SILENT DATA LOSS
**File**: `maintenance_store.rs:query_events()`, `query_tasks()`  
**Severity**: P1 CRITICAL  
**Description**: Queries fetch `LIMIT N` rows then filter in Rust. If filtering removes rows, caller gets fewer results than requested. For `query_events(pod_id=Some(3), limit=10)`, if only 2 of 10 rows match pod_id=3, caller receives 2 results instead of 10.

```rust
// VIOLATION — filter after LIMIT
pub async fn query_events(
    pool: &SqlitePool,
    pod_id: Option<u8>,
    since: Option<DateTime<Utc>>,
    limit: u32,
) -> anyhow::Result<Vec<MaintenanceEvent>> {
    let rows = sqlx::query_as::<_, EventRow>(
        "SELECT ... FROM maintenance_events
         ORDER BY detected_at DESC
         LIMIT ?1",  // LIMIT applied BEFORE filter
    )
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;

    for row in rows {
        let evt = row_to_event(row)?;
        if let Some(pid) = pod_id {
            if evt.pod_id != Some(pid) { continue; }  // FILTER AFTER LIMIT
        }
        // ...
    }
}

// FIX — push filters into SQL with dynamic query building
pub async fn query_events(
    pool: &SqlitePool,
    pod_id: Option<u8>,
    since: Option<DateTime<Utc>>,
    limit: u32,
) -> anyhow::Result<Vec<MaintenanceEvent>> {
    // Build query dynamically based on filters
    let (sql, mut binds) = match (pod_id, since) {
        (Some(pid), Some(s)) => (
            "SELECT ... FROM maintenance_events
             WHERE pod_id = ?1 AND detected_at >= ?2
             ORDER BY detected_at DESC LIMIT ?3",
            vec![
                sqlx::types::Json(i64::from(pid)),
                sqlx::types::Json(s.to_rfc3339()),
            ]
        ),
        (Some(pid), None) => (
            "SELECT ... FROM maintenance_events
             WHERE pod_id = ?1
             ORDER BY detected_at DESC LIMIT ?2",
            vec![sqlx::types::Json(i64::from(pid))]
        ),
        (None, Some(s)) => (
            "SELECT ... FROM maintenance_events
             WHERE detected_at >= ?1
             ORDER BY detected_at DESC LIMIT ?2",
            vec![sqlx::types::Json(s.to_rfc3339())]
        ),
        (None, None) => (
            "SELECT ... FROM maintenance_events
             ORDER BY detected_at DESC LIMIT ?1",
            vec![]
        ),
    };

    // Use sqlx query builder pattern for proper parameterized dynamic queries
    let mut query = sqlx::query_as::<_, EventRow>(sql);
    for bind_val in &binds {
        query = query.bind(bind_val);
    }
    query = query.bind(i64::from(limit));
    
    let rows = query.fetch_all(pool).await?;
    rows.into_iter().map(row_to_event).collect()
}

// Same fix pattern for query_tasks()
```

---

## P1-07: APPLY_ACTION SILENTLY IGNORES MOST HEALING ACTIONS
**File**: `self_healing.rs:apply_action()`  
**Severity**: P1 CRITICAL  
**Description**: Only `MarkPodDegraded` and `MarkPodUnavailable` are handled. `RestartPod`, `RestartGameProcess`, `ClearDiskSpace`, `KillOrphanProcesses`, and `EscalateToStaff` are silently discarded. Callers expect these actions to execute.

```rust
// VIOLATION — actions silently ignored
pub async fn apply_action(map: &PodAvailabilityMap, action: &HealingAction) {
    let mut m = map.write().await;
    match action {
        HealingAction::MarkPodDegraded(id) => { ... }
        HealingAction::MarkPodUnavailable(id) => { ... }
        _ => {}  // SILENTLY IGNORED — RestartPod, ClearDiskSpace, etc.
    }
}

// FIX — handle all actions or return unhandled status
#[derive(Debug, Clone, Serialize)]
pub struct ActionResult {
    pub action: String,
    pub handled: bool,
    pub message: String,
}

pub async fn apply_action(
    map: &PodAvailabilityMap,
    action: &HealingAction,
    pool: &SqlitePool,  // Add pool for escalation logging
) -> ActionResult {
    let result = match action {
        HealingAction::MarkPodDegraded(id) => {
            let mut m = map.write().await;
            if *id < 1 || *id > 8 {
                ActionResult { action: format!("{:?}", action), handled: false, message: format!("Invalid pod_id {}", id) }
            } else {
                m.insert(*id, PodAvailability::Degraded { reason: "Anomaly detected".into() });
                tracing::warn!(target: LOG_TARGET, pod = id, "Pod marked DEGRADED");
                ActionResult { action: format!("{:?}", action), handled: true, message: "Pod marked degraded".into() }
            }
        }
        HealingAction::MarkPodUnavailable(id) => {
            let mut m = map.write().await;
            if *id < 1 || *id > 8 {
                ActionResult { action: format!("{:?}", action), handled: false, message: format!("Invalid pod_id {}", id) }
            } else {
                m.insert(*id, PodAvailability::Unavailable { reason: "Critical anomaly".into() });
                tracing::error!(target: LOG_TARGET, pod = id, "Pod marked UNAVAILABLE");
                ActionResult { action: format!("{:?}", action), handled: true, message: "Pod marked unavailable".into() }
            }
        }
        HealingAction::RestartPod(id) => {
            // TODO: Implement pod restart via fleet manager
            tracing::warn!(target: LOG_TARGET, pod = id, "RestartPod action NOT YET IMPLEMENTED");
            ActionResult { action: format!("{:?}", action), handled: false, message: "RestartPod not implemented".into() }
        }
        HealingAction::ClearDiskSpace(id) => {
            // TODO: Implement disk cleanup
            tracing::warn!(target: LOG_TARGET, pod = id, "ClearDiskSpace action NOT YET IMPLEMENTED");
            ActionResult { action: format!("{:?}", action), handled: false, message: "ClearDiskSpace not implemented".into() }
        }
        HealingAction::KillOrphanProcesses(id) => {
            // TODO: Implement process cleanup
            tracing::warn!(target: LOG_TARGET, pod = id, "KillOrphanProcesses action NOT YET IMPLEMENTED");
            ActionResult { action: format!("{:?}", action), handled: false, message: "KillOrphanProcesses not implemented".into() }
        }
        HealingAction::RestartGameProcess(id, process) => {
            tracing::warn!(target: LOG_TARGET, pod = id, process, "RestartGameProcess action NOT YET IMPLEMENTED");
            ActionResult { action: format!("{:?}", action), handled: false, message: "RestartGameProcess not implemented".into() }
        }
        HealingAction::EscalateToStaff(id, reason) => {
            // Log escalation for staff dashboard
            tracing::error!(target: LOG_TARGET, pod = id, "ESCALATION: {}", reason);
            // TODO: Create maintenance event, send WhatsApp
            ActionResult { action: format!("{:?}", action), handled: true, message: format!("Escalated: {}", reason) }
        }
    };
    result
}
```

---

# P2 HIGH FINDINGS

## P2-01: UPDATE_EMPLOYEE WITHOUT TRANSACTION — PARTIAL UPDATES
**File**: `maintenance_store.rs:update_employee()`  
**Severity**: P2 HIGH  
**Description**: Seven separate UPDATE queries executed without transaction. If any query fails, partial update occurs leaving employee record in inconsistent state.

```rust
// VIOLATION — separate queries, no atomicity
if let Some(n) = name {
    let r = sqlx::query("UPDATE employees SET name = ?1 WHERE id = ?2")...
}
if let Some(r) = role {
    let r = sqlx::query("UPDATE employees SET role = ?1 WHERE id = ?2")...
}
// ... 5 more separate queries

// FIX — single query with COALESCE pattern
pub async fn update_employee(
    pool: &SqlitePool,
    id: &str,
    name: Option<&str>,
    role: Option<&StaffRole>,
    skills: Option<&[String]>,
    hourly_rate_paise: Option<i64>,
    phone: Option<&str>,
    is_active: Option<bool>,
    face_enrollment_id: Option<&str>,
) -> anyhow::Result<bool> {
    // Build a single UPDATE with only the fields that changed
    let mut set_clauses: Vec<String> = Vec::new();
    let mut bind_values: Vec<Box<dyn std::any::Any + Send>> = Vec::new();
    
    // This is complex with dynamic SQL, so use transaction approach instead:
    let mut tx = pool.begin().await?;
    
    let mut any_updated = false;

    if let Some(n) = name {
        let result = sqlx::query("UPDATE employees SET name = ?1 WHERE id = ?2")
            .bind(n).bind(id).execute(&mut *tx).await?;
        any_updated = any_updated || result.rows_affected() > 0;
    }
    if let Some(r) = role {
        let role_str = serde_json::to_string(r)?.replace('"', "");
        let result = sqlx::query("UPDATE employees SET role = ?1 WHERE id = ?2")
            .bind(&role_str).bind(id).execute(&mut *tx).await?;
        any_updated = any_updated || result.rows_affected() > 0;
    }
    // ... other fields ...

    if any_updated {
        tx.commit().await?;
    } else {
        tx.rollback().await?;
    }

    Ok(any_updated)
}
```

---

## P2-02: WRITE LOCK HELD TOO LONG IN ANOMALY SCAN
**File**: `anomaly_engine.rs:run_anomaly_scan()`  
**Severity**: P2 HIGH  
**Description**: Write lock on `EngineState` is held while processing all rows and all rules (potentially 8 pods × 10 rules = 80 iterations). This blocks any concurrent reader of `recent_alerts()` for the entire scan duration.

```rust
// VIOLATION — lock held for entire scan
let mut guard = state.write().await;  // WRITE LOCK ACQUIRED
for row in &rows {
    for rule in rules {
        // ... extensive processing ...
        guard.last_alert.insert(key.clone(), now);
        guard.first_violation.remove(&key);
        // ...
    }
}
guard.recent_alerts.extend(alerts.clone());
// ... only released when function returns

// FIX — collect results first, then acquire lock briefly
pub async fn run_anomaly_scan(
    pool: &SqlitePool,
    state: &Arc<RwLock<EngineState>>,
    rules: &[AnomalyRule],
) -> Vec<AnomalyAlert> {
    let now = Utc::now();
    let cutoff = (now - chrono::Duration::seconds(60)).to_rfc3339();

    let rows = match fetch_telemetry_rows(pool, &cutoff).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Anomaly scan: failed to query hardware_telemetry: {}", e);
            return Vec::new();
        }
    };

    // Read current state (brief read lock)
    let current_state = {
        let guard = state.read().await;
        (guard.last_alert.clone(), guard.first_violation.clone())
    };

    // Process WITHOUT holding any lock
    let mut alerts = Vec::new();
    let mut new_last_alert = HashMap::new();
    let mut cleared_violations = Vec::new();

    for row in &rows {
        for rule in rules {
            let key = (row.pod_id.clone(), rule.name.clone());
            // ... processing using current_state ...
            // Track what needs to be updated
            new_last_alert.insert(key.clone(), now);
            cleared_violations.push(key);
            alerts.push(alert);
        }
    }

    // Brief write lock only for state updates
    {
        let mut guard = state.write().await;
        for (k, v) in new_last_alert {
            guard.last_alert.insert(k, v);
        }
        for k in cleared_violations {
            guard.first_violation.remove(&k);
        }
        if !alerts.is_empty() {
            guard.recent_alerts.extend(alerts.clone());
            let len = guard.recent_alerts.len();
            if len > 200 {
                guard.recent_alerts.drain(..len - 200);
            }
        }
    }

    alerts
}
```

---

## P2-03: MEMORY LEAK IN ENGINESTATE HASHMAPS
**File**: `anomaly_engine.rs:EngineState`  
**Severity**: P2 HIGH  
**Description**: `last_alert` and `first_violation` HashMaps grow without bounds. If pods are renamed or many transient rule/pod combinations occur, memory grows forever. In a long-running server, this is a slow memory leak.

```rust
// VIOLATION — no cleanup mechanism
pub struct EngineState {
    last_alert: HashMap<(String, String), DateTime<Utc>>,      // grows forever
    first_violation: HashMap<(String, String), DateTime<Utc>>,  // grows forever
    recent_alerts: Vec<AnomalyAlert>,
}

// FIX — add periodic cleanup
impl EngineState {
    /// Remove stale entries older than max_age
    fn cleanup(&mut self, max_age: chrono::Duration) {
        let cutoff = Utc::now() - max_age;
        self.last_alert.retain(|_, ts| *ts > cutoff);
        self.first_violation.retain(|_, ts| *ts > cutoff);
    }
}

// In run_anomaly_scan, after acquiring write lock:
{
    let mut guard = state.write().await;
    // Clean up entries older than 24 hours
    guard.cleanup(chrono::Duration::hours(24));
    // ... rest of updates ...
}
```

---

## P2-04: INCORRECT METRIC CALCULATION (RECALL = PRECISION)
**File**: `feedback_loop.rs:calculate_feedback_metrics()`  
**Severity**: P2 HIGH  
**Description**: The code returns `precision` as `recall`, claiming it's "simplified". These are fundamentally different metrics:
- **Precision**: True Positives / (True Positives + False Positives) — "Of predicted failures, how many were real?"
- **Recall**: True Positives / (True Positives + False Negatives) — "Of actual failures, how many did we predict?"

Returning precision as recall produces misleading KPIs for the feedback loop.

```rust
// VIOLATION
Ok(FeedbackMetrics {
    precision,
    recall: precision, // WRONG — "simplified" comment doesn't make this correct
    // ...
})

// FIX — either calculate correctly or mark as unknown
Ok(FeedbackMetrics {
    precision,
    recall: f64::NAN,  // Honestly indicate we don't track this
    // ... with documentation explaining why
})

// BETTER FIX — if you need recall, track false negatives
// Add a column to track "actual failures that weren't predicted"
// Then: recall = accurate / (accurate + false_negatives)
```

---

## P2-05: INTEGER CASTS WITH `as` THROUGHOUT REMAINING FILES
**File**: Multiple files  
**Severity**: P2 HIGH  
**Description**: Additional `as` cast violations not covered in P1-02:

```rust
// anomaly_engine.rs HwRow::metric_value()
self.disk_smart_health_pct.map(|v| v as f64),      // should be f64::from(v)
self.process_handle_count.map(|v| v as f64),        // should be f64::from(v)
self.network_latency_ms.map(|v| v as f64),          // should be f64::from(v)

// maintenance_store.rs query_business_metrics()
sessions_count: row.sessions_count as u32,           // should use try_from
occupancy_rate_pct: row.occupancy_rate_pct as f32,   // precision loss acceptable but should be documented
peak_occupancy_pct: row.peak_occupancy_pct as f32,   // same

// maintenance_store.rs get_summary()
open_tasks: open_row.0 as u32,                       // should use try_from

// maintenance_store.rs calculate_kpis()
downtime_minutes: downtime_row.map(|(v,)| v as u32).unwrap_or(0),  // should use try_from
total_events: total_events as u32,                    // should use try_from
total_tasks: total_tasks as u32,                      // should use try_from
tasks_completed: tasks_completed as u32,              // should use try_from
tasks_open: tasks_open as u32,                        // should use try_from

// data_collector.rs collect_venue_snapshot()
open_maintenance_tasks: open_tasks as u32,            // should use try_from
critical_alerts_active: critical_alerts as u32,       // should use try_from
staff_on_duty: staff as u32,                          // should use try_from

// FIX pattern for all:
fn i64_to_u32_safe(v: i64) -> u32 {
    u32::try_from(v).unwrap_or(0)  // or propagate error
}

// For metric_value:
fn metric_value(&self, name: &str) -> Option<f64> {
    match name {
        "disk_smart_health_pct" => self.disk_smart_health_pct.map(f64::from),
        "process_handle_count" => self.process_handle_count.map(f64::from),
        "network_latency_ms" => self.network_latency_ms.map(f64::from),
        // ... others unchanged
    }
}
```

---

## P2-06: SPAWNED TASKS HAVE NO ERROR RECOVERY
**File**: `anomaly_engine.rs:spawn_anomaly_scanner()`, `business_alerts.rs:spawn_alert_checker()`, `data_collector.rs:spawn_data_collector()`  
**Severity**: P2 HIGH  
**Description**: All three spawned background tasks use bare `loop` without any error recovery. If the task panics, it dies silently and the background processing stops forever with no indication to operators.

```rust
// VIOLATION — no panic handling
tokio::spawn(async move {
    loop {
        interval.tick().await;
        let alerts = run_anomaly_scan(&pool, &state_clone, &rules).await;
        // If this panics, task dies forever
    }
});

// FIX — add panic boundary and restart logic
pub fn spawn_anomaly_scanner(pool: SqlitePool) -> Arc<RwLock<EngineState>> {
    let state = Arc::new(RwLock::new(EngineState::new()));
    let state_clone = Arc::clone(&state);
    let rules = default_rules();
    let pool_clone = pool.clone();

    tokio::spawn(async move {
        loop {
            // Wrap entire loop iteration in catch_unwind equivalent
            let result = std::panic::AssertUnwindSafe(async {
                run_scanner_loop(&pool_clone, &state_clone, &rules).await
            }).await;

            if let Err(e) = result {
                tracing::error!(
                    target: "anomaly-scanner",
                    error = %e,
                    "Scanner loop panicked, restarting in 30s"
                );
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                // Loop continues — task survives panic
            }
        }
    });

    state
}

async fn run_scanner_loop(
    pool: &SqlitePool,
    state: &Arc<RwLock<EngineState>>,
    rules: &[AnomalyRule],
) -> Result<(), anyhow::Error> {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
    interval.tick().await; // Skip first tick

    loop {
        interval.tick().await;
        
        // Check pool health
        if pool.is_closed() {
            anyhow::bail!("Database pool closed");
        }

        let alerts = run_anomaly_scan(pool, state, rules).await;
        if !alerts.is_empty() {
            tracing::info!("Anomaly scan: {} new alert(s)", alerts.len());
        }
    }
}
```

---

## P2-07: PATTERN CHECKING IGNORES LOOKBACK_MINUTES
**File**: `anomaly_engine.rs:check_patterns()`  
**Severity**: P2 HIGH  
**Description**: Each `FailurePattern` specifies a `lookback_minutes` window, but the code queries only the latest row per pod. A pattern with `lookback_minutes: 60` should analyze 60 minutes of data, not a single point-in-time snapshot.

```rust
// VIOLATION — lookback_minutes ignored, only latest row used
let rows: Result<Vec<HwRow>, sqlx::Error> = sqlx::query(
    "SELECT ... FROM hardware_telemetry
     WHERE collected_at > ?1
       AND (pod_id, collected_at) IN (
           SELECT pod_id, MAX(collected_at) FROM ...  // ONLY LATEST ROW
       )"
).bind(&cutoff).fetch_all(pool).await?;

// FIX — for patterns needing time-series, fetch actual window
pub async fn check_patterns(
    pool: &SqlitePool,
    patterns: &[FailurePattern],
) -> Vec<PatternAlert> {
    // Group patterns by lookback to minimize queries
    let mut by_lookback: HashMap<u32, Vec<&FailurePattern>> = HashMap::new();
    for p in patterns {
        by_lookback.entry(p.lookback_minutes).or_default().push(p);
    }

    let mut alerts = Vec::new();
    let now = Utc::now();

    for (lookback, pattern_group) in by_lookback {
        let cutoff = (now - chrono::Duration::minutes(lookback as i64)).to_rfc3339();
        
        // For patterns, we need to check if ANY row in the window matched
        // Aggregate: did the condition occur at any point in the window?
        let rows: Vec<HwAggRow> = sqlx::query_as(
            "SELECT 
                pod_id,
                MAX(gpu_temp_celsius) as max_gpu_temp,
                MIN(disk_smart_health_pct) as min_disk_health,
                MAX(memory_usage_pct) as max_memory,
                AVG(memory_usage_pct) as avg_memory,
                MAX(process_handle_count) as max_handles,
                MAX(cpu_usage_pct) as max_cpu
             FROM hardware_telemetry
             WHERE collected_at > ?1
             GROUP BY pod_id"
        ).bind(&cutoff).fetch_all(pool).await.unwrap_or_default();

        for row in &rows {
            for pattern in pattern_group {
                // Now check aggregated values against pattern conditions
                // A condition "metric > threshold" means "max > threshold" in window
                // A condition "metric < threshold" means "min < threshold" in window
                // ...
            }
        }
    }
    alerts
}
```

---

## P2-08: NO BUSINESS ALERT COOLDOWN
**File**: `business_alerts.rs:check_business_alerts()`  
**Severity**: P2 HIGH  
**Description**: Unlike anomaly alerts which have cooldowns, business alerts can fire every 30 minutes indefinitely. During a slow day, the same "low revenue" alert spams WhatsApp repeatedly.

```rust
// VIOLATION — no deduplication or cooldown
pub async fn check_business_alerts(pool: &sqlx::SqlitePool) -> Vec<BusinessAlert> {
    // ... generates same alert every call if conditions persist
}

// FIX — track last alert time per type
use std::collections::HashMap;
use std::sync::Mutex;
use once_cell::sync::Lazy;

static LAST_BUSINESS_ALERT: Lazy<Mutex<HashMap<String, DateTime<Utc>>>> = 
    Lazy::new(|| Mutex::new(HashMap::new()));

pub async fn check_business_alerts(pool: &sqlx::SqlitePool) -> Vec<BusinessAlert> {
    let mut alerts = Vec::new();
    let now = Utc::now();
    let cooldown = chrono::Duration::hours(4); // Same alert max once per 4 hours

    // ... revenue check ...
    if avg_rev > 0 && today_rev * 10 < avg_rev * 7 {
        let alert_key = "RevenueDropAlert";
        let should_fire = {
            let mut last = LAST_BUSINESS_ALERT.lock().unwrap();
            match last.get(alert_key) {
                Some(last_time) if *last_time + cooldown > now => false,
                _ => { last.insert(alert_key.to_string(), now); true }
            }
        };
        if should_fire {
            alerts.push(BusinessAlert { alert_type: "RevenueDropAlert".into(), ... });
        }
    }
    // ... same pattern for other alerts
    alerts
}
```

---

## P2-09: VENUE SNAPSHOT RETURNS HARDCODED STUBS
**File**: `data_collector.rs:collect_venue_snapshot()`  
**Severity**: P2 HIGH  
**Description**: `pod_count_online`, `pod_count_degraded`, `pod_count_unavailable`, `active_sessions`, and `occupancy_pct` are hardcoded/zero stubs. AI consuming this data makes decisions based on fiction.

```rust
// VIOLATION — hardcoded stubs
VenueSnapshot {
    timestamp: Utc::now(),
    pod_count_online: 8,        // HARDCODED — always 8
    pod_count_degraded: 0,      // HARDCODED — always 0
    pod_count_unavailable: 0,   // HARDCODED — always 0
    active_sessions: 0,         // TODO stub
    occupancy_pct: 0.0,         // TODO stub
    // ...
}

// FIX — query from availability map and session state
pub async fn collect_venue_snapshot(
    pool: &SqlitePool,
    availability_map: &PodAvailabilityMap,
) -> VenueSnapshot {
    let today = Utc::now().date_naive().to_string();

    // Query actual availability
    let avail = availability_map.read().await;
    let mut online = 0u8;
    let mut degraded = 0u8;
    let mut unavailable = 0u8;
    for pod_id in 1..=8 {
        match avail.get(&pod_id) {
            Some(PodAvailability::Available) => online += 1,
            Some(PodAvailability::Degraded { .. }) => degraded += 1,
            Some(PodAvailability::Unavailable { .. }) | 
            Some(PodAvailability::MaintenanceHold { .. }) => unavailable += 1,
            None => unavailable += 1,
        }
    }
    drop(avail);

    // Query active sessions from billing
    let active_sessions: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sessions WHERE status = 'active'"
    ).fetch_one(pool).await.unwrap_or(0);

    // Calculate occupancy from sessions vs available pods
    let occupancy_pct = if online > 0 {
        (active_sessions as f32 / online as f32) * 100.0
    } else {
        0.0
    };

    // ... rest of queries
    VenueSnapshot {
        timestamp: Utc::now(),
        pod_count_online: online,
        pod_count_degraded: degraded,
        pod_count_unavailable: unavailable,
        active_sessions: active_sessions as u32,
        occupancy_pct: occupancy_pct.min(100.0),
        // ...
    }
}
```

---

# P3 MEDIUM FINDINGS

## P3-01: Severity PARTIALORD IS SEMANTICALLY WRONG
**File**: `maintenance_models.rs`  
**Severity**: P3 MEDIUM  
**Description**: `#[derive(PartialOrd)]` on `Severity` enum orders variants by declaration order (`Critical < High < Medium < Low`). This is backwards — Critical should be "greater than" Low for sorting by severity.

```rust
// Current (wrong ordering for "highest first" sorts)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
pub enum Severity {
    Critical,  // 0
    High,      // 1
    Medium,    // 2
    Low,       // 3 — Low > Critical!
}

// FIX — either reverse order or implement custom Ord
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Low,       // 0 — least severe
    Medium,    // 1
    High,      // 2
    Critical,  // 3 — most severe
}

// Or implement manually:
impl PartialOrd for Severity {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let to_rank = |s: &Severity| match s {
            Severity::Critical => 3,
            Severity::High => 2,
            Severity::Medium => 1,
            Severity::Low => 0,
        };
        to_rank(self).partial_cmp(&to_rank(other))
    }
}
```

---

## P3-02: AttendanceRecord USES f64 FOR HOURS_WORKED
**File**: `maintenance_models.rs`  
**Severity**: P3 MEDIUM  
**Description**: `hours_worked: f64` accumulates floating-point errors over time. While the code tries to work in minutes, storage is in hours as f64.

```rust
// Current — f64 storage
pub struct AttendanceRecord {
    pub hours_worked: f64,
}

// Consider — store as minutes (i32) for precision
pub struct AttendanceRecord {
    pub minutes_worked: i32,  // 0-1440 (24 hours)
}

// Or keep f64 but document acceptable precision bounds
```

---

## P3-03: OLLAMA CLIENT CREATES NEW HTTP CLIENT PER CALL
**File**: `ollama_client.rs:diagnose()`  
**Severity**: P3 MEDIUM  
**Description**: Creates a new `reqwest::Client` on every `diagnose()` call, losing connection pooling benefits.

```rust
// VIOLATION
pub async fn diagnose(prompt: &str) -> anyhow::Result<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
        .build()?;  // New client every call
    // ...
}

// FIX — use lazy static or shared client
use once_cell::sync::Lazy;

static OLLAMA_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
        .pool_max_idle_per_host(4)
        .build()
        .expect("Failed to create Ollama HTTP client")
});

pub async fn diagnose(prompt: &str) -> anyhow::Result<String> {
    match call_ollama(&OLLAMA_CLIENT, DEFAULT_MODEL, prompt).await {
        // ...
    }
}
```

---

## P3-04: DATA COLLECTOR POD_ID PARSE SILENT FAIL
**File**: `data_collector.rs:check_rul_thresholds()`  
**Severity**: P3 MEDIUM  
**Description**: `pod_id_str.replace("pod_", "").parse::<i64>().unwrap_or(0)` silently creates tasks for pod 0 if parsing fails.

```rust
// VIOLATION
let pod_num = pod_id_str.replace("pod_", "").parse::<i64>().unwrap_or(0);

// FIX — return error or skip
let pod_num = pod_id_str
    .strip_prefix("pod_")
    .and_then(|s| s.parse::<i64>().ok())
    .filter(|&n| n >= 1 && n <= 8);

let pod_num = match pod_num {
    Some(n) => n,
    None => {
        tracing::warn!(target: LOG_TARGET, pod = %pod_id_str, "Invalid pod_id format, skipping");
        continue;
    }
};
```

---

## P3-05: FEEDBACK_METRICS TOTAL_EQUALS_ACCURATE_PLUS_FALSE_POS ASSUMPTION
**File**: `feedback_loop.rs:calculate_feedback_metrics()`  
**Severity**: P3 MEDIUM  
**Description**: Code assumes `total = accurate + false_pos`, but this ignores records where `was_accurate IS NULL` (pending evaluation).

```rust
// Current — may undercount
let total: i64 = sqlx::query_scalar(
    "SELECT COUNT(*) FROM prediction_outcomes WHERE created_at > ?1"
).fetch_one(pool).await.unwrap_or(0);

// FIX — only count evaluated records for precision calculation
let evaluated: i64 = sqlx::query_scalar(
    "SELECT COUNT(*) FROM prediction_outcomes WHERE created_at > ?1 AND was_accurate IS NOT NULL"
).fetch_one(pool).await.unwrap_or(0);

let precision = if evaluated > 0 {
    accurate as f64 / evaluated as f64
} else {
    0.0
};
```

---

# SUMMARY

| Severity | Count | Key Issues |
|----------|-------|------------|
| P1 CRITICAL | 7 | Money as f64, `as` casts, data corruption, race conditions, silent data loss |
| P2 HIGH | 9 | Partial updates, lock contention, memory leaks, wrong metrics, no error recovery |
| P3 MEDIUM | 5 | Enum ordering, f64 precision, HTTP client pooling, parse failures |

**Must-fix before deployment**: P1-01 through P1-07  
**Should-fix this sprint**: P2-01 through P2-06  
**Technical debt**: P3-01 through P3-05