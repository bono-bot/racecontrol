## AUDIT REPORT: Racing Point eSports v29.0 Meshed Intelligence

**Reviewer:** Senior Rust/Security Auditor  
**Date:** Current  
**Total Findings:** 15 (P1: 3, P2: 8, P3: 3, P4: 1)

---

## P1 CRITICAL FINDINGS

### 1. P1 CRITICAL — SQL Injection in Dynamic Query Construction
**File:** `maintenance_persistence.rs:282-309` (update_employee function)  
**Description:** The `update_employee` function dynamically builds SQL queries by string concatenation without proper parameterization, creating SQL injection vulnerabilities. The function builds numbered placeholders incorrectly and then binds values as strings, bypassing SQLite's type-safe parameter binding.  
**Risk:** Attacker-controlled `face_enrollment_id`, `phone`, or other fields could inject arbitrary SQL commands.  
**Fix:**
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
    // Build SET clauses with proper parameterized queries
    let mut query_str = "UPDATE employees SET ".to_string();
    let mut bind_values: Vec<String> = Vec::new();
    let mut first = true;
    
    if let Some(n) = name {
        if !first { query_str.push_str(", "); }
        query_str.push_str("name = ?");
        bind_values.push(n.to_owned());
        first = false;
    }
    if let Some(r) = role {
        if !first { query_str.push_str(", "); }
        query_str.push_str("role = ?");
        bind_values.push(serde_json::to_string(r)?.replace('"', ""));
        first = false;
    }
    // ... similar for other fields
    
    if first {
        return Ok(false); // No fields to update
    }
    
    query_str.push_str(" WHERE id = ?");
    bind_values.push(id.to_owned());
    
    // Use sqlx::query with proper parameter binding
    let mut query = sqlx::query(&query_str);
    for value in &bind_values {
        query = query.bind(value);
    }
    
    let result = query.execute(pool).await?;
    Ok(result.rows_affected() > 0)
}
```

### 2. P1 CRITICAL — Potential Overflow in Integer Casting
**File:** `maintenance_persistence.rs:101, 115, 130, 173`  
**Description:** Multiple places cast `i64` database values directly to `u8` (pod_id) and `u32` (counters) without bounds checking. SQLite `INTEGER` can store up to 8 bytes, but `u8::MAX` is 255.  
**Risk:** Database corruption or malicious inserts could cause panics via integer overflow.  
**Fix:**
```rust
// In row_to_event function (line ~410):
pod_id: row.pod_id.and_then(|p| {
    if p >= 0 && p <= u8::MAX as i64 {
        Some(p as u8)
    } else {
        tracing::error!("Invalid pod_id value in DB: {}", p);
        None
    }
}),

// In row_to_task function (line ~447):
priority: if row.priority >= 0 && row.priority <= u8::MAX as i64 {
    row.priority as u8
} else {
    tracing::error!("Invalid priority value: {}", row.priority);
    3 // Default medium priority
},
```

### 3. P1 CRITICAL — Unsafe DateTime Parsing with Unwrap
**File:** `maintenance_persistence.rs:258, 271, 400, 430`  
**Description:** Multiple `unwrap_or_else` calls on date parsing failures use hardcoded dates (2000-01-01), corrupting business data silently.  
**Risk:** Invalid date strings in database cause incorrect business calculations without logging.  
**Fix:**
```rust
// Replace unwrap_or_else with proper error handling:
let date = chrono::NaiveDate::parse_from_str(&row.date, "%Y-%m-%d")
    .map_err(|e| {
        tracing::error!("Invalid date in DB: {} - {}", row.date, e);
        e
    })?; // Propagate error instead of silent corruption

// Or for query functions, return empty results:
match chrono::NaiveDate::parse_from_str(&row.date, "%Y-%m-%d") {
    Ok(date) => date,
    Err(e) => {
        tracing::warn!("Skipping invalid date record {}: {}", row.id, e);
        continue; // Skip this record
    }
}
```

---

## P2 HIGH FINDINGS

### 4. P2 HIGH — Race Condition in Anomaly Alert Cooldown Tracking
**File:** `anomaly_detection.rs:191-200` (run_anomaly_scan function)  
**Description:** The function removes `first_violation` entry after firing alert but while holding write lock. If another thread reads state between removal and next scan, sustained violation tracking resets incorrectly.  
**Risk:** Alerts may fire more frequently than cooldown period allows.  
**Fix:**
```rust
// Instead of removing first_violation, update it to now + cooldown
let cooldown_end = now + chrono::Duration::minutes(rule.cooldown_minutes as i64);
guard.first_violation.insert(key.clone(), cooldown_end);
guard.last_alert.insert(key.clone(), now);
```

### 5. P2 HIGH — Missing Foreign Key Constraints
**File:** `maintenance_persistence.rs:31-62` (table creation)  
**Description:** `maintenance_tasks.source_event_id` references `maintenance_events.id` but no `FOREIGN KEY` constraint defined. Similar for `attendance_records.employee_id`.  
**Risk:** Orphaned records causing inconsistent state.  
**Fix:**
```rust
// In init_maintenance_tables:
sqlx::query(
    "CREATE TABLE IF NOT EXISTS maintenance_tasks (
        ...
        source_event_id TEXT REFERENCES maintenance_events(id) ON DELETE SET NULL,
        ...
    )",
)
.execute(pool)
.await?;

// In init_hr_tables:
sqlx::query(
    "CREATE TABLE IF NOT EXISTS attendance_records (
        ...
        employee_id TEXT NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
        ...
    )",
)
.execute(pool)
.await?;
```

### 6. P2 HIGH — Incorrect MTTR Calculation in get_summary
**File:** `maintenance_persistence.rs:168-179`  
**Description:** MTTR calculation uses `DateTime::parse_from_rfc3339` without timezone conversion, then compares UTC timestamps with potentially local parsed times.  
**Risk:** Incorrect MTTR values, especially across timezone changes.  
**Fix:**
```rust
// Parse as UTC directly:
if let (Some(det), Some(res)) = (&row.detected_at_str, &row.resolved_at_str) {
    if let (Ok(d), Ok(r)) = (
        DateTime::<Utc>::from_rfc3339(det),
        DateTime::<Utc>::from_rfc3339(res),
    ) {
        let mins = (r - d).num_minutes() as f64;
        if mins >= 0.0 {
            total_ttrs += mins;
            resolved_count += 1;
        }
    }
}
```

### 7. P2 HIGH — Incorrect JSON Deserialization of TaskStatus
**File:** `maintenance_persistence.rs:456`  
**Description:** Task status string is wrapped in quotes for JSON deserialization, but this fails if the string contains quotes or special characters.  
**Risk:** Status deserialization failures causing task processing errors.  
**Fix:**
```rust
// Use proper JSON serialization/deserialization:
let status: TaskStatus = match row.status.as_str() {
    "Open" => TaskStatus::Open,
    "Assigned" => TaskStatus::Assigned,
    "InProgress" => TaskStatus::InProgress,
    "PendingValidation" => TaskStatus::PendingValidation,
    "Completed" => TaskStatus::Completed,
    "Failed" => TaskStatus::Failed,
    "Cancelled" => TaskStatus::Cancelled,
    _ => {
        tracing::warn!("Unknown task status: {}, defaulting to Open", row.status);
        TaskStatus::Open
    }
};
```

### 8. P2 HIGH — Unbounded Memory Growth in Recent Alerts
**File:** `anomaly_detection.rs:231-235`  
**Description:** Recent alerts capped at 200 but never cleaned by age, only by count. In low-alert scenarios, old alerts stay indefinitely.  
**Risk:** Memory leak over long-running system.  
**Fix:**
```rust
// In run_anomaly_scan after adding new alerts:
let cutoff = now - chrono::Duration::hours(24);
guard.recent_alerts.retain(|alert| alert.detected_at >= cutoff);
if guard.recent_alerts.len() > 200 {
    guard.recent_alerts.drain(..guard.recent_alerts.len() - 200);
}
```

### 9. P2 HIGH — Missing Input Validation in record_attendance
**File:** `maintenance_persistence.rs:612-635`  
**Description:** No validation that `employee_id` exists in employees table or that date is valid. Clock_in/clock_out parsing assumes 24-hour format without validation.  
**Risk:** Invalid attendance records corrupting payroll calculations.  
**Fix:**
```rust
pub async fn record_attendance(
    pool: &SqlitePool,
    employee_id: &str,
    date: &str,
    clock_in: Option<&str>,
    clock_out: Option<&str>,
    source: &str,
) -> anyhow::Result<()> {
    // Validate employee exists
    let emp_exists: Option<(i64,)> = sqlx::query_as(
        "SELECT 1 FROM employees WHERE id = ? AND is_active = 1"
    )
    .bind(employee_id)
    .fetch_optional(pool)
    .await?;
    
    if emp_exists.is_none() {
        return Err(anyhow::anyhow!("Employee {} not found or inactive", employee_id));
    }
    
    // Validate date format
    let _ = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|e| anyhow::anyhow!("Invalid date format: {}", e))?;
    
    // ... rest of function
}
```

### 10. P2 HIGH — Division by Zero in calculate_kpis
**File:** `maintenance_persistence.rs:783`  
**Description:** `total_events` used as divisor without zero check in self_heal_rate calculation.  
**Risk:** Potential panic if no events in period.  
**Fix:**
```rust
let self_heal_rate = if total_events > 0 {
    self_heal_count as f64 / total_events as f64
} else {
    0.0
};
```

### 11. P2 HIGH — Incorrect Payroll Calculation Rounding
**File:** `maintenance_persistence.rs:696`  
**Description:** Payroll uses `round()` which may round differently than business rules require (e.g., should round to nearest paise).  
**Risk:** Incorrect payroll amounts over many employees.  
**Fix:**
```rust
// Use integer arithmetic to avoid floating point issues:
let emp_total = (row.total_hours * row.hourly_rate_paise as f64) as i64;
// Or better, store hours in paise-minutes:
let minutes_worked = (row.total_hours * 60.0).round() as i64;
let emp_total = (minutes_worked * row.hourly_rate_paise) / 60;
```

---

## P3 MEDIUM FINDINGS

### 12. P3 MEDIUM — Inefficient Query in query_events
**File:** `maintenance_persistence.rs:95-120`  
**Description:** Fetches all events then filters in Rust instead of using SQL WHERE clause.  
**Performance Impact:** O(n) memory usage instead of O(limit).  
**Fix:**
```rust
pub async fn query_events(
    pool: &SqlitePool,
    pod_id: Option<u8>,
    since: Option<DateTime<Utc>>,
    limit: u32,
) -> anyhow::Result<Vec<MaintenanceEvent>> {
    let mut query = "SELECT ... FROM maintenance_events WHERE 1=1".to_string();
    let mut binds = Vec::new();
    
    if let Some(pid) = pod_id {
        query.push_str(" AND pod_id = ?");
        binds.push(pid as i64);
    }
    if let Some(s) = since {
        query.push_str(" AND detected_at >= ?");
        binds.push(s.to_rfc3339());
    }
    
    query.push_str(" ORDER BY detected_at DESC LIMIT ?");
    binds.push(limit as i64);
    
    let mut sqlx_query = sqlx::query_as::<_, EventRow>(&query);
    for bind in binds {
        sqlx_query = sqlx_query.bind(bind);
    }
    
    let rows = sqlx_query.fetch_all(pool).await?;
    rows.into_iter().map(row_to_event).collect()
}
```

### 13. P3 MEDIUM — Hardcoded Peak Hours Logic
**File:** `anomaly_detection.rs:391-402` (is_peak_hours)  
**Description:** Peak hours hardcoded without configuration or timezone consideration for daylight saving.  
**Impact:** Incorrect priority scoring in different timezones.  
**Fix:**
```rust
pub fn is_peak_hours(config: &AppConfig) -> bool {
    let now = Utc::now() + chrono::Duration::hours(config.timezone_offset_hours);
    let hour = now.hour();
    let weekday = now.weekday();
    
    match weekday {
        chrono::Weekday::Sat | chrono::Weekday::Sun => 
            hour >= config.weekend_peak_start && hour < config.weekend_peak_end,
        _ => 
            hour >= config.weekday_peak_start && hour < config.weekday_peak_end,
    }
}
```

### 14. P3 MEDIUM — Missing Index on maintenance_events.resolved_at
**File:** `maintenance_persistence.rs:31-62`  
**Description:** No index on `resolved_at` column used in MTTR calculations.  
**Performance Impact:** Slow summary queries as dataset grows.  
**Fix:**
```rust
sqlx::query(
    "CREATE INDEX IF NOT EXISTS idx_maint_events_resolved ON maintenance_events(resolved_at)",
)
.execute(pool)
.await?;
```

---

## P4 LOW FINDINGS

### 15. P4 LOW — Inconsistent Serde Naming
**File:** `maintenance_models.rs:17, 25, 33, 41, 49`  
**Description:** Enums use `PascalCase` serialization but some variant names don't follow PascalCase convention (e.g., "AIDiagnosisCompleted").  
**Impact:** Inconsistent API responses.  
**Fix:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")] // Or keep PascalCase but fix variants
pub enum MaintenanceEventType {
    SelfHealAttempted,
    Tier1FixApplied,
    PodHealerIntervention,
    AiDiagnosisCompleted, // lowercase 'i'
    PredictiveAlert,
    // ...
}
```

---

## SUMMARY RECOMMENDATIONS

1. **Immediate Action Required (P1):**
   - Fix SQL injection in `update_employee`
   - Add bounds checking for integer casts
   - Fix date parsing to propagate errors

2. **High Priority (P2):**
   - Add foreign key constraints
   - Fix race conditions in anomaly detection
   - Add input validation for attendance records

3. **Technical Debt (P3/P4):**
   - Add database indexes for performance
   - Extract configuration for business rules
   - Standardize serialization formats

**Overall Assessment:** The codebase demonstrates good architectural separation but lacks defensive programming practices critical for production systems. SQL injection vulnerability (P1#1) is particularly severe given the HR data exposure risk. The Windows 11 deployment context amplifies risks of privilege escalation if database is compromised.