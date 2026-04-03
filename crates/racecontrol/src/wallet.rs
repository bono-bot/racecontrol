use std::sync::Arc;

use sqlx::Acquire;
use uuid::Uuid;

use crate::accounting;
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
/// Resolve the wallet owner for a driver. If the driver is a linked racer,
/// returns the parent's driver_id (whose wallet is charged). Otherwise returns the driver's own id.
pub async fn resolve_wallet_owner(state: &Arc<AppState>, driver_id: &str) -> Result<String, String> {
    let linked_to: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT linked_to FROM drivers WHERE id = ?",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    match linked_to {
        Some((Some(parent_id),)) => Ok(parent_id),
        _ => Ok(driver_id.to_string()),
    }
}

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

/// Credit (add) funds to a driver's wallet within an EXISTING transaction (FATM-01).
/// Caller owns the transaction and commits/rolls back.
/// Does NOT post accounting journal — caller must do that after commit.
/// Returns (new_balance, txn_id).
pub async fn credit_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    driver_id: &str,
    amount_paise: i64,
    txn_type: &str,
    reference_id: Option<&str>,
    notes: Option<&str>,
    staff_id: Option<&str>,
    idempotency_key: Option<&str>,
    venue_id: &str,
) -> Result<(i64, String), String> {
    if amount_paise <= 0 {
        return Err("Credit amount must be positive".to_string());
    }

    let txn_id = Uuid::new_v4().to_string();

    // Update wallet balance
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
    .execute(&mut **tx)
    .await
    .map_err(|e| format!("DB error updating wallet: {}", e))?;

    // Get new balance within transaction
    let row = sqlx::query_as::<_, (i64,)>(
        "SELECT balance_paise FROM wallets WHERE driver_id = ?",
    )
    .bind(driver_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| format!("DB error reading balance: {}", e))?;
    let new_balance = row.map(|r| r.0).unwrap_or(0);

    // Record transaction
    sqlx::query(
        "INSERT INTO wallet_transactions \
         (id, driver_id, amount_paise, balance_after_paise, txn_type, reference_id, notes, staff_id, idempotency_key, venue_id) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&txn_id)
    .bind(driver_id)
    .bind(amount_paise)
    .bind(new_balance)
    .bind(txn_type)
    .bind(reference_id)
    .bind(notes)
    .bind(staff_id)
    .bind(idempotency_key)
    .bind(venue_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| format!("DB error recording transaction: {}", e))?;

    Ok((new_balance, txn_id))
}

/// Credit (add) funds to a driver's wallet. Returns new balance.
/// Uses a SQLite transaction for atomicity.
/// Automatically posts a double-entry journal entry.
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

    // Use a transaction to ensure wallet update + transaction record are atomic
    let mut conn = state.db.acquire().await
        .map_err(|e| format!("DB error acquiring connection: {}", e))?;
    let mut tx = conn.begin().await
        .map_err(|e| format!("DB error starting transaction: {}", e))?;

    let (new_balance, txn_id) = credit_in_tx(
        &mut tx,
        driver_id,
        amount_paise,
        txn_type,
        reference_id,
        notes,
        staff_id,
        None, // no idempotency key for standalone credit
        &state.config.venue.venue_id,
    ).await?;

    tx.commit().await
        .map_err(|e| format!("DB error committing credit transaction: {}", e))?;

    // Post double-entry journal entry (outside wallet tx — wallet_transactions table
    // inside the tx is the source of truth for reconciliation if journal fails).
    // APP-05: removed per-call warn spam; accounting functions log on actual failure.
    match txn_type {
        "topup_cash" | "topup_card" | "topup_upi" | "topup_online" => {
            accounting::post_topup(state, driver_id, amount_paise, txn_type, staff_id, Some(&txn_id)).await;
        }
        "bonus" => {
            accounting::post_bonus(state, driver_id, amount_paise, Some(&txn_id)).await;
        }
        "refund_session" | "refund_manual" => {
            accounting::post_refund(state, driver_id, amount_paise, reference_id).await;
        }
        "adjustment" => {
            // Adjustment credit: treat as manual correction
            accounting::post_topup(state, driver_id, amount_paise, "topup_cash", staff_id, Some(&txn_id)).await;
        }
        _ => {}
    }

    tracing::info!(
        "Wallet credit: {} +{}p = {}p ({})",
        driver_id,
        amount_paise,
        new_balance,
        txn_type
    );

    Ok(new_balance)
}

/// Act 2: Standalone wallet debit — creates its own transaction.
/// Used for per-minute periodic debits where the caller doesn't need a transaction.
/// Returns Ok(new_balance) or Err(reason).
pub async fn debit_wallet(
    db: &sqlx::SqlitePool,
    driver_id: &str,
    amount_paise: i64,
    txn_type: &str,
    reference_id: Option<&str>,
    notes: Option<&str>,
    venue_id: &str,
) -> Result<i64, String> {
    let mut tx = db.begin().await.map_err(|e| format!("DB error: {}", e))?;
    let (new_balance, _txn_id) = debit_in_tx(
        &mut tx,
        driver_id,
        amount_paise,
        txn_type,
        reference_id,
        notes,
        None, // no idempotency key for periodic debits
        venue_id,
    )
    .await?;
    tx.commit().await.map_err(|e| format!("DB commit failed: {}", e))?;
    Ok(new_balance)
}

/// Debit wallet within an EXISTING transaction (FATM-01).
/// Caller owns the transaction and commits/rolls back.
/// Does NOT post accounting journal — caller must do that after commit.
/// Returns (new_balance, txn_id).
pub async fn debit_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    driver_id: &str,
    amount_paise: i64,
    txn_type: &str,
    reference_id: Option<&str>,
    notes: Option<&str>,
    idempotency_key: Option<&str>,
    venue_id: &str,
) -> Result<(i64, String), String> {
    if amount_paise <= 0 {
        return Err("Debit amount must be positive".to_string());
    }

    // Atomic debit: UPDATE only if balance is sufficient (prevents TOCTOU race — FATM-03)
    // The WHERE balance_paise >= amount means only one concurrent debit can succeed
    // when the balance is exactly equal to the amount.
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
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| format!("DB error updating wallet: {}", e))?;

    let new_balance = match result {
        Some((balance,)) => balance,
        None => {
            // Caller will drop tx to roll back
            return Err(format!(
                "Insufficient balance: need {}p",
                amount_paise
            ));
        }
    };

    let txn_id = Uuid::new_v4().to_string();

    // Record transaction after successful debit
    sqlx::query(
        "INSERT INTO wallet_transactions \
         (id, driver_id, amount_paise, balance_after_paise, txn_type, reference_id, notes, idempotency_key, venue_id) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&txn_id)
    .bind(driver_id)
    .bind(-amount_paise)
    .bind(new_balance)
    .bind(txn_type)
    .bind(reference_id)
    .bind(notes)
    .bind(idempotency_key)
    .bind(venue_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| format!("DB error recording transaction: {}", e))?;

    Ok((new_balance, txn_id))
}

/// Debit (subtract) funds from a driver's wallet. Returns (new_balance, txn_id).
/// Fails if insufficient balance.
/// Automatically posts a double-entry journal entry.
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

    // Use a transaction to ensure wallet debit + transaction record are atomic
    let mut conn = state.db.acquire().await
        .map_err(|e| format!("DB error acquiring connection: {}", e))?;
    let mut tx = conn.begin().await
        .map_err(|e| format!("DB error starting transaction: {}", e))?;

    let (new_balance, txn_id) = match debit_in_tx(
        &mut tx,
        driver_id,
        amount_paise,
        txn_type,
        reference_id,
        notes,
        None, // no idempotency key for standalone debit
        &state.config.venue.venue_id,
    ).await {
        Ok(result) => result,
        Err(e) => {
            drop(tx);
            // Try to report current balance for insufficient-balance errors
            if e.contains("Insufficient balance") {
                let current = get_balance(state, driver_id).await.unwrap_or(0);
                return Err(format!(
                    "Insufficient balance: have {}p, need {}p",
                    current, amount_paise
                ));
            }
            return Err(e);
        }
    };

    tx.commit().await
        .map_err(|e| format!("DB error committing debit transaction: {}", e))?;

    // Post double-entry journal entry (outside wallet tx — wallet_transactions table
    // inside the tx is the source of truth for reconciliation if journal fails).
    accounting::post_wallet_debit(state, driver_id, amount_paise, txn_type, Some(&txn_id)).await;

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
