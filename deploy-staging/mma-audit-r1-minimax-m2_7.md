# Security & Quality Audit — Racing Point eSports v29.0

---

## P1 CRITICAL

### 1. Corrupt Telemetry Aggregation — Wrong Rows Selected Per Pod

**File:** `anomaly.rs` — `run_anomaly_scan`, line ~530  
**File:** `anomaly.rs` — `check_patterns`, line ~650

```sql
SELECT ... FROM hardware_telemetry
WHERE collected_at > ?1
GROUP BY pod_id
HAVING MAX(collected_at)
```

**Problem:** `GROUP BY pod_id` groups rows but `HAVING MAX(collected_at)` only filters groups — it does **not** guarantee the returned row values (gpu_temp, cpu_usage, etc.) come from the row with the maximum timestamp. In SQLite, the other selected columns come from an indeterminate row within each group. You may be checking today's GPU temp against last week's CPU usage.

**Fix:**

```rust
let rows: Result<Vec<HwRow>, sqlx::Error> = sqlx::query(
    "SELECT ht.*
     FROM hardware_telemetry ht
     INNER JOIN (
         SELECT pod_id, MAX(collected_at) AS max_ts
         FROM hardware_telemetry
         WHERE collected_at > ?1
         GROUP BY pod_id
     ) latest ON ht.pod_id = latest.pod_id AND ht.collected_at = latest.max_ts"
)
.bind(&cutoff)
.fetch_all(pool)
// ...
```

Apply the same fix to `check_patterns`.

---

### 2. Dynamic SQL String Manipulation in `update_employee`

**File:** persistence (maintenance persistence), line ~545

```rust
let numbered_sets: Vec<String> = sets
    .iter()
    .enumerate()
    .map(|(i, s)| s.replace('?', &format!("?{}", i + 1)))
    .collect();
let sql = format!(
    "UPDATE employees SET {} WHERE id = ?{}",
    numbered_sets.join(", "),
    id_idx
);
```

**Problem:** `.replace('?', ...)` replaces **every** `?` character in the SQL fragment string, not just the placeholder. A column name or value containing `?` (e.g., a regex pattern stored in a text field) would corrupt the query. Additionally, building SQL via string concatenation (`sets.join`) is a fragile anti-pattern that is one mistake away from injection if `sets` ever sources from user input.

**Fix — use a fixed array of optional SET clauses:**

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
    let mut query = sqlx::query("UPDATE employees SET ");
    let mut has_updates = false;

    if let Some(n) = name {
        query = query.bind(n);
        query = sqlx::query(&format!("UPDATE employees SET name = ?{} ", queryBare().0));
        // Safer: keep a counter
    }
    // ... or build a Vec<String> of "SET name = ?1" etc., then join safely
    // Omit the rest for brevity
}
```

Alternatively, build the query string safely with numbered placeholders from the start rather than post-hoc replacement.

---

### 3. Unchecked `.unwrap()` on Database Calls — Panic Risk

**File:** persistence, `get_summary` — `open_row` fetch

```rust
let open_row: (i64,) = sqlx::query_as(
    "SELECT COUNT(*) FROM maintenance_tasks WHERE status IN ('Open','Assigned','InProgress')",
)
.fetch_one(pool)
.await
.unwrap_or((0,));  // ← unwrap_or shields the panic but hides errors
```

**File:** persistence, `calculate_kpis` — multiple occurrences

```rust
.fetch_one(pool).await.unwrap_or((0,));
```

**Problem:** `.unwrap_or()` silences `sqlx::Error`. Network partitions, DB locks, or schema mismatches will be logged as "0 tasks" rather than propagated as errors. For a preventive maintenance system, silent failures can mask infrastructure problems.

**Fix:**

```rust
let open_row: (i64,) = sqlx::query_as(
    "SELECT COUNT(*) FROM maintenance_tasks WHERE status IN ('Open','Assigned','InProgress')",
)
.fetch_one(pool)
.await
.context("Failed to count open maintenance tasks")?;
```

---

## P2 HIGH

### 4. Floating-Point Arithmetic on Monetary Values

**File:** persistence, `calculate_monthly_payroll`, line ~720

```rust
let emp_total = (row.total_hours * row.hourly_rate_paise as f64).round() as i64;
```

**Problem:** Multiplies `f64` (hours, inherently imprecise) by `i64` (paise, exact). `0.1 + 0.2 ≠ 0.3` in floating-point. A 7.6-hour shift at 500 paise/hour could compute as `3799` or `3800` paise depending on rounding behavior.

**Fix — use integer arithmetic throughout:**

```rust
let total_paise = (row.total_hours * 1000.0).round() as i64; // convert hours to minutes
let emp_total = total_paise * row.hourly_rate_paise / 60;    // paise/min * min = paise
// Or store hours as a fixed-point integer (e.g., hundredths of an hour)
```

---

### 5. Overnight Shift Hours Return Zero

**File:** persistence, `record_attendance`, line ~610

```rust
let secs = (o - i).num_seconds();
if secs > 0 { secs as f64 / 3600.0 } else { 0.0 }
```

**Problem:** If an employee clocks in at 22:00 and clocks out at 06:00 (night shift), `o - i` produces a **negative** value, and the code returns `0.0` hours worked instead of `8.0`.

**Fix:**

```rust
let t_in = chrono::NaiveTime::parse_from_str(ci, "%H:%M").ok();
let t_out = chrono::NaiveTime::parse_from_str(co, "%H:%M").ok();
let hours = match (t_in, t_out) {
    (Some(i), Some(o)) => {
        let diff_secs = if o >= i {
            (o - i).num_seconds()
        } else {
            // Overnight shift: add 24 hours
            (chrono::Duration::days(1) - chrono::Duration::seconds((i - o).num_seconds()))
                .num_seconds()
        };
        diff_secs as f64 / 3600.0
    }
    _ => 0.0,
};
```

---

### 6. Potential Integer Overflow in Priority Scoring

**File:** `anomaly.rs`, `calculate_priority`, line ~770

```rust
let score = (base as f64 * peak_factor * session_factor).min(100.0);
```

**Problem:** While the final cast to `u8` is clamped via `.min(100.0)`, intermediate multiplication of `u8` values could theoretically overflow before the cast if `base` were ever raised above 100 (e.g., during future refactoring). The `.min()` is applied post-cast, not pre-cast.

**Fix — clamp at source:**

```rust
let base = match severity {
    "Critical" => 80u8,
    "High" => 60,
    "Medium" => 40,
    _ => 20,
};
let peak_bonus = if is_peak { 30 } else { 0 };
let session_bonus = if has_active_session { 20 } else { 0 };
let score = (base.saturating_add(peak_bonus).saturating_add(session_bonus)).min(100);
```

---

### 7. Division by Zero in `get_ebitda_summary`

**File:** persistence, `get_ebitda_summary`, line ~450

```rust
let avg_daily = if days > 0 { ebitda / days as i64 } else { 0 };
```

**Problem:** When `days == 0` (no data in range), returns `0`. This conflates "no data" with "zero EBITDA." Callers cannot distinguish between a valid zero-EBITDA day and an empty result set.

**Fix:**

```rust
if days == 0 {
    return Err(anyhow::anyhow!("No business metrics found in range {} to {}", start_date, end_date));
}
let avg_daily = ebitda / days as i64;
```

---

### 8. `Uuid::parse_str` Propagation Without Context

**File:** persistence, `row_to_event`, `row_to_task`, `row_to_attendance`

```rust
id: Uuid::parse_str(&row.id)?,
```

**Problem:** If a UUID in the database is corrupted (malformed, truncated, or non-UUID text), this returns a generic `ParseError` with no indication of which row or table caused the failure, making debugging difficult.

**Fix:**

```rust
id: Uuid::parse_str(&row.id)
    .with_context(|| format!("Invalid UUID '{}' in maintenance_events.id", row.id))?,
```

---

### 9. Missing Index on `maintenance_events.detected_at` — Query Perf

**File:** persistence, `init_maintenance_tables`

**Finding:** An index exists (`idx_maint_events_detected ON maintenance_events(detected_at)`), but `get_summary` and `calculate_kpis` both filter `WHERE detected_at >= ?1` on a **past timestamp**. With high event volume, this range scan is necessary but will be slow if the table grows large without periodic pruning.

**Recommendation:** Add a `WHERE detected_at >= date('now', '-N days')` clause in the application to bound the scan, plus a TTL cron job to purge old events (e.g., after 90 days).

---

### 10. `spawn_anomaly_scanner` — No Graceful Shutdown Signal

**File:** `anomaly.rs`, `spawn_anomaly_scanner`, line ~580

```rust
tokio::spawn(async move {
    // ...
    loop {
        interval.tick().await;
        let alerts = run_anomaly_scan(&pool, &state_clone, &rules).await;
        // ...
    }
});
```

**Problem:** The background task runs forever with no mechanism to stop it. On application shutdown (e.g., SIGTERM on Windows), the task will be dropped abruptly, potentially leaving in-flight DB transactions uncommitted.

**Fix — accept a cancellation signal:**

```rust
pub fn spawn_anomaly_scanner(
    pool: SqlitePool,
    shutdown: tokio::sync::broadcast::Receiver<()>,
) -> Arc<RwLock<EngineState>> {
    let state = Arc::new(RwLock::new(EngineState::new()));
    let state_clone = Arc::clone(&state);
    let rules = default_rules();

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        interval.tick().await; // skip first tick

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let alerts = run_anomaly_scan(&pool, &state_clone, &rules).await;
                    // ...
                }
                _ = shutdown.recv() => {
                    tracing::info!("Anomaly scanner shutting down");
                    break;
                }
            }
        }
    });

    state
}
```

---

## P3 MEDIUM

### 11. In-Memory Filtering Instead of SQL WHERE Clauses

**File:** persistence, `query_events`, line ~195 and `query_tasks`, line ~260

```rust
let rows = sqlx::query_as::<_, EventRow>(
    "SELECT ... FROM maintenance_events ORDER BY detected_at DESC LIMIT ?1"
)
.bind(limit as i64)
.fetch_all(pool)
.await?;

// Then filter in Rust:
if let Some(pid) = pod_id {
    if evt.pod_id != Some(pid) { continue; }
}
```

**Problem:** Fetches up to `limit` rows unconditionally, then discards those that don't match `pod_id` or `since`. Under load, this doubles or triples DB traffic unnecessarily.

**Fix — build the WHERE clause dynamically:**

```rust
let mut sql = String::from(
    "SELECT id, pod_id, event_type, severity, component, description,
            detected_at, resolved_at, resolution_method, source,
            correlation_id, revenue_impact_paise, customers_affected,
            downtime_minutes, cost_estimate_paise, assigned_staff_id, metadata
     FROM maintenance_events WHERE 1=1"
);
let mut binds: Vec<String> = Vec::new();

if pod_id.is_some() { sql.push_str(" AND pod_id = ?"); }
if since.is_some()  { sql.push_str(" AND detected_at >= ?"); }
sql.push_str(" ORDER BY detected_at DESC LIMIT ?");

// Build query with parameterized binds
let mut query = sqlx::query_as::<_, EventRow>(&sql);
if let Some(pid) = pod_id { query = query.bind(pid.map(|p| p as i64)); }
if let Some(s) = since { query = query.bind(s.to_rfc3339()); }
query = query.bind(limit as i64);
```

---

### 12. Enum Serialization Inconsistency

**Problem:** Enums are serialized via `serde_json::to_string()` (producing `"VariantName"` with quotes), then stored in SQLite TEXT columns. The `.replace('"', "")` stripping on read is fragile — any corruption to the stored value (e.g., already unquoted, or containing literal `"` in data) will fail `serde_json::from_str`.

**Recommendation:** Define a consistent serialization strategy. Either:
- Store as raw strings (`"SelfHealAttempted"` without outer quotes) using `#[serde(rename = "SelfHealAttempted")]` with a custom serializer, or
- Always store valid JSON (`"\"SelfHealAttempted\""` with outer quotes) and never strip quotes on read.

---

## Summary Table

| # | Category | File | Issue |
|---|----------|------|-------|
| 1 | P1 CRITICAL | `anomaly.rs:530,650` | Wrong row per pod from GROUP BY/HAVING |
| 2 | P1 CRITICAL | persistence | Dynamic SQL with `.replace('?')` |
| 3 | P1 CRITICAL | persistence | `.unwrap()` on DB calls masks errors |
| 4 | P2 HIGH | persistence:720 | f64 monetary arithmetic |
| 5 | P2 HIGH | persistence:610 | Overnight shifts = 0 hours |
| 6 | P2 HIGH | `anomaly.rs:770` | Potential overflow in priority calc |
| 7 | P2 HIGH | persistence:450 | Div/0 returns ambiguous zero |
| 8 | P2 HIGH | persistence | Uuid parse without row context |
| 9 | P2 HIGH | persistence | No TTL/purge on events table |
| 10 | P2 HIGH | `anomaly.rs:580` | No graceful shutdown |
| 11 | P3 MEDIUM | persistence | In-memory filter instead of SQL |
| 12 | P3 MEDIUM | All files | Inconsistent enum serialization |