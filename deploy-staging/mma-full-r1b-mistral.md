Here's a comprehensive security audit of the provided Rust code for v29.0 Meshed Intelligence, focusing on P1 (Critical) and P2 (High) severity findings:

---

### **P1 CRITICAL Findings**

#### **1. SQL Injection Vulnerability in `forecast_week` (Phase 18)**
**File:** `demand_forecast.rs`
**Function:** `forecast_week` (line ~25)
**Description:**
The query uses `strftime('%w', date)` which is concatenated directly into the SQL string. While the input (`dow.num_days_from_sunday()`) is numeric, the query is still constructed dynamically, violating the "no string formatting" rule. An attacker could manipulate the `date` field in `daily_business_metrics` to inject malicious SQL.

**Fix:**
Use parameterized queries for all dynamic values, including the `strftime` format:
```rust
let row: Option<(f64, f64)> = sqlx::query_as(
    "SELECT COALESCE(AVG(occupancy_rate_pct), 0), COALESCE(AVG(sessions_count), 0)
     FROM daily_business_metrics
     WHERE date >= date('now', '-30 days')
     AND strftime('%w', date) = ?1"
)
.bind(dow.num_days_from_sunday().to_string())  // Convert to string for strftime
.fetch_optional(pool)
.await?;
```

---

#### **2. Integer Overflow in `recommend_pricing` (Phase 19)**
**File:** `dynamic_pricing.rs`
**Function:** `recommend_pricing` (line ~50)
**Description:**
The calculation `current_price_paise * change_bp / 10000` can overflow if `current_price_paise` is large (e.g., `i64::MAX`). This violates the "all money must be `i64`" rule and could lead to incorrect pricing or crashes.

**Fix:**
Use checked arithmetic and enforce a maximum price:
```rust
const MAX_PRICE_PAISE: i64 = 1_000_000_000; // 10M INR
let change_bp = (change_pct * 100.0).round() as i64;
let recommended = current_price_paise.saturating_add(
    current_price_paise.saturating_mul(change_bp).checked_div(10000).unwrap_or(0)
);
let recommended = recommended.min(MAX_PRICE_PAISE);
```

---

#### **3. Unbounded Buffer Growth in `writer_loop` (Phase 251)**
**File:** `telemetry_writer.rs`
**Function:** `writer_loop` (line ~150)
**Description:**
If the database is unavailable, the buffer grows indefinitely until OOM. The current mitigation (dropping oldest samples) is insufficient because:
1. It only triggers at 500 samples (too late for OOM).
2. It doesn’t log the root cause (DB failure).

**Fix:**
Add a hard cap and log DB failures:
```rust
if buffer.len() > 500 {
    tracing::error!(
        "TelemetryWriter buffer overflow (DB unavailable). Dropping {} samples.",
        buffer.len() - 500
    );
    buffer.drain(..buffer.len() - 500);
}
```

---

#### **4. Time-of-Check/Time-of-Use (TOCTOU) in `apply_approved_pricing` (Phase 32)**
**File:** `pricing_bridge.rs`
**Function:** `apply_approved_pricing` (line ~100)
**Description:**
The function fetches approved proposals and applies them in separate transactions. A proposal could be modified (e.g., rejected) between the fetch and apply steps, leading to inconsistent state.

**Fix:**
Use a single transaction with row locking:
```rust
pub async fn apply_approved_pricing(pool: &SqlitePool) -> anyhow::Result<u32> {
    let mut tx = pool.begin().await?;
    let approved: Vec<(String, i64)> = sqlx::query_as(
        "SELECT id, proposed_price_paise FROM pricing_proposals
         WHERE status = 'approved' FOR UPDATE"
    ).fetch_all(&mut *tx).await?;

    for (id, price) in &approved {
        // Push price to billing config (omitted for brevity)
        sqlx::query(
            "UPDATE pricing_proposals SET status = 'applied', applied_at = ?1 WHERE id = ?2"
        )
        .bind(Utc::now().to_rfc3339())
        .bind(id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(approved.len() as u32)
}
```

---

#### **5. Missing Input Validation in `build_diagnosis_prompt` (Phase 8)**
**File:** `ai_diagnosis.rs`
**Function:** `build_diagnosis_prompt` (line ~50)
**Description:**
The `sanitize_for_prompt` function strips control characters but doesn’t validate:
1. **Length**: Long strings could cause prompt injection (e.g., `req.telemetry_summary`).
2. **Content**: User-controlled strings (e.g., `anomalies`) could include malicious JSON or code.

**Fix:**
Add strict validation:
```rust
fn sanitize_for_prompt(input: &str, max_len: usize) -> String {
    input
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(*c, ' ' | '.' | ',' | '-' | '_'))
        .take(max_len)
        .collect()
}
```

---

### **P2 HIGH Findings**

#### **1. Race Condition in `spawn_business_aggregator` (Phase 26)**
**File:** `business_aggregator.rs`
**Function:** `spawn_business_aggregator` (line ~100)
**Description:**
The `SqlitePool` is moved into the async task without `Arc<Mutex<>>`, but SQLite connections are not thread-safe. While `SqlitePool` manages connections internally, concurrent writes could cause `SQLITE_BUSY` errors.

**Fix:**
Use `Arc<SqlitePool>` and ensure the pool is configured for concurrent access:
```rust
pub fn spawn_business_aggregator(pool: Arc<SqlitePool>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
            if let Err(e) = aggregate_daily_revenue(&pool, Utc::now().date_naive()).await {
                tracing::warn!(error = %e, "Revenue aggregation failed");
            }
        }
    });
}
```

---

#### **2. Deadlock Risk in `flush_buffer` (Phase 251)**
**File:** `telemetry_writer.rs`
**Function:** `flush_buffer` (line ~200)
**Description:**
The function holds a mutable reference to `buffer` while awaiting the database transaction. If another task tries to access `buffer` (e.g., during a flush), it will deadlock.

**Fix:**
Clone the buffer before the transaction:
```rust
async fn flush_buffer(pool: &SqlitePool, buffer: &mut Vec<TelemetryFrame>) {
    if buffer.is_empty() {
        return;
    }
    let frames = std::mem::take(buffer); // Replace buffer with empty vec
    let result = async {
        let mut tx = pool.begin().await?;
        for frame in frames.iter() {
            // ... insert logic ...
        }
        tx.commit().await?;
        Ok(())
    }.await;
    // Handle result...
}
```

---

#### **3. Silent Failure in `run_hourly_aggregation` (Phase 3)**
**File:** `telemetry_writer.rs`
**Function:** `run_hourly_aggregation` (line ~400)
**Description:**
The function swallows errors (e.g., DB connection failures) without logging the metric name or error details. This could hide critical failures.

**Fix:**
Log errors with context:
```rust
if let Err(e) = sqlx::query(&query).execute(pool).await {
    tracing::error!(
        "Hourly aggregation failed for metric '{}': {}",
        metric, e
    );
}
```

---

#### **4. Missing Error Handling in `store_extended_telemetry` (Phase 3)**
**File:** `telemetry_writer.rs`
**Function:** `store_extended_telemetry` (line ~500)
**Description:**
The function spawns a task but doesn’t handle the `JoinError` if the task panics. A panic in the task (e.g., due to invalid data) would go unnoticed.

**Fix:**
Use `unwrap_or_else` to log panics:
```rust
tokio::spawn(async move {
    if let Err(e) = sqlx::query(...).execute(&pool).await {
        tracing::warn!("Failed to store hardware telemetry: {}", e);
    }
}).unwrap_or_else(|e| {
    tracing::error!("Hardware telemetry task panicked: {}", e);
});
```

---

#### **5. Insecure Defaults in `init_telemetry_db` (Phase 251)**
**File:** `telemetry_writer.rs`
**Function:** `init_telemetry_db` (line ~50)
**Description:**
The function enables `WAL` mode but doesn’t set:
1. **`PRAGMA foreign_keys = ON`**: Could lead to referential integrity violations.
2. **`PRAGMA busy_timeout`**: Default is 0, causing immediate `SQLITE_BUSY` errors under contention.

**Fix:**
Add missing pragmas:
```rust
sqlx::query("PRAGMA foreign_keys = ON").execute(&pool).await?;
sqlx::query("PRAGMA busy_timeout = 5000").execute(&pool).await?;
```

---

### **P3 MEDIUM Findings**

#### **1. Suboptimal Pattern in `aggregate_daily_revenue` (Phase 26)**
**File:** `business_aggregator.rs`
**Function:** `aggregate_daily_revenue` (line ~20)
**Description:**
The function makes 3 separate queries to the same table (`billing_sessions`) with overlapping filters. This is inefficient and could be combined into a single query.

**Fix:**
Combine queries:
```rust
let (gaming, sessions): (i64, i64) = sqlx::query_as(
    "SELECT COALESCE(SUM(wallet_debit_paise), 0), COUNT(*)
     FROM billing_sessions
     WHERE DATE(ended_at) = ?1 AND status IN ('completed', 'ended_early')"
)
.bind(&date_str)
.fetch_one(pool)
.await
.unwrap_or((0, 0));
```

---

#### **2. Missing Validation in `recommend_parts` (Phase 23)**
**File:** `spare_parts.rs`
**Function:** `recommend_parts` (line ~50)
**Description:**
The function doesn’t validate `rul_data` for:
1. Negative `rul_hours`.
2. Duplicate `(pod_id, component)` pairs.

**Fix:**
Add validation:
```rust
for (pod_id, component, rul_hours) in rul_data {
    if *rul_hours < 0.0 {
        tracing::warn!("Negative RUL for pod {} component {}", pod_id, component);
        continue;
    }
    // ... rest of logic ...
}
```

---

#### **3. Performance Bottleneck in `get_metric_trend` (Phase 3)**
**File:** `telemetry_writer.rs`
**Function:** `get_metric_trend` (line ~600)
**Description:**
The function fetches all daily aggregates for a metric, even if only a few are needed. This is inefficient for large datasets.

**Fix:**
Limit the query to the window:
```rust
let rows = sqlx::query_as::<_, (String, f64)>(
    "SELECT period_start, avg_val FROM telemetry_aggregates
     WHERE pod_id = ?1 AND metric_name = ?2 AND period_hours = 24
       AND period_start >= ?3
     ORDER BY period_start ASC LIMIT ?4"
)
.bind(pod_id)
.bind(metric_name)
.bind(&cutoff)
.bind(window_days as i32)
.fetch_all(pool)
.await?;
```

---

### **Summary of Severity Counts**
| Severity | Count |
|----------|-------|
| P1 Critical | 5 |
| P2 High | 5 |
| P3 Medium | 3 |

### **Recommendations**
1. **Immediate Action**: Fix all P1 issues (SQL injection, integer overflow, TOCTOU, etc.).
2. **High Priority**: Address P2 issues (race conditions, deadlocks, silent failures).
3. **Tooling**: Integrate `sqlx-cli` for compile-time SQL validation and `clippy` with `pedantic` lints.
4. **Testing**: Add property-based tests for integer arithmetic and fuzz testing for SQL inputs.