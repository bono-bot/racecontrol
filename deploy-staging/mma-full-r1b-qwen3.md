### **Audit Findings: Racing Point eSports v29.0 Rust Codebase**

#### **P1 CRITICAL**
---

**1. Data Corruption via Existing Metrics Query Failure**  
**File:** `Phase 26: Auto-aggregate revenue`  
**Location:** `aggregate_daily_revenue` function  
**Description:** When fetching existing business metrics with `maintenance_store::query_business_metrics`, any error (transient or permanent) results in discarding the existing data and using a default `DailyBusinessMetrics`. This leads to:  
- Overwriting valid `peak_occupancy_pct`, `expenses`, etc., with zeros.  
- Loss of critical historical data (e.g., rental costs, salaries) if the query fails temporarily.  

**Fix:**  
Handle the error explicitly instead of ignoring it:
```rust
let existing = maintenance_store::query_business_metrics(pool, &date_str, &date_str).await?;
let base = existing
    .as_ref()
    .and_then(|vec| vec.first())
    .cloned()
    .unwrap_or(DailyBusinessMetrics {/* defaults */});
```

---

**2. Integer Overflow in Pricing Calculation**  
**File:** `Phase 19: Dynamic pricing`  
**Location:** `recommend_pricing` function  
**Description:**  
```rust
let recommended = current_price_paise + (current_price_paise * change_bp / 10000);
```  
Multiplying `current_price_paise` (i64) by `change_bp` (i64) can overflow if the product exceeds `i64::MAX`, leading to:  
- Negative values.  
- Panic in debug builds.  
- Incorrect pricing recommendations.  

**Fix:**  
Use **checked arithmetic** and return an error or panic:
```rust
let recommended = current_price_paise
    .checked_add(
        (current_price_paise * change_bp)
            .checked_div(10000)
            .unwrap_or(0)
    )
    .unwrap_or(i64::MAX);
```

---

**3. Partial Updates in `apply_approved_pricing`**  
**File:** `Phase 32: Dynamic pricing write-through`  
**Location:** `apply_approved_pricing` function  
**Description:** The loop updates pricing proposals one-by-one. If any update fails:  
- The database is left in a **partially updated state** (some approved prices applied, others not).  
- The error is propagated correctly (`?`), but partial writes are not rolled back, risking inconsistency.  

**Fix:**  
Use a transaction to **enforce atomicity**:
```rust
let tx = pool.begin().await?;
for (id, _) in &approved {
    sqlx::query!(...)
        .execute(&mut *tx)
        .await?;
}
tx.commit().await?;
```

---

**4. Command Injection via Relative Paths**  
**File:** `Predictive Maintenance`  
**Location:** `check_gpu_temp`, `auto_cleanup_old_logs`, and `collect_network_latency`  
**Description:**  
```rust
Command::new("nvidia-smi").args([...]);
Command::new("powershell").args([...]);
```  
Using relative paths (e.g., `"nvidia-smi"`) risks executing malicious binaries if:  
- The search path is manipulated.  
- A malicious binary is placed in a directory searched before the legitimate one.  

**Fix:**  
Use **absolute paths** for system commands:
```rust
Command::new("C:\\Windows\\System32\\nvidia-smi").args([...]);
Command::new("C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe").args([...]);
```

---

**5. Overflow in `recommend_parts` Cost Calculation**  
**File:** `Phase 23: Predictive spare parts`  
**Location:** `recommend_parts` function  
**Description:**  
```rust
estimated_cost_paise: component_cost_paise(component) * critical_pods.len() as i64,
```  
Multiplying a fixed cost (`i64`) by `len()` (which could be large) risks overflow if the product exceeds `i64::MAX`, causing:  
- Negative costs.  
- Panic in debug builds.  

**Fix:**  
Use **checked multiplication** and handle overflow:
```rust
estimated_cost_paise: 
    component_cost_paise(component)
    .checked_mul(critical_pods.len() as i64)
    .unwrap_or(i64::MAX),
```

---

#### **P2 HIGH**
---

**6. Silent Failures in SQL Queries**  
**File:** `Phase 26: Auto-aggregate revenue`  
**Location:** `aggregate_daily_revenue`  
**Description:**  
- `.fetch_one(pool).await.unwrap_or(0)` silences query errors (e.g., syntax errors, connection issues).  
- Silent data loss: If a query fails (e.g., invalid date), `0` is used, making revenue appear zero for the day.  

**Fix:**  
Handle errors explicitly:
```rust
let gaming = sqlx::query_scalar(...).fetch_one(pool).await?;
```

---

**7. Silent Failures in `forecast_week`**  
**File:** `Phase 18: Demand forecasting`  
**Location:** `forecast_week`  
**Description:**  
```rust
.unwrap_or((0.0, 0.0))
```  
If the historical data query fails, the function proceeds with `avg_occ = 0.0` and `avg_sessions = 0.0`, leading to:  
- Incorrect demand forecasts.  
- Silently discarded error.  

**Fix:**  
Return the error and avoid overwriting valid data:
```rust
let row = sqlx::query_as(...).fetch_optional(pool).await?
    .ok_or_else(|| anyhow::anyhow!("No historical data"))?;
```

---

#### **P3 MEDIUM**
---

**8. Missing Validation for Telemetry Input**  
**File:** `telemetry_aggregates`  
**Location:** `run_hourly_aggregation`  
**Description:**  
```rust
format!("INSERT OR REPLACE INTO telemetry_aggregates ...")
```  
The SQL query uses `INSERT OR REPLACE`, which overwrites existing rows with the same composite key. If the key is malformed (e.g., invalid `pod_id` or `metric_name`), it may overwrite unintended data.  

**Fix:**  
Validate keys before generating the query or use `INSERT INTO ... ON CONFLICT UPDATE`.

---

**9. Performance: Excessive Memory Usage in Telemetry Writer**  
**File:** `nightly cleanup`  
**Location:** `run_nightly_cleanup`  
**Description:**  
If the cleanup loop fails repeatedly, the telemetry buffer grows indefinitely, risking memory exhaustion.  

**Fix:**  
Cap the buffer size with a hard limit and implement proper retry limits.

---

### **Summary of Actions**
| Severity | Fix Description | Files Affected |
|---------|------------------|----------------|
| **P1** | Handle query errors, use checked math, enforce transaction atomicity, use absolute paths | All modules |
| **P2** | Replace `.unwrap_or` with explicit error handling | `Phase 26`, `forecast_week` |
| **P3** | Validate telemetry keys, cap telemetry buffer | `telemetry_aggregates`, `nightly cleanup` |

All critical and high findings must be addressed to prevent data loss, security breaches, and runtime panics.