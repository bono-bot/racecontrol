Looking at this post-fix v29.0 code, I'll verify the Round 1 fixes and hunt for new bugs.

## Round 1 Fix Verification

**✅ All 13 Round 1 fixes are correctly implemented:**

1. **P1: Integer truncation** - Fixed with `try_from + clamp` in `row_to_event/row_to_task`
2. **P1: SQL GROUP BY** - Fixed with proper subquery in anomaly detection
3. **P1: f64 money** - Fixed with basis points integer math in pricing
4. **P1: Silent date fallback** - Fixed with `tracing::warn` on parse failures
5. **P1: SQL injection** - Fixed with explicit parameterized queries in `update_employee`
6. **P1: Payroll boundary** - Fixed with exclusive upper bound using `next_month_start`
7. **P1: Missing PendingValidation** - Fixed in KPI tasks_open query
8. **P2: Overnight shifts** - Fixed with +24h adjustment
9. **P2: Unbounded queries** - Fixed with LIMIT clauses
10. **P2: Prompt injection** - Fixed with `sanitize_for_prompt`
11. **P2: f64 attendance** - Fixed with integer minutes then /60
12. **P2: RUL infinity** - Fixed with < 0.001 guard
13. **P2: Enum serialization** - Verified consistent

## NEW BUGS FOUND

### **P1 BUGS**

**P1-NEW-1: SQL Injection in Business Metrics Query**
**File:** `maintenance_store.rs:505-518`
```rust
pub async fn query_business_metrics(
    pool: &SqlitePool,
    start_date: &str,  // ⚠️ Raw string interpolation
    end_date: &str,    // ⚠️ Raw string interpolation
) -> anyhow::Result<Vec<DailyBusinessMetrics>> {
    let rows = sqlx::query_as::<_, BusinessMetricsRow>(
        "SELECT date, revenue_gaming_paise, revenue_cafe_paise, revenue_other_paise,
                expense_rent_paise, expense_utilities_paise, expense_salaries_paise,
                expense_maintenance_paise, expense_other_paise,
                sessions_count, occupancy_rate_pct, peak_occupancy_pct
         FROM daily_business_metrics
         WHERE date >= ?1 AND date <= ?2  // ✅ Properly parameterized
         ORDER BY date ASC",
    )
    .bind(start_date)  // ✅ Safe
    .bind(end_date)    // ✅ Safe
```
**Status:** Actually safe - uses proper parameterized queries. False alarm.

**P1-NEW-1: Unsafe Enum Serialization Inconsistency**
**File:** `maintenance_store.rs:360`
```rust
fn row_to_task(row: TaskRow) -> anyhow::Result<MaintenanceTask> {
    // Wrap status string in quotes for JSON deserialization of enum variant
    let status_json = format!("\"{}\"", row.status);  // ⚠️ Manual quote wrapping
    // ...
    status: serde_json::from_str(&status_json)?,      // ⚠️ Brittle deserialization
```
**Fix:** Use consistent enum handling like other places:
```rust
let status: TaskStatus = serde_json::from_str(&format!("\"{}\"", row.status))
    .unwrap_or(TaskStatus::Open);  // Safe fallback
```

**P1-NEW-2: Integer Overflow in Payroll Calculation**
**File:** `maintenance_store.rs:948`
```rust
let emp_total = (row.total_hours * row.hourly_rate_paise as f64).round() as i64;
```
**Issue:** No overflow protection when casting f64 back to i64
**Fix:**
```rust
let emp_total = (row.total_hours * row.hourly_rate_paise as f64)
    .round()
    .min(i64::MAX as f64)
    .max(i64::MIN as f64) as i64;
```

**P1-NEW-3: Date Parse Fallback Without Logging**
**File:** `maintenance_store.rs:491`
```rust
let date = chrono::NaiveDate::parse_from_str(&row.date, "%Y-%m-%d")
    .unwrap_or_else(|_| chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap());
```
**Issue:** Silent fallback to year 2000 without warning log (inconsistent with Round 1 fixes)
**Fix:**
```rust
let date = match chrono::NaiveDate::parse_from_str(&row.date, "%Y-%m-%d") {
    Ok(d) => d,
    Err(e) => {
        tracing::warn!(
            "business_metrics: date parse failed for row '{}' — {}. Using 2000-01-01 fallback.",
            row.date, e
        );
        chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap()
    }
};
```

### **P2 BUGS**

**P2-NEW-1: Unbounded Query in RUL Data**
**File:** `anomaly_detection.rs:467`
```rust
let rows: Result<Vec<HwRow>, sqlx::Error> = sqlx::query(
    "SELECT pod_id, gpu_temp_celsius, cpu_temp_celsius, gpu_power_watts,
            // ... more fields
    FROM hardware_telemetry
    WHERE collected_at > ?1
      AND (pod_id, collected_at) IN (
          SELECT pod_id, MAX(collected_at)
          FROM hardware_telemetry
          WHERE collected_at > ?1
          GROUP BY pod_id
      )"
)  // ⚠️ Missing LIMIT clause
```
**Fix:** Add `LIMIT 50` to prevent unbounded results if many pods exist

**P2-NEW-2: Potential Division by Zero**
**File:** `spare_parts.rs:45-46`
```rust
let recommended = current_price_paise + (current_price_paise * change_bp / 10000);
```
**Issue:** If `current_price_paise` is 0, calculation works but semantically wrong
**Fix:** Add validation:
```rust
if current_price_paise <= 0 {
    return error or default pricing;
}
```

**P2-NEW-3: String Formatting Injection in Explanation**
**File:** `ai_diagnosis.rs:121`
```rust
input_summary: format!(
    "{} = {:.1} (threshold: {:.1}, sustained: {}min)",
    rule_name, metric_value, threshold, sustained_minutes  // ⚠️ rule_name not sanitized
),
```
**Issue:** `rule_name` could contain format specifiers if it comes from user input
**Fix:**
```rust
input_summary: format!(
    "{} = {:.1} (threshold: {:.1}, sustained: {}min)",
    sanitize_for_prompt(rule_name, 100), metric_value, threshold, sustained_minutes
),
```

## SUMMARY

**Round 1 fixes:** ✅ All 13 correctly implemented
**New P1 bugs:** 3 found
**New P2 bugs:** 3 found

The code quality has significantly improved post-Round 1. The new bugs are more subtle edge cases around overflow protection, logging consistency, and input sanitization - indicating the major structural issues were properly addressed in Round 1.