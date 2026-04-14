use std::sync::Arc;

use uuid::Uuid;

use crate::accounting;
use crate::state::AppState;

/// Refund funds back to a driver's wallet. Returns new balance.
pub async fn refund(
    state: &Arc<AppState>,
    driver_id: &str,
    amount_paise: i64,
    reference_id: Option<&str>,
    notes: Option<&str>,
) -> Result<i64, String> {
    super::credit(
        state,
        driver_id,
        amount_paise,
        "refund_session",
        reference_id,
        notes,
        None,
    )
    .await
}

/// Get the maximum cash refund amount for a driver (per D-13, D-14).
/// Formula: rupee_deposited_paise - rupee_refunded_paise - total_debited_paise
/// Clamped to 0..=balance_paise.
pub async fn get_max_cash_refund(state: &Arc<AppState>, driver_id: &str) -> Result<i64, String> {
    let row = sqlx::query_as::<_, (i64, i64, i64, i64)>(
        "SELECT balance_paise, rupee_deposited_paise, rupee_refunded_paise, total_debited_paise
         FROM wallets WHERE driver_id = ?",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    match row {
        Some((balance, deposited, refunded, debited)) => {
            let raw = deposited - refunded - debited;
            Ok(raw.max(0).min(balance)) // clamp to [0, balance_paise]
        }
        None => Ok(0), // no wallet = no refund
    }
}

/// Cash refund: return real money to customer (per D-07 through D-12).
/// Decrements balance_paise, increments rupee_refunded_paise.
/// Capped at max_cash_refund (rupee_deposited - rupee_refunded - total_debited, floor 0).
/// Returns (new_balance, txn_id) on success.
///
/// Signature per D-07: cash_refund(state, driver_id, amount_paise, staff_id, notes)
/// Refund method defaults to "cash" for accounting. Phase 339 API can extend if needed.
pub async fn cash_refund(
    state: &Arc<AppState>,
    driver_id: &str,
    amount_paise: i64,
    staff_id: Option<&str>,
    notes: Option<&str>,
) -> Result<(i64, String), String> {
    if amount_paise <= 0 {
        return Err("Cash refund amount must be positive".to_string());
    }

    // Begin transaction FIRST — all checks happen inside the tx to prevent TOCTOU race.
    // Pattern: state.db.begin() (matches debit_wallet at line 258, avoids acquire+begin borrow issues)
    let mut tx = state.db.begin().await
        .map_err(|e| format!("DB error starting transaction: {}", e))?;

    // D-08, D-14: Compute max cash refund cap INSIDE the transaction.
    // This prevents TOCTOU race: a concurrent refund between cap-check and UPDATE
    // cannot allow over-refunding because we hold the tx lock.
    let cap_row = sqlx::query_as::<_, (i64, i64, i64, i64)>(
        "SELECT balance_paise, rupee_deposited_paise, rupee_refunded_paise, total_debited_paise
         FROM wallets WHERE driver_id = ?",
    )
    .bind(driver_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| format!("DB error reading wallet: {}", e))?;

    let (balance, max_refund) = match cap_row {
        Some((balance, deposited, refunded, debited)) => {
            let raw = deposited - refunded - debited;
            (balance, raw.max(0).min(balance))
        }
        None => return Err("Wallet not found".to_string()),
    };

    if amount_paise > max_refund {
        return Err(format!(
            "Cash refund {}p exceeds maximum allowed {}p (net rupee deposits minus spending)",
            amount_paise, max_refund
        ));
    }

    if amount_paise > balance {
        return Err(format!(
            "Insufficient balance for cash refund: have {}p, need {}p",
            balance, amount_paise
        ));
    }

    // D-09: Decrement balance_paise AND increment rupee_refunded_paise atomically.
    // The WHERE clause also validates balance >= amount as a safety net.
    let result = sqlx::query_as::<_, (i64,)>(
        "UPDATE wallets SET
            balance_paise = balance_paise - ?,
            rupee_refunded_paise = rupee_refunded_paise + ?,
            updated_at = datetime('now')
         WHERE driver_id = ? AND balance_paise >= ?
         RETURNING balance_paise",
    )
    .bind(amount_paise)
    .bind(amount_paise)
    .bind(driver_id)
    .bind(amount_paise)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| format!("DB error updating wallet: {}", e))?;

    let new_balance = match result {
        Some((balance,)) => balance,
        None => {
            return Err(format!(
                "Insufficient balance for cash refund: need {}p",
                amount_paise
            ));
        }
    };

    // D-10: txn_type = 'refund_cash', currency_type = 'rupee'
    let txn_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO wallet_transactions \
         (id, driver_id, amount_paise, balance_after_paise, txn_type, reference_id, notes, staff_id, venue_id, currency_type) \
         VALUES (?, ?, ?, ?, 'refund_cash', NULL, ?, ?, ?, 'rupee')",
    )
    .bind(&txn_id)
    .bind(driver_id)
    .bind(-amount_paise)  // negative for refund (money going out)
    .bind(new_balance)
    .bind(notes)
    .bind(staff_id)
    .bind(&state.config.venue.venue_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("DB error recording cash refund transaction: {}", e))?;

    tx.commit().await
        .map_err(|e| format!("DB error committing cash refund: {}", e))?;

    // D-12: Post accounting journal (Dr. acc_wallet Cr. acc_cash)
    // Default method is "cash" — Phase 339 API layer determines actual method if needed.
    accounting::post_cash_refund(state, driver_id, amount_paise, "cash", staff_id, Some(&txn_id)).await;

    tracing::info!(
        "Cash refund: {} -{}p = {}p (refund_cash, method=cash)",
        driver_id,
        amount_paise,
        new_balance,
    );

    Ok((new_balance, txn_id))
}

/// Get transaction history for a driver.
pub async fn get_transactions(
    state: &Arc<AppState>,
    driver_id: &str,
    limit: i64,
) -> Vec<rc_common::types::WalletTransaction> {
    let rows = sqlx::query_as::<_, (String, String, i64, i64, String, Option<String>, Option<String>, Option<String>, String, String)>(
        "SELECT id, driver_id, amount_paise, balance_after_paise, txn_type, reference_id, notes, staff_id, currency_type, created_at
         FROM wallet_transactions WHERE driver_id = ? ORDER BY created_at DESC LIMIT ?",
    )
    .bind(driver_id)
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    rows.into_iter()
        .map(|r| rc_common::types::WalletTransaction {
            id: r.0,
            driver_id: r.1,
            amount_paise: r.2,
            balance_after_paise: r.3,
            txn_type: r.4,
            reference_id: r.5,
            notes: r.6,
            staff_id: r.7,
            currency_type: r.8,
            created_at: r.9,
        })
        .collect()
}
