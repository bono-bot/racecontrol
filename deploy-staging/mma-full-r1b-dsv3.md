# FULL AUDIT: v29.0 Meshed Intelligence Bundle B

## CRITICAL SEVERITY FINDINGS (P1)

### 1. **Integer Cast Overflows & Silent Loss of Precision**
**Severity:** P1 CRITICAL  
**File:** `Phase 26: aggregate_daily_revenue` (lines 50-52)  
**Description:**  
```rust
let occupancy = if sessions > 0 {
    (sessions as f32 / (total_pods * operating_hours) * 100.0).min(100.0)
} else {
    0.0
};
```
`sessions as f32` can silently truncate values > 16,777,216 (24-bit mantissa limit), causing occupancy calculation errors.

**Fix:**
```rust
let occupancy = if sessions > 0 {
    (sessions as f64 / (total_pods as f64 * operating_hours as f64) * 100.0)
        .min(100.0) as f32
} else {
    0.0
};
```

### 2. **Unchecked Database Query Failures - Silent Data Corruption**
**Severity:** P1 CRITICAL  
**File:** `Phase 26: aggregate_daily_revenue` (multiple locations)  
**Description:** All SQL queries use `.unwrap_or(0)` on `fetch_one()`. If query fails (e.g., table missing, syntax error), it silently returns 0 instead of propagating error, corrupting metrics.

**Example:**
```rust
let gaming: i64 = sqlx::query_scalar("...")
    .bind(&date_str)
    .fetch_one(pool)
    .await
    .unwrap_or(0);  // P1: Silent failure → corrupt revenue data
```

**Fix:**
```rust
let gaming: i64 = sqlx::query_scalar("...")
    .bind(&date_str)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!(target: LOG_TARGET, error = %e, "Failed to fetch gaming revenue");
        anyhow::anyhow!("Database query failed: {}", e)
    })?;
```

### 3. **SQL Injection via String Formatting**
**Severity:** P1 CRITICAL  
**File:** `Phase 3: run_hourly_aggregation` (lines 294-310)  
**Description:** Dynamic SQL generation with string interpolation of column names:
```rust
let query = format!(
    "INSERT OR REPLACE INTO telemetry_aggregates ... {metric} ...",
    metric = metric,
);
```
`METRIC_COLUMNS` are trusted, but pattern creates attack surface for future changes.

**Fix:**
```rust
// Whitelist validation
const ALLOWED_METRICS: &[&str] = &["gpu_temp_celsius", /* ... */];
if !ALLOWED_METRICS.contains(&metric) {
    return Err(anyhow::anyhow!("Invalid metric: {}", metric));
}

// Use match statement for safe query building
let query = match metric {
    "gpu_temp_celsius" => "INSERT ... gpu_temp_celsius ...",
    "cpu_usage_pct" => "INSERT ... cpu_usage_pct ...",
    _ => unreachable!(), // Already validated
};
```

### 4. **Race Condition in Background Task Initialization**
**Severity:** P1 CRITICAL  
**File:** `Phase 26: spawn_business_aggregator` (lines 86-106)  
**Description:** No synchronization between task startup and database readiness. Task may start before tables exist.

**Fix:**
```rust
pub async fn spawn_business_aggregator(pool: SqlitePool) -> anyhow::Result<()> {
    // Verify table exists first
    sqlx::query("SELECT 1 FROM daily_business_metrics LIMIT 1")
        .fetch_optional(&pool)
        .await
        .context("Daily business metrics table not ready")?;
    
    tokio::spawn(async move {
        // ... existing code
    });
    
    Ok(())
}
```

### 5. **Division by Zero Risk**
**Severity:** P1 CRITICAL  
**File:** `Phase 31: recommend_pod_rotation` (lines 265-268)  
**Description:** 
```rust
let avg_hours: f64 = pod_usage_hours.iter().map(|(_, h)| h).sum::<f64>() 
    / pod_usage_hours.len() as f64;  // P1: pod_usage_hours.len() can be 0
```

**Fix:**
```rust
if pod_usage_hours.is_empty() {
    return Vec::new();
}
let avg_hours: f64 = pod_usage_hours.iter().map(|(_, h)| h).sum::<f64>() 
    / pod_usage_hours.len() as f64;
```

## HIGH SEVERITY FINDINGS (P2)

### 6. **Database Connection Leak in Background Tasks**
**Severity:** P2 HIGH  
**File:** `Phase 26: spawn_business_aggregator` (line 87)  
**Description:** Pool cloned into spawned task without lifetime management. If main application restarts, orphaned connection pool may leak.

**Fix:**
```rust
pub fn spawn_business_aggregator(pool: SqlitePool) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let pool = pool; // Take ownership
        // ... existing code
        
        // Explicit cleanup on shutdown
        tokio::select! {
            _ = interval.tick() => { /* normal operation */ }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!(target: LOG_TARGET, "Business aggregator shutting down");
                pool.close().await;
            }
        }
    })
}
```

### 7. **Unbounded Memory Growth in Telemetry Writer**
**Severity:** P2 HIGH  
**File:** `Phase 251: writer_loop` (lines 186-190)  
**Description:** Buffer retry logic can cause unbounded growth:
```rust
if buffer.len() > 500 {
    let excess = buffer.len() - 500;
    buffer.drain(..excess);  // P2: Drops old data without alerting
}
```

**Fix:**
```rust
const MAX_BUFFER_SIZE: usize = 1000;
const RETRY_LIMIT: u8 = 3;

// Track retry count
static mut FLUSH_RETRY_COUNT: u8 = 0;

if buffer.len() >= MAX_BUFFER_SIZE {
    unsafe {
        if FLUSH_RETRY_COUNT >= RETRY_LIMIT {
            tracing::error!("Telemetry buffer overflow, dropping {} samples", buffer.len());
            buffer.clear();
            FLUSH_RETRY_COUNT = 0;
        }
    }
}
```

### 8. **Blocking I/O in Async Context**
**Severity:** P2 HIGH  
**File:** `predictive_maintenance.rs: check_gpu_temp()` (lines 232-240)  
**Description:** Synchronous command execution blocks tokio runtime:
```rust
let output = std::process::Command::new("nvidia-smi")  // BLOCKING
    .args(["--query-gpu=temperature.gpu", "--format=csv,noheader,nounits"])
    .output()  // P2: Blocks entire thread
    .ok()?;
```

**Fix:**
```rust
use tokio::process::Command;

async fn check_gpu_temp() -> Option<PredictiveAlert> {
    let output = Command::new("nvidia-smi")
        .args(["--query-gpu=temperature.gpu", "--format=csv,noheader,nounits"])
        .output()
        .await
        .ok()?;
    // ... rest of function
}
```

### 9. **Missing Input Validation - Integer Overflow**
**Severity:** P2 HIGH  
**File:** `Phase 19: recommend_pricing` (lines 139-144)  
**Description:** Integer multiplication without overflow check:
```rust
let recommended = current_price_paise + (current_price_paise * change_bp / 10000);
// P2: current_price_paise * change_bp can overflow i64
```

**Fix:**
```rust
use std::num::Wrapping;

let change_bp = (change_pct * 100.0).round() as i64;
let recommended = current_price_paise
    .checked_mul(change_bp)
    .and_then(|v| v.checked_div(10000))
    .and_then(|v| current_price_paise.checked_add(v))
    .unwrap_or_else(|| {
        tracing::warn!("Price calculation overflow, using current price");
        current_price_paise
    });
```

### 10. **Unsafe HashMap Access in Async Context**
**Severity:** P2 HIGH  
**File:** `Phase 251: writer_loop` (lines 169-174)  
**Description:** HashMap accessed across await points without synchronization:
```rust
let mut last_sample_ts: HashMap<String, DateTime<Utc>> = HashMap::new();
// ...
if let Some(last_ts) = last_sample_ts.get(&frame.pod_id) {  // OK
    // ...
}
last_sample_ts.insert(frame.pod_id.clone(), frame.timestamp);  // P2: Concurrent modification possible
```

**Fix:**
```rust
use dashmap::DashMap;

let last_sample_ts: Arc<DashMap<String, DateTime<Utc>>> = Arc::new(DashMap::new());
// Clone for async block
let ts_map = last_sample_ts.clone();

tokio::spawn(async move {
    // Use DashMap's atomic methods
    if let Some(entry) = ts_map.get(&frame.pod_id) {
        // ...
    }
    ts_map.insert(frame.pod_id.clone(), frame.timestamp);
});
```

### 11. **Incorrect Date Comparison Logic**
**Severity:** P2 HIGH  
**File:** `Phase 26: aggregate_daily_revenue` (lines 27-30)  
**Description:** `DATE(ended_at) = ?` may fail due to timezone issues. SQLite's DATE() uses UTC, while application may use local time.

**Fix:**
```rust
// Use parameterized date range
let date_start = date.and_hms_opt(0, 0, 0).unwrap();
let date_end = date.and_hms_opt(23, 59, 59).unwrap();

sqlx::query_scalar(
    "SELECT COALESCE(SUM(wallet_debit_paise), 0) FROM billing_sessions \
     WHERE ended_at >= ?1 AND ended_at <= ?2 AND status IN ('completed', 'ended_early')",
)
.bind(date_start)
.bind(date_end)
// ...
```

### 12. **Silent File System Errors**
**Severity:** P2 HIGH  
**File:** `Phase 251: init_telemetry_db` (lines 108-111)  
**Description:** `std::fs::create_dir_all()` may fail silently:
```rust
if let Some(parent) = Path::new(db_path).parent() {
    std::fs::create_dir_all(parent)?;  // P2: ? operator in async context
}
```

**Fix:**
```rust
use tokio::fs;

if let Some(parent) = Path::new(db_path).parent() {
    fs::create_dir_all(parent)
        .await
        .context(format!("Failed to create directory: {:?}", parent))?;
}
```

## MEDIUM SEVERITY FINDINGS (P3)

### 13. **Inefficient String Allocation**
**Severity:** P3 MEDIUM  
**File:** `Phase 20: build_diagnosis_prompt` (lines 130-155)  
**Description:** Multiple string allocations in hot path:
```rust
let pod_id = sanitize_for_prompt(&req.pod_id, 32);
let anomalies = req.anomalies.iter()
    .map(|a| sanitize_for_prompt(a, 200))
    .collect::<Vec<_>>()
    .join(", ");  // P3: O(n²) allocation
```

**Fix:**
```rust
use std::fmt::Write;

let mut prompt = String::with_capacity(2048);
write!(&mut prompt, 
    "You are an AI maintenance technician for a racing simulator venue.\n\
     Pod: {}\n\
     Active anomalies: ",
    sanitize_for_prompt(&req.pod_id, 32)
).unwrap();

for (i, anomaly) in req.anomalies.iter().enumerate() {
    if i > 0 {
        prompt.push_str(", ");
    }
    prompt.push_str(&sanitize_for_prompt(anomaly, 200));
}
// ... continue building
```

### 14. **Missing Index on Frequently Queried Column**
**Severity:** P3 MEDIUM  
**File:** `Phase 251: telemetry_samples table`  
**Description:** `pod_id` column queried in `writer_loop` but not indexed:
```rust
if let Some(last_ts) = last_sample_ts.get(&frame.pod_id) {
```

**Fix:** Add index creation:
```rust
sqlx::query("CREATE INDEX IF NOT EXISTS idx_telemetry_pod ON telemetry_samples(pod_id)")
    .execute(&pool)
    .await?;
```

### 15. **Incorrect Float Comparison**
**Severity:** P3 MEDIUM  
**File:** `Phase 3: get_metric_trend` (lines 436-440)  
**Description:** Direct floating point comparison:
```rust
if denom.abs() < 1e-12 {  // P3: Magic number, may fail for small values
```

**Fix:**
```rust
const EPSILON: f64 = f64::EPSILON * 100.0;

if denom.abs() < EPSILON {
    // Handle degenerate case
}
```

### 16. **Unoptimized Database Queries**
**Severity:** P3 MEDIUM  
**File:** `Phase 18: forecast_week` (lines 68-78)  
**Description:** N+1 query pattern - runs 7 separate queries for 7 days:
```rust
for day_offset in 0..7 {
    let target_date = today + chrono::Duration::days(day_offset);
    // Query runs 7 times
    let row: Option<(f64, f64)> = sqlx::query_as("...").fetch_optional(pool).await?;
}
```

**Fix:**
```rust
// Single query for all days
let rows: Vec<(i32, f64, f64)> = sqlx::query_as(
    "SELECT CAST(strftime('%w', date) AS INTEGER) as dow, 
            COALESCE(AVG(occupancy_rate_pct), 0),
            COALESCE(AVG(sessions_count), 0)
     FROM daily_business_metrics 
     WHERE date >= date('now', '-30 days')
     GROUP BY CAST(strftime('%w', date) AS INTEGER)"
)
.fetch_all(pool)
.await?;

// Convert to map for O(1) lookup
let mut dow_map = HashMap::new();
for (dow, occ, sessions) in rows {
    dow_map.insert(dow, (occ, sessions));
}
```

## SUMMARY

**CRITICAL (P1):** 5 findings - Focus on integer overflow, silent database failures, and SQL injection vectors immediately.

**HIGH (P2):** 7 findings - Address connection leaks, blocking I/O, and race conditions in background tasks.

**MEDIUM (P3):** 4 findings - Optimize performance and fix minor logic issues in next sprint.

**IMMEDIATE ACTION REQUIRED:**
1. Fix all P1 findings before next deployment
2. Add comprehensive error handling to replace all `.unwrap_or()` patterns
3. Implement connection pool lifecycle management
4. Add integration tests for telemetry writer buffer management

**AUDITOR NOTE:** The codebase shows good architectural patterns but lacks defensive programming. Most critical issues stem from optimistic error handling assumptions. Recommend implementing a centralized error reporting system and adding circuit breakers for database operations.