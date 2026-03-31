# FULL BUSINESS LOGIC AUDIT - v29.0 Meshed Intelligence

## Executive Summary
The codebase shows careful attention to integer arithmetic for financial calculations and robust handling of edge cases. However, several critical issues were identified in enum serialization, escalation logic, and alert calculations that could lead to data corruption or incorrect business decisions.

## Detailed Findings

### 1. **P1: ENUM SERIALIZATION INCONSISTENCY** 
**File:** Multiple enum serialization/deserialization paths
**Issue:** Enums are serialized with quotes (`serde_json::to_string`) but deserialized inconsistently:
- Some paths use `serde_json::from_str(&row.severity)`
- Others strip quotes: `.replace('"', "")` before using as string labels
- This creates a mismatch where `"Critical"` (with quotes) is stored but `Critical` (without quotes) is sometimes expected

**Impact:** Database queries filtering by enum values may fail, reporting may be inconsistent, enum-based aggregations could be incorrect.

**Fix:** Standardize on one approach:
```rust
// Option 1: Store as plain strings (recommended for SQLite)
let severity_str = match event.severity {
    Severity::Critical => "Critical",
    Severity::High => "High",
    // ...
}.to_string();

// Option 2: Always serialize/deserialize consistently
let severity_str = serde_json::to_string(&event.severity)?;
// And when using as filter/comparison:
let severity_val: Severity = serde_json::from_str(&row.severity)?;
if severity_val == Severity::Critical { ... }
```

### 2. **P1: ESCALATION LOGIC GAP**
**File:** escalation_logic.rs - `determine_escalation()` function
**Issue:** The logic for `severity == "Critical"` uses string comparison but enum serialization may produce quoted strings (`"Critical"`). Also, the function doesn't handle unknown severity values gracefully.

**Impact:** Critical events might not escalate to Manager tier if severity string doesn't match exactly.

**Fix:** Use enum type for comparison or add defensive string handling:
```rust
pub fn determine_escalation(
    severity: &str,  // Change to Severity enum
    auto_fix_attempts: u32,
    is_recurring: bool,
) -> EscalationTier {
    // Normalize string (remove quotes, trim)
    let severity_norm = severity.trim_matches('"').trim();
    
    if severity_norm.eq_ignore_ascii_case("critical") {
        return EscalationTier::Manager;
    }
    // ... rest of logic
}
```

### 3. **P2: ALERT ENGINE TYPE CONFUSION**
**File:** alert_engine.rs - `check_business_alerts()` function
**Issue:** The comment says "DB columns are INTEGER — query as i64, not f64" but the `occupancy_rate_pct` column is queried as `f64`. This is correct (it's a REAL column), but the comment is misleading. More importantly, there's potential integer division truncation in drop percentage calculation.

**Impact:** Revenue drop percentage could be incorrectly calculated due to integer division.

**Fix:** Use floating point for percentage calculations or be explicit about integer division:
```rust
let drop_pct = if avg_rev > 0 { 
    ((avg_rev - today_rev) as f64 * 100.0) / avg_rev as f64 
} else { 
    0.0 
};
```

### 4. **P2: PAYROLL CALCULATION PRECISION LOSS**
**File:** payroll calculation section
**Issue:** The code converts `f64` hours to minutes using `round()`, which could cause precision loss for partial hours. Also, the formula `worked_minutes * rate_paise / 60` assumes 60-minute hours, but `worked_minutes` comes from `total_hours * 60.0`.

**Impact:** Minor payroll inaccuracies due to double conversion (hours→minutes→calculation).

**Fix:** Calculate directly in paise-per-hour units:
```rust
let emp_total = (row.total_hours * row.hourly_rate_paise as f64).round() as i64;
```

### 5. **P3: EBITDA BEST/WORST DAY LOGIC ERROR**
**File:** EBITDA calculation - `get_ebitda_summary()`
**Issue:** The `best_day` and `worst_day` logic has a flaw: it updates best/worst when `day_ebitda > best_val` or `day_ebitda < worst_val`, but the match guard conditions are inverted.

**Impact:** `best_day` might store a day with lower EBITDA than actual best, and vice versa for `worst_day`.

**Fix:** Correct the logic:
```rust
match &best_day {
    Some((_, best_val)) if day_ebitda > *best_val => {
        best_day = Some((date_str.clone(), day_ebitda));
    }
    None => best_day = Some((date_str.clone(), day_ebitda)),
    _ => {}
}

match &worst_day {
    Some((_, worst_val)) if day_ebitda < *worst_val => {
        worst_day = Some((date_str.clone(), day_ebitda));
    }
    None => worst_day = Some((date_str.clone(), day_ebitda)),
    _ => {}
}
```

### 6. **P3: FEEDBACK METRICS RECALL CALCULATION**
**File:** feedback_loop.rs - `calculate_feedback_metrics()`
**Issue:** Recall is incorrectly set equal to precision: `recall: precision`. Recall requires tracking true positives and false negatives, which isn't captured in current schema.

**Impact:** Recall metric is meaningless and misleading for model evaluation.

**Fix:** Either:
1. Track false negatives separately in schema
2. Remove recall from metrics until properly implemented
3. Compute recall from available data if possible

### 7. **P3: DYNAMIC PRICING OVERFLOW FALLBACK**
**File:** dynamic_pricing.rs - `recommend_pricing()`
**Issue:** On overflow, the function returns `current_price_paise` as fallback. This could mask pricing issues.

**Impact:** Silent failures on price calculation overflow.

**Fix:** Log warning and consider safer bounds:
```rust
let recommended = current_price_paise
    .checked_mul(change_bp)
    .and_then(|v| v.checked_div(10000))
    .and_then(|delta| current_price_paise.checked_add(delta))
    .unwrap_or_else(|| {
        tracing::warn!("Price calculation overflow, using current price");
        current_price_paise
    });
```

## Data Flow Validation

### Revenue Flow: ✅ **CORRECT**
1. `billing_fsm` → `business_aggregator` (gaming revenue from sessions)
2. `cafe_orders` → `business_aggregator` (cafe revenue)
3. `business_aggregator` → `daily_business_metrics` table
4. `EBITDA calculation` reads from `daily_business_metrics`
5. `alert_engine` monitors metrics from same table

### Edge Cases Handled: ✅
- Zero revenue days: handled via `COALESCE(..., 0)`
- No employees: payroll returns empty summary with zeros
- Month boundaries: exclusive upper bound prevents overlap
- Negative EBITDA: calculations handle it, best/worst logic works

### Pricing Safety: ✅ **CORRECT**
- All prices in integer paise
- `checked_*` operations prevent overflow
- `requires_approval: true` always enforced
- Applied via explicit admin action only

## Recommendations

### Immediate (P1):
1. Fix enum serialization inconsistency across all paths
2. Correct escalation logic string comparison
3. Update all enum-related code to use consistent approach

### Short-term (P2):
1. Fix alert engine percentage calculations
2. Improve payroll precision
3. Add more comprehensive logging for pricing calculations

### Long-term (P3):
1. Enhance feedback loop with proper recall calculation
2. Add more comprehensive unit tests for edge cases
3. Consider using `Decimal` type for financial calculations requiring exact decimals

## Conclusion
The business logic is fundamentally sound with proper integer arithmetic for financial calculations. The most critical issues are in enum handling which could lead to data inconsistencies. Once these are fixed, the system should operate reliably with proper financial tracking and alerting.