# Racing Point eSports v29.0 — Security & Reliability Audit Report

After a thorough review of the 9 modules (~2700 lines), I identified **4 P1 (Critical)** and **5 P2 (High)** findings. Here is the full breakdown.

---

## P1 — CRITICAL

### 1. Broken SQL in Anomaly Scanner — `run_anomaly_scan` will fail at runtime

**File:** `anomaly_detection.rs` — `run_anomaly_scan` (and identically in `check_patterns`)

```rust
// BROKEN — HAVING needs a boolean predicate, not a bare aggregate
"SELECT ... FROM hardware_telemetry
 WHERE collected_at > ?1
 GROUP BY pod_id
 HAVING MAX(collected_at)"
```

`HAVING MAX(collected_at)` is not valid SQL semantics. SQLite treats it as `HAVING MAX(collected_at) <> 0` — meaning every row where the latest timestamp isn't the Unix epoch passes. Worse, which row's *other* columns are returned per group is **undefined** in SQLite. The anomaly engine will evaluate against **arbitrary** telemetry snapshots, not the latest one. Every threshold check downstream is unreliable.

**Fix:**

```sql
SELECT h.pod_id, h.gpu_temp_celsius, h.cpu_temp_celsius,
       h.gpu_power_watts, h.disk_smart_health_pct,
       h.process_handle_count, h.cpu_usage_pct,
       h.memory_usage_pct, h.disk_usage_pct, h.network_latency_ms
FROM hardware_telemetry h
INNER JOIN (
    SELECT pod_id, MAX(collected_at) AS latest
    FROM hardware_telemetry
    WHERE collected_at > ?1
    GROUP BY pod_id
) latest_rows
  ON h.pod_id = latest_rows.pod_id
 AND h.collected_at = latest_rows.latest
```

The same broken pattern exists in `check_patterns` and must be fixed there too.

---

### 2. Silent Data Loss in `query_events` — Filter-After-Limit

**File:** `maintenance_store.rs` — `query_events`

```rust
pub async fn query_events(
    pool: &SqlitePool,
    pod_id: Option<u8>,
    since: Option<DateTime<Utc>>,
    limit: u32,
) -> anyhow::Result<Vec<MaintenanceEvent>> {
    // Fetches `limit` rows ordered by detected_at DESC, THEN filters in Rust
    let rows = sqlx::query_as::<_, EventRow>("... LIMIT ?1")
        .bind(limit as i64)
        .fetch_all(pool).await?;

    for row in rows {
        // Silently drops rows that don't match filters
        if let Some(pid) = pod_id {
            if evt.pod_id != Some(pid) { continue; }
        }
        ...
    }
}
```

If `limit=1000` but only 3 of those 1000 rows match `pod_id=3`, the caller receives 3 rows and has no way to know more exist. This breaks pagination and can hide active incidents from the dashboard.

**Fix:** Push filters into the SQL WHERE clause:

```rust
pub async fn query_events(
    pool: &SqlitePool,
    pod_id: Option<u8>,
    since: Option<DateTime<Utc>>,
    limit: u32,
) -> anyhow::Result<Vec<MaintenanceEvent>> {
    let mut sql = String::from(
        "SELECT id, pod_id, event_type, severity, component, description,
                detected_at, resolved_at, resolution_method, source,
                correlation_id, revenue_impact_paise, customers_affected,
                downtime_minutes, cost_estimate_paise, assigned_staff_id, metadata
         FROM maintenance_events WHERE 1=1"
    );
    let mut param_idx = 1usize;
    let mut binds: Vec<String> = Vec::new();

    if let Some(pid) = pod_id {
        sql.push_str(&format!(" AND pod_id = ?{}", param_idx));
        binds.push((pid as i64).to_string());
        param_idx += 1;
    }
    if let Some(ref s) = since {
        sql.push_str(&format!(" AND detected_at >= ?{}", param_idx));
        binds.push(s.to_rfc3339());
        param_idx += 1;
    }
    sql.push_str(&format!(" ORDER BY detected_at DESC LIMIT ?{}", param_idx));
    binds.push((limit as i64).to_string());

    let mut query = sqlx::query_as::<_, EventRow>(&sql);
    for b in &binds { query = query.bind(b); }
    let rows = query.fetch_all(pool).await?;

    rows.into_iter().map(row_to_event).collect()
}
```

---

### 3. Inconsistent Crash vs. Default on Corrupt Enum Data

**File:** `maintenance_store.rs` — multiple functions

`get_summary` silently defaults on corrupt data:
```rust
let sev: Severity = serde_json::from_str(&row.severity).unwrap_or(Severity::Medium);
```

But `row_to_event` (called by `query_events`) propagates the error, crashing the query:
```rust
event_type: serde_json::from_str(&row.event_type)?,  // <-- panics on corrupt row
```

This means:
- The dashboard **summary** loads fine (hiding corruption).
- The **event list** crashes with `Err(...)` when any row has a corrupt `event_type`, `severity`, or `component` — blocking staff from seeing *any* events.

**Fix:** Make `row_to_event` resilient with logging, matching `get_summary`'s approach:

```rust
fn row_to_event(row: EventRow) -> anyhow::Result<MaintenanceEvent> {
    // ...detected_at parsing...

    let event_type: MaintenanceEventType = serde_json::from_str(&row.event_type)
        .unwrap_or_else(|e| {
            tracing::warn!("Corrupt event_type '{}' for event {}: {}", row.event_type, row.id, e);
            MaintenanceEventType::SelfHealAttempted
        });
    let severity: Severity = serde_json::from_str(&row.severity)
        .unwrap_or_else(|e| {
            tracing::warn!("Corrupt severity '{}' for event {}: {}", row.severity, row.id, e);
            Severity::Medium
        });
    let component: ComponentType = serde_json::from_str(&row.component)
        .unwrap_or_else(|e| {
            tracing::warn!("Corrupt component '{}' for event {}: {}", row.component, row.id, e);
            ComponentType::Software
        });

    Ok(MaintenanceEvent {
        id: Uuid::parse_str(&row.id)?,
        event_type,
        severity,
        component,
        // ...remaining fields...
    })
}
```

Apply the same pattern to `row_to_task` for `component` and `status`.

---

### 4. Payroll End-Date Includes Spill-Over Into Next Month

**File:** `maintenance_store.rs` — `calculate_monthly_payroll`

```rust
let start_date = format!("{:04}-{:02}-01", year, month);
let end_date = format!("{:04}-{:02}-31", year, month); // BUG
```

For February, `end_date = "2025-02-31"`. SQLite rolls this forward to `"2025-03-03"`, so the query `date <= '2025-03-03'` **includes the first 2–3 days of March** in February's payroll. Same issue for April, June, September, November (30-day months).

**Fix:**

```rust
let start_date = format!("{:04}-{:02}-01", year, month);
let end_date = format!(
    "{:04}-{:02}-{}",
    year,
    month,
    last_day_of_month(year, month)
);

// Helper
fn last_day_of_month(year: i32, month: u32) -> u32 {
    // First day of next month minus one day
    let (ny, nm) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    chrono::NaiveDate::from_ymd_opt(ny, nm, 1)
        .unwrap()
        .signed_duration_since(
            chrono::NaiveDate::from_ymd_opt(year, month, 1).unwrap()
        )
        .num_days() as u32
}
```

Or use SQLite's built-in: `date(?1, 'start of month', '+1 month', '-1 day')`.

---

## P2 — HIGH

### 5. Write Lock Held During Database Query — Blocks All API Readers

**File:** `anomaly_detection.rs` — `run_anomaly_scan`

```rust
let mut guard = state.write().await;  // <-- LOCK ACQUIRED HERE

// ...60 seconds of database I/O happens while lock is held...
for row in &rows {
    for rule in rules {
        // DB query already done, but the lock spans the entire function logic
        guard.first_violation.entry(key.clone())...
        guard.last_alert.insert(key.clone(), now);
        guard.recent_alerts.push(alert);
    }
}
```

The `state.write()` is acquired before the main loop and held for the entire function body. Any API handler calling `state.read().await` to serve recent alerts is blocked for the duration of the scan (potentially hundreds of ms with DB queries). Under load, this causes cascading latency on the `/alerts` endpoint.

**Fix:** Acquire the write lock only for the HashMap mutations:

```rust
pub async fn run_anomaly_scan(
    pool: &SqlitePool,
    state: &Arc<RwLock<EngineState>>,
    rules: &[AnomalyRule],
) -> Vec<AnomalyAlert> {
    let now = Utc::now();
    let cutoff = (now - chrono::Duration::seconds(60)).to_rfc3339();

    // 1. Query DB WITHOUT holding any lock
    let rows = match fetch_latest_telemetry(pool, &cutoff).await {
        Ok(r) => r,
        Err(e) => { tracing::warn!(...); return Vec::new(); }
    };

    // 2. Read current state snapshot
    let (last_alerts, first_violations) = {
        let guard = state.read().await;
        (guard.last_alert.clone(), guard.first_violation.clone())
    };

    // 3. Compute alerts without any lock
    let mut new_alerts = Vec::new();
    let mut updated_last_alert = last_alerts;
    let mut updated_first_violation = first_violations;

    for row in &rows {
        for rule in rules {
            let key = (row.pod_id.clone(), rule.name.clone());
            // ...evaluation logic using local copies...
            if should_fire {
                updated_last_alert.insert(key.clone(), now);
                updated_first_violation.remove(&key);
                new_alerts.push(alert);
            }
        }
    }

    // 4. Brief write lock to commit changes
    {
        let mut guard = state.write().await;
        guard.last_alert = updated_last_alert;
        guard.first_violation = updated_first_violation;
        guard.recent_alerts.extend(new_alerts.clone());
        let len = guard.recent_alerts.len();
        if len > 200 { guard.recent_alerts.drain(..len - 200); }
    }

    new_alerts
}
```

---

### 6. Occupancy Rate Precision Loss Through f32→f64→f32 Round-Trip

**File:** `maintenance_store.rs` — `upsert_daily_metrics` + `query_business_metrics`

Insert:
```rust
.bind(metrics.occupancy_rate_pct as f64)  // f32 -> f64
```

Read:
```rust
occupancy_rate_pct: row.occupancy_rate_pct as f32,  // f64 -> f32
```

While `f32→f64` is lossless, this signals a design smell. SQLite REAL is 64-bit; storing as f32 discards precision for no benefit. For business-critical occupancy metrics that drive pricing decisions, use `f64` end-to-end.

**Fix:** Change the model:
```rust
pub struct DailyBusinessMetrics {
    // ...
    pub occupancy_rate_pct: f64,   // was f32
    pub peak_occupancy_pct: f64,   // was f32
}
```

And update the insert to `.bind(metrics.occupancy_rate_pct)` (no cast needed).

---

### 7. Missing Input Validation — Multiple Fields

**File:** `maintenance_store.rs` — various insert functions

| Field | Issue |
|-------|-------|
| `pod_id: Option<u8>` | No bounds check on insert; a caller could pass `Some(300)` and the `as i64` cast silently truncates to `44` |
| `priority: u8` | Stored as INTEGER with no CHECK constraint; should be 1–100 |
| `phone: String` | No format validation; could be empty string or contain HTML/JS |
| `hourly_rate_paise: i64` | No positivity check; negative rates would produce negative payroll |
| `Employee.name` | Could be empty string |
| `description` | Could be empty or arbitrarily long (DoS via memory) |

**Fix — add validation at insert boundaries:**

```rust
pub async fn insert_employee(pool: &SqlitePool, employee: &Employee) -> anyhow::Result<()> {
    // Validate before hitting DB
    anyhow::ensure!(!employee.name.trim().is_empty(), "Employee name cannot be empty");
    anyhow::ensure!(employee.hourly_rate_paise >= 0, "Hourly rate cannot be negative");
    anyhow::ensure!(employee.phone.len() >= 7, "Phone number too short");

    // ...existing insert logic...
}

pub async fn insert_task(pool: &SqlitePool, task: &MaintenanceTask) -> anyhow::Result<()> {
    anyhow::ensure!(!task.title.trim().is_empty(), "Task title cannot be empty");
    anyhow::ensure!(task.priority >= 1 && task.priority <= 100,
        "Priority must be 1-100, got {}", task.priority);
    if let Some(pid) = task.pod_id {
        anyhow::ensure!(pid >= 1 && pid <= 8, "pod_id must be 1-8, got {}", pid);
    }
    // ...existing insert logic...
}
```

Also add SQLite CHECK constraints in `init_*_tables`:
```sql
priority INTEGER NOT NULL DEFAULT 3 CHECK(priority >= 1 AND priority <= 100),
hourly_rate_paise INTEGER DEFAULT 0 CHECK(hourly_rate_paise >= 0),
```

---

### 8. RUL Calculation Can Produce Infinity — Division by Zero

**File:** `anomaly_detection.rs` — `calculate_rul`

```rust
let rul_hours = if is_declining_health {
    let gap = trend.current_value - failure_threshold;
    if gap <= 0.0 { 0.0 } else {
        (gap / trend.rate_per_day.abs()) * 24.0  // DIVISION BY ZERO if rate = 0
    }
} else if is_rising_usage {
    let gap = failure_threshold - trend.current_value;
    if gap <= 0.0 { 0.0 } else {
        (gap / trend.rate_per_day.abs()) * 24.0  // SAME
    }
} else { return None; };
```

If `trend.rate_per_day` is exactly `0.0` (metric is flat at the boundary), `rul_hours` becomes `f64::INFINITY`. This serializes to `null` in JSON (serde drops inf/nan), causing downstream null-reference errors in the dashboard and inventory modules.

**Fix:**

```rust
if trend.rate_per_day.abs() < f64::EPSILON {
    // Flat trend — no degradation, no RUL concern
    return None;
}

let rul_hours = if is_declining_health {
    let gap = trend.current_value - failure_threshold;
    if gap <= 0.0 { 0.0 } else {
        (gap / trend.rate_per_day.abs()) * 24.0
    }
} else if is_rising_usage {
    let gap = failure_threshold - trend.current_value;
    if gap <= 0.0 { 0.0 } else {
        (gap / trend.rate_per_day.abs()) * 24.0
    }
} else { return None; };

if !rul_hours.is_finite() || rul_hours < 0.0 {
    tracing::warn!("RUL calculation produced invalid value for {}:{} on pod {}",
        component, metric_name, pod_id);
    return None;
}
```

---

### 9. `update_employee` Dynamic SQL — Fragile Placeholder Substitution

**File:** `maintenance_store.rs` — `update_employee`

```rust
let numbered_sets: Vec<String> = sets
    .iter()
    .enumerate()
    .map(|(i, s)| s.replace('?', &format!("?{}", i + 1)))
    .collect();
```

Each `s` is a hardcoded string like `"name = ?"` with exactly one `?`. The `.replace('?', ...)` works, but:

1. **If a future developer adds a literal `?` to a SET clause** (e.g., a JSON update), the replacement corrupts the SQL.
2. **Mixed type binding**: all binds go through the `binds: Vec<String>` path. While SQLite is dynamically typed and this works, it bypasses any future type checking from sqlx.

**Fix — use individual queries in a transaction instead of dynamic SQL:**

```rust
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
    let mut tx = pool.begin().await?;
    let mut any_updated = false;

    if let Some(n) = name {
        sqlx::query("UPDATE employees SET name = ?1 WHERE id = ?2")
            .bind(n).bind(id).execute(&mut *tx).await?;
        any_updated = true;
    }
    if let Some(r) = role {
        let role_str = serde_json::to_string(r)?.replace('"', "");
        sqlx::query("UPDATE employees SET role = ?1 WHERE id = ?2")
            .bind(&role_str).bind(id).execute(&mut *tx).await?;
        any_updated = true;
    }
    if let Some(s) = skills {
        let skills_json = serde_json::to_string(s)?;
        sqlx::query("UPDATE employees SET skills = ?1 WHERE id = ?2")
            .bind(&skills_json).bind(id).execute(&mut *tx).await?;
        any_updated = true;
    }
    if let Some(rate) = hourly_rate_paise {
        sqlx::query("UPDATE employees SET hourly_rate_paise = ?1 WHERE id = ?2")
            .bind(rate).bind(id).execute(&mut *tx).await?;
        any_updated = true;
    }
    if let Some(p) = phone {
        sqlx::query("UPDATE employees SET phone = ?1 WHERE id = ?2")
            .bind(p).bind(id).execute(&mut *tx).await?;
        any_updated = true;
    }
    if let Some(a) = is_active {
        sqlx::query("UPDATE employees SET is_active = ?1 WHERE id = ?2")
            .bind(a as i64).bind(id).execute(&mut *tx).await?;
        any_updated = true;
    }
    if let Some(f) = face_enrollment_id {
        sqlx::query("UPDATE employees SET face_enrollment_id = ?1 WHERE id = ?2")
            .bind(f).bind(id).execute(&mut *tx).await?;
        any_updated = true;
    }

    tx.commit().await?;
    Ok(any_updated)
}
```

This is type-safe, auditable, and eliminates the fragile string substitution entirely.

---

## P3 — MEDIUM

| # | Location | Issue | Fix |
|---|----------|-------|-----|
| 10 | `row_to_event`, `row_to_task` | `unwrap_or_else(Utc::now)` silently replaces corrupt timestamps with current time — hides data corruption | Log a warning: `unwrap_or_else(\|\| { tracing::warn!("..."); Utc::now() })` |
| 11 | `get_summary` | `by_severity`/`by_type` use `HashMap<String, u32>` — typo-prone string keys | Use `HashMap<Severity, u32>` and `HashMap<MaintenanceEventType, u32>` with serde `#[serde(serialize_with = ...)]` or a wrapper |
| 12 | `is_peak_hours` | Hardcoded `+5h30m` for IST — doesn't handle DST or timezone DB updates | Use `chrono-tz` crate: `use chrono_tz::Asia::Kolkata; let now = Utc::now().with_timezone(&Kolkata);` |
| 13 | `insert_event`, `insert_task` | No description length cap — a 10 MB description exhausts memory | Add `anyhow::ensure!(description.len() <= 10_000, "Description too long")` |

## P4 — LOW

| # | Location | Issue | Fix |
|---|----------|-------|-----|
| 14 | `insert_event` | Enums serialized via `serde_json::to_string` (JSON strings like `"\"Critical\""`) — stores extra quotes in TEXT column, inconsistent with `status` which stores plain strings | Serialize enums as plain strings: `format!("{:?}", event.severity)` or a custom `as_str()` method |
| 15 | `insert_task`, `row_to_task` | Multiple `.replace('"', "")` calls — works but fragile if enum names ever contain quotes | Use `#[serde(rename_all = "PascalCase")]` with a `as_str()` helper that returns `&'static str` |
| 16 | `calculate_rul` | `component` string matching duplicates the `ComponentType` enum | Accept `ComponentType` directly: `fn calculate_rul(..., component: ComponentType, ...)` |

---

## Summary Table

| Priority | Count | Key Risk |
|----------|-------|----------|
| **P1 Critical** | 4 | Anomaly engine evaluates wrong data; event queries silently drop rows; inconsistent crash on corrupt data; payroll includes next-month entries |
| **P2 High** | 5 | API latency spikes from lock contention; precision loss in billing metrics; missing validation allows corrupt inserts; RUL produces infinity; fragile dynamic SQL |
| **P3 Medium** | 4 | Silent timestamp corruption; stringly-typed maps; hardcoded timezone; unbounded input sizes |
| **P4 Low** | 3 | Inconsistent enum serialization; redundant string ops; duplicated type matching |

**Recommended fix order:** P1 items 1→3→4→2, then P2 items 5→7→8→9→6. Items 1 and 3 (anomaly scanner + event query) are the most operationally dangerous since they silently produce wrong results in the preventive maintenance pipeline.