# **FULL BUSINESS LOGIC AUDIT: v29.0 Meshed Intelligence**
*Auditor: Senior Business Logic Auditor*
*Scope: All provided code snippets*

---

## **1. FINANCIAL CORRECTNESS**

### **Finding 1.1 - PAYROLL CALCULATION ROUNDING ERROR**
**P1 | `maintenance_store.rs:calculate_monthly_payroll`**
**Description:** Payroll uses `(row.total_hours * 60.0).round() as i64` to convert hours to minutes. However, `total_hours` is `f64` from `SUM(hours_worked)`. If `hours_worked` values have fractional minutes (e.g., 8.1666... hours = 8h 10m), the multiplication and rounding may produce off-by-one errors. More critically, the comment says "convert back to whole minutes" but `hours_worked` is stored as `f64` (minutes/60), implying minutes precision was lost at storage time.

**Fix:** Store attendance in minutes (`INTEGER`) not fractional hours. If keeping `f64`, use `(total_hours * 60.0 + 0.5).floor() as i64` for banker's rounding, or better: `((total_hours * 60000.0).round() / 1000.0)` to preserve milliminute precision then round.

---

### **Finding 1.2 - EBITDA AVERAGE CAN TRUNCATE PENNIES**
**P2 | `maintenance_store.rs:get_ebitda_summary`**
**Description:** `avg_daily = ebitda / days as i64` performs integer division, discarding fractional paise. Over 30 days, a daily EBITDA of ₹1.01 (101 paise) would sum to ₹30.30 (3030 paise), but average = 3030/30 = 101 paise (₹1.01) - correct here. But if ebitda = 100 paise over 3 days, average = 33 paise, losing 1 paise. This isn't wrong per se (you can't have fractional paise in daily summary), but the lost paise should be tracked or the field documented as "truncated average".

**Fix:** Either (a) return average as `f64` and let UI round, or (b) add a `remainder_paise: i64` field for the lost paise from integer division.

---

### **Finding 1.3 - DYNAMIC PRICING CAN GO TO ZERO OR NEGATIVE**
**P1 | `dynamic_pricing.rs:recommend_pricing`**
**Description:** If `current_price_paise` is small (e.g., 100 paise = ₹1) and `change_pct = -15.0`, `delta = 100 * -1500 / 10000 = -15`. `recommended = 100 - 15 = 85` (fine). But if `current_price_paise = 5` (5 paise), `delta = 5 * -1500 / 10000 = 0` (integer division truncates toward zero). **However**, if `change_pct = -100.0` (100% discount, possible if `forecast_occupancy_pct` is extremely low), `recommended = 0`. Zero price is business-dangerous (free service). Negative is prevented by `checked_add` but zero is allowed.

**Fix:** Add `recommended.max(MINIMUM_PRICE_PAISE)` where `MINIMUM_PRICE_PAISE` is business-defined (e.g., 100 paise = ₹1). Also guard against `change_pct` calculation errors.

---

### **Finding 1.4 - ALERT ENGINE REVENUE DROP DETECTION USES INTEGER COMPARISON**
**P2 | `alert_engine.rs:check_business_alerts`**
**Description:** `today_rev * 10 < avg_rev * 7` checks if `today_rev < 0.7 * avg_rev`. This integer math is clever but can overflow if `avg_rev` is very large (> i64::MAX/7). With `i64::MAX ≈ 9.2e18`, and revenue in paise, max revenue before overflow = 9.2e18/7 ≈ 1.3e18 paise = ₹13 billion daily - unlikely but possible in aggregate metrics.

**Fix:** Use `today_rev < (avg_rev * 7) / 10` or checked arithmetic. The current order (`today_rev * 10 < avg_rev * 7`) avoids some overflow since `today_rev <= avg_rev` in drop scenario, but still risky.

---

## **2. ENUM CONSISTENCY**

### **Finding 2.1 - INCONSISTENT QUOTE HANDLING IN ENUM SERIALIZATION**
**P1 | Multiple locations**
**Description:** Write path (`insert_event`) uses `serde_json::to_string(&event.severity)` which produces `"\"Critical\""` (JSON string with quotes). Read path (`row_to_event`) uses `serde_json::from_str(&row.severity)` expecting JSON. **But** in aggregation (`calculate_kpis`), the code does:
```rust
let sev: Severity = serde_json::from_str(&row.severity).unwrap_or(Severity::Medium);
let sev_label = serde_json::to_string(&sev).unwrap_or_default().replace('"', "");
```
This double conversion is wasteful and error-prone. If a developer manually inserts data without quotes, `serde_json::from_str` fails and defaults to `Medium`, silently corrupting data.

**Fix:** Standardize: either (a) store enums as plain strings without JSON quotes (change `insert_event` to use `format!("{:?}", event.severity)` or a custom `to_string()`), or (b) keep JSON but use a helper `fn severity_to_db(s: &Severity) -> String` that ensures consistency.

---

### **Finding 2.2 - TASK STATUS SERIALIZATION INCONSISTENCY**
**P2 | `maintenance_store.rs:insert_task` and `query_tasks`**
**Description:** `insert_task` does `serde_json::to_string(&task.status)?.replace('"', "")` - removing quotes. But `row_to_task` uses `serde_json::from_str(&status_json)` which **requires** quotes. If `status_json` is `"Open"` (with quotes), it works. But if stored without quotes (`Open`), `serde_json::from_str` fails.

**Fix:** Either always store with quotes (remove `.replace('"', "")`) or implement `FromStr` for `TaskStatus` and use `status_json.parse()`.

---

## **3. ESCALATION LOGIC**

### **Finding 3.1 - CRITICAL SEVERITY ALWAYS GOES TO MANAGER, EVEN FOR AUTO-FIXABLE**
**P2 | `escalation_logic.rs:determine_escalation`**
**Description:** The first check returns `Manager` for any `severity == "Critical"`, regardless of `auto_fix_attempts` or `is_recurring`. A critical-but-trivially-auto-fixable issue (e.g., "Critical: Fan speed below threshold" with known auto-remediation) still escalates to Manager. This could cause alert fatigue.

**Fix:** Consider: `if severity == "Critical" && (auto_fix_attempts > 0 || is_recurring) { Manager } else if severity == "Critical" { Technician }`

---

### **Finding 3.2 - ESCALATION LOGIC DOESN'T HANDLE UNKNOWN SEVERITY**
**P3 | `escalation_logic.rs:determine_escalation`**
**Description:** If `severity` is not "Critical", "High", "Medium", or "Low" (e.g., "Info" or typo), the logic falls through: first `if` fails, second `if` checks `severity != "High"` which is true, so it returns `Auto` for unknown severities. This is dangerous.

**Fix:** Add a match/default: `let severity = Severity::from_str(severity).unwrap_or(Severity::Medium);` at the start, or explicit handling of unknown strings to `Technician`.

---

## **4. DATA FLOW**

### **Finding 4.1 - BUSINESS AGGREGATOR DOESN'T UPDATE EXPENSES**
**P1 | `business_aggregator.rs:aggregate_daily_revenue`**
**Description:** The aggregator creates a `DailyBusinessMetrics` with revenue from billing/cafe but **preserves existing expense data** from the DB. However, if expenses change (e.g., salary payments, maintenance costs recorded elsewhere), the aggregator doesn't recompute them. The EBITDA calculation will use stale expense data.

**Fix:** The aggregator should either (a) also pull expense data from source tables (payroll, maintenance costs) or (b) have a separate expense aggregator that runs in tandem.

---

### **Finding 4.2 - ALERT ENGINE REVENUE QUERY IGNORES `revenue_other_paise`**
**P2 | `alert_engine.rs:check_business_alerts`**
**Description:** Revenue drop check uses `revenue_gaming_paise + revenue_cafe_paise` but ignores `revenue_other_paise`. If "other" revenue is significant, a drop there won't trigger alerts.

**Fix:** Use `revenue_gaming_paise + revenue_cafe_paise + revenue_other_paise` for total revenue comparison.

---

## **5. EDGE CASES**

### **Finding 5.1 - ZERO REVENUE DAY CAUSES DIVISION BY ZERO IN ALERT**
**P2 | `alert_engine.rs:check_business_alerts`**
**Description:** If `avg_rev == 0` (e.g., first day of operation, or 7-day window has no data), the condition `avg_rev > 0` prevents division, but `drop_pct` calculation uses `((avg_rev - today_rev) * 100) / avg_rev`. This is guarded by `if avg_rev > 0` but the alert's `threshold: avg_rev as f64 * 0.7` would be 0.0, which is confusing.

**Fix:** Skip alert entirely if `avg_rev <= 0`. Also handle `month_rev == 0` in maintenance cost alert.

---

### **Finding 5.2 - NO EMPLOYEES SCENARIO IN PAYROLL**
**P2 | `maintenance_store.rs:calculate_monthly_payroll`**
**Description:** If `is_active = 0` for all employees or table is empty, `rows` will be empty. `total_hours = 0.0`, `total_paise = 0`, `by_employee = []`. This is correct but the `PayrollSummary` with zero values might be misinterpreted as "no data" vs "zero payroll". 

**Fix:** Add a `has_data: bool` field or return `Option<PayrollSummary>`.

---

### **Finding 5.3 - MONTH BOUNDARY IN PAYROLL QUERY**
**P3 | `maintenance_store.rs:calculate_monthly_payroll`**
**Description:** The query uses `a.date >= ?1 AND a.date < ?2` with `next_month_start`. This correctly excludes the next month. However, if an employee works a night shift ending at 00:30 on the 1st of next month, the attendance record's `date` field (if stored as `YYYY-MM-DD`) would be the next month's date, so it's excluded from current month. This is correct but worth documenting.

**Fix:** None needed, but add comment: "Attendance is attributed to the date the shift ends."

---

## **6. PRICING SAFETY**

### **Finding 6.1 - PRICING CAN BE APPLIED WITHOUT VERIFICATION**
**P2 | `pricing_bridge.rs:apply_approved_pricing`**
**Description:** The function marks proposals as "applied" but the comment says `// actual price push to billing config would go here`. If this is stubbed, prices are marked applied but never actually set in billing. Also, there's no rollback if the push fails.

**Fix:** Implement actual billing config update. Use a transaction that rolls back status if update fails. Add `applied_price_paise` field to verify what was actually set.

---

### **Finding 6.2 - NO PRICE CHANGE VALIDATION**
**P3 | `dynamic_pricing.rs:recommend_pricing`**
**Description:** No validation that `recommended_price_paise` is within reasonable bounds (e.g., > 0, < 100000 paise = ₹1000). A miscalculation could produce absurd prices.

**Fix:** Add bounds checking: `recommended = recommended.clamp(MIN_PRICE, MAX_PRICE)`.

---

## **7. FEEDBACK ACCURACY**

### **Finding 7.1 - PRECISION/RECALL CALCULATION IS WRONG**
**P1 | `feedback_loop.rs:calculate_feedback_metrics`**
**Description:** The code calculates:
- `precision = accurate / total`
- `fpr = false_pos / total`
- `recall = precision` (simplified)

This is **incorrect**. In prediction systems:
- **Precision** = `true_positives / (true_positives + false_positives)`
- **Recall** = `true_positives / (true_positives + false_negatives)`
- **False Positive Rate** = `false_positives / (false_positives + true_negatives)`

The current code assumes all predictions are positive predictions, and treats `was_accurate = 0` as false positive. But we need to know: of the actual failures, how many did we predict? (recall) Of our predictions, how many were correct? (precision).

**Fix:** Need two more data points: (a) actual failures that were **not** predicted (false negatives), and (b) non-failures that were correctly not predicted (true negatives). Redesign schema to track prediction type (positive/negative) and actual outcome.

---

### **Finding 7.2 - FEEDBACK METRICS USE `created_at` NOT `predicted_at`**
**P3 | `feedback_loop.rs:calculate_feedback_metrics`**
**Description:** Metrics are filtered by `created_at > ?1` (when the outcome was recorded), not when the prediction was made or when the actual event occurred. A prediction made 30 days ago but recorded today would count toward today's metrics.

**Fix:** Use `predicted_at` for the period filter, with an optional lag window for predictions that haven't materialized yet.

---

## **SUMMARY OF FINDINGS**

| Priority | Count | Critical Issues |
|----------|-------|----------------|
| **P1** | 4 | Pricing can go to zero; Payroll rounding; EBITDA data flow; Precision/recall wrong |
| **P2** | 7 | Enum inconsistency; Escalation logic gaps; Alert revenue exclusion; Overflow risks |
| **P3** | 5 | Minor edge cases; Documentation needs; Redundant conversions |

**Most Critical Path:** The data flow from `billing_fsm` → `business_aggregator` → `EBITDA` → `alert_engine` has a gap: **expenses aren't updated by the aggregator**, making EBITDA potentially stale. This undermines financial reporting accuracy.

**Immediate Actions Required:**
1. Fix the precision/recall calculation - it's providing misleading metrics.
2. Ensure dynamic pricing can never produce zero or negative prices.
3. Fix enum serialization inconsistency before it causes data corruption.
4. Complete the pricing bridge implementation (currently a stub).

---
*Audit completed. All money handling uses i64 paise as required, but several precision, consistency, and logic gaps exist that could lead to financial reporting errors or operational issues.*