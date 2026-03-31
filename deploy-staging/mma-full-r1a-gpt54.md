Below is a **full audit of Bundle A** with emphasis on **P1/P2** per your rules.

---

# Executive summary

This bundle is **not safe to ship as-is** for a venue-control system handling maintenance, payroll, attendance, and automated recovery.

## Top critical themes
- **P1 data corruption / logic corruption** from silent fallback behavior and invalid narrowing conversions.
- **P1/P2 money/time precision violations** due to widespread `f64` usage for attendance/payroll/business metrics.
- **P1 race/business-logic bugs** in task auto-assignment and duplicate task creation.
- **P2 silent failure patterns** (`unwrap_or`, defaulting on DB/query/parse failures) causing hidden bad state.
- **P2 schema/model mismatch** for enum serialization and string comparisons.
- **P3 performance issues** from fetching broad sets then filtering in Rust.

No obvious SQL injection was found in this bundle: queries are parameterized.  
No lock-across-`.await` issue was found with Tokio `RwLock` usage in the shown code.  
However, there are multiple **atomicity/race** issues.

---

# Findings

---

## 1) P1 CRITICAL — Silent DB corruption masking via clamped/nonsensical conversions
**Location:** `row_to_event`, `row_to_task`  
- `maintenance_store.rs` equivalent:
  - `row_to_event`: pod_id/customers_affected/downtime_minutes conversions
  - `row_to_task`: pod_id/priority conversions

### Problem
The code claims “safe narrowing cast” but actually **silently rewrites corrupt DB values into different valid values**:

```rust
pod_id: row.pod_id.and_then(|p| {
    u8::try_from(p.clamp(0, 255)).ok().map(|v| v.clamp(1, 8))
}),
```

Examples:
- DB `pod_id = 0` becomes `Some(1)`
- DB `pod_id = 99` becomes `Some(8)`
- DB `pod_id = -5` becomes `Some(1)`

Likewise:
```rust
customers_affected: row.customers_affected.map(|c| {
    u32::try_from(c.max(0)).unwrap_or(u32::MAX)
}),
```

This silently converts negative corrupt values to `0`, and huge values to `u32::MAX`.

This is **data corruption masking**: downstream logic sees valid-looking values that were never true.

### Impact
- Wrong pod gets maintenance action / KPI attribution
- Wrong payroll/business stats
- Corrupt rows become undetectable operationally
- Maintenance automation may act on pod 1/8 incorrectly

### Required fix
Use `try_from` exactly as required, and **reject invalid DB rows** instead of clamping.

### Concrete fix code
```rust
fn parse_pod_id(raw: Option<i64>, row_id: &str) -> anyhow::Result<Option<u8>> {
    match raw {
        None => Ok(None),
        Some(v) => {
            let pod = u8::try_from(v)
                .map_err(|_| anyhow::anyhow!("invalid pod_id {} for row {}", v, row_id))?;
            if !(1..=8).contains(&pod) {
                anyhow::bail!("out-of-range pod_id {} for row {}", pod, row_id);
            }
            Ok(Some(pod))
        }
    }
}

fn parse_u32_opt(field: &str, raw: Option<i64>, row_id: &str) -> anyhow::Result<Option<u32>> {
    match raw {
        None => Ok(None),
        Some(v) => {
            let n = u32::try_from(v)
                .map_err(|_| anyhow::anyhow!("invalid {} {} for row {}", field, v, row_id))?;
            Ok(Some(n))
        }
    }
}

fn row_to_event(row: EventRow) -> anyhow::Result<MaintenanceEvent> {
    let detected_at = parse_rfc3339_required("detected_at", row.detected_at_str.as_deref(), &row.id)?;
    let resolved_at = parse_rfc3339_optional("resolved_at", row.resolved_at_str.as_deref(), &row.id)?;

    Ok(MaintenanceEvent {
        id: Uuid::parse_str(&row.id)?,
        pod_id: parse_pod_id(row.pod_id, &row.id)?,
        event_type: serde_json::from_str(&row.event_type)?,
        severity: serde_json::from_str(&row.severity)?,
        component: serde_json::from_str(&row.component)?,
        description: row.description,
        detected_at,
        resolved_at,
        resolution_method: row.resolution_method.as_deref().map(serde_json::from_str).transpose()?,
        source: row.source,
        correlation_id: row.correlation_id.as_deref().map(Uuid::parse_str).transpose()?,
        revenue_impact_paise: row.revenue_impact_paise,
        customers_affected: parse_u32_opt("customers_affected", row.customers_affected, &row.id)?,
        downtime_minutes: parse_u32_opt("downtime_minutes", row.downtime_minutes, &row.id)?,
        cost_estimate_paise: row.cost_estimate_paise,
        assigned_staff_id: row.assigned_staff_id,
        metadata: serde_json::from_str(&row.metadata)?,
    })
}
```

---

## 2) P1 CRITICAL — Silent timestamp fallback to `Utc::now()` corrupts historical data
**Location:** `row_to_event`, `row_to_task`

### Problem
If `detected_at` / `created_at` parsing fails, code logs warning and substitutes `Utc::now()`.

```rust
Err(e) => {
    tracing::warn!(...);
    Utc::now()
}
```

This is catastrophic for operational forensics. A corrupt or malformed DB row is transformed into a **fresh event/task**, changing:
- incident ordering
- KPI windows
- MTTR
- dashboards
- escalation timing

### Impact
- Historical records become “now”
- false active incidents
- incorrect payroll/task due-time sequencing
- incident response confusion

### Required fix
Do **not** substitute current time for persisted data. Fail row decoding.

### Concrete fix code
```rust
fn parse_rfc3339_required(field: &str, raw: Option<&str>, row_id: &str) -> anyhow::Result<DateTime<Utc>> {
    let s = raw.ok_or_else(|| anyhow::anyhow!("{} is NULL for row {}", field, row_id))?;
    let dt = DateTime::parse_from_rfc3339(s)
        .map_err(|e| anyhow::anyhow!("invalid {} '{}' for row {}: {}", field, s, row_id, e))?;
    Ok(dt.with_timezone(&Utc))
}

fn parse_rfc3339_optional(
    field: &str,
    raw: Option<&str>,
    row_id: &str,
) -> anyhow::Result<Option<DateTime<Utc>>> {
    match raw {
        None => Ok(None),
        Some(s) => {
            let dt = DateTime::parse_from_rfc3339(s)
                .map_err(|e| anyhow::anyhow!("invalid {} '{}' for row {}: {}", field, s, row_id, e))?;
            Ok(Some(dt.with_timezone(&Utc)))
        }
    }
}
```

---

## 3) P1 CRITICAL — Attendance/payroll uses `f64` for hours, violating monetary rules and causing payroll drift
**Location:**  
- model: `AttendanceRecord.hours_worked: f64`
- model: `PayrollSummary.total_hours: f64`
- model: `EmployeePayroll.hours_worked: f64`
- `record_attendance`
- `calculate_monthly_payroll`

### Problem
Your rule says:
- **All money MUST be integer paise (i64), NEVER f64**

The code computes wages from floating hours:

```rust
let hours = total_minutes as f64 / 60.0;
...
let worked_minutes = (row.total_hours * 60.0).round() as i64;
let emp_total = worked_minutes.max(0) * row.hourly_rate_paise / 60;
```

This introduces:
- binary float precision issues
- non-idempotent round-trips
- disagreement between stored attendance and payroll totals

### Impact
- under/over-payment
- audit failure
- staff disputes
- monthly payroll totals drift by repeated sums

### Required fix
Store **worked minutes as integer**, not float hours. Derive human-readable hours only at presentation layer.

### Concrete fix code
Schema migration:
```rust
sqlx::query(
    "ALTER TABLE attendance_records ADD COLUMN worked_minutes INTEGER NOT NULL DEFAULT 0"
).execute(pool).await?;
```

Model changes:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttendanceRecord {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub date: NaiveDate,
    pub clock_in: Option<String>,
    pub clock_out: Option<String>,
    pub source: String,
    pub worked_minutes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayrollSummary {
    pub year: i32,
    pub month: u32,
    pub total_minutes: i64,
    pub total_paise: i64,
    pub by_employee: Vec<EmployeePayroll>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmployeePayroll {
    pub employee_id: String,
    pub name: String,
    pub worked_minutes: i64,
    pub rate_paise: i64,
    pub total_paise: i64,
}
```

Record attendance:
```rust
let worked_minutes: i64 = match (clock_in, clock_out) {
    (Some(ci), Some(co)) => {
        let i = chrono::NaiveTime::parse_from_str(ci, "%H:%M")?;
        let o = chrono::NaiveTime::parse_from_str(co, "%H:%M")?;
        let mut mins = (o - i).num_minutes();
        if mins < 0 {
            mins += 24 * 60;
        }
        mins
    }
    _ => 0,
};

sqlx::query(
    "INSERT INTO attendance_records
     (id, employee_id, date, clock_in, clock_out, source, worked_minutes)
     VALUES (?1,?2,?3,?4,?5,?6,?7)",
)
.bind(&id)
.bind(employee_id)
.bind(date)
.bind(clock_in)
.bind(clock_out)
.bind(source)
.bind(worked_minutes)
.execute(pool)
.await?;
```

Payroll:
```rust
#[derive(sqlx::FromRow)]
struct PayrollRow {
    employee_id: String,
    name: String,
    hourly_rate_paise: i64,
    total_minutes: i64,
}

let rows = sqlx::query_as::<_, PayrollRow>(
    "SELECT e.id AS employee_id, e.name, e.hourly_rate_paise,
            COALESCE(SUM(a.worked_minutes), 0) AS total_minutes
     FROM employees e
     LEFT JOIN attendance_records a
         ON a.employee_id = e.id
         AND a.date >= ?1 AND a.date < ?2
     WHERE e.is_active = 1
     GROUP BY e.id, e.name, e.hourly_rate_paise
     ORDER BY e.name ASC",
)
.bind(&start_date)
.bind(&next_month_start)
.fetch_all(pool)
.await?;

let mut total_minutes = 0i64;
let mut total_paise = 0i64;
let mut by_employee = Vec::with_capacity(rows.len());

for row in rows {
    let mins = row.total_minutes.max(0);
    let emp_total = mins
        .checked_mul(row.hourly_rate_paise)
        .ok_or_else(|| anyhow::anyhow!("payroll overflow for employee {}", row.employee_id))?
        / 60;

    total_minutes += mins;
    total_paise += emp_total;

    by_employee.push(EmployeePayroll {
        employee_id: row.employee_id,
        name: row.name,
        worked_minutes: mins,
        rate_paise: row.hourly_rate_paise,
        total_paise: emp_total,
    });
}
```

---

## 4) P1 CRITICAL — `calculate_kpis` uses unchecked integer casts from DB with `as`
**Location:** `calculate_kpis`

### Problem
Your rule says:
- **All integer casts from DB MUST use try_from, not `as`**

Violations:
```rust
let downtime_minutes = downtime_row.map(|(v,)| v as u32).unwrap_or(0);
...
total_events: total_events as u32,
total_tasks: total_tasks as u32,
tasks_completed: tasks_completed as u32,
tasks_open: tasks_open as u32,
```

### Impact
If DB is corrupted or unexpectedly large/negative:
- wrap/truncate semantics can produce invalid KPIs
- monitoring hides corruption

### Required fix
Use `try_from` and reject impossible values.

### Concrete fix code
```rust
fn i64_to_u32(name: &str, v: i64) -> anyhow::Result<u32> {
    u32::try_from(v)
        .map_err(|_| anyhow::anyhow!("{} out of range for u32: {}", name, v))
}

let downtime_minutes = match downtime_row {
    Some((v,)) => i64_to_u32("downtime_minutes", v)?,
    None => 0,
};

Ok(MaintenanceKPIs {
    period_days: days,
    total_events: i64_to_u32("total_events", total_events)?,
    total_tasks: i64_to_u32("total_tasks", total_tasks)?,
    mttr_minutes,
    mtbf_hours,
    self_heal_rate,
    prediction_accuracy: 0.0,
    false_positive_rate: 0.0,
    downtime_minutes,
    tasks_completed: i64_to_u32("tasks_completed", tasks_completed)?,
    tasks_open: i64_to_u32("tasks_open", tasks_open)?,
})
```

---

## 5) P1 CRITICAL — `collect_venue_snapshot` also violates DB cast rule with unchecked `as`
**Location:** `collect_venue_snapshot`

### Problem
Unchecked DB casts:
```rust
open_maintenance_tasks: open_tasks as u32,
critical_alerts_active: critical_alerts as u32,
staff_on_duty: staff as u32,
```

### Impact
Corrupt DB values can silently become bogus counts.

### Concrete fix code
```rust
fn db_count_to_u32(field: &str, v: i64) -> u32 {
    u32::try_from(v).unwrap_or_else(|_| {
        tracing::error!("{} out of range from DB: {}", field, v);
        0
    })
}
...
open_maintenance_tasks: db_count_to_u32("open_maintenance_tasks", open_tasks),
critical_alerts_active: db_count_to_u32("critical_alerts_active", critical_alerts),
staff_on_duty: db_count_to_u32("staff_on_duty", staff),
```

Prefer returning error instead of zero if this feeds automation.

---

## 6) P1 CRITICAL — `query_business_metrics` uses unchecked DB casts and silent date fallback
**Location:** `query_business_metrics`

### Problem
Violations:
```rust
let date = ...unwrap_or_else(|_| NaiveDate::from_ymd_opt(2000, 1, 1).unwrap());
sessions_count: row.sessions_count as u32,
occupancy_rate_pct: row.occupancy_rate_pct as f32,
peak_occupancy_pct: row.peak_occupancy_pct as f32,
```

- silent fallback date corrupts reporting period
- `sessions_count as u32` violates rule
- occupancy values not validated (NaN, negative, >100)

### Impact
- EBITDA and business reporting can silently include fake date `2000-01-01`
- occupancy dashboards can show invalid values

### Concrete fix code
```rust
let date = chrono::NaiveDate::parse_from_str(&row.date, "%Y-%m-%d")
    .map_err(|e| anyhow::anyhow!("invalid business metric date '{}': {}", row.date, e))?;

let sessions_count = u32::try_from(row.sessions_count)
    .map_err(|_| anyhow::anyhow!("invalid sessions_count {}", row.sessions_count))?;

if !row.occupancy_rate_pct.is_finite() || !(0.0..=100.0).contains(&row.occupancy_rate_pct) {
    anyhow::bail!("invalid occupancy_rate_pct {}", row.occupancy_rate_pct);
}
if !row.peak_occupancy_pct.is_finite() || !(0.0..=100.0).contains(&row.peak_occupancy_pct) {
    anyhow::bail!("invalid peak_occupancy_pct {}", row.peak_occupancy_pct);
}

results.push(DailyBusinessMetrics {
    date,
    revenue_gaming_paise: row.revenue_gaming_paise,
    revenue_cafe_paise: row.revenue_cafe_paise,
    revenue_other_paise: row.revenue_other_paise,
    expense_rent_paise: row.expense_rent_paise,
    expense_utilities_paise: row.expense_utilities_paise,
    expense_salaries_paise: row.expense_salaries_paise,
    expense_maintenance_paise: row.expense_maintenance_paise,
    expense_other_paise: row.expense_other_paise,
    sessions_count,
    occupancy_rate_pct: row.occupancy_rate_pct as f32,
    peak_occupancy_pct: row.peak_occupancy_pct as f32,
});
```

---

## 7) P1 CRITICAL — Auto-assignment race condition can overwrite manual/staff assignment
**Location:** `auto_assign_task`

### Problem
This function:
1. reads candidate employees
2. computes load
3. updates task assignment

There is **no transaction** and no compare-and-set guard. Two concurrent callers can:
- assign same task twice
- overwrite a manual assignment
- pick same “least loaded” tech simultaneously

```rust
UPDATE maintenance_tasks SET assigned_to = ?1, status = 'Assigned' WHERE id = ?2
```

No check that `assigned_to IS NULL`, no transaction.

### Impact
- task ownership corruption
- lost staff assignment
- workload imbalance
- hard-to-debug support incidents

### Concrete fix code
Use transaction + conditional update.

```rust
pub async fn auto_assign_task(
    pool: &SqlitePool,
    task_id: &str,
) -> anyhow::Result<Option<String>> {
    let mut tx = pool.begin().await?;

    let component: Option<String> = sqlx::query_scalar(
        "SELECT component FROM maintenance_tasks WHERE id = ?1"
    )
    .bind(task_id)
    .fetch_optional(&mut *tx)
    .await?;

    let component = match component {
        Some(c) => c,
        None => return Ok(None),
    };

    let employees = sqlx::query_as::<_, (String, String)>(
        "SELECT id, skills
         FROM employees
         WHERE is_active = 1
           AND (role = 'Technician' OR role = 'Manager')
         LIMIT 20"
    )
    .fetch_all(&mut *tx)
    .await?;

    let component_lower = component.to_lowercase().replace('"', "");
    let mut best_id: Option<String> = None;
    let mut best_load = i64::MAX;

    for (emp_id, skills_json) in &employees {
        let skills: Vec<String> = serde_json::from_str(skills_json)?;
        let has_skill = skills.iter().any(|s| s.to_lowercase().contains(&component_lower))
            || skills.iter().any(|s| s.eq_ignore_ascii_case("general"));

        if has_skill || skills.is_empty() {
            let load: i64 = sqlx::query_scalar(
                "SELECT COUNT(*)
                 FROM maintenance_tasks
                 WHERE assigned_to = ?1
                   AND status NOT IN ('Completed', 'Failed', 'Cancelled')"
            )
            .bind(emp_id)
            .fetch_one(&mut *tx)
            .await?;

            if load < best_load {
                best_load = load;
                best_id = Some(emp_id.clone());
            }
        }
    }

    if let Some(ref emp_id) = best_id {
        let result = sqlx::query(
            "UPDATE maintenance_tasks
             SET assigned_to = ?1, status = 'Assigned'
             WHERE id = ?2
               AND assigned_to IS NULL
               AND status = 'Open'"
        )
        .bind(emp_id)
        .bind(task_id)
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() == 0 {
            tx.rollback().await?;
            return Ok(None);
        }
    }

    tx.commit().await?;
    Ok(best_id)
}
```

---

## 8) P1 CRITICAL — Duplicate RUL task creation race
**Location:** `check_rul_thresholds`

### Problem
Classic check-then-insert race:

```rust
let existing: i64 = SELECT COUNT(*) ...
if existing == 0 {
    INSERT INTO maintenance_tasks ...
}
```

Two concurrent collector runs or retries can both insert duplicate open tasks.

### Impact
- duplicate maintenance tasks
- staff confusion
- inflated open-task counts
- repeated notifications

### Concrete fix code
Add a uniqueness key and use atomic insert.

Schema:
```rust
sqlx::query(
    "CREATE UNIQUE INDEX IF NOT EXISTS ux_maint_open_rul_task
     ON maintenance_tasks(pod_id, component, title, status)"
).execute(pool).await?;
```

Better: add source fields:
```rust
ALTER TABLE maintenance_tasks ADD COLUMN source_kind TEXT;
ALTER TABLE maintenance_tasks ADD COLUMN source_key TEXT;
CREATE UNIQUE INDEX IF NOT EXISTS ux_maint_task_source_open
ON maintenance_tasks(source_kind, source_key)
WHERE status NOT IN ('Completed', 'Failed', 'Cancelled');
```

Insert atomically:
```rust
let source_key = format!("rul:{}:{}", pod_num, metric);

let result = sqlx::query(
    "INSERT INTO maintenance_tasks
     (id, title, description, pod_id, component, priority, status, created_at, source_kind, source_key)
     SELECT ?1, ?2, ?3, ?4, ?5, ?6, 'Open', ?7, 'rul', ?8
     WHERE NOT EXISTS (
         SELECT 1
         FROM maintenance_tasks
         WHERE source_kind = 'rul'
           AND source_key = ?8
           AND status NOT IN ('Completed', 'Failed', 'Cancelled')
     )"
)
.bind(&id)
.bind(&title)
.bind(&title)
.bind(pod_num)
.bind(component_json)
.bind(70i64)
.bind(&now)
.bind(&source_key)
.execute(pool)
.await?;
```

---

## 9) P1 CRITICAL — Enum storage format is inconsistent across tables/functions
**Location:** multiple
- `insert_event`: stores enum fields as JSON strings like `"Critical"`
- `insert_task`: stores task status as bare string `Open`
- `calculate_kpis`, `get_summary`, `collect_venue_snapshot`, `auto_assign_task`, `check_rul_thresholds` compare raw SQL strings inconsistently
- `check_rul_thresholds` inserts `component` as plain `"Cooling"` / `"Storage"` while `insert_task` stores JSON string `\"Cooling\"`

### Problem
There are two storage conventions:
- JSON-serialized enum strings with quotes
- plain enum labels without quotes

Examples:
- `maintenance_events.severity` inserted with `serde_json::to_string(&event.severity)` => `"Critical"`
- `collect_venue_snapshot` queries:
  ```sql
  WHERE severity = 'Critical'
  ```
  This will **not match** `"Critical"`.

Likewise:
- `maintenance_tasks.component` via `insert_task` is JSON string with quotes
- `check_rul_thresholds` inserts plain component strings directly
- then it uses `LIKE '%Storage%'` to work around mismatch

This is brittle and already broken.

### Impact
- unresolved critical events not counted
- dashboards/KPIs incorrect
- auto-assign skill matching inconsistent
- query logic depends on ad hoc `LIKE` hacks

### Required fix
Choose **one canonical storage format**: plain text enum labels, not JSON strings.

### Concrete fix code
Define explicit conversions:
```rust
impl Severity {
    fn as_db_str(&self) -> &'static str {
        match self {
            Severity::Critical => "Critical",
            Severity::High => "High",
            Severity::Medium => "Medium",
            Severity::Low => "Low",
        }
    }

    fn from_db_str(s: &str) -> anyhow::Result<Self> {
        match s {
            "Critical" => Ok(Self::Critical),
            "High" => Ok(Self::High),
            "Medium" => Ok(Self::Medium),
            "Low" => Ok(Self::Low),
            _ => anyhow::bail!("invalid Severity '{}'", s),
        }
    }
}
```

Then:
```rust
.bind(event.severity.as_db_str())
.bind(event.event_type.as_db_str())
.bind(event.component.as_db_str())
```

And:
```rust
severity: Severity::from_db_str(&row.severity)?,
```

Do same for `TaskStatus`, `ComponentType`, `StaffRole`, `MaintenanceEventType`.

Also migrate legacy data:
```sql
UPDATE maintenance_events SET severity = trim(severity, '"');
UPDATE maintenance_events SET event_type = trim(event_type, '"');
UPDATE maintenance_events SET component = trim(component, '"');
UPDATE maintenance_tasks SET component = trim(component, '"');
```

---

## 10) P1 CRITICAL — Critical alert count query is wrong due to timestamp format mismatch
**Location:** `collect_venue_snapshot`

### Problem
Query:
```sql
SELECT COUNT(*) FROM maintenance_events
WHERE severity = 'Critical'
  AND resolved_at IS NULL
  AND detected_at > datetime('now', '-1 hour')
```

But `detected_at` is stored as RFC3339 via `to_rfc3339()` (e.g. `2026-03-30T12:34:56+00:00`), while SQLite `datetime('now', '-1 hour')` produces `YYYY-MM-DD HH:MM:SS`.

Text comparison between different timestamp formats is unreliable. Combined with enum quote mismatch, this count is likely wrong most of the time.

### Impact
- active critical alerts undercounted or missed entirely
- AI snapshots wrong
- escalation paths may not trigger

### Concrete fix code
Use same format on both sides:
```rust
let cutoff = (Utc::now() - chrono::Duration::hours(1)).to_rfc3339();

let critical_alerts: i64 = sqlx::query_scalar(
    "SELECT COUNT(*)
     FROM maintenance_events
     WHERE severity = ?1
       AND resolved_at IS NULL
       AND detected_at > ?2"
)
.bind("Critical")
.bind(&cutoff)
.fetch_one(pool)
.await?;
```

---

## 11) P2 HIGH — Silent query failure masking with `unwrap_or` hides outages/data corruption
**Location:** many
- `get_summary` open task count
- `calculate_kpis` total/task/self-heal/tasks counts
- `calculate_feedback_metrics`
- `check_business_alerts`
- `collect_venue_snapshot`
- `check_rul_thresholds`

### Problem
Multiple DB failures are silently replaced with zero/default values:
```rust
.fetch_one(pool).await.unwrap_or((0,))
.fetch_one(pool).await.unwrap_or(0.0)
.fetch_all(...).await.unwrap_or_default()
```

### Impact
- operational outages look like “0 incidents / 0 alerts / 0 revenue”
- false sense of health
- can suppress escalation and response
- impossible to distinguish “no data” from “DB broken”

### Required fix
Log and propagate, or return partial status with explicit error state.

### Concrete fix code
```rust
let open_row: (i64,) = sqlx::query_as(
    "SELECT COUNT(*) FROM maintenance_tasks WHERE status IN ('Open','Assigned','InProgress')",
)
.fetch_one(pool)
.await
.map_err(|e| anyhow::anyhow!("get_summary open task count failed: {}", e))?;
```

If non-fatal:
```rust
let open_tasks = match sqlx::query_scalar::<_, i64>(...)
    .fetch_one(pool)
    .await
{
    Ok(v) => v,
    Err(e) => {
        tracing::error!("open task count query failed: {}", e);
        return Err(e.into());
    }
};
```

---

## 12) P2 HIGH — `update_employee` performs non-atomic multi-column updates
**Location:** `update_employee`

### Problem
Each field is updated in a separate query. If midway one update fails, the employee row is left partially updated.

### Impact
- inconsistent HR records
- role changed but rate not changed, or active flag changed without face enrollment update
- auditability issue

### Concrete fix code
Use a transaction.

```rust
pub async fn update_employee(...) -> anyhow::Result<bool> {
    let mut tx = pool.begin().await?;
    let mut any_updated = false;

    if let Some(n) = name {
        let r = sqlx::query("UPDATE employees SET name = ?1 WHERE id = ?2")
            .bind(n)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        any_updated |= r.rows_affected() > 0;
    }
    // repeat for other fields...

    tx.commit().await?;
    Ok(any_updated)
}
```

Better: one static update with `COALESCE`/flags, but transaction is minimum fix.

---

## 13) P2 HIGH — `face_enrollment_id` cannot be cleared once set
**Location:** `update_employee`

### Problem
Signature:
```rust
face_enrollment_id: Option<&str>,
```
`None` means “don’t update”, so there is no way to set DB value to `NULL`.

### Impact
- stale biometric linkage
- privacy/compliance issue
- impossible to revoke enrollment cleanly

### Concrete fix code
Use tri-state:
```rust
pub enum FieldUpdate<'a, T> {
    Unchanged,
    Set(T),
    Clear,
}

pub async fn update_employee(
    ...
    face_enrollment_id: FieldUpdate<'_, &'_ str>,
) -> anyhow::Result<bool> {
    let mut tx = pool.begin().await?;
    let mut any_updated = false;

    match face_enrollment_id {
        FieldUpdate::Unchanged => {}
        FieldUpdate::Set(v) => {
            let r = sqlx::query("UPDATE employees SET face_enrollment_id = ?1 WHERE id = ?2")
                .bind(v)
                .bind(id)
                .execute(&mut *tx)
                .await?;
            any_updated |= r.rows_affected() > 0;
        }
        FieldUpdate::Clear => {
            let r = sqlx::query("UPDATE employees SET face_enrollment_id = NULL WHERE id = ?1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            any_updated |= r.rows_affected() > 0;
        }
    }

    tx.commit().await?;
    Ok(any_updated)
}
```

---

## 14) P2 HIGH — Attendance accepts invalid employee IDs/date/time and silently writes bad records
**Location:** `record_attendance`

### Problem
Validation is too weak:
- employee_id only checked for non-empty string, not UUID
- date only checked non-empty, not valid `%Y-%m-%d`
- parse errors for time become `hours_worked = 0.0` and record still inserted

### Impact
- orphan attendance rows
- payroll omission without visible failure
- bad records inserted from API mistakes/operator typos

### Concrete fix code
```rust
pub async fn record_attendance(
    pool: &SqlitePool,
    employee_id: &str,
    date: &str,
    clock_in: Option<&str>,
    clock_out: Option<&str>,
    source: &str,
) -> anyhow::Result<()> {
    let _employee_uuid = Uuid::parse_str(employee_id)
        .map_err(|e| anyhow::anyhow!("invalid employee_id '{}': {}", employee_id, e))?;

    let _date = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|e| anyhow::anyhow!("invalid date '{}': {}", date, e))?;

    let worked_minutes = match (clock_in, clock_out) {
        (Some(ci), Some(co)) => {
            let i = chrono::NaiveTime::parse_from_str(ci, "%H:%M")
                .map_err(|e| anyhow::anyhow!("invalid clock_in '{}': {}", ci, e))?;
            let o = chrono::NaiveTime::parse_from_str(co, "%H:%M")
                .map_err(|e| anyhow::anyhow!("invalid clock_out '{}': {}", co, e))?;
            let mut mins = (o - i).num_minutes();
            if mins < 0 { mins += 24 * 60; }
            mins
        }
        (None, None) => 0,
        _ => anyhow::bail!("clock_in and clock_out must both be present or both absent"),
    };

    // optional: verify employee exists
    let exists: Option<String> = sqlx::query_scalar("SELECT id FROM employees WHERE id = ?1")
        .bind(employee_id)
        .fetch_optional(pool)
        .await?;
    if exists.is_none() {
        anyhow::bail!("employee_id '{}' does not exist", employee_id);
    }

    ...
}
```

---

## 15) P2 HIGH — Attendance/payroll can create duplicate records for same employee/date/source
**Location:** schema + `record_attendance`

### Problem
No uniqueness constraint prevents duplicate insertions.

### Impact
- double payroll
- accidental multiple clock-ins/out entries
- silent inflation of hours

### Concrete fix code
If one record per employee/date/source:
```rust
sqlx::query(
    "CREATE UNIQUE INDEX IF NOT EXISTS ux_attendance_employee_date_source
     ON attendance_records(employee_id, date, source)"
).execute(pool).await?;
```

If supporting multiple shifts, require shift key and explicit model.

For upsert:
```rust
sqlx::query(
    "INSERT INTO attendance_records
     (id, employee_id, date, clock_in, clock_out, source, worked_minutes)
     VALUES (?1,?2,?3,?4,?5,?6,?7)
     ON CONFLICT(employee_id, date, source)
     DO UPDATE SET clock_in = excluded.clock_in,
                   clock_out = excluded.clock_out,
                   worked_minutes = excluded.worked_minutes"
)
```

---

## 16) P2 HIGH — `calculate_feedback_metrics` uses `created_at` with SQLite default format versus RFC3339 comparison
**Location:** `init_feedback_tables`, `calculate_feedback_metrics`

### Problem
`created_at` is default:
```sql
DEFAULT (datetime('now'))
```
=> format `YYYY-MM-DD HH:MM:SS`

But filter uses:
```rust
let since = (Utc::now() - ...).to_rfc3339();
WHERE created_at > ?1
```

String comparison across formats is wrong.

### Impact
- metrics for last N days are incorrect
- false precision/recall calculations
- monitoring blind spots

### Concrete fix code
Store RFC3339 consistently or query in SQLite datetime terms.

Best fix: store RFC3339 explicitly on insert and stop using DB default.
```rust
sqlx::query(
    "CREATE TABLE IF NOT EXISTS prediction_outcomes (
        id TEXT PRIMARY KEY,
        prediction_type TEXT NOT NULL,
        predicted_at TEXT NOT NULL,
        predicted_value REAL,
        actual_value REAL,
        was_accurate INTEGER,
        lead_time_hours REAL,
        notes TEXT DEFAULT '',
        created_at TEXT NOT NULL
    )"
).execute(pool).await?;
```

Insert:
```rust
.bind(Utc::now().to_rfc3339())
```

Or compare using SQLite:
```rust
let total: i64 = sqlx::query_scalar(
    "SELECT COUNT(*) FROM prediction_outcomes WHERE datetime(created_at) > datetime('now', ?1)"
)
.bind(format!("-{} days", days))
.fetch_one(pool)
.await?;
```

---

## 17) P2 HIGH — `check_business_alerts` uses `f64` for paise amounts
**Location:** `check_business_alerts`

### Problem
Queries read paise totals as `f64`:
```rust
let today_rev: f64 = ...
let avg_rev: f64 = ...
let maint_cost: f64 = ...
let month_rev: f64 = ...
```

This violates your money rule. Monetary arithmetic must remain integer paise.

### Impact
- threshold comparisons and message values can drift
- precision loss for larger aggregates
- inconsistent financial logic

### Concrete fix code
Use integer paise, convert only for display:
```rust
let today_rev_paise: i64 = sqlx::query_scalar(
    "SELECT COALESCE(revenue_gaming_paise + revenue_cafe_paise, 0)
     FROM daily_business_metrics
     WHERE date = ?1"
).bind(&today).fetch_one(pool).await?;

let avg_rev_paise: i64 = sqlx::query_scalar(
    "SELECT CAST(COALESCE(AVG(revenue_gaming_paise + revenue_cafe_paise), 0) AS INTEGER)
     FROM daily_business_metrics
     WHERE date >= date('now', '-7 days')"
).fetch_one(pool).await?;

if avg_rev_paise > 0 && today_rev_paise * 100 < avg_rev_paise * 70 {
    alerts.push(BusinessAlert {
        ...
        value: today_rev_paise as f64,
        threshold: (avg_rev_paise * 70 / 100) as f64,
        ...
    });
}
```

Better: also change `BusinessAlert.value/threshold` to integer paise for money alerts.

---

## 18) P2 HIGH — `BusinessAlert` mixes monetary and percentage thresholds in `f64 value/threshold`
**Location:** `BusinessAlert`

### Problem
A single generic `f64 value` / `threshold` field is used for:
- revenue paise
- maintenance cost paise
- occupancy percentages

This is type-unsafe and encourages money-as-float.

### Impact
- serialization ambiguity
- consumer bugs
- accidental unit confusion

### Concrete fix code
Use tagged payload:
```rust
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "unit", content = "data")]
pub enum AlertValue {
    Paise { value_paise: i64, threshold_paise: i64 },
    Percentage { value_pct: f32, threshold_pct: f32 },
}

#[derive(Debug, Clone, Serialize)]
pub struct BusinessAlert {
    pub alert_type: String,
    pub severity: String,
    pub message: String,
    pub channel: AlertChannel,
    pub timestamp: String,
    pub value: AlertValue,
}
```

---

## 19) P2 HIGH — `spawn_alert_checker` ignores WhatsApp send failures
**Location:** `spawn_alert_checker`

### Problem
Background task rule says:
- **Background tasks: must have error handling, not just unwrap**

This task does not unwrap, but it also **does not check the result** of:
```rust
crate::whatsapp_alerter::send_whatsapp(&config, &msg).await;
```
If that function returns `Result`, failure is silently discarded.

### Impact
- missed escalations
- operators assume alert sent
- no retry/logging

### Concrete fix code
```rust
if matches!(alert.channel, AlertChannel::WhatsApp | AlertChannel::Both) {
    let msg = format!("[{}] {}: {}", alert.severity, alert.alert_type, alert.message);
    if let Err(e) = crate::whatsapp_alerter::send_whatsapp(&config, &msg).await {
        tracing::error!(target: LOG_TARGET, error = %e, "Failed to send WhatsApp alert");
    }
}
```

Also add top-level loop error isolation:
```rust
loop {
    interval.tick().await;
    match check_business_alerts(&pool).await {
        alerts => { ... }
    }
}
```

---

## 20) P2 HIGH — `spawn_anomaly_scanner_with_healing` background task has no panic containment / task lifecycle handle
**Location:** `spawn_anomaly_scanner_with_healing`

### Problem
Task is fire-and-forget:
```rust
tokio::spawn(async move { ... loop { ... } });
```

No `JoinHandle`, no shutdown, no restart policy, no panic monitoring.

### Impact
- scanner may die permanently on panic
- no visibility in supervisor
- venue automation quietly stops

### Concrete fix code
Return `JoinHandle<()>` or wrap body with error/panic logging.

```rust
pub fn spawn_anomaly_scanner_with_healing(
    pool: SqlitePool,
    availability_map: Option<crate::self_healing::PodAvailabilityMap>,
) -> (Arc<RwLock<EngineState>>, tokio::task::JoinHandle<()>) {
    let state = Arc::new(RwLock::new(EngineState::new()));
    let state_clone = Arc::clone(&state);
    let rules = default_rules();

    let handle = tokio::spawn(async move {
        tracing::info!(...);
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        interval.tick().await;

        loop {
            interval.tick().await;
            let alerts = run_anomaly_scan(&pool, &state_clone, &rules).await;
            ...
        }
    });

    (state, handle)
}
```

---

## 21) P2 HIGH — `check_patterns` can fire duplicate alerts every run with no cooldown/state
**Location:** `check_patterns`

### Problem
Unlike anomaly scan, pattern alerts have no cooldown or deduplication state. Every invocation can emit the same pattern repeatedly.

### Impact
- alert storms
- staff fatigue
- duplicate maintenance events if integrated later

### Concrete fix code
Add state similar to anomaly engine:
```rust
pub struct PatternState {
    last_alert: HashMap<(String, String), DateTime<Utc>>,
}
```

Then enforce cooldown:
```rust
if let Some(last) = state.last_alert.get(&(row.pod_id.clone(), pattern.name.clone())) {
    if (now - *last).num_minutes() < 30 {
        continue;
    }
}
state.last_alert.insert((row.pod_id.clone(), pattern.name.clone()), now);
```

---

## 22) P2 HIGH — `calculate_rul` pod ID parsing is inconsistent and can silently produce pod 0
**Location:** `calculate_rul`

### Problem
```rust
let pod_num: u8 = pod_id
    .trim_start_matches("pod")
    .trim_start_matches("pod-")
    .parse()
    .unwrap_or(0);
```

This parse order is wrong for values like `pod-3`:
- trim `"pod"` => `"-3"`
- trim `"pod-"` no longer matches
- parse fails => `0`

Also silently maps invalid input to pod 0.

### Impact
- wrong/no tasking for real pod
- corrupted RUL output
- availability map may ignore pod

### Concrete fix code
```rust
fn parse_pod_label(pod_id: &str) -> anyhow::Result<u8> {
    let trimmed = pod_id
        .strip_prefix("pod_")
        .or_else(|| pod_id.strip_prefix("pod-"))
        .or_else(|| pod_id.strip_prefix("pod"))
        .unwrap_or(pod_id);

    let pod = trimmed.parse::<u8>()
        .map_err(|e| anyhow::anyhow!("invalid pod_id '{}': {}", pod_id, e))?;

    if !(1..=8).contains(&pod) {
        anyhow::bail!("pod_id out of range '{}'", pod_id);
    }
    Ok(pod)
}
```

---

## 23) P2 HIGH — `spawn_anomaly_scanner_with_healing` also silently maps invalid pod IDs to 0
**Location:** background healing loop

### Problem
```rust
let pod_num: u8 = alert.pod_id
    .trim_start_matches("pod_")
    .trim_start_matches("pod")
    .parse()
    .unwrap_or(0);
if pod_num > 0 { ... }
```

Bad labels are silently skipped.

### Impact
- anomalies detected but healing not applied
- silent mismatch between telemetry pod naming and control pod naming

### Concrete fix code
Use the shared strict parser above and log failures:
```rust
match parse_pod_label(&alert.pod_id) {
    Ok(pod_num) => {
        let action = crate::self_healing::recommend_action(&alert.rule_name, &alert.severity, pod_num);
        crate::self_healing::apply_action(avail_map, &action).await;
    }
    Err(e) => {
        tracing::error!("invalid anomaly pod_id '{}': {}", alert.pod_id, e);
    }
}
```

---

## 24) P2 HIGH — Payroll arithmetic can overflow without checked math
**Location:** `calculate_monthly_payroll`

### Problem
```rust
let emp_total = worked_minutes.max(0) * row.hourly_rate_paise / 60;
total_paise += emp_total;
```

Even with realistic values this is usually fine, but for corrupted DB values it can overflow `i64`.

### Impact
- panic in debug / wrap in release depending on settings
- corrupted payroll totals

### Concrete fix code
```rust
let minutes = worked_minutes.max(0);
let emp_total = minutes
    .checked_mul(row.hourly_rate_paise)
    .ok_or_else(|| anyhow::anyhow!("overflow computing payroll for {}", row.employee_id))?
    / 60;

total_paise = total_paise
    .checked_add(emp_total)
    .ok_or_else(|| anyhow::anyhow!("overflow accumulating payroll"))?;
```

---

## 25) P2 HIGH — `query_events` and `query_tasks` can return fewer than requested after Rust-side filtering
**Location:** `query_events`, `query_tasks`

### Problem
Functions fetch `LIMIT N` rows first, then filter in Rust. If filters exclude many rows, caller gets fewer than requested even though more matching rows exist in DB.

### Impact
- incomplete dashboards
- paging bugs
- operational blind spots

### Concrete fix
Move filters into SQL with `QueryBuilder`.

### Concrete fix code
```rust
use sqlx::QueryBuilder;

pub async fn query_events(
    pool: &SqlitePool,
    pod_id: Option<u8>,
    since: Option<DateTime<Utc>>,
    limit: u32,
) -> anyhow::Result<Vec<MaintenanceEvent>> {
    let mut qb = QueryBuilder::new(
        "SELECT id, pod_id, event_type, severity, component, description,
                detected_at, resolved_at, resolution_method, source,
                correlation_id, revenue_impact_paise, customers_affected,
                downtime_minutes, cost_estimate_paise, assigned_staff_id, metadata
         FROM maintenance_events WHERE 1=1"
    );

    if let Some(pid) = pod_id {
        qb.push(" AND pod_id = ");
        qb.push_bind(i64::from(pid));
    }
    if let Some(s) = since {
        qb.push(" AND detected_at >= ");
        qb.push_bind(s.to_rfc3339());
    }

    qb.push(" ORDER BY detected_at DESC LIMIT ");
    qb.push_bind(i64::from(limit));

    let rows: Vec<EventRow> = qb.build_query_as().fetch_all(pool).await?;
    rows.into_iter().map(row_to_event).collect()
}
```

---

## 26) P2 HIGH — `record_attendance` overnight logic can turn malformed inputs into valid long shifts
**Location:** `record_attendance`

### Problem
If `clock_out < clock_in`, code assumes overnight and adds 24h. This means accidental swapped inputs like `17:00` / `09:00` become a 16-hour shift.

### Impact
- payroll inflation
- unnoticed bad time entry
- abuse vector

### Concrete fix code
Require explicit overnight flag or max duration validation:
```rust
let mut mins = (o - i).num_minutes();
if mins < 0 {
    mins += 24 * 60;
}

if mins > 12 * 60 {
    anyhow::bail!(
        "implausible shift duration {} minutes for employee {} on {}",
        mins, employee_id, date
    );
}
```

---

## 27) P2 HIGH — No DB-level CHECK constraints for critical business invariants
**Location:** all schema init functions

### Problem
Application code tries to validate, but schema allows:
- negative paise values where nonsensical
- pod_id outside 1..8
- occupancy > 100 or negative
- priority > 100
- negative downtime/customers
- invalid status strings

### Impact
- any alternate writer / migration / manual edit can poison system
- app-level assumptions break

### Concrete fix code
Example:
```rust
sqlx::query(
    "CREATE TABLE IF NOT EXISTS maintenance_tasks (
        id TEXT PRIMARY KEY,
        title TEXT NOT NULL,
        description TEXT NOT NULL,
        pod_id INTEGER CHECK (pod_id IS NULL OR (pod_id BETWEEN 1 AND 8)),
        component TEXT NOT NULL,
        priority INTEGER NOT NULL DEFAULT 3 CHECK (priority BETWEEN 0 AND 100),
        status TEXT NOT NULL DEFAULT 'Open'
            CHECK (status IN ('Open','Assigned','InProgress','PendingValidation','Completed','Failed','Cancelled')),
        created_at TEXT NOT NULL,
        due_by TEXT,
        assigned_to TEXT,
        source_event_id TEXT,
        before_metrics TEXT,
        after_metrics TEXT,
        cost_estimate_paise INTEGER CHECK (cost_estimate_paise IS NULL OR cost_estimate_paise >= 0),
        actual_cost_paise INTEGER CHECK (actual_cost_paise IS NULL OR actual_cost_paise >= 0)
    )"
)
.execute(pool)
.await?;
```

Likewise for business metrics:
```sql
CHECK (occupancy_rate_pct BETWEEN 0 AND 100),
CHECK (peak_occupancy_pct BETWEEN 0 AND 100),
CHECK (sessions_count >= 0)
```

---

## 28) P2 HIGH — `row_to_employee` silently defaults malformed skills/date
**Location:** `row_to_employee`

### Problem
```rust
let skills: Vec<String> = serde_json::from_str(&row.skills).unwrap_or_default();
let hired_at = ...unwrap_or_else(|_| 2000-01-01)
```

Again hides DB corruption.

### Impact
- skill-based assignment stops working
- fake hire dates pollute HR reporting

### Concrete fix code
```rust
let skills: Vec<String> = serde_json::from_str(&row.skills)
    .map_err(|e| anyhow::anyhow!("invalid skills JSON for employee {}: {}", row.id, e))?;

let hired_at = chrono::NaiveDate::parse_from_str(&row.hired_at, "%Y-%m-%d")
    .map_err(|e| anyhow::anyhow!("invalid hired_at '{}' for employee {}: {}", row.hired_at, row.id, e))?;
```

---

## 29) P2 HIGH — `query_attendance` row conversion silently normalizes negative hours to zero
**Location:** `row_to_attendance`

### Problem
```rust
hours_worked: row.hours_worked.max(0.0),
```
Another case of hiding corrupt DB values instead of surfacing them.

### Impact
- payroll discrepancies become invisible
- impossible forensic reconstruction

### Concrete fix code
```rust
if row.hours_worked < 0.0 || !row.hours_worked.is_finite() {
    anyhow::bail!("invalid hours_worked {} for attendance row {}", row.hours_worked, row.id);
}
```

Prefer integer minutes schema as above.

---

## 30) P2 HIGH — `calculate_kpis` counts `Verified` status that does not exist in enum
**Location:** `calculate_kpis`

### Problem
```sql
status IN ('Completed', 'Verified')
```
But `TaskStatus` enum has no `Verified`; it has `PendingValidation`, `Completed`, etc.

### Impact
- misleading KPI logic
- schema/model drift
- dead query branch indicates operational confusion

### Concrete fix code
```rust
let (tasks_completed,): (i64,) = sqlx::query_as(
    "SELECT COUNT(*) FROM maintenance_tasks
     WHERE created_at >= ?1 AND status = 'Completed'"
)
.bind(&since_str)
.fetch_one(pool)
.await?;
```

If validation-complete is separate, add enum variant explicitly and use it consistently.

---

## 31) P3 MEDIUM — `MaintenanceSummary`, `AttendanceRecord`, `PayrollSummary` expose float business values likely to be reused incorrectly
**Location:** models

### Problem
- `mttr_minutes: f64`
- `self_heal_rate: f64`
- `hours_worked: f64`
- `total_hours: f64`
- `occupancy_rate_pct: f32`
- `rul_hours: f32`

Not all floats are wrong, but some are dangerous. Time worked is the biggest issue. Occupancy/RUL percentages may be acceptable, but ensure not used for billing/payroll.

### Recommendation
- Keep analytical ratios as float if necessary
- Convert worked time to integer minutes
- Consider basis points for occupancy if you want deterministic reports

---

## 32) P3 MEDIUM — `run_anomaly_scan` / `check_patterns` select latest per pod using tuple `IN`, may still be ambiguous with equal timestamps
**Location:** anomaly + pattern queries

### Problem
If multiple telemetry rows share the same `pod_id,collected_at`, multiple rows can be returned.

### Impact
- duplicate alerts in edge case
- non-deterministic row selection

### Concrete fix code
Prefer `rowid` or primary key:
```sql
SELECT h.*
FROM hardware_telemetry h
JOIN (
    SELECT pod_id, MAX(collected_at) AS max_collected_at
    FROM hardware_telemetry
    WHERE collected_at > ?1
    GROUP BY pod_id
) latest
ON h.pod_id = latest.pod_id
AND h.collected_at = latest.max_collected_at
```

Best: join on unique telemetry ID with window function if available.

---

## 33) P3 MEDIUM — `calculate_priority` truncates float to `u8`
**Location:** `calculate_priority`

### Problem
```rust
score as u8
```
Not DB-sourced, but integer cast should still be explicit for correctness. Fractional score truncates, not rounds.

### Fix
```rust
score.round().clamp(0.0, 100.0) as u8
```
or integer math.

---

## 34) P3 MEDIUM — `insert_employee` "invalid UUID-format string" comment is misleading
**Location:** `insert_employee`

### Problem
Comment says validation for UUID string, but `employee.id` is already `Uuid`. Actual check only rejects nil UUID and impossible empty string from `to_string()`.

### Impact
Confusing audit/documentation trail.

### Fix
Simplify:
```rust
if employee.id.is_nil() {
    anyhow::bail!("insert_employee: nil employee id not allowed");
}
```

---

## 35) P3 MEDIUM — `check_rul_thresholds` uses `LIKE` against component because storage format is broken
**Location:** `check_rul_thresholds`

### Problem
```sql
component LIKE ?2
```
This is a symptom of enum format inconsistency and prevents index-friendly exact matching.

### Fix
After normalizing component storage to plain text:
```sql
WHERE pod_id = ?1
  AND component = ?2
  AND status NOT IN ('Completed', 'Failed', 'Cancelled')
```

---

## 36) P3 MEDIUM — Hardcoded Ollama URL and model names are operationally brittle
**Location:** `ollama_client`

### Problem
Hardcoded internal IP:
```rust
const OLLAMA_URL: &str = "http://192.168.31.27:11434/api/generate";
```

### Impact
- deploy fragility
- no environment separation
- difficult failover

### Concrete fix code
```rust
pub struct OllamaConfig {
    pub base_url: String,
    pub primary_model: String,
    pub fallback_model: String,
    pub timeout_secs: u64,
}
```

---

## 37) P3 MEDIUM — `call_ollama` creates body from unbounded prompt string without length guard
**Location:** `call_ollama`

### Problem
Large prompt could create excessive memory/latency.

### Fix
```rust
const MAX_PROMPT_BYTES: usize = 32 * 1024;
if prompt.len() > MAX_PROMPT_BYTES {
    anyhow::bail!("prompt too large: {} bytes", prompt.len());
}
```

---

# Most urgent remediation order

## Immediate block-release fixes
1. **Remove all silent fallback decoding** (`Utc::now()`, `2000-01-01`, default skills, clamp-to-valid)
2. **Fix all DB integer casts** to `try_from` and fail on invalid values
3. **Migrate attendance/payroll from float hours to integer worked minutes**
4. **Normalize enum storage format** to plain strings consistently
5. **Fix auto-assign and RUL task creation races** with transactions/conditional updates
6. **Remove silent `unwrap_or` DB failure masking** in KPI/alert/snapshot paths

## Next wave
7. Add DB `CHECK` constraints and unique indexes
8. Fix timestamp format consistency (`RFC3339` vs SQLite datetime text)
9. Add background task handles and failure visibility
10. Validate IDs/dates/times strictly at persistence boundary

---

# Quick compliance score against your rules

## Rules audit
- **All money MUST be integer paise (i64), NEVER f64**  
  **FAIL** — business alerts use `f64`; attendance/payroll pipeline uses float-derived money logic

- **All integer casts from DB MUST use try_from, not `as`**  
  **FAIL** — multiple violations in KPI/snapshot/business metrics

- **SQL queries MUST use parameterized binds, never string formatting**  
  **PASS** in shown bundle

- **Async code: never hold locks across .await**  
  **PASS** in shown bundle

- **Background tasks: must have error handling, not just unwrap**  
  **PARTIAL FAIL** — no unwraps in task loops, but failures are often silently ignored / no task supervision

---

# Final verdict

**Bundle A fails audit for production.**

Biggest blockers:
- **P1:** silent data rewriting on decode
- **P1:** float-based attendance/payroll architecture
- **P1:** non-atomic auto-assignment / duplicate task creation race
- **P1/P2:** enum/timestamp storage/query inconsistency causing wrong counts and broken logic
- **P2:** widespread silent failure masking

If you want, I can do the next step as a **patch-oriented remediation pass**, rewriting the worst functions (`row_to_event`, `row_to_task`, `record_attendance`, `calculate_monthly_payroll`, `auto_assign_task`, `check_rul_thresholds`, `collect_venue_snapshot`) into audit-clean Rust/SQL.