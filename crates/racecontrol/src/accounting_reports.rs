use std::sync::Arc;

use serde_json::{Value, json};

use crate::state::AppState;

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
