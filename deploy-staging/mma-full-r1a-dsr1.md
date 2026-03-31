# V29.0 MESHED INTELLIGENCE - SECURITY AUDIT REPORT

## EXECUTIVE SUMMARY
**CRITICAL ISSUES FOUND: 8** | **HIGH SEVERITY: 12** | **MEDIUM: 7**

The codebase shows good architectural intent but contains multiple P1/P2 security vulnerabilities including SQL injection vectors, data corruption risks, silent error handling, and business logic flaws. Immediate remediation required.

---

## P1 CRITICAL FINDINGS

### 1. SQL Injection Vulnerability - Dynamic Query Building
**Severity**: P1 CRITICAL  
**Location**: `maintenance_store.rs:update_employee` (lines 445-494)  
**Description**: Multiple individual UPDATE statements create race conditions and allow SQL injection through string concatenation in skill field serialization. The `skills` parameter accepts arbitrary JSON strings without validation.  
**Fix**:
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
    // Validate employee exists
    let exists: bool = sqlx::query_scalar("SELECT 1 FROM employees WHERE id = ?1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .is_some();
    
    if !exists {
        return Ok(false);
    }
    
    // Validate skills JSON to prevent injection
    if let Some(skills) = skills {
        for skill in skills {
            if skill.contains('"') || skill.contains('\\') {
                anyhow::bail!("Invalid skill format: {}", skill);
            }
        }
    }
    
    // Single parameterized update with COALESCE
    let mut updates = vec![];
    let mut params: Vec<String> = vec![];
    
    if let Some(n) = name {
        updates.push("name = ?");
        params.push(n.to_string());
    }
    if let Some(r) = role {
        updates.push("role = ?");
        params.push(serde_json::to_string(r)?.replace('"', ""));
    }
    if let Some(s) = skills {
        updates.push("skills = ?");
        params.push(serde_json::to_string(s)?);
    }
    if let Some(rate) = hourly_rate_paise {
        updates.push("hourly_rate_paise = ?");
        params.push(rate.to_string());
    }
    if let Some(p) = phone {
        updates.push("phone = ?");
        params.push(p.to_string());
    }
    if let Some(a) = is_active {
        updates.push("is_active = ?");
        params.push(if a { "1" } else { "0" }.to_string());
    }
    if let Some(f) = face_enrollment_id {
        updates.push("face_enrollment_id = ?");
        params.push(f.to_string());
    }
    
    if updates.is_empty() {
        return Ok(false);
    }
    
    let query = format!(
        "UPDATE employees SET {} WHERE id = ?",
        updates.join(", ")
    );
    
    let mut q = sqlx::query(&query);
    for param in &params {
        q = q.bind(param);
    }
    q = q.bind(id);
    
    let result = q.execute(pool).await?;
    Ok(result.rows_affected() > 0)
}
```

### 2. Integer Overflow in Payroll Calculation
**Severity**: P1 CRITICAL  
**Location**: `maintenance_store.rs:calculate_monthly_payroll` (lines 628-660)  
**Description**: `worked_minutes * rate_paise / 60` can overflow i64 with high hours/rates. Example: 1000 hours at ₹200/hr = 1000×60×200×100 = 1.2B paise potential overflow.  
**Fix**:
```rust
let worked_minutes = (row.total_hours * 60.0).round() as i64;
// Use checked arithmetic
let emp_total = worked_minutes.max(0)
    .checked_mul(row.hourly_rate_paise)
    .and_then(|v| v.checked_div(60))
    .unwrap_or(i64::MAX);

total_hours += row.total_hours;
total_paise = total_paise
    .checked_add(emp_total)
    .unwrap_or(i64::MAX);
```

### 3. Unbounded Result Sets in Public Queries
**Severity**: P1 CRITICAL  
**Location**: `maintenance_store.rs:query_attendance` (lines 538-591)  
**Description**: All query branches have LIMIT but `query_events` (line 139) and `query_tasks` (line 327) accept caller-controlled `limit: u32` without validation. Attacker can pass `u32::MAX` causing OOM.  
**Fix**:
```rust
pub async fn query_events(
    pool: &SqlitePool,
    pod_id: Option<u8>,
    since: Option<DateTime<Utc>>,
    limit: u32,
) -> anyhow::Result<Vec<MaintenanceEvent>> {
    // Enforce maximum limit
    let limit = limit.min(1000);
    
    let rows = sqlx::query_as::<_, EventRow>(
        "SELECT id, pod_id, event_type, severity, component, description,
                detected_at, resolved_at, resolution_method, source,
                correlation_id, revenue_impact_paise, customers_affected,
                downtime_minutes, cost_estimate_paise, assigned_staff_id, metadata
         FROM maintenance_events
         ORDER BY detected_at DESC
         LIMIT ?1",
    )
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;
    // ... rest unchanged
}
```

### 4. Silent Date Parsing Failures
**Severity**: P1 CRITICAL  
**Location**: `maintenance_store.rs:row_to_event` (lines 379-388) and `row_to_task` (lines 407-416)  
**Description**: Date parsing failures default to `Utc::now()` silently corrupting historical data. No error propagation.  
**Fix**:
```rust
fn row_to_event(row: EventRow) -> anyhow::Result<MaintenanceEvent> {
    let detected_at = match row.detected_at_str.as_deref() {
        Some(s) => DateTime::parse_from_rfc3339(s)
            .map_err(|e| anyhow::anyhow!("Invalid detected_at for event {}: {} - {}", row.id, s, e))?
            .with_timezone(&Utc),
        None => anyhow::bail!("detected_at is NULL for event {}", row.id),
    };
    // ... rest unchanged
}
```

### 5. Unsafe Enum Deserialization
**Severity**: P1 CRITICAL  
**Location**: `maintenance_store.rs:row_to_task` (line 424)  
**Description**: Status string wrapped in quotes for JSON deserialization (`format!("\"{}\"", row.status)`) is a SQL injection vector if `row.status` contains escaped quotes.  
**Fix**:
```rust
let status_json = match serde_json::to_string(&row.status) {
    Ok(s) => s,
    Err(_) => {
        // Fallback to safe default
        "\"Open\"".to_string()
    }
};
let status: TaskStatus = serde_json::from_str(&status_json)?;
```

### 6. Missing Input Validation - Employee Phone/Skills
**Severity**: P1 CRITICAL  
**Location**: `maintenance_store.rs:insert_employee` (lines 435-436)  
**Description**: Phone number and skills JSON accept arbitrary strings enabling XSS/injection in downstream systems.  
**Fix**:
```rust
/// Insert a new employee with validation
pub async fn insert_employee(pool: &SqlitePool, employee: &Employee) -> anyhow::Result<()> {
    // Validate phone format (Indian mobile)
    let phone_regex = regex::Regex::new(r"^[6-9]\d{9}$").unwrap();
    if !phone_regex.is_match(&employee.phone) {
        anyhow::bail!("Invalid Indian phone number: {}", employee.phone);
    }
    
    // Validate skills don't contain injection characters
    for skill in &employee.skills {
        if skill.contains(['\"', '\'', '\\', '<', '>', '&'].as_ref()) {
            anyhow::bail!("Invalid character in skill: {}", skill);
        }
    }
    // ... rest unchanged
}
```

### 7. Time-of-Check-Time-of-Use (TOCTOU) in Auto-Assignment
**Severity**: P1 CRITICAL  
**Location**: `maintenance_store.rs:auto_assign_task` (lines 744-783)  
**Description**: Race condition between checking employee load and updating task assignment. Simultaneous requests can over-assign technicians.  
**Fix**:
```rust
pub async fn auto_assign_task(
    pool: &SqlitePool,
    task_id: &str,
) -> anyhow::Result<Option<String>> {
    // Start transaction
    let mut tx = pool.begin().await?;
    
    // Use SELECT FOR UPDATE to lock the task
    let component: Option<String> = sqlx::query_scalar(
        "SELECT component FROM maintenance_tasks WHERE id = ?1"
    )
    .bind(task_id)
    .fetch_optional(&mut *tx)
    .await?;
    
    // ... find best employee logic
    
    if let Some(ref emp_id) = best_id {
        // Check load within transaction
        let load: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM maintenance_tasks \
             WHERE assigned_to = ?1 AND status NOT IN ('Completed', 'Failed', 'Cancelled') \
             FOR UPDATE"
        )
        .bind(emp_id)
        .fetch_one(&mut *tx)
        .await?;
        
        if load < 5 { // Max concurrent tasks per employee
            sqlx::query(
                "UPDATE maintenance_tasks SET assigned_to = ?1, status = 'Assigned' WHERE id = ?2"
            )
            .bind(emp_id)
            .bind(task_id)
            .execute(&mut *tx)
            .await?;
            
            tx.commit().await?;
            tracing::info!(target: "maint-store", task_id, employee_id = %emp_id, "Task auto-assigned");
            return Ok(Some(emp_id.clone()));
        }
    }
    
    // No assignment
    tx.rollback().await?;
    Ok(None)
}
```

### 8. Unauthenticated Ollama Endpoint Exposure
**Severity**: P1 CRITICAL  
**Location**: `ollama_client.rs:diagnose` (entire module)  
**Description**: Ollama endpoint at fixed IP `192.168.31.27:11434` exposes unrestricted AI model access. No authentication, rate limiting, or input sanitization.  
**Fix**:
```rust
const OLLAMA_URL: &str = "http://localhost:11434/api/generate"; // Bind to localhost only

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
    system: String, // Add system prompt for safety
}

// Add prompt validation
fn validate_prompt(prompt: &str) -> anyhow::Result<()> {
    if prompt.len() > 10000 {
        anyhow::bail!("Prompt too long");
    }
    if prompt.contains(['`', ';', '\\', '\"', '\''].as_ref()) {
        anyhow::bail!("Invalid characters in prompt");
    }
    Ok(())
}

pub async fn diagnose(prompt: &str) -> anyhow::Result<String> {
    validate_prompt(prompt)?;
    
    let req = OllamaRequest {
        model: DEFAULT_MODEL.to_string(),
        prompt: prompt.to_string(),
        stream: false,
        system: "You are a technical assistant for pod racing simulator maintenance. Only provide factual diagnostic information.".to_string(),
    };
    // ... rest unchanged
}
```

---

## P2 HIGH SEVERITY FINDINGS

### 1. Floating Point for Monetary Values
**Severity**: P2 HIGH  
**Location**: `maintenance_models.rs:PayrollSummary` (line 166 `total_hours: f64`) and `EmployeePayroll` (line 173 `hours_worked: f64`)  
**Description**: `total_hours` and `hours_worked` stored as f64 causing cumulative rounding errors in payroll. Violates "all money MUST be integer paise" rule.  
**Fix**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayrollSummary {
    pub year: i32,
    pub month: u32,
    pub total_minutes: i64, // Store as integer minutes
    pub total_paise: i64,
    pub by_employee: Vec<EmployeePayroll>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmployeePayroll {
    pub employee_id: String,
    pub name: String,
    pub minutes_worked: i64, // Integer minutes
    pub rate_paise: i64,
    pub total_paise: i64,
}
```

### 2. Missing Foreign Key Constraints
**Severity**: P2 HIGH  
**Location**: `maintenance_store.rs:init_maintenance_tables` (lines 46-83)  
**Description**: No FOREIGN KEY constraints between `maintenance_tasks.source_event_id` and `maintenance_events.id`, allowing orphaned tasks.  
**Fix**:
```rust
sqlx::query(
    "CREATE TABLE IF NOT EXISTS maintenance_tasks (
        id TEXT PRIMARY KEY,
        title TEXT NOT NULL,
        description TEXT NOT NULL,
        pod_id INTEGER,
        component TEXT NOT NULL,
        priority INTEGER NOT NULL DEFAULT 3,
        status TEXT NOT NULL DEFAULT 'Open',
        created_at TEXT NOT NULL,
        due_by TEXT,
        assigned_to TEXT,
        source_event_id TEXT,
        before_metrics TEXT,
        after_metrics TEXT,
        cost_estimate_paise INTEGER,
        actual_cost_paise INTEGER,
        FOREIGN KEY (source_event_id) REFERENCES maintenance_events(id) ON DELETE SET NULL
    )",
)
.execute(pool)
.await?;
```

### 3. Missing Index on Maintenance Event Resolution
**Severity**: P2 HIGH  
**Location**: `maintenance_store.rs:init_maintenance_tables` (missing index)  
**Description**: Frequent queries for unresolved events (`resolved_at IS NULL`) scan entire table.  
**Fix**:
```rust
sqlx::query(
    "CREATE INDEX IF NOT EXISTS idx_maint_events_resolved ON maintenance_events(resolved_at) WHERE resolved_at IS NULL",
)
.execute(pool)
.await?;
```

### 4. Unsafe Cast with `as` Operator
**Severity**: P2 HIGH  
**Location**: Multiple locations using `as` for type conversion instead of `try_from`  
**Example**: `maintenance_store.rs:insert_event` line 117: `.bind(event.pod_id.map(|p| p as i64))`  
**Fix**:
```rust
.bind(event.pod_id.map(|p| i64::try_from(p).unwrap_or(0)))
```

### 5. Error Swallowing in Background Tasks
**Severity**: P2 HIGH  
**Location**: `anomaly_detection.rs:spawn_anomaly_scanner` (lines 299-331)  
**Description**: Background task uses `unwrap_or` and doesn't propagate errors, causing silent failures.  
**Fix**:
```rust
tokio::spawn(async move {
    let mut consecutive_failures = 0;
    loop {
        interval.tick().await;
        match run_anomaly_scan(&pool, &state_clone, &rules).await {
            Ok(alerts) => {
                consecutive_failures = 0;
                // Process alerts
            }
            Err(e) => {
                consecutive_failures += 1;
                tracing::error!("Anomaly scan failed: {}", e);
                if consecutive_failures > 3 {
                    tracing::error!("Anomaly scanner stopping after 3 consecutive failures");
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            }
        }
    }
});
```

### 6. No Connection Pool Limits
**Severity**: P2 HIGH  
**Context**: Missing from code but critical for production - SQLite connection exhaustion risk.  
**Fix** (add to application initialization):
```rust
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

let pool = SqlitePoolOptions::new()
    .max_connections(20)
    .min_connections(5)
    .acquire_timeout(std::time::Duration::from_secs(30))
    .idle_timeout(std::time::Duration::from_secs(300))
    .max_lifetime(std::time::Duration::from_secs(1800))
    .connect_with(connect_options)
    .await?;
```

### 7. Lack of Request Timeouts
**Severity**: P2 HIGH  
**Context**: API endpoints missing request timeouts enabling DoS via slow queries.  
**Fix** (Axum middleware example):
```rust
use tower::timeout::TimeoutLayer;
use std::time::Duration;

let app = Router::new()
    .route("/api/maintenance/events", get(query_events_handler))
    .layer(TimeoutLayer::new(Duration::from_secs(30))) // 30s timeout
    .layer(DefaultBodyLimit::max(1024 * 1024)); // 1MB limit
```

### 8. No Audit Logging for Sensitive Operations
**Severity**: P2 HIGH  
**Description**: Employee updates, payroll calculations, and maintenance task assignments lack audit trails.  
**Fix** (add audit function):
```rust
pub async fn audit_log(
    pool: &SqlitePool,
    user: &str,
    action: &str,
    resource_type: &str,
    resource_id: &str,
    details: &serde_json::Value,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO audit_log (id, timestamp, user_id, action, resource_type, resource_id, details) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(Utc::now().to_rfc3339())
    .bind(user)
    .bind(action)
    .bind(resource_type)
    .bind(resource_id)
    .bind(serde_json::to_string(details)?)
    .execute(pool)
    .await?;
    Ok(())
}
```

### 9. Business Logic Error in EBITDA Calculation
**Severity**: P2 HIGH  
**Location**: `maintenance_store.rs:get_ebitda_summary` (lines 280-285)  
**Description**: Best/worst day comparison logic flawed - uses `<=` and `>=` incorrectly, may not update on equal values.  
**Fix**:
```rust
let mut best_day: Option<(String, i64)> = None;
let mut worst_day: Option<(String, i64)> = None;

for m in &metrics {
    let day_ebitda = day_rev - day_exp;
    let date_str = m.date.format("%Y-%m-%d").to_string();
    
    match best_day {
        None => best_day = Some((date_str.clone(), day_ebitda)),
        Some((_, best_val)) if day_ebitda > best_val => best_day = Some((date_str.clone(), day_ebitda)),
        _ => {}
    }
    
    match worst_day {
        None => worst_day = Some((date_str, day_ebitda)),
        Some((_, worst_val)) if day_ebitda < worst_val => worst_day = Some((date_str, day_ebitda)),
        _ => {}
    }
}
```

### 10. Missing Input Sanitization in Anomaly Detection
**Severity**: P2 HIGH  
**Location**: `anomaly_detection.rs:run_anomaly_scan` (line 211-213)  
**Description**: Rule names and metric names from database not validated before use in HashMap keys/logging.  
**Fix**:
```rust
fn sanitize_rule_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .collect::<String>()
        .truncate(100)
}
```

### 11. Potential Deadlock in Availability Map
**Severity**: P2 HIGH  
**Location**: `self_healing_orchestration.rs:apply_action` (line 85)  
**Description**: Holding write lock across async boundary when calling external functions.  
**Fix**:
```rust
pub async fn apply_action(map: &PodAvailabilityMap, action: &HealingAction) {
    let update = match action {
        HealingAction::MarkPodDegraded(id) => Some((*id, PodAvailability::Degraded {
            reason: "Anomaly detected".into(),
        })),
        HealingAction::MarkPodUnavailable(id) => Some((*id, PodAvailability::Unavailable {
            reason: "Critical anomaly".into(),
        })),
        _ => None,
    };
    
    if let Some((id, availability)) = update {
        let mut m = map.write().await;
        m.insert(id, availability);
        // Release lock before logging
        drop(m);
        tracing::warn!(target: LOG_TARGET, pod = id, "Pod availability updated");
    }
}
```

### 12. No Rate Limiting on Business Alert Checker
**Severity**: P2 HIGH  
**Location**: `business_alert_engine.rs:spawn_alert_checker` (line 86)  
**Description**: Fixed 30-minute interval with no jitter or backoff, could cause thundering herd.  
**Fix**:
```rust
tokio::spawn(async move {
    tokio::time::sleep(std::time::Duration::from_secs(300)).await;
    
    // Add jitter to prevent synchronized execution
    let mut rng = rand::thread_rng();
    let jitter = rng.gen_range(0..300); // 0-5 minute jitter
    tokio::time::sleep(std::time::Duration::from_secs(jitter)).await;
    
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(1800));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    
    loop {
        interval.tick().await;
        // ... alert checking
    }
});
```

---

## P3 MEDIUM FINDINGS

### 1. Inefficient Query in Maintenance Summary
**Severity**: P3 MEDIUM  
**Location**: `maintenance_store.rs:get_summary` (lines 162-164)  
**Description**: Fetches 5000 rows to calculate summary, should aggregate in SQL.  
**Fix**:
```rust
let summary = sqlx::query_as::<_, (i64, String, String)>(
    "SELECT 
        COUNT(*) as count,
        severity,
        event_type
     FROM maintenance_events 
     WHERE detected_at >= ?1
     GROUP BY severity, event_type"
)
.bind(since.to_rfc3339())
.fetch_all(pool)
.await?;
```

### 2. Missing Compression for JSON Metadata
**Severity**: P3 MEDIUM  
**Location**: All JSON fields in database tables  
**Description**: Large JSON blobs (`metadata`, `before_metrics`, `after_metrics`) not compressed, bloating database.  
**Fix**:
```rust
use flate2::{write::GzEncoder, Compression};

fn compress_json(value: &serde_json::Value) -> anyhow::Result<Vec<u8>> {
    let json = serde_json::to_string(value)?;
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    std::io::Write::write_all(&mut encoder, json.as_bytes())?;
    encoder.finish().map_err(Into::into)
}
```

### 3. No Connection Pool Monitoring
**Severity**: P3 MEDIUM  
**Context**: Missing metrics for connection pool health.  
**Fix**:
```rust
use metrics::{counter, gauge};

pub async fn monitor_pool(pool: &SqlitePool) {
    let stats = pool.acquire().await.map(|conn| {
        gauge!("db.pool.size").set(pool.size() as f64);
        gauge!("db.pool.idle").set(pool.num_idle() as f64);
        gauge!("db.pool.used").set((pool.size() - pool.num_idle()) as f64);
    });
}
```

### 4. Suboptimal HashMap Usage
**Severity**: P3 MEDIUM  
**Location**: `maintenance_store.rs:get_summary` (lines 176-180)  
**Description**: Creating temporary strings for HashMap keys inefficient.  
**Fix**:
```rust
use std::collections::HashMap;

let mut by_severity = HashMap::new();
let mut by_type = HashMap::new();

// Use enum directly as key
*by_severity.entry(sev).or_insert(0) += 1;
*by_type.entry(etype).or_insert(0) += 1;
```

### 5. Missing Prepared Statement Caching
**Severity**: P3 MEDIUM  
**Context**: Repeated queries don't use prepared statement caching.  
**Fix** (using sqlx's built-in caching):
```rust
// Ensure connection pool has statement caching enabled
let options = SqliteConnectOptions::new()
    .filename("racecontrol.db")
    .create_if_missing(true)
    .statement_cache_capacity(100); // Cache 100 statements
```

### 6. No Database Vacuum Scheduling
**Severity**: P3 MEDIUM  
**Context**: SQLite fragmentation not managed.  
**Fix**:
```rust
pub async fn schedule_vacuum(pool: SqlitePool) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(86400)); // Daily
        loop {
            interval.tick().await;
            if let Err(e) = sqlx::query("VACUUM").execute(&pool).await {
                tracing::error!("Failed to vacuum database: {}", e);
            }
        }
    });
}
```

### 7. Missing Text Search Index
**Severity**: P3 MEDIUM  
**Location**: Maintenance event/task descriptions  
**Description**: No full-text search capability for maintenance descriptions.  
**Fix**:
```rust
sqlx::query(
    "CREATE VIRTUAL TABLE IF NOT EXISTS maintenance_events_fts 
     USING fts5(id, description, content='maintenance_events', content_rowid='rowid')"
)
.execute(pool)
.await?;
```

---

## PRIORITY RECOMMENDATIONS

### IMMEDIATE (Next 24 hours):
1. Fix P1#1 SQL injection in `update_employee`
2. Fix P1#2 integer overflow in payroll
3. Fix P1#3 unbounded result sets
4. Implement Ollama endpoint authentication

### SHORT-TERM (Next week):
1. Add foreign key constraints
2. Implement connection pooling limits
3. Add audit logging
4. Fix business logic errors

### LONG-TERM (Next 30 days):
1. Implement comprehensive input validation
2. Add rate limiting
3. Add monitoring and metrics
4. Implement database backup/restore procedures

---

## OVERALL ASSESSMENT
**Risk Level: HIGH**

The codebase demonstrates thoughtful architecture but contains critical security flaws that require immediate remediation. The most urgent issues involve SQL injection vectors and data corruption risks that could compromise the entire system. 

**Strongest aspects**: Good use of parameterized queries in most places, clear separation of concerns, comprehensive error logging.

**Critical weaknesses**: Missing input validation, unsafe type conversions, silent error handling, and lack of database constraints.

**Recommendation**: Halt production deployment until P1 issues are resolved. Implement security code review as part of CI/CD pipeline.