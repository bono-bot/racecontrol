use std::sync::Arc;

use uuid::Uuid;

use crate::state::AppState;

/// Ensure a wallet row exists for the driver. Creates one if missing.
pub async fn ensure_wallet(state: &Arc<AppState>, driver_id: &str) -> Result<(), String> {
    sqlx::query(
        "INSERT OR IGNORE INTO wallets (driver_id, balance_paise, total_credited_paise, total_debited_paise)
         VALUES (?, 0, 0, 0)",
    )
    .bind(driver_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error creating wallet: {}", e))?;

    Ok(())
}

/// Get wallet balance in paise. Returns 0 if wallet doesn't exist yet.
pub async fn get_balance(state: &Arc<AppState>, driver_id: &str) -> Result<i64, String> {
    let row = sqlx::query_as::<_, (i64,)>(
        "SELECT balance_paise FROM wallets WHERE driver_id = ?",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    Ok(row.map(|r| r.0).unwrap_or(0))
}

/// Get full wallet info. Returns None if wallet doesn't exist.
pub async fn get_wallet_info(
    state: &Arc<AppState>,
    driver_id: &str,
) -> Result<Option<rc_common::types::WalletInfo>, String> {
    let row = sqlx::query_as::<_, (String, i64, i64, i64, Option<String>)>(
        "SELECT driver_id, balance_paise, total_credited_paise, total_debited_paise, updated_at
         FROM wallets WHERE driver_id = ?",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    Ok(row.map(|r| rc_common::types::WalletInfo {
        driver_id: r.0,
        balance_paise: r.1,
        total_credited_paise: r.2,
        total_debited_paise: r.3,
        updated_at: r.4,
    }))
}

/// Credit (add) funds to a driver's wallet. Returns new balance.
/// Uses a SQLite transaction for atomicity.
pub async fn credit(
    state: &Arc<AppState>,
    driver_id: &str,
    amount_paise: i64,
    txn_type: &str,
    reference_id: Option<&str>,
    notes: Option<&str>,
    staff_id: Option<&str>,
) -> Result<i64, String> {
    if amount_paise <= 0 {
        return Err("Credit amount must be positive".to_string());
    }

    // Ensure wallet exists
    ensure_wallet(state, driver_id).await?;

    let txn_id = Uuid::new_v4().to_string();

    // Update wallet balance atomically
    sqlx::query(
        "UPDATE wallets SET
            balance_paise = balance_paise + ?,
            total_credited_paise = total_credited_paise + ?,
            updated_at = datetime('now')
         WHERE driver_id = ?",
    )
    .bind(amount_paise)
    .bind(amount_paise)
    .bind(driver_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error updating wallet: {}", e))?;

    // Get new balance
    let new_balance = get_balance(state, driver_id).await?;

    // Record transaction
    sqlx::query(
        "INSERT INTO wallet_transactions (id, driver_id, amount_paise, balance_after_paise, txn_type, reference_id, notes, staff_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&txn_id)
    .bind(driver_id)
    .bind(amount_paise)
    .bind(new_balance)
    .bind(txn_type)
    .bind(reference_id)
    .bind(notes)
    .bind(staff_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error recording transaction: {}", e))?;

    tracing::info!(
        "Wallet credit: {} +{}p = {}p ({})",
        driver_id,
        amount_paise,
        new_balance,
        txn_type
    );

    Ok(new_balance)
}

/// Debit (subtract) funds from a driver's wallet. Returns (new_balance, txn_id).
/// Fails if insufficient balance.
pub async fn debit(
    state: &Arc<AppState>,
    driver_id: &str,
    amount_paise: i64,
    txn_type: &str,
    reference_id: Option<&str>,
    notes: Option<&str>,
) -> Result<(i64, String), String> {
    if amount_paise <= 0 {
        return Err("Debit amount must be positive".to_string());
    }

    // Atomic debit: UPDATE only if balance is sufficient (prevents TOCTOU race)
    let result = sqlx::query_as::<_, (i64,)>(
        "UPDATE wallets SET
            balance_paise = balance_paise - ?,
            total_debited_paise = total_debited_paise + ?,
            updated_at = datetime('now')
         WHERE driver_id = ? AND balance_paise >= ?
         RETURNING balance_paise",
    )
    .bind(amount_paise)
    .bind(amount_paise)
    .bind(driver_id)
    .bind(amount_paise)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error updating wallet: {}", e))?;

    let new_balance = match result {
        Some((balance,)) => balance,
        None => {
            let current = get_balance(state, driver_id).await.unwrap_or(0);
            return Err(format!(
                "Insufficient balance: have {}p, need {}p",
                current, amount_paise
            ));
        }
    };

    let txn_id = Uuid::new_v4().to_string();

    // Record transaction after successful debit
    sqlx::query(
        "INSERT INTO wallet_transactions (id, driver_id, amount_paise, balance_after_paise, txn_type, reference_id, notes)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&txn_id)
    .bind(driver_id)
    .bind(-amount_paise)
    .bind(new_balance)
    .bind(txn_type)
    .bind(reference_id)
    .bind(notes)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error recording transaction: {}", e))?;

    tracing::info!(
        "Wallet debit: {} -{}p = {}p ({})",
        driver_id,
        amount_paise,
        new_balance,
        txn_type
    );

    Ok((new_balance, txn_id))
}

/// Refund funds back to a driver's wallet. Returns new balance.
pub async fn refund(
    state: &Arc<AppState>,
    driver_id: &str,
    amount_paise: i64,
    reference_id: Option<&str>,
    notes: Option<&str>,
) -> Result<i64, String> {
    credit(
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

/// Get transaction history for a driver.
pub async fn get_transactions(
    state: &Arc<AppState>,
    driver_id: &str,
    limit: i64,
) -> Vec<rc_common::types::WalletTransaction> {
    let rows = sqlx::query_as::<_, (String, String, i64, i64, String, Option<String>, Option<String>, Option<String>, String)>(
        "SELECT id, driver_id, amount_paise, balance_after_paise, txn_type, reference_id, notes, staff_id, created_at
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
            created_at: r.8,
        })
        .collect()
}
