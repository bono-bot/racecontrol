Below is a focused security/systems audit of the provided Rust code. I’ve prioritized **P1/P2** issues and referenced concrete functions/locations from the snippet.

---

## Executive summary

Main risks found:

- **P1:** silent **data corruption / invalid domain values** from unchecked integer casts (`i64 -> u8/u32`) in DB row decoding
- **P1:** **incorrect telemetry query logic** (`GROUP BY ... HAVING MAX(collected_at)`) returns nondeterministic rows, breaking anomaly/pattern detection
- **P1:** **financial rule violation**: pricing calculation uses `f64` for money
- **P2:** multiple **silent failure** patterns (`unwrap_or`, fallback dates/times, swallowed DB errors) hide corrupt data and operational faults
- **P2:** attendance/payroll business logic can **underpay / miscompute shifts** (overnight shifts become 0 hours)
- **P2:** KPI/task logic contains **status mismatches** and inconsistent enum/string storage

No direct SQL injection was found in the shown code because dynamic SQL is limited to column assignments and values are still parameter-bound. However, some dynamic-query and serialization patterns are fragile.

---

# Findings

---

## 1) P1 CRITICAL — Invalid DB values can corrupt pod/task/event models via unchecked narrowing casts

**File: maintenance persistence module**  
**Lines:** `row_to_event` / `row_to_task` conversions  
- `row_to_event`: `pod_id: row.pod_id.map(|p| p as u8)` and `customers_affected: row.customers_affected.map(|c| c as u32)`, `downtime_minutes: row.downtime_minutes.map(|d| d as u32)`
- `row_to_task`: `pod_id: row.pod_id.map(|p| p as u8)`, `priority: row.priority as u8`

### Description
SQLite is weakly typed and can contain negative or oversized integers. These casts will silently wrap/truncate:

- `-1i64 as u8 == 255`
- `300i64 as u8 == 44`
- `-5i64 as u32` becomes a huge value

That can misroute maintenance to the wrong pod, create impossible downtime/customer counts, and corrupt priority ordering.

### Concrete fix
Validate ranges with `try_from` and reject invalid rows.

```rust
fn row_to_event(row: EventRow) -> anyhow::Result<MaintenanceEvent> {
    use anyhow::{anyhow, Context};

    let detected_at = row
        .detected_at_str
        .as_deref()
        .context("maintenance_events.detected_at is NULL")?
        .parse::<DateTime<chrono::FixedOffset>>()
        .context("invalid detected_at RFC3339")?
        .with_timezone(&Utc);

    let resolved_at = row
        .resolved_at_str
        .as_deref()
        .map(|s| {
            s.parse::<DateTime<chrono::FixedOffset>>()
                .map(|d| d.with_timezone(&Utc))
                .context("invalid resolved_at RFC3339")
        })
        .transpose()?;

    let pod_id = row
        .pod_id
        .map(|p| u8::try_from(p).map_err(|_| anyhow!("invalid pod_id {}", p)))
        .transpose()?;

    let customers_affected = row
        .customers_affected
        .map(|c| u32::try_from(c).map_err(|_| anyhow!("invalid customers_affected {}", c)))
        .transpose()?;

    let downtime_minutes = row
        .downtime_minutes
        .map(|d| u32::try_from(d).map_err(|_| anyhow!("invalid downtime_minutes {}", d)))
        .transpose()?;

    Ok(MaintenanceEvent {
        id: Uuid::parse_str(&row.id)?,
        pod_id,
        event_type: serde_json::from_str(&row.event_type)?,
        severity: serde_json::from_str(&row.severity)?,
        component: serde_json::from_str(&row.component)?,
        description: row.description,
        detected_at,
        resolved_at,
        resolution_method: row
            .resolution_method
            .as_deref()
            .map(serde_json::from_str)
            .transpose()?,
        source: row.source,
        correlation_id: row
            .correlation_id
            .as_deref()
            .map(Uuid::parse_str)
            .transpose()?,
        revenue_impact_paise: row.revenue_impact_paise,
        customers_affected,
        downtime_minutes,
        cost_estimate_paise: row.cost_estimate_paise,
        assigned_staff_id: row.assigned_staff_id,
        metadata: serde_json::from_str(&row.metadata)?,
    })
}

fn row_to_task(row: TaskRow) -> anyhow::Result<MaintenanceTask> {
    use anyhow::{anyhow, Context};

    let created_at = row
        .created_at_str
        .as_deref()
        .context("maintenance_tasks.created_at is NULL")?
        .parse::<DateTime<chrono::FixedOffset>>()
        .context("invalid created_at RFC3339")?
        .with_timezone(&Utc);

    let due_by = row
        .due_by_str
        .as_deref()
        .map(|s| {
            s.parse::<DateTime<chrono::FixedOffset>>()
                .map(|d| d.with_timezone(&Utc))
                .context("invalid due_by RFC3339")
        })
        .transpose()?;

    let pod_id = row
        .pod_id
        .map(|p| u8::try_from(p).map_err(|_| anyhow!("invalid pod_id {}", p)))
        .transpose()?;

    let priority = u8::try_from(row.priority)
        .map_err(|_| anyhow!("invalid priority {}", row.priority))?;

    let status_json = format!("\"{}\"", row.status);

    Ok(MaintenanceTask {
        id: Uuid::parse_str(&row.id)?,
        title: row.title,
        description: row.description,
        pod_id,
        component: serde_json::from_str(&row.component)?,
        priority,
        status: serde_json::from_str(&status_json)?,
        created_at,
        due_by,
        assigned_to: row.assigned_to,
        source_event_id: row
            .source_event_id
            .as_deref()
            .map(Uuid::parse_str)
            .transpose()?,
        before_metrics: row
            .before_metrics
            .as_deref()
            .map(serde_json::from_str)
            .transpose()?,
        after_metrics: row
            .after_metrics
            .as_deref()
            .map(serde_json::from_str)
            .transpose()?,
        cost_estimate_paise: row.cost_estimate_paise,
        actual_cost_paise: row.actual_cost_paise,
    })
}
```

Also add DB constraints:

```rust
sqlx::query(
    "CREATE TABLE IF NOT EXISTS maintenance_tasks (
        id TEXT PRIMARY KEY,
        title TEXT NOT NULL,
        description TEXT NOT NULL,
        pod_id INTEGER CHECK (pod_id IS NULL OR (pod_id BETWEEN 1 AND 8)),
        component TEXT NOT NULL,
        priority INTEGER NOT NULL DEFAULT 3 CHECK (priority BETWEEN 0 AND 255),
        status TEXT NOT NULL DEFAULT 'Open',
        created_at TEXT NOT NULL,
        due_by TEXT,
        assigned_to TEXT,
        source_event_id TEXT,
        before_metrics TEXT,
        after_metrics TEXT,
        cost_estimate_paise INTEGER,
        actual_cost_paise INTEGER
    )",
)
```

---

## 2) P1 CRITICAL — Telemetry “latest row per pod” query is logically wrong and nondeterministic

**File: anomaly detection module**  
**Lines:** `run_anomaly_scan`, `check_patterns`  
**Query:**
```sql
FROM hardware_telemetry
WHERE collected_at > ?1
GROUP BY pod_id
HAVING MAX(collected_at)
```

### Description
This query does **not** reliably return the latest row per pod in SQLite. `GROUP BY pod_id` with non-aggregated selected columns returns arbitrary row values from each group. `HAVING MAX(collected_at)` is just a truthy aggregate expression, not a row picker.

Impact:
- anomaly detection may evaluate stale or random telemetry
- pattern matching may be wrong
- alerts can be missed or falsely raised
- preventive maintenance decisions become unsafe

This is a core functionality break.

### Concrete fix
Use a proper correlated subquery or join on max timestamp.

```rust
let rows: Result<Vec<HwRow>, sqlx::Error> = sqlx::query(
    "SELECT h.pod_id,
            h.gpu_temp_celsius,
            h.cpu_temp_celsius,
            h.gpu_power_watts,
            h.disk_smart_health_pct,
            h.process_handle_count,
            h.cpu_usage_pct,
            h.memory_usage_pct,
            h.disk_usage_pct,
            h.network_latency_ms
     FROM hardware_telemetry h
     INNER JOIN (
         SELECT pod_id, MAX(collected_at) AS max_collected_at
         FROM hardware_telemetry
         WHERE collected_at > ?1
         GROUP BY pod_id
     ) latest
       ON latest.pod_id = h.pod_id
      AND latest.max_collected_at = h.collected_at"
)
.bind(&cutoff)
.fetch_all(pool)
.await
.map(|rows| {
    rows.into_iter()
        .map(|r| {
            use sqlx::Row;
            HwRow {
                pod_id: r.get("pod_id"),
                gpu_temp_celsius: r.get("gpu_temp_celsius"),
                cpu_temp_celsius: r.get("cpu_temp_celsius"),
                gpu_power_watts: r.get("gpu_power_watts"),
                disk_smart_health_pct: r.get("disk_smart_health_pct"),
                process_handle_count: r.get("process_handle_count"),
                cpu_usage_pct: r.get("cpu_usage_pct"),
                memory_usage_pct: r.get("memory_usage_pct"),
                disk_usage_pct: r.get("disk_usage_pct"),
                network_latency_ms: r.get("network_latency_ms"),
            }
        })
        .collect()
});
```

Apply the same fix in `check_patterns`.

Recommended supporting index:

```rust
sqlx::query(
    "CREATE INDEX IF NOT EXISTS idx_hw_telemetry_pod_collected_at
     ON hardware_telemetry(pod_id, collected_at DESC)"
).execute(pool).await?;
```

---

## 3) P1 CRITICAL — Money calculation violates “integer paise only, never f64”

**File: dynamic pricing module**  
**Lines:** `recommend_pricing`

```rust
let recommended =
    (current_price_paise as f64 * (1.0 + change_pct as f64 / 100.0)) as i64;
```

### Description
You explicitly require all monetary values to use integer paise and **never f64**. This code computes price using floating point and truncates. That creates rounding drift and policy violations.

### Concrete fix
Represent percentage changes in integer basis points or integer percent and compute with integer math only.

```rust
pub fn recommend_pricing(
    forecast_occupancy_pct: f32,
    current_price_paise: i64,
    is_peak: bool,
    is_weekend: bool,
) -> PricingRecommendation {
    let (change_pct_display, change_bps, reason) = if forecast_occupancy_pct > 80.0 {
        let pct = if is_peak { 15 } else { 10 };
        (
            pct as f32,
            pct * 100,
            format!("High forecasted demand ({:.0}% occupancy)", forecast_occupancy_pct),
        )
    } else if forecast_occupancy_pct < 30.0 {
        let pct = if is_weekend { -10 } else { -15 };
        (
            pct as f32,
            pct * 100,
            format!(
                "Low forecasted demand ({:.0}% occupancy) — discount to drive traffic",
                forecast_occupancy_pct
            ),
        )
    } else {
        (
            0.0,
            0,
            format!(
                "Normal demand ({:.0}% occupancy) — no change recommended",
                forecast_occupancy_pct
            ),
        )
    };

    // integer-only price calculation with rounding half away from zero
    let numerator = current_price_paise
        .saturating_mul(10_000 + change_bps as i64);
    let recommended_price_paise = if numerator >= 0 {
        (numerator + 5_000) / 10_000
    } else {
        (numerator - 5_000) / 10_000
    };

    PricingRecommendation {
        date: chrono::Utc::now().to_rfc3339(),
        current_price_paise,
        recommended_price_paise,
        change_pct: change_pct_display,
        reason,
        confidence: if forecast_occupancy_pct > 0.0 { 0.5 } else { 0.1 },
        requires_approval: true,
    }
}
```

---

## 4) P2 HIGH — Silent fallback to `Utc::now()` or dummy dates hides corrupt DB data

**File: maintenance persistence module**  
**Lines:** `row_to_event`, `row_to_task`, `query_business_metrics`, `row_to_employee`, `row_to_attendance`

Examples:
- invalid/missing `detected_at` => `Utc::now()`
- invalid/missing `created_at` => `Utc::now()`
- invalid dates => `2000-01-01`
- invalid skills => default empty vec

### Description
Corrupt persisted data is silently converted into plausible values. That poisons summaries, KPI windows, due dates, payroll periods, and audit trails.

This is especially dangerous in preventive maintenance because MTTR / forecasting / labor calculation depend on trustworthy timestamps.

### Concrete fix
Fail closed on invalid required fields. Only default optional/non-critical fields intentionally.

```rust
let detected_at = row
    .detected_at_str
    .as_deref()
    .ok_or_else(|| anyhow::anyhow!("maintenance_events.detected_at missing"))?
    .parse::<DateTime<chrono::FixedOffset>>()
    .map(|d| d.with_timezone(&Utc))?;

let created_at = row
    .created_at_str
    .as_deref()
    .ok_or_else(|| anyhow::anyhow!("maintenance_tasks.created_at missing"))?
    .parse::<DateTime<chrono::FixedOffset>>()
    .map(|d| d.with_timezone(&Utc))?;
```

For dates:

```rust
let date = chrono::NaiveDate::parse_from_str(&row.date, "%Y-%m-%d")
    .map_err(|e| anyhow::anyhow!("invalid business metric date '{}': {}", row.date, e))?;
```

For skills JSON:

```rust
let skills: Vec<String> = serde_json::from_str(&row.skills)
    .map_err(|e| anyhow::anyhow!("invalid employee skills JSON for {}: {}", row.id, e))?;
```

---

## 5) P2 HIGH — `get_summary` and KPI functions swallow database failures and report false healthy state

**File:** maintenance persistence module  
**Lines:** `get_summary`, `calculate_kpis`

Examples:
```rust
.fetch_one(pool).await.unwrap_or((0,))
```

### Description
On DB failure, these functions convert errors into zeros. Operational dashboards then show:
- 0 open tasks
- 0 events
- 0 self-heals
- 0 tasks completed/open

That is a dangerous silent failure and can suppress escalation.

### Concrete fix
Propagate DB errors.

```rust
let open_row: (i64,) = sqlx::query_as(
    "SELECT COUNT(*) FROM maintenance_tasks WHERE status IN ('Open','Assigned','InProgress')",
)
.fetch_one(pool)
.await?;
```

And similarly in `calculate_kpis`:

```rust
let (total_events,): (i64,) = sqlx::query_as(
    "SELECT COUNT(*) FROM maintenance_events WHERE detected_at >= ?1",
)
.bind(&since_str)
.fetch_one(pool)
.await?;
```

If you want resilience, log and return explicit degraded status instead of fake zeroes.

---

## 6) P2 HIGH — Payroll uses `f64` hours and floating multiplication for paise, violating financial precision requirements

**File:** attendance/payroll module  
**Lines:** `AttendanceRecord.hours_worked: f64`, `PayrollSummary.total_hours: f64`, `calculate_monthly_payroll`

```rust
let emp_total = (row.total_hours * row.hourly_rate_paise as f64).round() as i64;
```

### Description
Your stated rule is strict for money, but payroll still derives money from `f64 * i64`. This can drift over time and produce inconsistent paise totals depending on binary floating rounding.

### Concrete fix
Store worked time in integer minutes (or seconds), and derive paise using integer arithmetic.

Example schema + model direction:
```rust
pub struct AttendanceRecord {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub date: NaiveDate,
    pub clock_in: Option<String>,
    pub clock_out: Option<String>,
    pub source: String,
    pub minutes_worked: i32,
}
```

DB:
```rust
"CREATE TABLE IF NOT EXISTS attendance_records (
    id TEXT PRIMARY KEY,
    employee_id TEXT NOT NULL,
    date TEXT NOT NULL,
    clock_in TEXT,
    clock_out TEXT,
    source TEXT DEFAULT 'manual',
    minutes_worked INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (employee_id) REFERENCES employees(id)
)"
```

Computation:
```rust
let minutes_worked = match (clock_in, clock_out) {
    (Some(ci), Some(co)) => {
        let t_in = chrono::NaiveTime::parse_from_str(ci, "%H:%M")?;
        let t_out = chrono::NaiveTime::parse_from_str(co, "%H:%M")?;
        let mins = (t_out - t_in).num_minutes();
        i32::try_from(mins.max(0)).unwrap_or(0)
    }
    _ => 0,
};

let emp_total = (i64::from(row.total_minutes) * row.hourly_rate_paise + 30) / 60;
```

---

## 7) P2 HIGH — Overnight shifts are miscomputed as 0 hours

**File:** attendance module  
**Lines:** `record_attendance`

```rust
let secs = (o - i).num_seconds();
if secs > 0 { secs as f64 / 3600.0 } else { 0.0 }
```

### Description
If a shift spans midnight, `clock_out < clock_in`, so hours become `0.0`. That silently underpays staff and corrupts labor analytics.

### Concrete fix
Explicitly support overnight shifts.

```rust
let minutes_worked = match (clock_in, clock_out) {
    (Some(ci), Some(co)) => {
        let t_in = chrono::NaiveTime::parse_from_str(ci, "%H:%M")?;
        let t_out = chrono::NaiveTime::parse_from_str(co, "%H:%M")?;
        let mins = if t_out >= t_in {
            (t_out - t_in).num_minutes()
        } else {
            // overnight shift
            ((chrono::NaiveTime::from_hms_opt(23, 59, 59).unwrap() - t_in).num_minutes() + 1)
                + (t_out - chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap()).num_minutes()
        };
        i32::try_from(mins).unwrap_or(0)
    }
    _ => 0,
};
```

Better: store full timestamps, not separate date + times.

---

## 8) P2 HIGH — `update_employee` cannot clear `face_enrollment_id` to NULL

**File:** HR module  
**Lines:** `update_employee`

### Description
`face_enrollment_id: Option<&str>` is used as “field present?” and also as “value”. This means:
- `None` => do not update
- there is **no way** to set the DB value to `NULL`

That creates stale biometric enrollment references, a security/privacy issue.

### Concrete fix
Use nested option semantics: `Option<Option<&str>>`.

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
    face_enrollment_id: Option<Option<&str>>,
) -> anyhow::Result<bool> {
    let mut sets = Vec::new();
    let mut query = sqlx::QueryBuilder::<sqlx::Sqlite>::new("UPDATE employees SET ");
    let mut separated = query.separated(", ");

    if let Some(n) = name {
        separated.push("name = ").push_bind(n);
    }
    if let Some(r) = role {
        separated.push("role = ").push_bind(serde_json::to_string(r)?.replace('"', ""));
    }
    if let Some(s) = skills {
        separated.push("skills = ").push_bind(serde_json::to_string(s)?);
    }
    if let Some(rate) = hourly_rate_paise {
        separated.push("hourly_rate_paise = ").push_bind(rate);
    }
    if let Some(p) = phone {
        separated.push("phone = ").push_bind(p);
    }
    if let Some(a) = is_active {
        separated.push("is_active = ").push_bind(a as i64);
    }
    if let Some(face) = face_enrollment_id {
        separated.push("face_enrollment_id = ").push_bind(face);
    }

    query.push(" WHERE id = ").push_bind(id);
    let result = query.build().execute(pool).await?;
    Ok(result.rows_affected() > 0)
}
```

This also fixes type confusion in issue #9.

---

## 9) P2 HIGH — `update_employee` binds all values as `String`, risking type inconsistency and subtle query behavior

**File:** HR module  
**Lines:** `update_employee`

### Description
The function stores all dynamic bind values in `Vec<String>`, including integers and booleans. SQLite may coerce them, but this is fragile and can defeat indexes / create weird type affinity behavior.

Not an injection bug, but definitely a correctness and maintenance risk.

### Concrete fix
Use `sqlx::QueryBuilder` and bind typed values directly, as shown in the fix for finding #8.

---

## 10) P2 HIGH — KPI logic uses impossible status `'Verified'`, so completed task counts are wrong

**File:** KPI module  
**Lines:** `calculate_kpis`

```sql
status IN ('Completed', 'Verified')
```

### Description
`TaskStatus` enum has no `Verified` variant. Completed tasks may be counted correctly, but `'Verified'` suggests an old/removed state. More importantly, inconsistent string enums are used all over the codebase. This is a business logic mismatch that can break operational reporting.

### Concrete fix
Use canonical enum string values only.

```rust
let (tasks_completed,): (i64,) = sqlx::query_as(
    "SELECT COUNT(*) FROM maintenance_tasks
     WHERE created_at >= ?1 AND status = 'Completed'",
)
.bind(&since_str)
.fetch_one(pool)
.await?;
```

Consider helper functions for canonical status strings.

---

## 11) P2 HIGH — `calculate_kpis` MTBF formula is materially wrong for maintenance analytics

**File:** KPI module  
**Lines:** `calculate_kpis`

```rust
let mtbf_hours = if total_events > 0 {
    total_hours / total_events as f64
} else {
    total_hours
};
```

### Description
This divides period hours by **all events**, not by actual failure events. Preventive alerts, scheduled maintenance, software updates, self-heals, etc. will all reduce MTBF incorrectly. This breaks core KPI semantics.

### Concrete fix
Restrict numerator/denominator to failure-class events only.

```rust
let (failure_events,): (i64,) = sqlx::query_as(
    "SELECT COUNT(*) FROM maintenance_events
     WHERE detected_at >= ?1
       AND event_type IN ('\"EmergencyShutdown\"','\"PredictiveAlert\"','\"PodHealerIntervention\"','\"Tier1FixApplied\"')"
)
.bind(&since_str)
.fetch_one(pool)
.await?;

let mtbf_hours = if failure_events > 0 {
    (days as f64 * 24.0) / failure_events as f64
} else {
    days as f64 * 24.0
};
```

Better: define explicit failure taxonomy instead of string literals.

---

## 12) P2 HIGH — `check_patterns` ignores each pattern’s `lookback_minutes`

**File:** anomaly/pattern module  
**Lines:** `check_patterns`

### Description
Code fetches latest rows within the max lookback, but then does not enforce per-pattern lookback when evaluating. Since only one latest row is used anyway, `lookback_minutes` is effectively ignored.

This is core business logic drift: pattern alerts may fire based on telemetry outside the intended pattern horizon.

### Concrete fix
Either fetch a full time window and evaluate per pattern, or at minimum ensure latest row timestamp is available and compare to each pattern’s lookback.

Example with timestamp added:

```rust
struct HwRow {
    pod_id: String,
    collected_at: DateTime<Utc>,
    // ...
}
```

Query:
```rust
SELECT h.pod_id, h.collected_at, ...
```

Pattern check:
```rust
if row.collected_at < now - chrono::Duration::minutes(pattern.lookback_minutes as i64) {
    continue;
}
```

---

## 13) P2 HIGH — `calculate_rul` pod ID parsing is broken for `"pod-1"` format

**File:** anomaly/RUL module  
**Lines:** `calculate_rul`

```rust
let pod_num: u8 = pod_id
    .trim_start_matches("pod")
    .trim_start_matches("pod-")
    .parse()
    .unwrap_or(0);
```

### Description
For `"pod-1"`:
- first trim removes `"pod"` => `"-1"`
- second trim on `"-1"` does nothing
- parse fails => `0`

This silently converts a valid pod into pod 0.

### Concrete fix
Strip `"pod-"` before `"pod"` or use `strip_prefix`.

```rust
let pod_num: u8 = pod_id
    .strip_prefix("pod-")
    .or_else(|| pod_id.strip_prefix("pod"))
    .unwrap_or(pod_id)
    .parse()
    .unwrap_or(0);
```

Prefer erroring instead of defaulting to 0:

```rust
let pod_num: u8 = pod_id
    .strip_prefix("pod-")
    .or_else(|| pod_id.strip_prefix("pod"))
    .unwrap_or(pod_id)
    .parse()
    .ok()?;
```

---

## 14) P2 HIGH — XAI pod ID parsing inconsistent with rest of system

**File:** XAI module  
**Lines:** `explain_anomaly`

```rust
pod_id: pod_id.strip_prefix("pod_").and_then(|s| s.parse().ok()),
```

### Description
Elsewhere pods appear as `"pod1"` / `"pod-1"`. Here only `"pod_"` is supported. Explanations may lose pod association, harming auditability during incident review.

### Concrete fix
Normalize all accepted prefixes.

```rust
fn parse_pod_id(pod_id: &str) -> Option<u8> {
    pod_id
        .strip_prefix("pod-")
        .or_else(|| pod_id.strip_prefix("pod_"))
        .or_else(|| pod_id.strip_prefix("pod"))
        .unwrap_or(pod_id)
        .parse()
        .ok()
}
```

Use:
```rust
pod_id: parse_pod_id(pod_id),
```

---

## 15) P2 HIGH — No foreign key enforcement is enabled for SQLite

**File:** DB init functions  
**Lines:** all table initialization (`init_maintenance_tables`, `init_hr_tables`, etc.)

### Description
SQLite does not enforce foreign keys unless `PRAGMA foreign_keys = ON` is set per connection. You define:
```sql
FOREIGN KEY (employee_id) REFERENCES employees(id)
```
but unless enabled, orphan attendance rows can be inserted silently.

### Concrete fix
Enable PRAGMA on startup / per pool connection.

```rust
pub async fn init_sqlite_pragmas(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query("PRAGMA foreign_keys = ON;").execute(pool).await?;
    sqlx::query("PRAGMA journal_mode = WAL;").execute(pool).await?;
    sqlx::query("PRAGMA busy_timeout = 5000;").execute(pool).await?;
    Ok(())
}
```

Call this before table creation.

---

## 16) P2 HIGH — Missing DB constraints allow invalid percentages, negative money, and invalid pod ranges

**File:** schema creation functions  
**Lines:** `init_maintenance_tables`, `init_business_tables`, `init_hr_tables`

### Description
Application code assumes valid business-domain values, but SQLite schema does not enforce them. Corrupt inserts from other modules/tools will propagate.

Examples:
- occupancy > 100
- negative sessions_count
- pod_id = 255
- hourly_rate_paise < 0
- negative downtime/customers

### Concrete fix
Add `CHECK` constraints.

```rust
sqlx::query(
    "CREATE TABLE IF NOT EXISTS daily_business_metrics (
        date TEXT PRIMARY KEY,
        revenue_gaming_paise INTEGER NOT NULL DEFAULT 0,
        revenue_cafe_paise INTEGER NOT NULL DEFAULT 0,
        revenue_other_paise INTEGER NOT NULL DEFAULT 0,
        expense_rent_paise INTEGER NOT NULL DEFAULT 0,
        expense_utilities_paise INTEGER NOT NULL DEFAULT 0,
        expense_salaries_paise INTEGER NOT NULL DEFAULT 0,
        expense_maintenance_paise INTEGER NOT NULL DEFAULT 0,
        expense_other_paise INTEGER NOT NULL DEFAULT 0,
        sessions_count INTEGER NOT NULL DEFAULT 0 CHECK (sessions_count >= 0),
        occupancy_rate_pct REAL NOT NULL DEFAULT 0 CHECK (occupancy_rate_pct >= 0 AND occupancy_rate_pct <= 100),
        peak_occupancy_pct REAL NOT NULL DEFAULT 0 CHECK (peak_occupancy_pct >= 0 AND peak_occupancy_pct <= 100)
    )",
).execute(pool).await?;
```

Similarly for employees/events/tasks.

---

## 17) P3 MEDIUM — `query_events` and `query_tasks` fetch broad result sets then filter in Rust

**File:** maintenance persistence module  
**Lines:** `query_events`, `query_tasks`

### Description
For large tables this is inefficient and can distort result counts. `LIMIT` is applied before Rust-side filtering, so callers may receive fewer matching rows than requested.

### Concrete fix
Build SQL dynamically with parameter binding.

```rust
pub async fn query_events(
    pool: &SqlitePool,
    pod_id: Option<u8>,
    since: Option<DateTime<Utc>>,
    limit: u32,
) -> anyhow::Result<Vec<MaintenanceEvent>> {
    let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
        "SELECT id, pod_id, event_type, severity, component, description,
                detected_at, resolved_at, resolution_method, source,
                correlation_id, revenue_impact_paise, customers_affected,
                downtime_minutes, cost_estimate_paise, assigned_staff_id, metadata
         FROM maintenance_events"
    );

    let mut has_where = false;
    if let Some(pid) = pod_id {
        qb.push(if has_where { " AND " } else { " WHERE " });
        has_where = true;
        qb.push("pod_id = ").push_bind(pid as i64);
    }
    if let Some(s) = since {
        qb.push(if has_where { " AND " } else { " WHERE " });
        qb.push("detected_at >= ").push_bind(s.to_rfc3339());
    }

    qb.push(" ORDER BY detected_at DESC LIMIT ").push_bind(limit as i64);

    let rows: Vec<EventRow> = qb.build_query_as().fetch_all(pool).await?;
    rows.into_iter().map(row_to_event).collect()
}
```

---

## 18) P3 MEDIUM — `should_use_ai` contradicts its own doc comment

**File:** AI diagnosis module  
**Lines:** `should_use_ai`

Comment says:
- more than 2 anomalies **and**
- max severity Critical/High

Code does:
```rust
anomaly_count > 2 || max_severity == "Critical" || max_severity == "High"
```

### Description
This triggers AI for every High severity single anomaly, contrary to stated budget-saving design.

### Concrete fix
Match documented behavior:

```rust
pub fn should_use_ai(anomaly_count: usize, max_severity: &str) -> bool {
    anomaly_count > 2 && matches!(max_severity, "Critical" | "High")
}
```

If current behavior is intended, update docs.

---

## 19) P3 MEDIUM — `default_patterns` references `gpu_usage_pct` not present in `HwRow`

**File:** anomaly/pattern module  
**Lines:** `default_patterns`, `HwRow::metric_value`

### Description
Pattern `"GPU Thermal Throttle"` includes `gpu_usage_pct`, but `HwRow` and `metric_value` do not support it. This condition will always be ignored silently.

### Concrete fix
Either add the column to query + row model, or remove the condition.

```rust
struct HwRow {
    pod_id: String,
    gpu_temp_celsius: Option<f64>,
    cpu_temp_celsius: Option<f64>,
    gpu_power_watts: Option<f64>,
    gpu_usage_pct: Option<f64>,
    // ...
}

fn metric_value(&self, name: &str) -> Option<f64> {
    match name {
        "gpu_usage_pct" => self.gpu_usage_pct,
        // ...
        _ => None,
    }
}
```

And include in SQL select.

---

## 20) P3 MEDIUM — Attendance source/date/time fields lack validation and normalization

**File:** attendance module  
**Lines:** `record_attendance`

### Description
`date`, `clock_in`, `clock_out`, and `source` are accepted as raw strings. Invalid formats become 0 hours but still persist. This creates dirty data and audit weakness.

### Concrete fix
Validate before insert.

```rust
let parsed_date = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")?;
let parsed_clock_in = clock_in
    .map(|s| chrono::NaiveTime::parse_from_str(s, "%H:%M"))
    .transpose()?;
let parsed_clock_out = clock_out
    .map(|s| chrono::NaiveTime::parse_from_str(s, "%H:%M"))
    .transpose()?;

if source.trim().is_empty() {
    anyhow::bail!("attendance source must not be empty");
}

sqlx::query(
    "INSERT INTO attendance_records
        (id, employee_id, date, clock_in, clock_out, source, minutes_worked)
     VALUES (?1,?2,?3,?4,?5,?6,?7)",
)
.bind(&id)
.bind(employee_id)
.bind(parsed_date.format("%Y-%m-%d").to_string())
.bind(parsed_clock_in.map(|t| t.format("%H:%M").to_string()))
.bind(parsed_clock_out.map(|t| t.format("%H:%M").to_string()))
.bind(source.trim())
.bind(minutes_worked)
.execute(pool)
.await?;
```

---

# Additional notes

## SQL injection review
I did **not** identify direct SQL injection in the provided code:
- value inputs are parameter-bound with `.bind(...)`
- dynamic SQL in `update_employee` constructs only fixed column assignments, not user-controlled identifiers

However, the dynamic update approach is still risky from a correctness standpoint and should be replaced with `QueryBuilder`.

## Windows-specific concerns
The provided code does not directly touch Session 0/1 isolation or static CRT issues. No audit findings specific to those areas are visible in this snippet.

---

# Top fixes to do first

1. **Fix telemetry latest-row query** in `run_anomaly_scan` and `check_patterns`
2. **Eliminate unchecked integer casts** in all row decoders
3. **Remove f64 from price/payroll money calculations**
4. **Stop swallowing DB/data parse errors**
5. **Fix overnight attendance/payroll logic**
6. **Enable SQLite foreign keys and add CHECK constraints**

---

If you want, I can next turn this into a **patch-style diff** grouped by file/function, or produce a **ranked remediation plan** with estimated implementation effort.