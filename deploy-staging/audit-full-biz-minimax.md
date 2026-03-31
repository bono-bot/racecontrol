# FULL BUSINESS LOGIC AUDIT — v29.0 Meshed Intelligence

---

## 1. FINANCIAL CORRECTNESS

### P1 — `feedback_loop.rs:feedback_metrics` — `total_predictions` includes NULL `was_accurate` rows, corrupting precision and FPR

**File/Function:** `feedback_loop.rs:calculate_feedback_metrics`

```rust
let total: i64 = sqlx::query_scalar(
    "SELECT COUNT(*) FROM prediction_outcomes WHERE created_at > ?1",  // ← counts ALL rows
)
let accurate: i64 = sqlx::query_scalar(
    "SELECT COUNT(*) FROM prediction_outcomes WHERE created_at > ?1 AND was_accurate = 1",  // ← excludes NULL
)
let false_pos: i64 = sqlx::query_scalar(
    "SELECT COUNT(*) FROM prediction_outcomes WHERE created_at > ?1 AND was_accurate = 0",  // ← excludes NULL
)
```

`insert_outcome` inserts `was_accurate.map(|b| b as i32)` — which produces `NULL` when `was_accurate` is `None` (unevaluated predictions). The `total` query counts every row including NULLs, but `accurate` and `false_pos` queries use `= 1`/`= 0` which exclude NULLs. So if any prediction is unevaluated:

```
total = 100 (including 20 NULLs)
accurate = 60, false_pos = 20
precision = 60/100 = 60%  ← understated (true precision is 75%)
fpr = 20/100 = 20%       ← overstated
```

**Fix:**
```rust
let total: i64 = sqlx::query_scalar(
    "SELECT COUNT(*) FROM prediction_outcomes WHERE created_at > ?1 AND was_accurate IS NOT NULL",
)
```

---

### P1 — `feedback_loop.rs:calculate_feedback_metrics` — `recall` is hardcoded equal to `precision`

**File/Function:** `feedback_loop.rs:calculate_feedback_metrics`

```rust
recall: precision, // simplified — need separate tracking of missed failures
```

Recall = `TP / (TP + FN)` (predictions that *should have* fired but didn't). Precision = `TP / (TP + FP)`. These are fundamentally different. Setting `recall = precision` will:
- Show 0% recall when there are missed failures but all fired predictions were correct (precision = 100%, recall = 0%)
- Hide systematic false negatives

**Fix:** Add a `missed_failures` column to `prediction_outcomes` and compute:
```rust
let missed: i64 = sqlx::query_scalar(
    "SELECT COUNT(*) FROM prediction_outcomes WHERE created_at > ?1 AND was_missed_failure = 1",
).bind(&since).fetch_one(pool).await?;
let recall = if (accurate + missed) > 0 {
    accurate as f64 / (accurate + missed) as f64
} else { 0.0 };
```

---

### P2 — `alert_engine.rs:check_business_alerts` — Integer division truncates alert percentages

**File/Function:** `alert_engine.rs:check_business_alerts`

```rust
let drop_pct = if avg_rev > 0 { ((avg_rev - today_rev) * 100) / avg_rev } else { 0 };
// …
message: format!("… {}% below …", drop_pct, …),
```

`avg_rev = 300000` paise (₹3000), `today_rev = 200000` paise (₹2000).
Actual drop = 33.33%. Computed: `(100000 * 100) / 300000 = 33` (integer div).
User sees "33% below" instead of "33.3% below". Minor but misleading for alerts.

**Fix:**
```rust
let drop_pct = if avg_rev > 0 {
    ((avg_rev - today_rev) as f64 * 100.0 / avg_rev as f64).round() as i32
} else { 0 };
// format with {:.1}
message: format!("… {:.1}% below …", drop_pct as f64, …),
```

Same issue in maintenance cost alert: `(maint_cost * 100) / month_rev` truncates.

---

### P2 — `biz_aggregator.rs:aggregate_daily_revenue` — `occupancy` can exceed 100%

**File/Function:** `biz_aggregator.rs:aggregate_daily_revenue`

```rust
let occupancy = if sessions > 0 {
    (sessions as f32 / (total_pods * operating_hours) * 100.0).min(100.0)
} else {
    0.0
};
```

The `.min(100.0)` is correctly applied. However, the formula itself assumes a maximum of `total_pods * operating_hours` sessions — but each pod can run multiple concurrent sessions if the system allows overlapping times (e.g., sessions start before previous ones end). The hardcoded ceiling of 8 pods × 12 hours = 96 is not enforced as a real limit.

More critically: `peak_occupancy_pct` is `occupancy.max(base.peak_occupancy_pct)`. If the formula underestimates real occupancy (because it ignores concurrent sessions), `peak_occupancy_pct` will be wrong, causing missed low-occupancy alerts.

**Fix:**
```rust
// Derive actual max from billing session data, not a hardcoded formula:
let max_possible_sessions: f32 = total_pods * operating_hours
    * 2.0; // conservative: 2 sessions/hour/pod average ceiling
let occupancy = (sessions as f32 / max_possible_sessions * 100.0).min(100.0);
```

---

## 2. ENUM CONSISTENCY

### P1 — `maintenance_store.rs` — `ResolutionMethod` with data serializes unpredictably

**File/Function:** `maintenance_store.rs:insert_event`

```rust
let resolution_str = event
    .resolution_method
    .as_ref()
    .map(|r| serde_json::to_string(r))
    .transpose()?;
```

`serde_json::to_string(&ResolutionMethod::AutoHealed("GPU fan replaced"))` produces:
```json
"{\"AutoHealed\":\"GPU fan replaced\"}"
```
The string comparison in the read path:
```rust
if let Ok(ResolutionMethod::AutoHealed(_)) = serde_json::from_str(rm) {
```
works by accident, but `by_type` aggregation does:
```rust
let type_label = serde_json::to_string(&etype).unwrap_or_default().replace('"', "");
// Result: "{AutoHealed:GPU fan replaced}"  ← includes data, inconsistent with unit variants
```

Unit variants like `SelfHealAttempted` serialize to `"SelfHealAttempted"` cleanly. Variants with data include it. Labels are inconsistent across enum population.

**Fix:** Use a dedicated `label()` method on each enum instead of JSON round-tripping:
```rust
impl ResolutionMethod {
    pub fn label(&self) -> &'static str {
        match self {
            ResolutionMethod::AutoHealed(_) => "AutoHealed",
            ResolutionMethod::ManualFix(_) => "ManualFix",
            // …
        }
    }
}
let resolution_str = event.resolution_method.as_ref().map(|r| r.label());
```

---

### P2 — `escalation.rs` — `severity` is `&str` instead of `Severity` enum throughout

**File/Function:** `escalation.rs:determine_escalation`

```rust
pub fn determine_escalation(
    severity: &str,  // ← string, not enum
    auto_fix_attempts: u32,
    is_recurring: bool,
) -> EscalationTier {
    if severity == "Critical" {  // ← string comparison
```

All callers pass string literals. Invalid strings (e.g., `"crit"`, `"CRITICAL"`, `"Medium "`) silently fall through to the default path. The `Severity` enum exists but is unused here.

**Fix:**
```rust
pub fn determine_escalation(
    severity: &Severity,
    auto_fix_attempts: u32,
    is_recurring: bool,
) -> EscalationTier {
    match severity {
        Severity::Critical => EscalationTier::Manager,
        // …
    }
}
```

---

## 3. ESCALATION LOGIC

### P1 — `escalation.rs:determine_escalation` — `High + >2 attempts` wrongly returns `Technician`

**File/Function:** `escalation.rs:determine_escalation`

```rust
if auto_fix_attempts <= 2 || severity == "High" {  // ← wrong: OR means High always gets Technician
    EscalationTier::Technician;
}
if auto_fix_attempts > 2 && severity != "High" {  // ← never reached for "High"
    EscalationTier::Manager;
}
```

Logic flow for `severity = "High", attempts = 3`:
1. `severity == "Critical"` → false
2. `auto_fix_attempts == 0 && !is_recurring && severity != "High"` → false (severity is "High")
3. `auto_fix_attempts <= 2 || severity == "High"` → true (severity is "High") → **`Technician`** ← wrong

Per the 3-tier escalation model, 3 failed auto-fix attempts on a High issue should escalate to Manager, not Technician.

**Fix:**
```rust
if severity == "High" && auto_fix_attempts <= 2 {
    EscalationTier::Technician
} else if auto_fix_attempts > 2 || severity == "Critical" {
    EscalationTier::Manager
} else {
    EscalationTier::Auto
}
```

---

### P2 — `escalation.rs` — Missing test coverage for critical interaction paths

**File/Function:** `escalation.rs:tests`

No tests for:
- `determine_escalation("Critical", 3, false)` → should be `Manager`
- `determine_escalation("High", 3, false)` → should be `Manager` (currently broken — see P1)
- `determine_escalation("High", 0, true)` → is recurring, should be `Technician` (tested indirectly)
- Empty/whitespace severity strings

**Fix:** Add explicit tests for the above cases.

---

## 4. DATA FLOW

### P2 — `biz_aggregator.rs` — `revenue_other_paise` is never populated and silently stays at 0

**File/Function:** `biz_aggregator.rs:aggregate_daily_revenue`

```rust
revenue_other_paise: base.revenue_other_paise,  // ← carries forward from existing row or 0
```

If there is no existing row (new day), `revenue_other_paise` is always 0. There is no query or source for "other" revenue. Any revenue category not gaming or cafe will permanently show as ₹0 in EBITDA reports. If other revenue exists (merchandise, sponsorships, etc.), it is permanently invisible.

**Fix:** Either add a query for other revenue sources, or document that `revenue_other_paise` must be manually inserted, or add a `TODO` comment with a severity marker.

---

### P3 — `biz_aggregator.rs` — Timezone inconsistency between aggregator and alert engine

**File/Function:** `biz_aggregator.rs:aggregate_daily_revenue` vs `alert_engine.rs:check_business_alerts`

- **Aggregator:** Uses `Utc::now().date_naive()` directly — UTC date
- **Alert engine:** Explicitly converts to IST (`+ chrono::Duration::minutes(330)`) for peak hour detection

The billing session `ended_at` timestamps are stored as UTC (standard practice). `DATE(ended_at)` extracts the UTC date. If the pod operates on IST business days:
- At 03:00–05:29 UTC (08:30–10:59 IST), `DATE(ended_at)` will show the *previous IST date* for sessions that "belong" to today's IST business.
- E.g., a session ending at 09:00 IST (03:30 UTC) is grouped under yesterday's UTC date.

This is a business-definition issue (UTC vs IST day boundary) rather than a code bug, but it means the EBITDA dashboard may show revenue attributed to the wrong calendar day relative to the pod's operating hours. The alert engine uses IST but the aggregator uses UTC.

**Fix:** If the pod uses IST business days, store `ended_at` in IST or add an IST offset to the date extraction:
```rust
let ended_at_ist = DateTime::parse_from_rfc3339(&row.ended_at)
    .map(|dt| dt.with_timezone(&FixedOffset::east(5 * 3600 + 30 * 60)))
    .ok();
```

---

## 5. EDGE CASES

### P2 — `biz_aggregator.rs` — `sessions_count` silently truncates to 0 on overflow

**File/Function:** `biz_aggregator.rs:aggregate_daily_revenue`

```rust
sessions_count: u32::try_from(sessions.max(0)).unwrap_or(0),
```

`sessions` is `i64` from `COUNT(*)`. `try_from` succeeds for any non-negative `i64` ≤ 4,294,967,295. `unwrap_or(0)` silently drops data if sessions exceed u32 max — this is astronomically unlikely but the silent failure is bad practice.

**Fix:**
```rust
sessions_count: u32::try_from(sessions).map_err(|_| {
    tracing::error!(sessions, "sessions_count overflowed u32");
    sessions
})?,
```

---

## 6. PRICING SAFETY

### P1 — `dynamic_pricing.rs:recommend_pricing` — Zero base price propagates to recommended price

**File/Function:** `dynamic_pricing.rs:recommend_pricing`

```rust
let recommended = current_price_paise
    .checked_mul(change_bp)
    // …
    .and_then(|delta| current_price_paise.checked_add(delta))
    .unwrap_or(current_price_paise);
```

If `current_price_paise = 0` (free session / promotional price):
- `0 * change_bp = 0`, `0 / 10000 = 0`, `0 + 0 = 0`
- Recommended price = ₹0.00
- Alert says "10% premium" but price stays free

This can produce a misleading `change_pct` label (15.0%) for a ₹0 price.

**Fix:**
```rust
if current_price_paise == 0 {
    // Cannot compute % change from zero; return unchanged
    return PricingRecommendation {
        recommended_price_paise: 0,
        change_pct: 0.0,
        reason: "Current price is ₹0 — cannot compute percentage change".to_string(),
        // …
    };
}
```

---

### P2 — `dynamic_pricing.rs:recommend_pricing` — `confidence` uses `> 0.0` instead of `>= 0.0`

**File/Function:** `dynamic_pricing.rs:recommend_pricing`

```rust
confidence: if forecast_occupancy_pct > 0.0 { 0.5 } else { 0.1 },
```

If `forecast_occupancy_pct` is exactly `0.0` (valid: 0% forecasted occupancy), confidence drops to 0.1. This is fine semantically (no forecast = low confidence), but the boundary `>= 0.0` would be more correct for a percentage that legitimately includes 0.

---

## 7. FEEDBACK ACCURACY

### P1 — Already listed above (Section 1, Finding 1) — NULL corruption of precision/FPR
### P1 — Already listed above (Section 1, Finding 2) — recall = precision

---

## SUMMARY TABLE

| ID | Priority | Category | File:Function | Finding |
|---|---|---|---|---|
| F01 | **P1** | Financial | `feedback_loop.rs:calculate_feedback_metrics` | `total_predictions` includes NULL rows, corrupts precision/FPR |
| F02 | **P1** | Feedback | `feedback_loop.rs:calculate_feedback_metrics` | `recall` hardcoded equal to `precision` |
| F03 | **P1** | Escalation | `escalation.rs:determine_escalation` | `High + >2 attempts` → `Technician` instead of `Manager` |
| F04 | **P1** | Pricing | `dynamic_pricing.rs:recommend_pricing` | Zero base price produces ₹0 recommendation with misleading `%` label |
| F05 | **P2** | Enum | `maintenance_store.rs:insert_event` | `ResolutionMethod` with data serializes unpredictably; inconsistent labels |
| F06 | **P2** | Enum | `escalation.rs:determine_escalation` | Uses `&str` severity instead of `Severity` enum; no input validation |
| F07 | **P2** | Financial | `alert_engine.rs:check_business_alerts` | Integer division truncates `drop_pct` and `cost_pct` in alert messages |
| F08 | **P2** | Data Flow | `biz_aggregator.rs:aggregate_daily_revenue` | `revenue_other_paise` never populated; permanently ₹0 |
| F09 | **P2** | Data Flow | `biz_aggregator.rs:aggregate_daily_revenue` | `sessions_count` silently truncates to 0 on u32 overflow |
| F10 | **P2** | Edge Case | `biz_aggregator.rs:aggregate_daily_revenue` | `occupancy` formula uses hardcoded ceiling; ignores concurrent sessions |
| F11 | **P3** | Data Flow | `biz_aggregator.rs` vs `alert_engine.rs` | UTC vs IST timezone mismatch for date attribution |
| F12 | **P3** | Enum | `escalation.rs:tests` | Missing test coverage for critical escalation paths |

**Priority counts: P1 = 4, P2 = 7, P3 = 2**

The four P1 issues (F01, F02, F03, F04) represent concrete correctness failures that will produce wrong financial metrics, wrong KPI reports, wrong escalation routing, and misleading pricing recommendations — all visible to operators. F01+F02 together mean the ML feedback loop has no reliable accuracy measurement. F03 can delay critical escalation by one tier. F04 can corrupt pricing proposals. All four warrant immediate fixes before v29.1.