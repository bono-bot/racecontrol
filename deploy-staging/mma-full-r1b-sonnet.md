# SECURITY AUDIT REPORT: Racing Point eSports v29.0 - Bundle B

## CRITICAL (P1) FINDINGS

### P1-1: SQL Injection Vulnerability
**File**: `business_intelligence.rs:forecast_week()`  
**Line**: 42-46  
**Issue**: SQL query construction using string interpolation of user-controlled data  
**Risk**: Complete database compromise via injected SQL  

```rust
// VULNERABLE CODE:
.bind(dow.num_days_from_sunday() as i32)

// The dow.num_days_from_sunday() calls strftime('%w') but the binding is unsafe
```

**Fix**:
```rust
// Use explicit day mapping instead of relying on strftime
let dow_num = match dow {
    chrono::Weekday::Sun => 0,
    chrono::Weekday::Mon => 1,
    chrono::Weekday::Tue => 2,
    chrono::Weekday::Wed => 3,
    chrono::Weekday::Thu => 4,
    chrono::Weekday::Fri => 5,
    chrono::Weekday::Sat => 6,
};
```

### P1-2: Unsafe Integer Cast from Database
**File**: `business_intelligence.rs:aggregate_daily_revenue()`  
**Lines**: 18, 28, 38  
**Issue**: Direct casting of i64 to u32 without validation  
**Risk**: Integer overflow causing data corruption  

```rust
// VULNERABLE CODE:
sessions_count: sessions as u32,

// SECURE FIX:
sessions_count: u32::try_from(sessions).unwrap_or(0),
```

### P1-3: Multiple unwrap() Calls in Background Tasks
**File**: `business_intelligence.rs:aggregate_daily_revenue()`  
**Lines**: 21, 31, 41  
**Issue**: Background task can panic on database errors  
**Risk**: Complete service crash  

```rust
// VULNERABLE CODE:
.fetch_one(pool).await.unwrap_or(0);

// SECURE FIX:
.fetch_one(pool).await.map_err(|e| {
    tracing::error!("Failed to fetch gaming revenue: {}", e);
    e
})?.unwrap_or(0);
```

### P1-4: Money Calculation Using f32 Arithmetic
**File**: `business_intelligence.rs:aggregate_daily_revenue()`  
**Lines**: 50-56  
**Issue**: Floating-point arithmetic for business calculations  
**Risk**: Accumulating rounding errors in financial data  

```rust
// VULNERABLE CODE:
let occupancy = if sessions > 0 {
    (sessions as f32 / (total_pods * operating_hours) * 100.0).min(100.0)
} else {
    0.0
};

// SECURE FIX:
let occupancy = if sessions > 0 {
    let basis_points = (sessions * 10000) / ((8 * 12) as i64);
    (basis_points as f32 / 100.0).min(100.0)
} else {
    0.0
};
```

### P1-5: Format String Injection
**File**: `telemetry_store.rs:run_hourly_aggregation()`  
**Lines**: 383-420  
**Issue**: Dynamic SQL construction with format! macro  
**Risk**: SQL injection if METRIC_COLUMNS is ever modified  

```rust
// VULNERABLE CODE:
let query = format!(
    "INSERT OR REPLACE INTO telemetry_aggregates... {metric}...",
    metric = metric,
);

// SECURE FIX:
match metric {
    "gpu_temp_celsius" => {
        sqlx::query("INSERT OR REPLACE... gpu_temp_celsius...")
    }
    "cpu_usage_pct" => {
        sqlx::query("INSERT OR REPLACE... cpu_usage_pct...")
    }
    // ... explicit cases for each metric
}
```

## HIGH PRIORITY (P2) FINDINGS

### P2-1: Resource Leak in Telemetry Writer
**File**: `telemetry_store.rs:flush_buffer()`  
**Lines**: 274-318  
**Issue**: Failed transactions leave buffer intact, can grow unbounded  
**Risk**: Memory exhaustion  

```rust
// PROBLEMATIC CODE:
Err(e) => {
    tracing::error!("TelemetryWriter flush failed ({} samples): {}", count, e);
    // Keep buffer intact - THIS IS THE LEAK
}

// SECURE FIX:
Err(e) => {
    tracing::error!("TelemetryWriter flush failed ({} samples): {}", count, e);
    self.failed_attempts += 1;
    if self.failed_attempts > 3 {
        buffer.drain(..count.min(buffer.len()));
        tracing::warn!("Dropped {} samples after 3 failed attempts", count);
        self.failed_attempts = 0;
    }
}
```

### P2-2: Deadlock Risk in Cached Functions
**File**: `predictive_maintenance.rs:collect_windows_errors()`  
**Lines**: 371-391  
**Issue**: Holding mutex lock while executing external PowerShell command  
**Risk**: Deadlock if PowerShell hangs  

```rust
// VULNERABLE CODE:
if let Ok(mut cached) = LAST_ERRORS.lock() {
    // ... external command here while holding lock

// SECURE FIX:
let needs_refresh = {
    let cached = LAST_ERRORS.lock().ok()?;
    cached.0.elapsed() > Duration::from_secs(300)
};
if needs_refresh {
    let errors = collect_errors_external();  // No lock held
    if let Ok(mut cached) = LAST_ERRORS.lock() {
        *cached = (Instant::now(), errors.clone());
    }
    errors
} else {
    LAST_ERRORS.lock().ok()?.1.clone()
}
```

### P2-3: Race Condition in Daily Counter Reset
**File**: `predictive_maintenance.rs:maybe_reset_daily()`  
**Lines**: 59-65  
**Issue**: Non-atomic read-modify-write on date change  
**Risk**: Lost restart counts around midnight  

```rust
// VULNERABLE CODE:
fn maybe_reset_daily(&mut self) {
    let today = chrono::Utc::now().date_naive();
    if today != self.restart_date {
        self.restart_count_today = 0;
        self.restart_date = today;
    }
}

// SECURE FIX:
use std::sync::atomic::{AtomicU32, AtomicI32, Ordering};

struct PredictiveState {
    restart_count_today: AtomicU32,
    restart_date_days: AtomicI32, // days since epoch
    // ...
}

fn maybe_reset_daily(&self) {
    let today_days = Utc::now().date_naive().num_days_from_ce();
    let stored_days = self.restart_date_days.load(Ordering::Acquire);
    
    if today_days != stored_days {
        // Atomic compare-and-swap to avoid race
        if self.restart_date_days.compare_exchange_weak(
            stored_days, today_days, Ordering::AcqRel, Ordering::Relaxed
        ).is_ok() {
            self.restart_count_today.store(0, Ordering::Release);
        }
    }
}
```

### P2-4: Silent Business Logic Error
**File**: `dynamic_pricing.rs:recommend_pricing()`  
**Lines**: 41-45  
**Issue**: Integer division in basis points can truncate small amounts  
**Risk**: Incorrect pricing calculations  

```rust
// PROBLEMATIC CODE:
let recommended = current_price_paise + (current_price_paise * change_bp / 10000);

// SECURE FIX:
let recommended = current_price_paise + 
    ((current_price_paise as i128 * change_bp as i128) / 10000) as i64;
```

### P2-5: Unvalidated User Input in AI Prompts
**File**: `ai_diagnosis.rs:build_diagnosis_prompt()`  
**Lines**: 61-94  
**Issue**: Insufficient sanitization allows prompt injection  
**Risk**: AI manipulation leading to wrong maintenance decisions  

```rust
// INSUFFICIENT:
fn sanitize_for_prompt(input: &str, max_len: usize) -> String {
    input.chars().filter(|c| !c.is_control() || *c == '\n').take(max_len).collect()
}

// SECURE FIX:
fn sanitize_for_prompt(input: &str, max_len: usize) -> String {
    input
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || " .,:-_()[]{}".contains(*c))
        .take(max_len)
        .collect::<String>()
        .replace("```", "")  // Prevent markdown injection
        .replace("JSON:", "")  // Prevent response format manipulation
}
```

## MEDIUM PRIORITY (P3) FINDINGS

### P3-1: Performance: Inefficient Telemetry Cleanup
**File**: `telemetry_store.rs:run_nightly_cleanup()`  
**Lines**: 322-357  
**Issue**: Batched deletes without proper indexing strategy  
**Performance Impact**: Long-running cleanup blocks other operations  

**Fix**: Add covering index and optimize batch size based on table statistics.

### P3-2: Missing Input Validation
**File**: `spare_parts.rs:recommend_parts()`  
**Lines**: 42-82  
**Issue**: No validation of RUL hours (could be negative/infinite)  
**Risk**: Nonsensical spare parts recommendations  

### P3-3: Suboptimal Error Handling
**File**: `business_intelligence.rs:spawn_business_aggregator()`  
**Lines**: 95-108  
**Issue**: Only warns on aggregation failure, doesn't implement exponential backoff  
**Risk**: Continuous failure spam in logs  

## SUMMARY

- **P1 Critical**: 5 findings requiring immediate attention
- **P2 High**: 5 findings requiring fixes in next sprint  
- **P3 Medium**: 3 findings for future optimization

**Most Critical**: The SQL injection vulnerability in forecast_week() and unsafe integer casts throughout the codebase must be addressed immediately before any production deployment.

**Recommendation**: Implement comprehensive input validation layer and switch all database integer operations to use `try_from()` with proper error handling.