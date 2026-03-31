### FULL BUSINESS LOGIC AUDIT of v29.0 Meshed Intelligence

---

#### **1. FINANCIAL CORRECTNESS**  
**Findings**:  
- **P1 (FINANCE)**  
  **File**: `maintenance/dynamic_pricing.rs`  
  **Function**: `recommend_pricing`  
  **Description**:  
  Dynamic pricing logic allows the recommended price to fall below zero.  
  **Example**:  
  If `current_price_paise = 100` and the change is `-150%`, the calculation becomes:  
  ```rust
  let recommended = current_price_paise.checked_add(-150).unwrap_or(current_price_paise);
  ```
  This results in `-50` paise (negative price).  
  **Fix**:  
  Clamp the final price to a minimum of `0` paise:  
  ```rust
  let recommended = current_price_paise
      .checked_mul(change_bp)
      .and_then(|v| v.checked_div(10000))
      .and_then(|delta| current_price_paise.checked_add(delta))
      .map(|p| p.max(0))  // Clamp to zero
      .unwrap_or(current_price_paise);
  ```

- **P2 (FINANCE)**  
  **File**: `maintenance/ebitda.rs`  
  **Function**: `get_ebitda_summary`  
  **Description**:  
  Average EBITDA calculation uses integer division (`ebitda / days`), which truncates decimals instead of rounding.  
  **Fix**:  
  Round using `ebitda.div_euclid(days as i64)` or `ebitda / days as i64` for truncation, but add a comment clarifying this is intentional.  

- **P3 (FINANCE)**  
  **File**: `maintenance/payroll.rs`  
  **Function**: `calculate_monthly_payroll`  
  **Description**:  
  Hourly rate is prorated using `(hours_worked * 60.0).round() as i64`, which can round to negative minutes (e.g., `-0.1` hours → `-6` minutes).  
  **Fix**:  
  Clamp worked minutes to non-negative:  
  ```rust
  let worked_minutes = (row.total_hours * 60.0).round().max(0.0) as i64;
  ```  

---

#### **2. ENUM CONSISTENCY**  
**Findings**:  
- **P1 (ENUM)**  
  **File**: `maintenance/maintenance_store.rs` + `maintenance/init_maintenance_tables.rs`  
  **Function**: `insert_task` & `row_to_task`  
  **Description**:  
  `TaskStatus` is serialized as a JSON object (e.g., `{"Assigned":{}}`), but `replace('"', "")` converts it to `{Assigned:{}}`, which is invalid JSON. This causes deserialization failures.  
  **Fix**:  
  Remove `.replace('"', "")` and store the full JSON string. Use `serde_json::from_str` correctly.  

- **P2 (ENUM)**  
  **File**: `maintenance/models.rs`  
  **Function**: `format_alert_message`  
  **Description**:  
  Alert tiers are matched on `EscalationTier::Auto`, which requires empty message logic, but the code assumes `EscalationTier::Auto` is handled implicitly.  
  **Fix**:  
  Use `if let EscalationTier::Auto = tier` instead of checking all other cases.  

- **P2 (ENUM)**  
  **File**: `maintenance/pricing_bridge.rs`  
  **Function**: `create_proposal`  
  **Description**:  
  Status `status TEXT NOT NULL DEFAULT 'pending'` in SQL schema contradicts enum representation (e.g., `pending`, `approved`).  
  **Fix**:  
  Use `CHECK(status IN ('pending', 'approved', 'rejected', 'applied'))` for data integrity.  

---

#### **3. ESCALATION LOGIC**  
**Findings**:  
- **P2 (ESCALATION)**  
  **File**: `maintenance/escalation.rs`  
  **Function**: `determine_escalation`  
  **Description**:  
  High-severity alerts **never escalate** to `Manager`, even after many failed attempts.  
  **Example**:  
  ```rust
  if severity == "High" { return Technician }
  ```
  **Fix**:  
  Allow tier escalation for High-severity alerts after a threshold (e.g., 3 attempts):  
  ```rust
  if auto_fix_attempts > 2 || severity == "Critical" {
      EscalationTier::Manager
  }  
  ```  

---

#### **4. DATA FLOW**  
**Findings**:  
- **P1 (DATAFLOW)**  
  **File**: `maintenance/business_aggregator.rs`  
  **Function**: `aggregate_daily_revenue`  
  **Description**:  
  Gaming revenue is aggregated incorrectly from `billing_sessions.wallet_debit_paise`, but the column is named `wallet_debit_paice` in the schema.  
  **Fix**:  
  Correct column name in SQL query:  
  ```sql
  SELECT COALESCE(SUM(wallet_debit_paise), 0) ...
  ```  

- **P2 (DATAFLOW)**  
  **File**: `maintenance/alert_engine.rs`  
  **Function**: `check_business_alerts`  
  **Description**:  
  Occupancy validation uses `occupancy_rate_pct` from `daily_business_metrics`, but it’s stored as `REAL` and could be `NaN`.  
  **Fix**:  
  Enforce `occupancy_rate_pct >= 0.0` in DB constraints or validation logic.  

---

#### **5. EDGE CASES**  
**Findings**:  
- **P1 (EDGE)**  
  **File**: `maintenance/ebitda.rs`  
  **Function**: `get_ebitda_summary`  
  **Description**:  
  No handling for empty metrics list (`days == 0`). Best/Worst days could be `None`, causing unwrap errors in downstream logic.  
  **Fix**:  
  Default best/worst days to the first/last day in the range if metrics are empty.  

- **P2 (EDGE)**  
  **File**: `maintenance/business_aggregator.rs`  
  **Function**: `aggregate_daily_revenue`  
  **Description**:  
  Edge case for `occupancy_rate_pct` when sessions are zero but pods/existing occupancy exists.  
  **Fix**:  
  Use `occupancy_rate_pct.max(0.0)` to avoid negative values.  

---

#### **6. PRICING SAFETY**  
**Findings**:  
- **P1 (PRICING)**  
  **File**: `maintenance/dynamic_pricing.rs`  
  **Function**: `recommend_pricing`  
  **Description**:  
  The code allows **dynamic suggestions** for negative prices via unchecked arithmetic (see FINANCE P1).  
  **Fix**:  
  Clamp to `max(0)` as above.  

- **P2 (PRICING)**  
  **File**: `maintenance/pricing_bridge.rs`  
  **Function**: `apply_approved_pricing`  
  **Description**:  
  No transaction isolation; concurrent updates could apply conflicting pricing changes.  
  **Fix**:  
  Use `BEGIN IMMEDIATE TRANSACTION` to prevent race conditions.  

---

#### **7. FEEDBACK ACCURACY**  
**Findings**:  
- **P1 (FEEDBACK)**  
  **File**: `maintenance/feedback.rs`  
  **Function**: `calculate_feedback_metrics`  
  **Description**:  
  Recall is set equal to precision (`recall: precision`), which is statistically incorrect.  
  **Fix**:  
  Track `true_positives` and `false_negatives` separately:  
  ```rust
  recall = true_positives as f64 / (true_positives + false_negatives) as f64
  ```  
  Add a migration to store missed predictions.  

- **P2 (FEEDBACK)**  
  **File**: `maintenance/feedback.rs`  
  **Function**: `calculate_feedback_metrics`  
  **Description**:  
  `mean_lead_time_hours` is calculated only on `was_accurate = 1` entries, ignoring false negatives.  
  **Fix**:  
  Include all outcomes in lead time analysis or split metrics by accuracy.  

---

### Summary of Findings
| Priority | Category       | File                           | Description                                      | Fix                                                                 |
|---------|----------------|--------------------------------|--------------------------------------------------|---------------------------------------------------------------------|
| P1      | FINANCIAL      | `dynamic_pricing.rs`           | Dynamic pricing allows negative prices           | Clamp recommended price to `max(0)`                               |
| P1      | ENUM           | `maintenance_store.rs`         | TaskStatus serialization invalid after quote stripping | Remove `.replace('"',"")` and store valid JSON                  |
| P1      | DATA FLOW      | `business_aggregator.rs`       | Column name typo in billing revenue query        | Fix column name to `wallet_debit_paise`                           |
| P1      | FEEDBACK       | `feedback.rs`                  | Incorrect recall calculation                     | Track `true_positives` and `false_negatives` properly             |
| P2      | FINANCIAL      | `ebitda.rs`                    | Truncated average EBITDA calculation             | Document truncation or use `div_euclid`                           |
| P2      | ENUM           | `pricing_bridge.rs`            | Schema allows invalid status values              | Add CHECK constraint for status enum                              |
| P2      | ESCALATION     | `escalation.rs`                | High-severity alerts never escalate to Manager   | Update logic to escalate on attempt threshold                     |
| P2      | DATA FLOW      | `alert_engine.rs`              | Occupancy rate can be `NaN`                    | Validate `occupancy_rate_pct >= 0.0`                              |
| P2      | FEEDBACK       | `feedback.rs`                  | Lead time ignores false negatives                | Include false negatives or split metrics                          |
| P3      | FINANCIAL      | `payroll.rs`                   | Possible negative worked minutes               | Clamp to `max(0)`                                                 |