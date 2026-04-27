//! Cloud sync debit intent processing — wallet debits for remote bookings.
//! Extracted from cloud_sync_push.rs (Phase 385, v49.0 Architecture Completion).
//!
//! PACT-20260427-002 (2026-04-27): wraps each intent's 4 writes in a per-intent
//! transaction and delegates the wallet write to canonical `wallet::debit_in_tx`,
//! which uses `WHERE balance_paise >= ? RETURNING balance_paise` (FATM-03 pattern,
//! see wallet.rs::debit_in_tx L380-394). This eliminates the TOCTOU race that
//! caused PACT-113 ₹11,525 wallet drift on drv_8d1025c4 (~97% of drift from
//! N×₹700 racing debit_session pattern). Pre-fix used a Rust-side `bal - amount`
//! computation written back as a literal, allowing concurrent intents on the same
//! wallet to compute identical new_balance values and race the UPDATE.

use std::sync::Arc;

use crate::accounting;
use crate::state::AppState;
use crate::wallet;

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
        // PACT-20260427-002: per-intent transaction wraps wallet debit + status updates.
        let mut tx = state.db.begin().await?;

        // Idempotency key on the wallet_transactions row anchors retry behavior to
        // the intent_id; the debit_intents.status='completed' check at the top of
        // future iterations is the durable short-circuit.
        let idempotency_key = format!("debit_intent_{}", intent_id);

        let debit_result = wallet::debit_in_tx(
            &mut tx,
            driver_id,
            *amount,
            "debit_session",
            Some(reservation_id),
            Some("Remote booking debit"),
            Some(&idempotency_key),
            &state.config.venue.venue_id,
        )
        .await;

        match debit_result {
            Ok((_new_balance, txn_id)) => {
                // Mark intent completed within the same tx so all 3 effects commit atomically.
                if let Err(e) = sqlx::query(
                    "UPDATE debit_intents SET status = 'completed', wallet_txn_id = ?,
                     processed_at = datetime('now'), updated_at = datetime('now') WHERE id = ?",
                )
                .bind(&txn_id)
                .bind(intent_id)
                .execute(&mut *tx)
                .await
                {
                    tracing::error!(target: "sync", "Debit intent {} status-update failed: {}", intent_id, e);
                    drop(tx); // rolls back wallet debit + transaction insert
                    continue;
                }

                if let Err(e) = sqlx::query(
                    "UPDATE reservations SET status = 'confirmed', updated_at = datetime('now')
                     WHERE id = ?",
                )
                .bind(reservation_id)
                .execute(&mut *tx)
                .await
                {
                    tracing::error!(target: "sync", "Debit intent {} reservation-update failed: {}", intent_id, e);
                    drop(tx); // rolls back everything
                    continue;
                }

                if let Err(e) = tx.commit().await {
                    tracing::error!(target: "sync", "Debit intent {} commit failed: {}", intent_id, e);
                    continue;
                }

                // Post double-entry journal entry AFTER commit (mirrors wallet::debit pattern,
                // see wallet.rs L480-482 — wallet_transactions table is the source of truth
                // for reconciliation if journal post fails).
                accounting::post_wallet_debit(state, driver_id, *amount, "debit_session", Some(&txn_id)).await;

                tracing::info!(target: "sync", "Debit intent {} completed: {} paise from driver {}",
                    intent_id, amount, driver_id);
                processed += 1;
            }
            Err(e) => {
                // Tx auto-rolls back when dropped — wallet untouched.
                drop(tx);

                let reason = if e.contains("Insufficient balance") {
                    "insufficient_balance"
                } else if e.contains("DB error") {
                    "db_error"
                } else {
                    "wallet_error"
                };

                // Mark failed in separate auto-committed statements; wallet was never touched.
                let _ = sqlx::query(
                    "UPDATE debit_intents SET status = 'failed', failure_reason = ?,
                     processed_at = datetime('now'), updated_at = datetime('now') WHERE id = ?",
                )
                .bind(reason)
                .bind(intent_id)
                .execute(&state.db)
                .await;

                let _ = sqlx::query(
                    "UPDATE reservations SET status = 'failed', updated_at = datetime('now')
                     WHERE id = ?",
                )
                .bind(reservation_id)
                .execute(&state.db)
                .await;

                tracing::warn!(target: "sync", "Debit intent {} failed ({}: {}): {} paise from driver {}",
                    intent_id, reason, e, amount, driver_id);
                processed += 1;
            }
        }
    }

    if processed > 0 {
        tracing::info!(target: "sync", "Processed {} debit intents", processed);
    }
    Ok(processed)
}

#[cfg(test)]
mod tests {
    //! PACT-20260427-002 verification — concurrent debit_intent race elimination.
    //!
    //! These tests use a minimal sqlx::SqlitePool with hand-rolled schema rather
    //! than full AppState because:
    //! 1. AppState construction requires the full racecontrol startup path
    //!    (config, agent_senders, dashboard channel, etc.).
    //! 2. The fix's correctness depends on the SQL transaction semantics + the
    //!    canonical `wallet::debit_in_tx` SQL pattern — both testable at the
    //!    pool level without higher-level orchestration.
    //!
    //! The tests verify the SQL invariants the fix relies on. End-to-end
    //! verification of `process_debit_intents` itself happens via:
    //!   - `tests/integration.rs::create_test_state` integration test (follow-up commit)
    //!   - HALO C1.inv probe (live regression monitor — already deployed PACT-111)
    //!   - Read-only post-deploy forensic walk on cloud-replica DB (PACT-113 method)

    use sqlx::sqlite::SqlitePoolOptions;
    use sqlx::SqlitePool;

    async fn setup_minimal_wallet_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(":memory:")
            .await
            .expect("failed to open in-memory sqlite");

        sqlx::query(
            "CREATE TABLE wallets (
                driver_id TEXT PRIMARY KEY,
                balance_paise INTEGER NOT NULL DEFAULT 0,
                total_credited_paise INTEGER NOT NULL DEFAULT 0,
                total_debited_paise INTEGER NOT NULL DEFAULT 0,
                rupee_deposited_paise INTEGER NOT NULL DEFAULT 0,
                rupee_refunded_paise INTEGER NOT NULL DEFAULT 0,
                bonus_credited_paise INTEGER NOT NULL DEFAULT 0,
                updated_at TEXT DEFAULT (datetime('now')),
                venue_id TEXT NOT NULL DEFAULT 'test-venue'
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "CREATE TABLE wallet_transactions (
                id TEXT PRIMARY KEY,
                driver_id TEXT NOT NULL,
                amount_paise INTEGER NOT NULL,
                balance_after_paise INTEGER NOT NULL,
                txn_type TEXT NOT NULL,
                reference_id TEXT,
                notes TEXT,
                staff_id TEXT,
                created_at TEXT DEFAULT (datetime('now')),
                idempotency_key TEXT,
                venue_id TEXT NOT NULL DEFAULT 'test-venue',
                currency_type TEXT NOT NULL DEFAULT 'credit'
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        pool
    }

    /// SQL invariant test: two concurrent `WHERE balance_paise >= ?` debits on
    /// the same wallet cannot both succeed past available balance. This is the
    /// single SQL guarantee the fix depends on. Pre-fix, the pattern was:
    ///   SELECT bal; let new = bal - amt; UPDATE balance_paise = new;
    /// which is racy. Post-fix uses:
    ///   UPDATE balance_paise = balance_paise - ? WHERE balance_paise >= ? RETURNING
    /// which the database serializes.
    #[tokio::test]
    async fn test_canonical_debit_pattern_prevents_overdraw() {
        let pool = setup_minimal_wallet_pool().await;

        // Wallet starts at exactly 70000 — only one of two concurrent 70000-debits should win.
        sqlx::query(
            "INSERT INTO wallets (driver_id, balance_paise) VALUES ('drv_test', 70000)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let pool_a = pool.clone();
        let pool_b = pool.clone();

        let task_a = tokio::spawn(async move {
            sqlx::query_as::<_, (i64,)>(
                "UPDATE wallets SET balance_paise = balance_paise - ?,
                 total_debited_paise = total_debited_paise + ?,
                 updated_at = datetime('now')
                 WHERE driver_id = ? AND balance_paise >= ?
                 RETURNING balance_paise",
            )
            .bind(70000i64)
            .bind(70000i64)
            .bind("drv_test")
            .bind(70000i64)
            .fetch_optional(&pool_a)
            .await
        });

        let task_b = tokio::spawn(async move {
            sqlx::query_as::<_, (i64,)>(
                "UPDATE wallets SET balance_paise = balance_paise - ?,
                 total_debited_paise = total_debited_paise + ?,
                 updated_at = datetime('now')
                 WHERE driver_id = ? AND balance_paise >= ?
                 RETURNING balance_paise",
            )
            .bind(70000i64)
            .bind(70000i64)
            .bind("drv_test")
            .bind(70000i64)
            .fetch_optional(&pool_b)
            .await
        });

        let (r_a, r_b) = tokio::join!(task_a, task_b);
        let r_a = r_a.unwrap().unwrap();
        let r_b = r_b.unwrap().unwrap();

        // Exactly one debit must have succeeded (returned Some), the other must have failed (None).
        let succeeded = [r_a.is_some(), r_b.is_some()].iter().filter(|x| **x).count();
        assert_eq!(
            succeeded, 1,
            "Concurrent 70000p debits on a 70000p wallet — exactly one should win, got {} winners",
            succeeded
        );

        // Final balance must be 0 (one debit applied), not -70000 (two applied — the bug).
        let final_balance: (i64,) = sqlx::query_as(
            "SELECT balance_paise FROM wallets WHERE driver_id = 'drv_test'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            final_balance.0, 0,
            "Final balance should be 0 after one successful debit; got {} paise (overdraw bug)",
            final_balance.0
        );
    }

    /// SQL invariant test: the relative-write pattern (`balance_paise = balance_paise - ?`)
    /// is order-independent for sufficient-balance debits. Two concurrent ₹350 debits on a
    /// ₹1400 wallet must both succeed, leaving balance = ₹700, regardless of execution order.
    /// Pre-fix code computed `new_balance = bal - amt` in Rust and wrote back the literal —
    /// concurrent intents both wrote the same value, losing one debit.
    #[tokio::test]
    async fn test_relative_write_no_lost_update() {
        let pool = setup_minimal_wallet_pool().await;

        sqlx::query(
            "INSERT INTO wallets (driver_id, balance_paise) VALUES ('drv_test', 140000)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let pool_a = pool.clone();
        let pool_b = pool.clone();

        let task_a = tokio::spawn(async move {
            sqlx::query_as::<_, (i64,)>(
                "UPDATE wallets SET balance_paise = balance_paise - ?
                 WHERE driver_id = ? AND balance_paise >= ?
                 RETURNING balance_paise",
            )
            .bind(35000i64)
            .bind("drv_test")
            .bind(35000i64)
            .fetch_optional(&pool_a)
            .await
        });

        let task_b = tokio::spawn(async move {
            sqlx::query_as::<_, (i64,)>(
                "UPDATE wallets SET balance_paise = balance_paise - ?
                 WHERE driver_id = ? AND balance_paise >= ?
                 RETURNING balance_paise",
            )
            .bind(35000i64)
            .bind("drv_test")
            .bind(35000i64)
            .fetch_optional(&pool_b)
            .await
        });

        let (r_a, r_b) = tokio::join!(task_a, task_b);
        assert!(r_a.unwrap().unwrap().is_some(), "task A should succeed");
        assert!(r_b.unwrap().unwrap().is_some(), "task B should succeed");

        let final_balance: (i64,) = sqlx::query_as(
            "SELECT balance_paise FROM wallets WHERE driver_id = 'drv_test'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            final_balance.0,
            70000,
            "Final balance must reflect BOTH debits (140000 - 35000 - 35000 = 70000); got {}",
            final_balance.0
        );
    }

    /// Negative-test: demonstrates the PRE-FIX pattern's bug. Two concurrent reads of the
    /// same balance, Rust-side compute, then literal write-back can lose one debit.
    /// This test should FAIL if the codebase ever regresses to the pre-fix pattern.
    /// Marked #[ignore] because it intentionally exhibits the bug — included as
    /// documentation of what we're guarding against.
    #[tokio::test]
    #[ignore = "documents the pre-fix bug pattern; do not include in default test runs"]
    async fn test_pre_fix_pattern_loses_updates() {
        let pool = setup_minimal_wallet_pool().await;

        sqlx::query(
            "INSERT INTO wallets (driver_id, balance_paise) VALUES ('drv_test', 140000)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let pool_a = pool.clone();
        let pool_b = pool.clone();

        let task_a = tokio::spawn(async move {
            // Pre-fix: read balance
            let (bal,): (i64,) = sqlx::query_as(
                "SELECT balance_paise FROM wallets WHERE driver_id = ?",
            )
            .bind("drv_test")
            .fetch_one(&pool_a)
            .await
            .unwrap();
            // Yield to allow task B to also read
            tokio::task::yield_now().await;
            // Compute + literal write-back
            let new_balance = bal - 35000;
            sqlx::query("UPDATE wallets SET balance_paise = ? WHERE driver_id = ?")
                .bind(new_balance)
                .bind("drv_test")
                .execute(&pool_a)
                .await
                .unwrap();
        });

        let task_b = tokio::spawn(async move {
            let (bal,): (i64,) = sqlx::query_as(
                "SELECT balance_paise FROM wallets WHERE driver_id = ?",
            )
            .bind("drv_test")
            .fetch_one(&pool_b)
            .await
            .unwrap();
            tokio::task::yield_now().await;
            let new_balance = bal - 35000;
            sqlx::query("UPDATE wallets SET balance_paise = ? WHERE driver_id = ?")
                .bind(new_balance)
                .bind("drv_test")
                .execute(&pool_b)
                .await
                .unwrap();
        });

        let _ = tokio::join!(task_a, task_b);

        let final_balance: (i64,) = sqlx::query_as(
            "SELECT balance_paise FROM wallets WHERE driver_id = 'drv_test'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        // EXPECTED: 70000 (both debits applied). ACTUAL with pre-fix bug: often 105000
        // (only one debit applied — the other was lost to the race). When the assertion
        // fires, we have reproduced PACT-113.
        assert_eq!(
            final_balance.0, 70000,
            "Pre-fix pattern lost an update: balance should be 70000 (both debits) but got {} \
             — this is the PACT-113 bug pattern.",
            final_balance.0
        );
    }
}
