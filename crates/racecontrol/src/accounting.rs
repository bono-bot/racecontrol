use std::sync::Arc;

use serde_json::{Value, json};
use sqlx::{Column, Row};
use uuid::Uuid;

use crate::state::AppState;

// ─── Audit Log ──────────────────────────────────────────────────────────────

/// Record a change to any config table (pricing_rules, coupons, packages, etc.)
/// old_values/new_values should be JSON strings of the before/after state.
pub async fn log_audit(
    state: &Arc<AppState>,
    table_name: &str,
    row_id: &str,
    action: &str,
    old_values: Option<&str>,
    new_values: Option<&str>,
    staff_id: Option<&str>,
) {
    let id = Uuid::new_v4().to_string();
    if let Err(e) = sqlx::query(
        "INSERT INTO audit_log (id, table_name, row_id, action, old_values, new_values, staff_id)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(table_name)
    .bind(row_id)
    .bind(action)
    .bind(old_values)
    .bind(new_values)
    .bind(staff_id)
    .execute(&state.db)
    .await
    {
        tracing::error!("Failed to write audit log for {}.{}: {}", table_name, row_id, e);
    }
}

/// Record a sensitive admin action in audit_log with action_type classification.
/// Fire-and-forget: never blocks the caller on DB errors.
pub async fn log_admin_action(
    state: &Arc<AppState>,
    action_type: &str,
    details: &str,
    staff_id: Option<&str>,
    ip_address: Option<&str>,
) {
    let id = Uuid::new_v4().to_string();
    if let Err(e) = sqlx::query(
        "INSERT INTO audit_log (id, table_name, row_id, action, action_type, new_values, staff_id, ip_address)
         VALUES (?, 'admin_actions', ?, 'create', ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&id)
    .bind(action_type)
    .bind(details)
    .bind(staff_id)
    .bind(ip_address)
    .execute(&state.db)
    .await
    {
        tracing::error!("Failed to write admin audit log for {}: {}", action_type, e);
    }
}

/// Fetch the current row as JSON for audit trail (before an update/delete).
/// Returns None if the row doesn't exist.
pub async fn snapshot_row(
    state: &Arc<AppState>,
    table_name: &str,
    id: &str,
) -> Option<String> {
    // Build a simple SELECT * for the row. Since we know our table names,
    // we validate against an allowlist to prevent SQL injection.
    let allowed_tables = [
        "pricing_tiers", "pricing_rules", "coupons", "packages",
        "membership_tiers", "kiosk_experiences", "kiosk_settings",
        "tournaments",
    ];

    if !allowed_tables.contains(&table_name) {
        return None;
    }

    let query = format!("SELECT * FROM {} WHERE id = ?", table_name);

    // Use sqlx::query to get raw row, then convert to JSON manually.
    // Since different tables have different schemas, we use a generic approach.
    let row = sqlx::query(&query)
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

    row.map(|r| {
        let mut map = serde_json::Map::new();
        for col in r.columns() {
            let name = col.name();
            // Try to extract as string (most SQLite values can be retrieved this way)
            if let Ok(val) = r.try_get::<Option<String>, _>(name) {
                map.insert(
                    name.to_string(),
                    val.map(Value::String).unwrap_or(Value::Null),
                );
            } else if let Ok(val) = r.try_get::<Option<i64>, _>(name) {
                map.insert(
                    name.to_string(),
                    val.map(|v| Value::Number(v.into())).unwrap_or(Value::Null),
                );
            } else if let Ok(val) = r.try_get::<Option<f64>, _>(name) {
                map.insert(
                    name.to_string(),
                    val.and_then(|v| serde_json::Number::from_f64(v).map(Value::Number))
                        .unwrap_or(Value::Null),
                );
            }
        }
        serde_json::to_string(&map).unwrap_or_default()
    })
}

// ─── Journal Entries (Double-Entry Bookkeeping) ─────────────────────────────

/// A single debit or credit line in a journal entry.
pub struct JournalLine {
    pub account_id: String,
    pub debit_paise: i64,
    pub credit_paise: i64,
}

/// Post a balanced journal entry. Returns the entry ID.
/// Fails if total debits != total credits (the fundamental accounting rule).
pub async fn post_journal_entry(
    state: &Arc<AppState>,
    description: &str,
    reference_type: Option<&str>,
    reference_id: Option<&str>,
    staff_id: Option<&str>,
    lines: &[JournalLine],
) -> Result<String, String> {
    // Validate: at least 2 lines
    if lines.len() < 2 {
        return Err("Journal entry requires at least 2 lines".to_string());
    }

    // Validate: total debits == total credits
    let total_debit: i64 = lines.iter().map(|l| l.debit_paise).sum();
    let total_credit: i64 = lines.iter().map(|l| l.credit_paise).sum();

    if total_debit != total_credit {
        return Err(format!(
            "Entry does not balance: debits={}p, credits={}p",
            total_debit, total_credit
        ));
    }

    if total_debit == 0 {
        return Err("Journal entry cannot be zero".to_string());
    }

    // Validate: each line is either debit or credit, not both
    for line in lines {
        if line.debit_paise > 0 && line.credit_paise > 0 {
            return Err(format!(
                "Line for account {} has both debit and credit",
                line.account_id
            ));
        }
        if line.debit_paise == 0 && line.credit_paise == 0 {
            return Err(format!(
                "Line for account {} has zero amount",
                line.account_id
            ));
        }
    }

    let entry_id = Uuid::new_v4().to_string();

    // Use a transaction to ensure header + all lines are atomic
    let mut tx = state.db.begin().await
        .map_err(|e| format!("DB error starting transaction: {}", e))?;

    // Insert header
    sqlx::query(
        "INSERT INTO journal_entries (id, description, reference_type, reference_id, staff_id)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&entry_id)
    .bind(description)
    .bind(reference_type)
    .bind(reference_id)
    .bind(staff_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("DB error creating journal entry: {}", e))?;

    // Insert lines
    for line in lines {
        let line_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO journal_entry_lines (id, journal_entry_id, account_id, debit_paise, credit_paise)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&line_id)
        .bind(&entry_id)
        .bind(&line.account_id)
        .bind(line.debit_paise)
        .bind(line.credit_paise)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("DB error creating journal line: {}", e))?;
    }

    tx.commit().await
        .map_err(|e| format!("DB error committing journal entry: {}", e))?;

    tracing::debug!("Journal entry posted: {} ({}p)", entry_id, total_debit);
    Ok(entry_id)
}

// ─── Convenience: Auto-post journal entries for common wallet operations ─────

/// Post journal entry for a wallet topup.
/// Debit: Cash/Bank/UPI (asset) | Credit: Customer Wallet (liability)
pub async fn post_topup(
    state: &Arc<AppState>,
    driver_id: &str,
    amount_paise: i64,
    method: &str,
    staff_id: Option<&str>,
    txn_id: Option<&str>,
) {
    let asset_account = match method {
        "topup_cash" => "acc_cash",
        "topup_card" | "topup_upi" | "topup_online" => "acc_bank",
        _ => "acc_cash",
    };

    let desc = format!("Wallet topup ({}) for driver {}", method, driver_id);
    if let Err(e) = post_journal_entry(
        state,
        &desc,
        Some("wallet_transaction"),
        txn_id,
        staff_id,
        &[
            JournalLine {
                account_id: asset_account.to_string(),
                debit_paise: amount_paise,
                credit_paise: 0,
            },
            JournalLine {
                account_id: "acc_wallet".to_string(),
                debit_paise: 0,
                credit_paise: amount_paise,
            },
        ],
    )
    .await {
        tracing::error!("journal entry failed: {}", e);
    }
}

/// Post journal entry for a bonus credit.
/// Debit: Promotional Bonuses (expense) | Credit: Customer Wallet (liability)
pub async fn post_bonus(
    state: &Arc<AppState>,
    driver_id: &str,
    amount_paise: i64,
    txn_id: Option<&str>,
) {
    let desc = format!("Bonus credit for driver {}", driver_id);
    if let Err(e) = post_journal_entry(
        state,
        &desc,
        Some("wallet_transaction"),
        txn_id,
        None,
        &[
            JournalLine {
                account_id: "acc_promo_bonus".to_string(),
                debit_paise: amount_paise,
                credit_paise: 0,
            },
            JournalLine {
                account_id: "acc_wallet".to_string(),
                debit_paise: 0,
                credit_paise: amount_paise,
            },
        ],
    )
    .await {
        tracing::error!("journal entry failed: {}", e);
    }
}

/// Post journal entry for a session debit.
/// Debit: Customer Wallet (liability decreases) | Credit: Racing Revenue
pub async fn post_session_debit(
    state: &Arc<AppState>,
    driver_id: &str,
    amount_paise: i64,
    session_id: &str,
) {
    let desc = format!("Racing session {} for driver {}", session_id, driver_id);
    if let Err(e) = post_journal_entry(
        state,
        &desc,
        Some("billing_session"),
        Some(session_id),
        None,
        &[
            JournalLine {
                account_id: "acc_wallet".to_string(),
                debit_paise: amount_paise,
                credit_paise: 0,
            },
            JournalLine {
                account_id: "acc_racing_rev".to_string(),
                debit_paise: 0,
                credit_paise: amount_paise,
            },
        ],
    )
    .await {
        tracing::error!("journal entry failed: {}", e);
    }
}

/// Post journal entry for a wallet debit (cafe, merchandise, penalty).
/// Debit: Customer Wallet (liability) | Credit: appropriate revenue/expense
pub async fn post_wallet_debit(
    state: &Arc<AppState>,
    driver_id: &str,
    amount_paise: i64,
    txn_type: &str,
    txn_id: Option<&str>,
) {
    let (credit_account, desc_prefix) = match txn_type {
        "debit_cafe" => ("acc_cafe_rev", "Cafe purchase"),
        "debit_merchandise" => ("acc_merch_rev", "Merchandise purchase"),
        "debit_penalty" => ("acc_penalty_adj", "Penalty charge"),
        _ => ("acc_racing_rev", "Wallet debit"),
    };

    let desc = format!("{} for driver {}", desc_prefix, driver_id);
    if let Err(e) = post_journal_entry(
        state,
        &desc,
        Some("wallet_transaction"),
        txn_id,
        None,
        &[
            JournalLine {
                account_id: "acc_wallet".to_string(),
                debit_paise: amount_paise,
                credit_paise: 0,
            },
            JournalLine {
                account_id: credit_account.to_string(),
                debit_paise: 0,
                credit_paise: amount_paise,
            },
        ],
    )
    .await {
        tracing::error!("journal entry failed: {}", e);
    }
}

/// Post journal entry for a refund.
/// Debit: Refunds (expense) | Credit: Customer Wallet (liability increases)
pub async fn post_refund(
    state: &Arc<AppState>,
    driver_id: &str,
    amount_paise: i64,
    reference_id: Option<&str>,
) {
    let desc = format!("Refund to driver {}", driver_id);
    if let Err(e) = post_journal_entry(
        state,
        &desc,
        Some("refund"),
        reference_id,
        None,
        &[
            JournalLine {
                account_id: "acc_refunds".to_string(),
                debit_paise: amount_paise,
                credit_paise: 0,
            },
            JournalLine {
                account_id: "acc_wallet".to_string(),
                debit_paise: 0,
                credit_paise: amount_paise,
            },
        ],
    )
    .await {
        tracing::error!("journal entry failed: {}", e);
    }
}

// ─── Financial Reports ──────────────────────────────────────────────────────

/// Trial balance: sum of all debits and credits per account.
pub async fn get_trial_balance(
    state: &Arc<AppState>,
    from_date: Option<&str>,
    to_date: Option<&str>,
) -> Result<Value, String> {
    let mut query = String::from(
        "SELECT a.id, a.code, a.name, a.account_type,
                COALESCE(SUM(jel.debit_paise), 0) as total_debit,
                COALESCE(SUM(jel.credit_paise), 0) as total_credit
         FROM accounts a
         LEFT JOIN journal_entry_lines jel ON a.id = jel.account_id
         LEFT JOIN journal_entries je ON jel.journal_entry_id = je.id"
    );

    let mut conditions = Vec::new();
    if from_date.is_some() {
        conditions.push("je.date >= ?");
    }
    if to_date.is_some() {
        conditions.push("je.date <= ?");
    }

    if !conditions.is_empty() {
        query.push_str(" WHERE ");
        query.push_str(&conditions.join(" AND "));
    }

    query.push_str(" GROUP BY a.id ORDER BY a.code");

    let mut q = sqlx::query_as::<_, (String, i64, String, String, i64, i64)>(&query);
    if let Some(d) = from_date {
        q = q.bind(d);
    }
    if let Some(d) = to_date {
        q = q.bind(d);
    }

    let rows = q
        .fetch_all(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

    let mut total_debit = 0i64;
    let mut total_credit = 0i64;
    let accounts: Vec<Value> = rows
        .iter()
        .filter(|r| r.4 != 0 || r.5 != 0) // Skip accounts with no activity
        .map(|r| {
            total_debit = total_debit.checked_add(r.4).unwrap_or(i64::MAX);
            total_credit = total_credit.checked_add(r.5).unwrap_or(i64::MAX);
            let balance = r.4.checked_sub(r.5).unwrap_or(0);
            json!({
                "account_id": r.0,
                "code": r.1,
                "name": r.2,
                "account_type": r.3,
                "total_debit_paise": r.4,
                "total_credit_paise": r.5,
                "balance_paise": balance,
            })
        })
        .collect();

    Ok(json!({
        "accounts": accounts,
        "total_debit_paise": total_debit,
        "total_credit_paise": total_credit,
        "is_balanced": total_debit == total_credit,
    }))
}

/// Profit & Loss statement: Revenue - Expenses for a period.
pub async fn get_profit_loss(
    state: &Arc<AppState>,
    from_date: Option<&str>,
    to_date: Option<&str>,
) -> Result<Value, String> {
    // Revenue accounts: credits are positive (revenue earned)
    // Expense accounts: debits are positive (expenses incurred)
    let mut query = String::from(
        "SELECT a.id, a.code, a.name, a.account_type,
                COALESCE(SUM(jel.debit_paise), 0) as total_debit,
                COALESCE(SUM(jel.credit_paise), 0) as total_credit
         FROM accounts a
         JOIN journal_entry_lines jel ON a.id = jel.account_id
         JOIN journal_entries je ON jel.journal_entry_id = je.id
         WHERE a.account_type IN ('revenue', 'expense')"
    );

    if from_date.is_some() {
        query.push_str(" AND je.date >= ?");
    }
    if to_date.is_some() {
        query.push_str(" AND je.date <= ?");
    }

    query.push_str(" GROUP BY a.id ORDER BY a.code");

    let mut q = sqlx::query_as::<_, (String, i64, String, String, i64, i64)>(&query);
    if let Some(d) = from_date {
        q = q.bind(d);
    }
    if let Some(d) = to_date {
        q = q.bind(d);
    }

    let rows = q
        .fetch_all(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

    let mut revenue_items = Vec::new();
    let mut expense_items = Vec::new();
    let mut total_revenue = 0i64;
    let mut total_expenses = 0i64;

    for r in &rows {
        let amount = if r.3 == "revenue" {
            // Revenue: credit - debit (net credits)
            r.5.checked_sub(r.4).unwrap_or(0)
        } else {
            // Expense: debit - credit (net debits)
            r.4.checked_sub(r.5).unwrap_or(0)
        };

        if amount == 0 {
            continue;
        }

        let item = json!({
            "account_id": r.0,
            "code": r.1,
            "name": r.2,
            "amount_paise": amount,
        });

        if r.3 == "revenue" {
            total_revenue = total_revenue.checked_add(amount).unwrap_or(i64::MAX);
            revenue_items.push(item);
        } else {
            total_expenses = total_expenses.checked_add(amount).unwrap_or(i64::MAX);
            expense_items.push(item);
        }
    }

    Ok(json!({
        "revenue": revenue_items,
        "expenses": expense_items,
        "total_revenue_paise": total_revenue,
        "total_expenses_paise": total_expenses,
        "net_profit_paise": total_revenue.checked_sub(total_expenses).unwrap_or(0),
    }))
}

/// Balance sheet: Assets = Liabilities + Equity
pub async fn get_balance_sheet(state: &Arc<AppState>) -> Result<Value, String> {
    let rows = sqlx::query_as::<_, (String, i64, String, String, i64, i64)>(
        "SELECT a.id, a.code, a.name, a.account_type,
                COALESCE(SUM(jel.debit_paise), 0) as total_debit,
                COALESCE(SUM(jel.credit_paise), 0) as total_credit
         FROM accounts a
         LEFT JOIN journal_entry_lines jel ON a.id = jel.account_id
         WHERE a.account_type IN ('asset', 'liability', 'equity')
         GROUP BY a.id
         ORDER BY a.code",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    let mut assets = Vec::new();
    let mut liabilities = Vec::new();
    let mut equity = Vec::new();
    let mut total_assets = 0i64;
    let mut total_liabilities = 0i64;
    let mut total_equity = 0i64;

    for r in &rows {
        // Asset accounts have a normal debit balance (debit - credit)
        // Liability/Equity accounts have a normal credit balance (credit - debit)
        let balance = match r.3.as_str() {
            "asset" => r.4.checked_sub(r.5).unwrap_or(0),
            _ => r.5.checked_sub(r.4).unwrap_or(0),
        };

        if balance == 0 {
            continue;
        }

        let item = json!({
            "account_id": r.0,
            "code": r.1,
            "name": r.2,
            "balance_paise": balance,
        });

        match r.3.as_str() {
            "asset" => {
                total_assets = total_assets.checked_add(balance).unwrap_or(i64::MAX);
                assets.push(item);
            }
            "liability" => {
                total_liabilities = total_liabilities.checked_add(balance).unwrap_or(i64::MAX);
                liabilities.push(item);
            }
            "equity" => {
                total_equity = total_equity.checked_add(balance).unwrap_or(i64::MAX);
                equity.push(item);
            }
            _ => {}
        }
    }

    // Include retained earnings (net P&L to date)
    let pnl = get_profit_loss(state, None, None).await.unwrap_or(json!({}));
    let retained = pnl.get("net_profit_paise").and_then(|v| v.as_i64()).unwrap_or(0);

    if retained != 0 {
        total_equity = total_equity.checked_add(retained).unwrap_or(i64::MAX);
        equity.push(json!({
            "account_id": "acc_retained",
            "code": 3100,
            "name": "Retained Earnings (Net Profit)",
            "balance_paise": retained,
        }));
    }

    let is_balanced = total_assets == total_liabilities.checked_add(total_equity).unwrap_or(i64::MAX);

    Ok(json!({
        "assets": assets,
        "liabilities": liabilities,
        "equity": equity,
        "total_assets_paise": total_assets,
        "total_liabilities_paise": total_liabilities,
        "total_equity_paise": total_equity,
        "is_balanced": is_balanced,
    }))
}
