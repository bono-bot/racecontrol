# Rust Security Audit Report: Racing Point eSports v29.0

## Executive Summary

**Critical Severity Findings**: 4  
**High Severity Findings**: 6  
**Medium Severity Findings**: 8

The codebase exhibits several serious security and reliability issues requiring immediate attention. The most critical vulnerabilities include SQL injection, race conditions in anomaly detection, silent data corruption, and integer overflows in business-critical calculations.

---

## P1 CRITICAL Findings

### 1. SQL Injection Vulnerability
**Category**: P1 CRITICAL  
**File**: maintenance_db.rs:309-345 (`update_employee`)  
**Description**: Dynamic SQL query construction with string concatenation creates SQL injection vulnerability. The function builds SQL with raw `format!()` without proper sanitization, allowing attackers to modify employee records arbitrarily.  
**Concrete Fix**:
```rust
// Replace dynamic SQL construction with static SQL and conditional binds
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
    let mut query = String::from("UPDATE employees SET ");
    let mut binds = Vec::new();
    let mut conditions = Vec::new();
    let mut param_count = 1;

    if let Some(n) = name {
        conditions.push(format!("name = ?{}", param_count));
        binds.push(n);
        param_count += 1;
    }
    // ... similar for other parameters
    
    if conditions.is_empty() {
        return Ok(false);
    }
    
    query.push_str(&conditions.join(", "));
    query.push_str(" WHERE id = ?");
    binds.push(id);
    
    let mut sqlx_query = sqlx::query(&query);
    for bind in binds {
        sqlx_query = sqlx_query.bind(bind);
    }
    
    let result = sqlx_query.execute(pool).await?;
    Ok(result.rows_affected() > 0)
}
```

### 2. Race Condition in Anomaly Detection State
**Category**: P1 CRITICAL  
**File**: anomaly_detection.rs:168-234 (`run_anomaly_scan`)  
**Description**: The `first_violation` HashMap tracks rule violations per pod, but the sustained window calculation can be gamed. Concurrent alerts for the same pod+rule can cause incorrect sustained_min calculations and alert suppression.  
**Concrete Fix**:
```rust
// Add a mutex per (pod_id, rule_name) key
use std::collections::HashMap;
use tokio::sync::Mutex;

pub struct EngineState {
    last_alert: HashMap<(String, String), DateTime<Utc>>,
    first_violation: HashMap<(String, String), DateTime<Utc>>,
    violation_locks: HashMap<(String, String), Mutex<()>>,
    recent_alerts: Vec<AnomalyAlert>,
}

impl EngineState {
    fn new() -> Self {
        Self {
            last_alert: HashMap::new(),
            first_violation: HashMap::new(),
            violation_locks: HashMap::new(),
            recent_alerts: Vec::new(),
        }
    }
}

// In run_anomaly_scan:
let lock_key = (row.pod_id.clone(), rule.name.clone());
let lock = state.violation_locks
    .entry(lock_key.clone())
    .or_insert_with(|| Mutex::new(()))
    .lock()
    .await;

// Perform the violation check and update while holding the lock
```

### 3. Silent Data Corruption in Business Metrics
**Category**: P1 CRITICAL  
**File**: maintenance_db.rs:483-497 (`upsert_daily_metrics`)  
**Description**: Floating point `f32` values for `occupancy_rate_pct` and `peak_occupancy_pct` are cast to `f64` for database storage, causing precision errors when storing paise values. Subsequent calculations using these rounded values lead to financial discrepancies.  
**Concrete Fix**:
```rust
/// Fixed function signature and storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyBusinessMetrics {
    pub date: NaiveDate,
    pub revenue_gaming_paise: i64,
    pub revenue_cafe_paise: i64,
    pub revenue_other_paise: i64,
    pub expense_rent_paise: i64,
    pub expense_utilities_paise: i64,
    pub expense_salaries_paise: i64,
    pub expense_maintenance_paise: i64,
    pub expense_other_paise: i64,
    pub sessions_count: u32,
    pub occupancy_rate_bps: u16, // Basis points (0-10000 for 0.00-100.00%)
    pub peak_occupancy_bps: u16, // Same as above
}

// Store as integer basis points
.bind(metrics.occupancy_rate_bps as i64)
.bind(metrics.peak_occupancy_bps as i64)
```

### 4. Integer Overflow in Payroll Calculation
**Category**: P1 CRITICAL  
**File**: maintenance_db.rs:842-856 (`calculate_monthly_payroll`)  
**Description**: `hours_worked * hourly_rate_paise` can overflow i64 for high-wage employees working many hours. Example: ₹500/hour (50,000 paise) × 300 hours = 15,000,000 paise, safe. But technician working overtime could exceed.  
**Concrete Fix**:
```rust
// Use checked arithmetic
let emp_total = row.hourly_rate_paise
    .checked_mul((row.total_hours * 100.0) as i64) // Store hours in hundredths
    .and_then(|v| v.checked_div(100))
    .unwrap_or_else(|| {
        tracing::error!("Payroll overflow for employee {}", row.employee_id);
        0 // Or handle appropriately
    });
```

---

## P2 HIGH Findings

### 5. Deadlock Risk in Anomaly Scanner
**Category**: P2 HIGH  
**File**: anomaly_detection.rs:276-291 (`spawn_anomaly_scanner`)  
**Description**: The scanner holds a write lock on `EngineState` for the entire `run_anomaly_scan` duration (~seconds). Concurrent API calls to `recent_alerts()` will deadlock waiting for the read lock.  
**Concrete Fix**:
```rust
pub async fn run_anomaly_scan(
    pool: &SqlitePool,
    state: &Arc<RwLock<EngineState>>,
    rules: &[AnomalyRule],
) -> Vec<AnomalyAlert> {
    // ... compute alerts without holding lock
    let alerts = compute_alerts(pool, rules).await;
    
    // Only lock briefly to update state
    {
        let mut guard = state.write().await;
        guard.update_with_alerts(&alerts);
    }
    
    alerts
}
```

### 6. Incorrect Business Logic in Escalation
**Category**: P2 HIGH  
**File**: escalation.rs:19-30 (`determine_escalation`)  
**Description**: Critical severity always escalates to Manager tier, bypassing auto-fix attempts. This violates the principle of allowing one auto-fix attempt for transient critical issues.  
**Concrete Fix**:
```rust
pub fn determine_escalation(
    severity: &str,
    auto_fix_attempts: u32,
    is_recurring: bool,
) -> EscalationTier {
    match severity {
        "Critical" if auto_fix_attempts == 0 && !is_recurring => EscalationTier::Auto,
        "Critical" => EscalationTier::Manager,
        "High" if auto_fix_attempts <= 1 => EscalationTier::Technician,
        "High" => EscalationTier::Manager,
        "Medium" | "Low" if auto_fix_attempts == 0 && !is_recurring => EscalationTier::Auto,
        "Medium" | "Low" if auto_fix_attempts <= 2 => EscalationTier::Technician,
        _ => EscalationTier::Manager,
    }
}
```

### 7. Resource Leak in Database Connections
**Category**: P2 HIGH  
**File**: maintenance_db.rs:24-44 (`init_maintenance_tables`)  
**Description**: Multiple table creation functions called independently without connection pooling optimization. Each creates its own connection pool if not managed properly, exhausting SQLite connections on Windows.  
**Concrete Fix**:
```rust
// Create a single initialization function
pub async fn init_all_tables(pool: &SqlitePool) -> anyhow::Result<()> {
    init_maintenance_tables(pool).await?;
    init_business_tables(pool).await?;
    init_hr_tables(pool).await?;
    Ok(())
}

// Ensure connection pool is shared across application
pub struct AppState {
    pub db_pool: SqlitePool,
    // ... other state
}
```

### 8. Missing Validation in Hardware Telemetry
**Category**: P2 HIGH  
**File**: anomaly_detection.rs:115-133 (`HwRow::metric_value`)  
**Description**: No bounds checking on metric values. Negative temperatures, >100% usage values, or extreme outliers corrupt anomaly detection and RUL calculations.  
**Concrete Fix**:
```rust
fn metric_value(&self, name: &str) -> Option<f64> {
    let val = match name {
        "gpu_temp_celsius" => self.gpu_temp_celsius.filter(|&v| v >= -40.0 && v <= 125.0),
        "cpu_temp_celsius" => self.cpu_temp_celsius.filter(|&v| v >= -40.0 && v <= 110.0),
        "disk_smart_health_pct" => self.disk_smart_health_pct.map(|v| v as f64).filter(|&v| v >= 0.0 && v <= 100.0),
        "cpu_usage_pct" => self.cpu_usage_pct.filter(|&v| v >= 0.0 && v <= 100.0),
        // ... similar for others
        _ => None,
    };
    
    if val.is_none() {
        tracing::warn!("Invalid metric {} value detected", name);
    }
    val
}
```

### 9. Incorrect RUL Calculation for Stable Trends
**Category**: P2 HIGH  
**File**: anomaly_detection.rs:487-530 (`calculate_rul`)  
**Description**: When trend is "stable" (rate_per_day ~= 0), the function returns None, but stable low health still needs RUL estimation. Example: Disk health at 10% not changing still requires replacement.  
**Concrete Fix**:
```rust
let rul_hours = if trend.rate_per_day.abs() < 0.1 {
    // Stable trend: estimate based on current value vs threshold
    let gap = if trend.current_value > failure_threshold {
        trend.current_value - failure_threshold
    } else {
        failure_threshold - trend.current_value
    };
    // Assume very slow degradation of 0.1% per day
    (gap / 0.1) * 24.0
} else if is_declining_health {
    // ... existing logic
} else {
    // ... existing logic
};
```

### 10. Missing Index on Business Metrics Queries
**Category**: P2 HIGH  
**File**: maintenance_db.rs:417-444 (`query_business_metrics`)  
**Description**: Date range queries on `daily_business_metrics` table perform full table scans without index on `date` column, causing performance degradation as data grows.  
**Concrete Fix**:
```rust
// Add to init_business_tables function
sqlx::query(
    "CREATE INDEX IF NOT EXISTS idx_business_metrics_date 
     ON daily_business_metrics(date)"
)
.execute(pool)
.await?;
```

---

## P3 MEDIUM Findings

### 11. Performance: Inefficient Event Filtering
**Category**: P3 MEDIUM  
**File**: maintenance_db.rs:98-125 (`query_events`)  
**Description**: Applies filters in Rust after fetching all rows from database instead of using SQL WHERE clause. Transfers unnecessary data.  
**Fix**: Build dynamic SQL with conditional WHERE clauses.

### 12. Missing Validation: Task Priority Range
**Category**: P3 MEDIUM  
**File**: maintenance_models.rs:73 (`MaintenanceTask::priority`)  
**Description**: `priority: u8` field accepts values 0-255 but business logic expects 1-100. Invalid priorities break sorting and business-aware scoring.  
**Fix**: Add validation in `insert_task` and constructor.

### 13. Suboptimal Pattern: JSON String Manipulation
**Category**: P3 MEDIUM  
**File**: maintenance_db.rs:202-204 (status string manipulation)  
**Description**: Manual JSON string manipulation with `format!("\"{}\"", row.status)` is fragile. Use proper serde deserialization.  
**Fix**: Store status as JSON string in DB or use custom FromRow implementation.

### 14. Performance: Unbounded Memory in Recent Alerts
**Category**: P3 MEDIUM  
**File**: anomaly_detection.rs:238-246 (recent_alerts cap)  
**Description`: Fixed-size buffer with Vec drain is O(n). Use circular buffer or VecDeque.  
**Fix**: 
```rust
use std::collections::VecDeque;
recent_alerts: VecDeque<AnomalyAlert>,
// Then use push_back and if len() > 200 { pop_front(); }
```

### 15. Missing Validation: Employee Phone Format
**Category**: P3 MEDIUM  
**File**: maintenance_models.rs:89 (`Employee::phone`)  
**Description`: Phone numbers stored as String without validation. Invalid formats break WhatsApp notifications.  
**Fix**: Add validation regex or phone number library.

### 16. Incorrect Error Handling: Unwrapped Results
**Category**: P3 MEDIUM  
**Multiple locations** with `.unwrap()` on database queries  
**Description**: Silent failures when database returns errors.  
**Fix**: Propagate errors with `?` operator instead of unwrapping.

### 17. Suboptimal Pattern: Redundant Data Storage
**Category**: P3 MEDIUM  
**File**: maintenance_db.rs schema: `event_type` stored as JSON string  
**Description**: Storing enum variants as JSON strings wastes space and parsing time. Store as simple TEXT with enum mapping.  
**Fix**: Use `#[repr]` enums or integer mapping.

### 18. Missing: Foreign Key Constraints
**Category**: P3 MEDIUM  
**File**: maintenance_db.rs:573 (`attendance_records` table)  
**Description**: Missing `ON DELETE CASCADE` or similar referential integrity constraints.  
**Fix**: Add proper foreign key constraints.

---

## Critical Recommendations

### Immediate Actions (Before Deployment):
1. **Fix SQL injection** in `update_employee` function
2. **Implement proper locking** in anomaly detection engine
3. **Convert floating point percentages** to integer basis points
4. **Add checked arithmetic** for all financial calculations

### High Priority (Within 1 Week):
1. **Add comprehensive validation** for all input data
2. **Implement proper connection pooling**
3. **Fix escalation logic** for critical alerts
4. **Add database indices** for performance-critical queries

### Medium Priority (Within 1 Month):
1. **Refactor JSON string handling** to use proper serde
2. **Add foreign key constraints** for data integrity
3. **Implement circular buffer** for recent alerts
4. **Add phone number validation**

### Security Notes:
- **Windows Session 0/1 isolation**: Ensure telemetry services run in appropriate sessions
- **Static CRT**: Verify all dependencies link statically to avoid DLL hijacking
- **SQLite hardening**: Enable WAL mode, proper journaling, and connection limits
- **Input validation**: All API endpoints must validate pod_id ranges (1-8)

The codebase demonstrates good separation of concerns but requires significant hardening for production use. Pay special attention to financial calculations and real-time anomaly detection paths.