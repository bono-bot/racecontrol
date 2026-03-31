# Full Business Logic Audit Report - v29.0 Meshed Intelligence

## 1. FINANCIAL CORRECTNESS

### P1: EBITDA Calculation Integer Overflow Risk
**File:** `maintenance_store.rs` (EBITDA calculation)
**Function:** `get_ebitda_summary`
**Description:** The EBITDA calculation (`total_revenue - total_expenses`) uses `i64` but doesn't check for integer overflow/underflow when summing large revenue/expense values. With 8 pods and high traffic, daily revenue could exceed `i64::MAX` (9.2 quintillion paise = ₹92 billion).
**Fix:**
```rust
let day_rev = m.revenue_gaming_paise
    .checked_add(m.revenue_cafe_paise)?
    .checked_add(m.revenue_other_paise)?;
let day_exp = m.expense_rent_paise
    .checked_add(m.expense_utilities_paise)?
    .checked_add(m.expense_salaries_paise)?
    .checked_add(m.expense_maintenance_paise)?
    .checked_add(m.expense_other_paise)?;
let day_ebitda = day_rev.checked_sub(day_exp)?;
```

### P1: Payroll Calculation Precision Loss
**File:** `maintenance_store.rs` (Payroll calculation)
**Function:** `calculate_monthly_payroll`
**Description:** The payroll calculation converts `hours_worked` (f64) to minutes with rounding, but the rounding happens before the integer conversion. This can accumulate rounding errors across many employees.
**Fix:**
```rust
let worked_minutes = (row.total_hours * 60.0 + 0.5).floor() as i64; // Banker's rounding
```

### P2: EBITDA Best/Worst Day Logic Flaw
**File:** `maintenance_store.rs`
**Function:** `get_ebitda_summary`
**Description:** The best/worst day logic is inverted - it updates when `day_ebitda <= *best_val` (should be `>=`) and `day_ebitda >= *worst_val` (should be `<=`). This causes incorrect best/worst day identification.
**Fix:**
```rust
match &best_day {
    Some((_, best_val)) if day_ebitda > *best_val => best_day = Some((date_str.clone(), day_ebitda)),
    None => best_day = Some((date_str.clone(), day_ebitda)),
    _ => {}
}
match &worst_day {
    Some((_, worst_val)) if day_ebitda < *worst_val => worst_day = Some((date_str, day_ebitda)),
    None => worst_day = Some((date_str, day_ebitda)),
    _ => {}
}
```

## 2. ENUM CONSISTENCY

### P1: Enum Serialization Inconsistency
**File:** Multiple (enum serialization paths)
**Description:** Enums are serialized to JSON strings with quotes (e.g., `"Critical"`), but when reading back, the code sometimes uses `replace('"', "")` and sometimes doesn't. This creates inconsistency in storage format and potential deserialization failures.
**Fix:** Standardize on:
1. Always serialize with quotes (default `serde_json::to_string`)
2. Never strip quotes when deserializing
3. Update all deserialization paths to use `serde_json::from_str` without quote stripping

### P2: Missing Enum Validation
**File:** `maintenance_store.rs` (enum deserialization)
**Description:** Enum deserialization uses `unwrap_or` with default values, masking potential data corruption. A "Medium" severity event could be stored as "Mediium" and silently converted to Medium.
**Fix:** Add validation layer:
```rust
fn deserialize_severity(s: &str) -> anyhow::Result<Severity> {
    serde_json::from_str(s).map_err(|e| anyhow!("Invalid severity: {} - {}", s, e))
}
```

## 3. ESCALATION LOGIC

### P2: Escalation Logic Edge Case
**File:** `escalation.rs`
**Function:** `determine_escalation`
**Description:** The logic doesn't handle the case where `auto_fix_attempts > 2` but severity is "Low". This could lead to manager escalation for trivial issues.
**Fix:**
```rust
if severity == "Critical" {
    return EscalationTier::Manager;
}
if auto_fix_attempts == 0 && !is_recurring && severity == "Low" {
    return EscalationTier::Auto;
}
if auto_fix_attempts <= 2 && severity != "High" {
    return EscalationTier::Technician;
}
EscalationTier::Manager
```

### P3: Time Zone Handling in Alerts
**File:** `alert_engine.rs`
**Description:** The alert engine uses UTC for time comparisons but doesn't account for business hours in IST. A "peak hour" alert at 16:00 UTC (21:30 IST) would be outside actual peak hours.
**Fix:** Convert all time comparisons to IST:
```rust
let now_ist = Utc::now() + chrono::Duration::hours(5) + chrono::Duration::minutes(30);
let hour = now_ist.hour();
```

## 4. DATA FLOW

### P1: Revenue Aggregation Data Loss
**File:** `business_aggregator.rs`
**Function:** `aggregate_daily_revenue`
**Description:** The aggregation uses `unwrap_or(0)` for DB queries, silently masking database errors. A connection failure would result in zero revenue being recorded.
**Fix:**
```rust
let gaming: i64 = sqlx::query_scalar(...).await?;
let cafe: i64 = sqlx::query_scalar(...).await?;
```

### P2: EBITDA Chain Break
**File:** `business_aggregator.rs`
**Description:** The business aggregator only updates `revenue_gaming_paise` and `revenue_cafe_paise`, but the EBITDA calculation includes `revenue_other_paise`. This creates a disconnect where other revenue is never updated.
**Fix:** Either:
1. Add other revenue sources to aggregation, or
2. Document that `revenue_other_paise` must be manually updated

## 5. EDGE CASES

### P1: Zero Revenue Day Handling
**File:** `alert_engine.rs`
**Function:** `check_business_alerts`
**Description:** The revenue drop alert calculation (`today_rev * 10 < avg_rev * 7`) can produce false positives when `avg_rev` is very small (e.g., first week of operation). A ₹100 average with ₹70 today would trigger an alert.
**Fix:**
```rust
if avg_rev > 1000 && today_rev * 10 < avg_rev * 7 { // Minimum threshold ₹10
    // ...
}
```

### P2: Negative EBITDA Handling
**File:** `maintenance_store.rs`
**Function:** `get_ebitda_summary`
**Description:** Negative EBITDA values are stored but not handled specially in the alerting system. A negative EBITDA day should trigger immediate alerts.
**Fix:** Add negative EBITDA check in alert engine:
```rust
if ebitda < 0 {
    alerts.push(BusinessAlert {
        alert_type: "NegativeEbitdaAlert".into(),
        severity: "Critical".into(),
        message: format!("Negative EBITDA ₹{:.0} on {}", ebitda as f64 / 100.0, date_str),
        // ...
    });
}
```

### P3: Month Boundary Handling
**File:** `maintenance_store.rs`
**Function:** `calculate_monthly_payroll`
**Description:** The payroll calculation uses string comparison for dates (`a.date >= ?1 AND a.date < ?2`), which works for YYYY-MM-DD format but is fragile. A corrupted date string could include records from wrong months.
**Fix:** Use proper date comparison:
```rust
let rows = sqlx::query_as::<_, PayrollRow>(
    "SELECT ... FROM attendance_records a
     WHERE date(a.date) >= date(?1) AND date(a.date) < date(?2)",
)
```

## 6. PRICING SAFETY

### P1: Dynamic Pricing Integer Overflow
**File:** `dynamic_pricing.rs`
**Function:** `recommend_pricing`
**Description:** The pricing calculation uses `checked_mul` and `checked_div`, but the intermediate `change_bp` calculation could overflow for very large prices (e.g., ₹100,000 * 1500 basis points = 1.5 billion).
**Fix:**
```rust
let change_bp = (change_pct * 100.0).round().clamp(-10000, 10000) as i64;
```

### P2: Pricing Approval Bypass
**File:** `pricing_bridge.rs`
**Function:** `apply_approved_pricing`
**Description:** The pricing application doesn't verify that the proposed price is still valid (e.g., current price hasn't changed since proposal). This could apply stale prices.
**Fix:** Add validation:
```rust
for (id, proposed_price) in &approved {
    let current: i64 = sqlx::query_scalar(
        "SELECT current_price_paise FROM pricing_proposals WHERE id = ?1"
    ).bind(id).fetch_one(&mut *tx).await?;

    if current != *proposed_price {
        tracing::warn!(target: LOG_TARGET, id = %id, "Price changed since proposal - skipping");
        continue;
    }
    // ...
}
```

## 7. FEEDBACK ACCURACY

### P1: Precision/Recall Calculation Flaw
**File:** `feedback_loop.rs`
**Function:** `calculate_feedback_metrics`
**Description:** The code equates precision and recall (`recall: precision`), which is incorrect. Recall requires tracking missed predictions (false negatives), which isn't implemented.
**Fix:**
```rust
let recall = if (accurate + false_negatives) > 0 {
    accurate as f64 / (accurate + false_negatives) as f64
} else {
    0.0
};
```

### P2: Feedback Metrics Time Zone Issue
**File:** `feedback_loop.rs`
**Function:** `calculate_feedback_metrics`
**Description:** The feedback metrics use UTC for the date range calculation, but business metrics are recorded in IST. This could exclude the first/last day of the period.
**Fix:** Use IST for date comparisons:
```rust
let since = (Utc::now() + chrono::Duration::hours(5) + chrono::Duration::minutes(30))
    .date_naive()
    .and_hms_opt(0, 0, 0)
    .unwrap()
    - chrono::Duration::days(days as i64);
```

## Summary of Critical Findings (P1)

1. **EBITDA Integer Overflow Risk** - Could cause silent data corruption in financial calculations
2. **Payroll Precision Loss** - Rounding errors could accumulate to significant amounts
3. **EBITDA Best/Worst Day Logic Flaw** - Incorrect financial reporting
4. **Revenue Aggregation Data Loss** - Database errors could be silently ignored
5. **Dynamic Pricing Integer Overflow** - Could produce negative or wildly incorrect prices
6. **Zero Revenue Day False Alerts** - Could trigger unnecessary alerts during low-traffic periods

## Recommendations

1. Implement comprehensive integer arithmetic safety checks across all financial calculations
2. Standardize enum serialization/deserialization with proper validation
3. Add explicit edge case handling for all financial metrics (negative values, zero values, etc.)
4. Implement proper time zone handling throughout the system
5. Add validation for all data flows between components
6. Implement proper precision/recall tracking in the feedback system
7. Add unit tests for all edge cases identified in this audit