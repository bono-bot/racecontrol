### Security and Systems Audit Findings for Racing Point eSports v29.0

---

#### **P1 CRITICAL: Incorrect SQL Query in Anomaly Detection**  
**File:** `anomaly_detection.rs:123`  
**Description:**  
The SQL query in `run_anomaly_scan` uses `GROUP BY pod_id HAVING MAX(collected_at)`, which incorrectly retrieves arbitrary or outdated telemetry data instead of the latest per-pod entries. This breaks core anomaly detection logic, leading to potential false negatives/positives.  

**Concrete Fix:**  
Use a subquery to fetch the latest `collected_at` timestamp per pod and join it back to select the full row:  
```rust
let rows: Result<Vec<HwRow>, sqlx::Error> = sqlx::query(
    "SELECT h.*
     FROM hardware_telemetry h
     INNER JOIN (
         SELECT pod_id, MAX(collected_at) AS max_collected
         FROM hardware_telemetry
         WHERE collected_at > ?1
         GROUP BY pod_id
     ) latest
     ON h.pod_id = latest.pod_id AND h.collected_at = latest.max_collected"
)
.bind(&cutoff)
.fetch_all(pool)
.await
.map(|rows| {
    rows.into_iter()
        .map(|r| HwRow { ... })
        .collect()
});
```

---

#### **P1 CRITICAL: Logic Error in KPI Calculation for Completed Tasks**  
**File:** `maintenance_kpi.rs:145`  
**Description:**  
The SQL query in `calculate_kpis` filters tasks with `status IN ('Completed', 'Verified')`, but `'Verified'` is **not a valid `TaskStatus` variant**. This causes incorrect `tasks_completed` counts (excludes actual completed tasks).  

**Concrete Fix:**  
Remove `'Verified'` from the query:  
```rust
let (tasks_completed,): (i64,) = sqlx::query_as(
    "SELECT COUNT(*) FROM maintenance_tasks \
     WHERE created_at >= ?1 AND status IN ('Completed')"
)
.bind(&since_str)
.fetch_one(pool)
.await
.unwrap_or((0,));
```

---

#### **P1 CRITICAL: Incorrect Conversion of Pod ID (i64 to u8)**  
**File:** `maintenance_models.rs:230` (`row_to_event`) and `maintenance_models.rs:290` (`row_to_task`)  
**Description:**  
Converting `i64` from SQLite to `u8` for `pod_id` can **wrap values > 255** (e.g., 256 → 0), leading to data corruption in maintenance records.  

**Concrete Fix:**  
Validate pod_id during conversion and use saturating conversion:  
```rust
pod_id: row.pod_id.map(|p| p as u8).filter(|&id| id >= 1 && id <= 8),
// Fallback to 0 or add error handling if invalid
```

---

#### **P2 HIGH: Race Condition in Pre-Maintenance Checks**  
**File:** `pre_maintenance.rs:45` (`run_pre_checks`)  
**Description:**  
Race condition between checking pod state and maintenance execution: if a pod disconnects after `run_pre_checks` but before maintenance starts, checks pass erroneously.  

**Concrete Fix:**  
Re-check pod state immediately before maintenance with a lock or atomic update:  
```rust
let pods = state.pods.read().await;
if let Some(pod) = pods.values().find(|p| p.number == pod_id as u32 && p.is_connected) {
    // Proceed
}
```

---

#### **P2 HIGH: Silent Failure in Status Filter for Tasks**  
**File:** `maintenance_tasks.rs:67` (`query_tasks`)  
**Description:**  
Tasks are filtered in Rust after SQL fetch, leading to **silent exclusion of invalid statuses** (e.g., if a task has a corrupted status string).  

**Concrete Fix:**  
Add error handling for status deserialization:  
```rust
let status: TaskStatus = match serde_json::from_str(&row.status) {
    Ok(s) => s,
    Err(e) => {
        tracing::warn!("Invalid status in task {}: {}", row.id, e);
        continue; // Skip invalid
    }
};
```

---

#### **P3 MEDIUM: Floating-Point Monetary Calculations**  
**File:** `dynamic_pricing.rs:30` (`recommend_pricing`)  
**Description:**  
Using `f64` for monetary calculations (e.g., price scaling) can **truncate values** due to floating-point imprecision (e.g., 199.999999 → 199 paise instead of 200).  

**Concrete Fix:**  
Use integer arithmetic for paise-only calculations:  
```rust
let change_amount = current_price_paise * change_pct as i64 / 100;
let recommended = current_price_paise + change_amount;
```

---

#### **P3 MEDIUM: Inefficient Dynamic Query in Task Filtering**  
**File:** `maintenance_tasks.rs:67` (`query_tasks`)  
**Description:**  
Filtering tasks in Rust instead of SQL reduces performance for large datasets.  

**Concrete Fix:**  
Use SQL for status filtering:  
```rust
let rows = sqlx::query_as::<_, TaskRow>(
    "SELECT ... FROM maintenance_tasks WHERE status = ?1 ORDER BY ...",
)
.bind(status_filter)
.fetch_all(pool)
.await?;
```

---

#### **P4 LOW: Inconsistent Enum Naming**  
**File:** `maintenance_models.rs` (various enum definitions)  
**Description:**  
Enums like `MaintenanceEventType` use mixed naming (e.g., `SelfHealAttempted` vs. PascalCase in JSON).  

**Concrete Fix:**  
Ensure consistent naming (e.g., `SelfHealAttempted` → `SelfHealAttempted`).  

---

**Final Note:**  
Prioritize fixing the **P1 issues** to ensure data integrity and core functionality. The anomaly detection and KPI calculation errors directly impact system reliability.