//! Cloud sync debit intent processing — wallet debits for remote bookings.
//! Extracted from cloud_sync_push.rs (Phase 385, v49.0 Architecture Completion).

use std::sync::Arc;

use crate::state::AppState;

/// Process pending debit intents received from cloud.
/// Called after sync pull to process wallet debits on the local server.
/// Local is the single writer for wallet debits -- cloud NEVER directly modifies wallet.
pub(crate) async fn process_debit_intents(state: &Arc<AppState>) -> anyhow::Result<u64> {
    let pending = sqlx::query_as::<_, (String, String, i64, String)>(
        "SELECT id, driver_id, amount_paise, reservation_id
         FROM debit_intents WHERE status = 'pending' ORDER BY created_at ASC",
    )
    .fetch_all(&state.db)
    .await?;

    if pending.is_empty() {
        return Ok(0);
    }

    let mut processed = 0u64;
    for (intent_id, driver_id, amount, reservation_id) in &pending {
        let balance = sqlx::query_as::<_, (i64,)>(
            "SELECT balance_paise FROM wallets WHERE driver_id = ?",
        )
        .bind(driver_id)
        .fetch_optional(&state.db)
        .await?;

        match balance {
            Some((bal,)) if bal >= *amount => {
                let new_balance = bal - amount;
                let txn_id = uuid::Uuid::new_v4().to_string();

                // Debit wallet
                sqlx::query(
                    "UPDATE wallets SET balance_paise = ?, total_debited_paise = total_debited_paise + ?,
                     updated_at = datetime('now') WHERE driver_id = ?",
                )
                .bind(new_balance).bind(amount).bind(driver_id)
                .execute(&state.db).await?;

                // Record wallet transaction (per D-20: include currency_type = 'credit' for all debits per D-06)
                sqlx::query(
                    "INSERT INTO wallet_transactions (id, driver_id, amount_paise, balance_after_paise,
                     txn_type, reference_id, notes, created_at, venue_id, currency_type)
                     VALUES (?, ?, ?, ?, 'debit_session', ?, 'Remote booking debit', datetime('now'), ?, 'credit')",
                )
                .bind(&txn_id).bind(driver_id).bind(-amount).bind(new_balance).bind(reservation_id)
                .bind(&state.config.venue.venue_id)
                .execute(&state.db).await?;

                // Mark intent completed
                sqlx::query(
                    "UPDATE debit_intents SET status = 'completed', wallet_txn_id = ?,
                     processed_at = datetime('now'), updated_at = datetime('now') WHERE id = ?",
                )
                .bind(&txn_id).bind(intent_id)
                .execute(&state.db).await?;

                // Update reservation to confirmed
                sqlx::query(
                    "UPDATE reservations SET status = 'confirmed', updated_at = datetime('now')
                     WHERE id = ?",
                )
                .bind(reservation_id)
                .execute(&state.db).await?;

                tracing::info!(target: "sync", "Debit intent {} completed: {} paise from driver {}",
                    intent_id, amount, driver_id);
                processed += 1;
            }
            _ => {
                // Insufficient balance or no wallet
                let reason = if balance.is_none() { "no_wallet" } else { "insufficient_balance" };
                sqlx::query(
                    "UPDATE debit_intents SET status = 'failed', failure_reason = ?,
                     processed_at = datetime('now'), updated_at = datetime('now') WHERE id = ?",
                )
                .bind(reason).bind(intent_id)
                .execute(&state.db).await?;

                sqlx::query(
                    "UPDATE reservations SET status = 'failed', updated_at = datetime('now')
                     WHERE id = ?",
                )
                .bind(reservation_id)
                .execute(&state.db).await?;

                tracing::warn!(target: "sync", "Debit intent {} failed ({}): {} paise from driver {}",
                    intent_id, reason, amount, driver_id);
                processed += 1;
            }
        }
    }

    if processed > 0 {
        tracing::info!(target: "sync", "Processed {} debit intents", processed);
    }
    Ok(processed)
}
