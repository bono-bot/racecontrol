# FULL AUDIT — v29.0 Meshed Intelligence (Bundle A)

**Auditor:** MiMo-v2-pro (Senior Rust/Systems Security Auditor)
**Scope:** 9 source files — models, store, anomaly engine, self-healing, feedback, business alerts, escalation, data collector, ollama client
**Framework:** Rust/Axum, SQLite (sqlx), Windows 11, 8-pod racing venue

---

## P1 CRITICAL

### P1-1: `check_business_alerts` — Runtime panic from i64→f64 unwrap mismatch (3 sites)

**File:** `business_alerts.rs:check_business_alerts`

The `daily_business_metrics` schema stores monetary values as `INTEGER` (i64). `sqlx::query_scalar` returns `i64`, but `.unwrap_or(0.0)` demands `f64`. **This panics at runtime every time the alert checker fires (every 30 min).**

```rust
// BUG — query returns i64, unwrap expects f64 → panic
let today_rev: f64 = sqlx::query_scalar(
    "SELECT COALESCE(revenue_gaming_paise + revenue_cafe_paise, 0) FROM daily_business_metrics WHERE date = ?1"
).bind(&today).fetch_one(pool).await.unwrap_or(0.0);

let avg_rev: f64 = sqlx::query_scalar(
    "SELECT COALESCE(AVG(revenue_gaming_paise + revenue_cafe_paise), 0) FROM daily_business_metrics WHERE date >= date('now', '-7 days')"
).fetch_one(pool).await.unwrap_or(0.0);
```

Same bug on the occupancy and maintenance-cost queries later in the function:
```rust
let occ: f64 = sqlx::query_scalar(...).fetch_one(pool).await.unwrap_or(0.0);  // i64 → f64 panic
let maint_cost: f64 = sqlx::query_scalar(...).fetch_one(pool).await.unwrap_or(0.0);  // same
let month_rev: f64 = sqlx::query_scalar(...).fetch_one(pool).await.unwrap_or(0.0);  // same
```

**Fix** — query as `i64`, convert explicitly:

```rust
let today_rev_paise: i64 = sqlx::query_scalar(
    "SELECT COALESCE(revenue_gaming_paise + revenue_cafe_paise, 0) FROM daily_business_metrics WHERE date = ?1"
).bind(&today).fetch_one(pool).await.unwrap_or(0_i64);

let avg_rev_paise: i64 = sqlx::query_scalar(
    "SELECT COALESCE(AVG(revenue_gaming_paise + revenue_cafe_paise), 0) FROM daily_business_metrics WHERE date >= date('now', '-7 days')"
).fetch_one(pool).await.unwrap_or(0_i64);

if avg_rev_paise > 0 && today_rev_paise < (avg_rev_paise * 7) / 10 {
    alerts.push(BusinessAlert {
        alert_type: "RevenueDropAlert".into(),
        severity: "High".into(),
        message: format!(
            "Revenue today ₹{:.0} is {:.0}% below 7-day average ₹{:.0}",
            today_rev_paise as f64 / 100.0,
            (1.0 - today_rev_paise as f64 / avg_rev_paise as f64) * 100.0,
            avg_rev_paise as f64 / 100.0
        ),
        channel: AlertChannel::Both,
        timestamp: Utc::now().to_rfc3339(),
        value: today_rev_paise as f64,
        threshold: (avg_rev_paise * 7) / 10 as f64,
    });
}
```

Repeat for `occ`, `maint_cost`, `month_rev` — all must be `i64` then cast.

---

### P1-2: `spawn_anomaly_scanner` — First scan fires immediately (no startup delay)

**File:** `anomaly_engine.rs:spawn_anomaly_scanner_with_healing`

```rust
let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
// Skip the immediate first tick — let telemetry accumulate.
interval.tick().await;  // ← COMMENT SAYS SKIP, CODE DOES NOT SKIP

loop {
    interval.tick().await;  // fires at t=0
    let alerts = run_anomaly_scan(&pool, &state_clone, &rules).await;
```

`tokio::time::interval` fires its **first tick immediately**. The code calls `interval.tick().await` before the loop, then calls it again inside the loop — the first scan still fires at t=0, just one extra tick later. The "skip" comment is wrong; telemetry has zero accumulation time.

**Fix:**

```rust
let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
// Consume the immediate first tick — no scan yet
interval.tick().await;

loop {
    interval.tick().await;  // fires at t=60
    let alerts = run_anomaly_scan(&pool, &state_clone, &rules).await;
```

Also add `set_missed_tick_behavior(Skip)` — without it, if a scan takes >60s the next tick fires immediately, potentially creating a backlog of scans all hitting SQLite.

---

### P1-3: `run_anomaly_scan` — Write lock held across DB query (performance cliff)

**File:** `anomaly_engine.rs:run_anomaly_scan`

```rust
let mut guard = state.write().await;  // ← EXCLUSIVE LOCK ACQUIRED HERE

for row in &rows {
    for rule in rules {
        // ...evaluate rules, check cooldown via guard.last_alert...
    }
}

// Store alerts for API access
if !alerts.is_empty() {
    guard.recent_alerts.extend(alerts.clone());
    // ...
}
// guard dropped here — held for the entire loop
```

The write lock is acquired **before** the nested loop and held for its full duration (10 rules × N pods). Any concurrent call to `recent_alerts()` (API endpoint) blocks. Worse, if the scan is delayed by SQLite, the lock is held longer.

**Fix** — collect mutations, apply lock briefly:

```rust
let mut new_alerts = Vec::new();
let mut alert_times: Vec<((String, String), DateTime<Utc>)> = Vec::new();
let mut violations_to_clear: Vec<(String, String)> = Vec::new();

// Phase 1: evaluate rules (no lock)
for row in &rows {
    for rule in rules {
        let key = (row.pod_id.clone(), rule.name.clone());
        // ... evaluation logic, accumulate into temp structures ...
    }
}

// Phase 2: apply state mutations under brief lock
{
    let mut guard = state.write().await;
    for (key, time) in &alert_times {
        guard.last_alert.insert(key.clone(), *time);
        guard.first_violation.remove(key);
    }
    for key in &violations_to_clear {
        guard.first_violation.remove(key);
    }
    if !new_alerts.is_empty() {
        guard.recent_alerts.extend(new_alerts.clone());
        let len = guard.recent_alerts.len();
        if len > 200 {
            guard.recent_alerts.drain(..len - 200);
        }
    }
} // lock released

new_alerts
```

---

## P2 HIGH

### P2-1: SQL injection via string interpolation in `auto_assign_task`

**File:** `maintenance_store.rs:auto_assign_task`

```rust
sqlx::query(
    "UPDATE maintenance_tasks SET assigned_to = ?1, status = 'Assigned' WHERE id = ?2",
)
```

And the WHERE clause uses interpolated status strings:

```rust
"SELECT COUNT(*) FROM maintenance_tasks \
 WHERE assigned_to = ?1 AND status NOT IN ('Completed', 'Failed', 'Cancelled')"
```

While the status values are hardcoded, they are built from `serde_json::to_string().replace('"', "")` elsewhere. If the enum serialization ever changes (e.g., a rename attribute is added), the SQL breaks silently. **All queries must use parameterized binds per the project rules.**

**Fix** — bind status values as parameters:

```rust
// For the UPDATE
let assigned_status = "Assigned";
sqlx::query(
    "UPDATE maintenance_tasks SET assigned_to = ?1, status = ?2 WHERE id = ?3",
)
.bind(emp_id)
.bind(assigned_status)
.bind(task_id)
.execute(pool)
.await?;

// For the load count — use a parameterized NOT IN via multiple conditions
let load: i64 = sqlx::query_scalar(
    "SELECT COUNT(*) FROM maintenance_tasks \
     WHERE assigned_to = ?1 \
     AND status != 'Completed' AND status != 'Failed' AND status != 'Cancelled'",
)
.bind(emp_id)
.fetch_one(pool)
.await
.unwrap_or(0);
```

---

### P2-2: `check_rul_thresholds` — Invalid pod_id defaults to 0, writes phantom task

**File:** `data_collector.rs:check_rul_thresholds`

```rust
let pod_num = pod_id_str.replace("pod_", "").parse::<i64>().unwrap_or(0);
```

If telemetry contains a malformed pod identifier (e.g., `"pod_XYZ"`, `""`, `"gateway"`), this silently creates a `maintenance_tasks` row with `pod_id = 0` — no pod 0 exists, so this is a phantom task that will never be assigned or completed.

**Fix** — reject invalid pod IDs:

```rust
let pod_num = pod_id_str
    .strip_prefix("pod_")
    .and_then(|s| s.parse::<i64>().ok())
    .filter(|&p| (1..=8).contains(&p));

let pod_num = match pod_num {
    Some(p) => p,
    None => {
        tracing::warn!(target: LOG_TARGET, pod = %pod_id_str, "Invalid pod_id in RUL check — skipping");
        continue;
    }
};
```

---

### P2-3: `self_healing.rs:apply_action` — Unvalidated pod_id inserts phantom availability entries

**File:** `self_healing.rs` (called from `anomaly_engine.rs:spawn_anomaly_scanner_with_healing`)

```rust
let pod_num: u8 = alert.pod_id
    .trim_start_matches("pod_")
    .trim_start_matches("pod")
    .parse()
    .unwrap_or(0);
if pod_num > 0 {
    // applies action...
}
```

The trim logic is order-dependent and fragile. A string `"podpod_1"` → after `trim_start_matches("pod_")` → `"podpod_1"` (no match) → after `trim_start_matches("pod")` → `"pod_1"` → parse fails → `0`. But a string `"pod11"` → `trim_start_matches("pod")` → `"11"` → parse succeeds → `pod_num = 11`, which exceeds the 8-pod range.

**Fix** — strict validation:

```rust
let pod_num: u8 = alert.pod_id
    .strip_prefix("pod_")
    .or_else(|| alert.pod_id.strip_prefix("pod"))
    .and_then(|s| s.parse::<u8>().ok())
    .filter(|&p| (1..=8).contains(&p));

if let Some(pod_num) = pod_num {
    let action = recommend_action(&alert.rule_name, &alert.severity, pod_num);
    apply_action(avail_map, &action).await;
}
```

---

### P2-4: `insert_event` — Unsafe `as` cast for `u32 → i64` (customers_affected, downtime_minutes)

**File:** `maintenance_store.rs:insert_event`

```rust
.bind(event.customers_affected.map(|c| c as i64))
.bind(event.downtime_minutes.map(|d| d as i64))
```

While `u32 → i64` widening is safe, the project rules say "All integer casts from DB MUST use try_from, not `as`". This sets a bad precedent — auditors or future maintainers will see `as` and assume all casts are safe.

**Fix** — use `Into` which is infallible for widening:

```rust
.bind(event.customers_affected.map(i64::from))
.bind(event.downtime_minutes.map(i64::from))
```

Same pattern in `insert_task`:
```rust
.bind(task.priority as i64)  // u8 → i64
```
**Fix:**
```rust
.bind(i64::from(task.priority))
```

---

### P2-5: `spawn_alert_checker` / `spawn_data_collector` — Missing `set_missed_tick_behavior`

**File:** `business_alerts.rs:spawn_alert_checker`

```rust
let mut interval = tokio::time::interval(std::time::Duration::from_secs(1800));
interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
```

This one **does** set `Skip` ✓, but the first tick fires immediately — same issue as P1-2. The alert checker sleeps 5 min then starts ticking, but the first tick after the sleep fires immediately, not after 30 min.

**Fix:**

```rust
tokio::spawn(async move {
    tokio::time::sleep(std::time::Duration::from_secs(300)).await;
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(1800));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    interval.tick().await;  // consume immediate first tick
    loop {
        interval.tick().await;  // fires 30 min after sleep completes
        // ...
    }
});
```

---

### P2-6: `calculate_monthly_payroll` — f64 accumulation in hours_worked

**File:** `maintenance_store.rs:calculate_monthly_payroll`

```rust
let mut total_hours = 0.0f64;
// ...
for row in rows {
    let worked_minutes = (row.total_hours * 60.0).round() as i64;
    let emp_total = worked_minutes.max(0) * row.hourly_rate_paise / 60;
    total_hours += row.total_hours;  // ← f64 accumulation
```

The per-employee wage calculation correctly uses integer arithmetic (`worked_minutes * rate / 60`) ✓, but `total_hours` accumulates as f64, which drifts over many employees.

**Fix** — accumulate in whole minutes as i64:

```rust
let mut total_minutes: i64 = 0;
for row in rows {
    let worked_minutes = (row.total_hours * 60.0).round() as i64;
    let emp_total = worked_minutes.max(0) * row.hourly_rate_paise / 60;
    total_minutes += worked_minutes;
    // ...
}
let total_hours = total_minutes as f64 / 60.0;
```

---

### P2-7: `Severity` derives `PartialOrd` — order depends on declaration position

**File:** `maintenance_models.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
pub enum Severity {
    Critical,  // 0
    High,      // 1
    Medium,    // 2
    Low,       // 3
}
```

`PartialOrd` on enums uses ordinal position. `Critical < High < Medium < Low` semantically, but ordinal gives `Critical == 0 < High == 1`. This works coincidentally, but any reordering of variants silently breaks priority logic.

**Fix** — explicit ordering:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
}

impl PartialOrd for Severity {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Severity {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_u8().cmp(&other.as_u8())
    }
}

impl Severity {
    fn as_u8(&self) -> u8 {
        match self {
            Severity::Critical => 0,
            Severity::High => 1,
            Severity::Medium => 2,
            Severity::Low => 3,
        }
    }
}
```

---

### P2-8: `query_events` / `query_tasks` — Filtering in Rust instead of SQL

**File:** `maintenance_store.rs:query_events`

```rust
let rows = sqlx::query_as::<_, EventRow>(
    "SELECT ... FROM maintenance_events ORDER BY detected_at DESC LIMIT ?1"
)
.bind(limit as i64)
.fetch_all(pool).await?;

for row in rows {
    let evt = row_to_event(row)?;
    if let Some(pid) = pod_id {
        if evt.pod_id != Some(pid) { continue; }
    }
    if let Some(ref s) = since {
        if evt.detected_at < *s { continue; }
    }
    events.push(evt);
}
```

Fetches `limit` rows unconditionally, then discards non-matching ones. If `pod_id=1` has 5 events in the last 1000 rows, you get 5 results regardless of `limit`. This means the API returns fewer results than requested, and the caller has no way to paginate correctly.

**Fix** — build parameterized WHERE clause:

```rust
pub async fn query_events(
    pool: &SqlitePool,
    pod_id: Option<u8>,
    since: Option<DateTime<Utc>>,
    limit: u32,
) -> anyhow::Result<Vec<MaintenanceEvent>> {
    let mut query = String::from(
        "SELECT ... FROM maintenance_events WHERE 1=1"
    );
    let mut binds: Vec<String> = Vec::new();

    if let Some(pid) = pod_id {
        query.push_str(" AND pod_id = ?");
        binds.push(pid.to_string());
    }
    if let Some(ref s) = since {
        query.push_str(" AND detected_at >= ?");
        binds.push(s.to_rfc3339());
    }
    query.push_str(" ORDER BY detected_at DESC LIMIT ?");
    binds.push((limit as i64).to_string());

    // Use sqlx::QueryBuilder or prepared statement approach
    // (shown conceptually — in practice use query_builder! macro)
}
```

---

### P2-9: `check_rul_thresholds` — LIKE pattern could match wrong components

**File:** `data_collector.rs:check_rul_thresholds`

```rust
let existing: i64 = sqlx::query_scalar(
    "SELECT COUNT(*) FROM maintenance_tasks WHERE pod_id = ?1 AND component LIKE ?2 AND status NOT IN (...)"
)
.bind(pod_num)
.bind(format!("%{}%", component))
```

If `component = "Software"`, this matches `"SoftwareUpdate"`, `"GameSoftware"`, etc. Not currently a problem because component values are from a fixed enum, but the pattern is fragile.

**Fix** — match on the exact JSON-serialized value:

```rust
let component_json = format!("\"{}\"", component);
let existing: i64 = sqlx::query_scalar(
    "SELECT COUNT(*) FROM maintenance_tasks WHERE pod_id = ?1 AND component = ?2 AND status NOT IN ('Completed', 'Failed', 'Cancelled')"
)
.bind(pod_num)
.bind(&component_json)
```

---

## P3 MEDIUM (selected notable items)

### P3-1: `is_peak_hours()` — Hardcoded IST offset, not timezone-aware

**File:** `anomaly_engine.rs:is_peak_hours`

```rust
let now = Utc::now() + chrono::Duration::hours(5) + chrono::Duration::minutes(30);
```

Uses manual offset instead of `chrono_tz` or `chrono::FixedOffset`. During DST transitions or if the venue changes timezone, this silently breaks.

### P3-2: `ollama_client.rs` — Hardcoded IP address, no config

**File:** `ollama_client.rs`

```rust
const OLLAMA_URL: &str = "http://192.168.31.27:11434/api/generate";
```

Hardcoded private IP. If James's machine changes IP, the entire AI diagnosis system breaks without any config override.

### P3-3: `insert_employee` — Weak UUID validation

**File:** `maintenance_store.rs:insert_employee`

```rust
if id_str.is_empty() || id_str == "00000000-0000-0000-0000-000000000000" {
    anyhow::bail!("...");
}
```

Only rejects all-zeros UUID. A UUID like `"not-a-uuid"` passes this check and panics at `Uuid::parse_str` later. Should use `Uuid::parse_str` first.

### P3-4: `check_rul_thresholds` — Uses `LIKE` with `'%Completed%'` instead of equality on enum status

**File:** `data_collector.rs`

The status values are JSON-serialized enum variants (unquoted after `replace('"', "")`). Using `LIKE '%Completed%'` is unnecessarily broad.

---

## SUMMARY TABLE

| # | Sev | File | Issue |
|---|-----|------|-------|
| P1-1 | **CRITICAL** | `business_alerts.rs` | 5× i64→f64 unwrap mismatch → runtime panic every 30 min |
| P1-2 | **CRITICAL** | `anomaly_engine.rs` | `interval.tick()` not skipped → anomaly scan fires at t=0 with zero telemetry |
| P1-3 | **CRITICAL** | `anomaly_engine.rs` | Write lock held across nested DB-mapped loop → API blocks during scan |
| P2-1 | **HIGH** | `maintenance_store.rs` | Status strings interpolated in SQL (3 sites) — must use binds |
| P2-2 | **HIGH** | `data_collector.rs` | Invalid pod_id defaults to 0 → phantom maintenance tasks |
| P2-3 | **HIGH** | `self_healing.rs` | Pod ID parsing fragile, allows out-of-range IDs into availability map |
| P2-4 | **HIGH** | `maintenance_store.rs` | `as` casts violate project rule; use `i64::from()` |
| P2-5 | **HIGH** | `business_alerts.rs` | Alert checker first tick fires immediately (same interval bug) |
| P2-6 | **HIGH** | `maintenance_store.rs` | `total_hours` accumulates as f64 in payroll calc |
| P2-7 | **HIGH** | `maintenance_models.rs` | `Severity` partial ord depends on declaration order |
| P2-8 | **HIGH** | `maintenance_store.rs` | In-Rust filtering returns wrong result counts for API pagination |
| P2-9 | **MEDIUM** | `data_collector.rs` | LIKE `%Software%` matches unintended components |
| P3-1 | MEDIUM | `anomaly_engine.rs` | Hardcoded IST offset |
| P3-2 | MEDIUM | `ollama_client.rs` | Hardcoded IP |
| P3-3 | MEDIUM | `maintenance_store.rs` | Weak UUID validation in insert_employee |

**Critical path fix:** P1-1 is a **guaranteed panic** in production — the `spawn_alert_checker` background task crashes every 30 minutes, silently killing business alert monitoring. Fix this first.