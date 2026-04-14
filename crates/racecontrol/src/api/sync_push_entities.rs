//! Entity-level upsert helpers for sync_push.
//! Extracted from sync_cloud_push.rs to keep each file under 500 lines.

use serde_json::{Value, json};
use std::sync::Arc;

use crate::state::AppState;
use rc_common::protocol::DashboardEvent;

// ─── Wallets ───────────────────────────────────────────────────────────────

/// Upsert wallets with ID mismatch resolution by phone/email.
pub(crate) async fn upsert_wallets(state: &Arc<AppState>, wallets: &[Value]) -> u64 {
    let mut total = 0u64;
    for w in wallets {
        let driver_id = w.get("driver_id").and_then(|v| v.as_str()).unwrap_or_default();
        if driver_id.is_empty() { continue; }

        let balance = w.get("balance_paise").and_then(|v| v.as_i64()).unwrap_or(0);
        let credited = w.get("total_credited_paise").and_then(|v| v.as_i64()).unwrap_or(0);
        let debited = w.get("total_debited_paise").and_then(|v| v.as_i64()).unwrap_or(0);
        let updated = w.get("updated_at").and_then(|v| v.as_str()).unwrap_or("");

        // Try direct driver_id match first
        let r = sqlx::query(
            "UPDATE wallets SET
                balance_paise = ?, total_credited_paise = ?,
                total_debited_paise = ?, updated_at = ?
             WHERE driver_id = ?",
        )
        .bind(balance).bind(credited).bind(debited).bind(updated)
        .bind(driver_id)
        .execute(&state.db)
        .await;

        let rows = r.as_ref().map(|r| r.rows_affected()).unwrap_or(0);
        if rows > 0 {
            total += 1;
            continue;
        }

        // ID didn't match — try to find local driver by phone or email
        let phone = w.get("phone").and_then(|v| v.as_str()).unwrap_or("");
        let email = w.get("email").and_then(|v| v.as_str()).unwrap_or("");

        let resolved: Option<(String,)> = if !phone.is_empty() {
            let ph = state.field_cipher.hash_phone(phone);
            sqlx::query_as("SELECT id FROM drivers WHERE phone_hash = ?")
                .bind(&ph)
                .fetch_optional(&state.db)
                .await
                .ok()
                .flatten()
        } else if !email.is_empty() {
            sqlx::query_as("SELECT id FROM drivers WHERE email = ?")
                .bind(email)
                .fetch_optional(&state.db)
                .await
                .ok()
                .flatten()
        } else {
            None
        };

        if let Some((local_id,)) = resolved {
            let r2 = sqlx::query(
                "UPDATE wallets SET
                    balance_paise = ?, total_credited_paise = ?,
                    total_debited_paise = ?, updated_at = ?
                 WHERE driver_id = ?",
            )
            .bind(balance).bind(credited).bind(debited).bind(updated)
            .bind(&local_id)
            .execute(&state.db)
            .await;
            if r2.is_ok() {
                tracing::info!("Wallet sync: resolved {} -> {} by phone/email", driver_id, local_id);
                total += 1;
            }
        }
    }
    total
}

// ─── Wallet Transactions ───────────────────────────────────────────────────

/// Upsert wallet transactions (immutable — INSERT OR IGNORE by UUID) and run shadow verification.
pub(crate) async fn upsert_wallet_transactions(state: &Arc<AppState>, txns: &[Value]) -> u64 {
    let mut total = 0u64;
    for txn in txns {
        let id = txn.get("id").and_then(|v| v.as_str()).unwrap_or_default();
        if id.is_empty() { continue; }
        let r = sqlx::query(
            "INSERT OR IGNORE INTO wallet_transactions
                (id, driver_id, amount_paise, balance_after_paise, txn_type, reference_id, notes, staff_id, created_at, venue_id)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        )
        .bind(id)
        .bind(txn.get("driver_id").and_then(|v| v.as_str()))
        .bind(txn.get("amount_paise").and_then(|v| v.as_i64()).unwrap_or(0))
        .bind(txn.get("balance_after_paise").and_then(|v| v.as_i64()).unwrap_or(0))
        .bind(txn.get("txn_type").and_then(|v| v.as_str()).unwrap_or("adjustment"))
        .bind(txn.get("reference_id").and_then(|v| v.as_str()))
        .bind(txn.get("notes").and_then(|v| v.as_str()))
        .bind(txn.get("staff_id").and_then(|v| v.as_str()))
        .bind(txn.get("created_at").and_then(|v| v.as_str()))
        .bind(&state.config.venue.venue_id)
        .execute(&state.db)
        .await;
        if r.is_ok() { total += 1; }
    }
    tracing::info!("Sync push: {} wallet transactions", txns.len());

    // Shadow verification: compare latest transaction balance with wallet balance
    let mut driver_ids: Vec<String> = txns.iter()
        .filter_map(|t| t.get("driver_id").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .collect();
    driver_ids.sort();
    driver_ids.dedup();

    for did in &driver_ids {
        let txn_balance: Option<(i64,)> = sqlx::query_as(
            "SELECT balance_after_paise FROM wallet_transactions WHERE driver_id = ? ORDER BY created_at DESC LIMIT 1",
        )
        .bind(did)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        let wallet_balance: Option<(i64,)> = sqlx::query_as(
            "SELECT balance_paise FROM wallets WHERE driver_id = ?",
        )
        .bind(did)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        if let (Some((txn_bal,)), Some((wallet_bal,))) = (txn_balance, wallet_balance) {
            if txn_bal != wallet_bal {
                tracing::warn!(
                    driver_id = %did,
                    wallet_balance = wallet_bal,
                    txn_balance = txn_bal,
                    diff = wallet_bal - txn_bal,
                    "Wallet balance discrepancy detected in shadow verification"
                );
            }
        }
    }
    total
}

// ─── Billing Events ────────────────────────────────────────────────────────

pub(crate) async fn upsert_billing_events(state: &Arc<AppState>, events: &[Value]) -> u64 {
    let mut total = 0u64;
    for ev in events {
        let id = ev.get("id").and_then(|v| v.as_str()).unwrap_or_default();
        if id.is_empty() { continue; }
        let r = sqlx::query(
            "INSERT OR IGNORE INTO billing_events
                (id, billing_session_id, event_type, driving_seconds_at_event, metadata, created_at, venue_id)
             VALUES (?1,?2,?3,?4,?5,?6,?7)",
        )
        .bind(id)
        .bind(ev.get("billing_session_id").and_then(|v| v.as_str()))
        .bind(ev.get("event_type").and_then(|v| v.as_str()).unwrap_or("unknown"))
        .bind(ev.get("driving_seconds_at_event").and_then(|v| v.as_i64()).unwrap_or(0))
        .bind(ev.get("metadata").and_then(|v| v.as_str()))
        .bind(ev.get("created_at").and_then(|v| v.as_str()))
        .bind(&state.config.venue.venue_id)
        .execute(&state.db)
        .await;
        if r.is_ok() { total += 1; }
    }
    tracing::info!("Sync push: {} billing events", events.len());
    total
}

// ─── Staff Members ─────────────────────────────────────────────────────────

pub(crate) async fn upsert_staff_members(state: &Arc<AppState>, staff: &[Value]) -> u64 {
    let mut total = 0u64;
    for s in staff {
        let id = s.get("id").and_then(|v| v.as_str()).unwrap_or_default();
        if id.is_empty() { continue; }
        let r = sqlx::query(
            "INSERT INTO staff_members (id, name, phone, pin, is_active, role, created_at, updated_at, last_login_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name, phone = excluded.phone, pin = excluded.pin,
                is_active = excluded.is_active, role = excluded.role,
                updated_at = excluded.updated_at, last_login_at = excluded.last_login_at",
        )
        .bind(id)
        .bind(s.get("name").and_then(|v| v.as_str()))
        .bind(s.get("phone").and_then(|v| v.as_str()))
        .bind(s.get("pin").and_then(|v| v.as_str()))
        .bind(s.get("is_active").and_then(|v| v.as_i64()).unwrap_or(1))
        .bind(s.get("role").and_then(|v| v.as_str()).unwrap_or("staff"))
        .bind(s.get("created_at").and_then(|v| v.as_str()))
        .bind(s.get("updated_at").and_then(|v| v.as_str()))
        .bind(s.get("last_login_at").and_then(|v| v.as_str()))
        .execute(&state.db)
        .await;
        if r.is_ok() { total += 1; }
    }
    tracing::info!("Sync push: {} staff_members", staff.len());
    total
}

// ─── Reservations ──────────────────────────────────────────────────────────

pub(crate) async fn upsert_reservations(state: &Arc<AppState>, reservations: &[Value]) -> u64 {
    let mut total = 0u64;
    for r in reservations {
        let id = r.get("id").and_then(|v| v.as_str()).unwrap_or_default();
        if id.is_empty() { continue; }
        let res = sqlx::query(
            "INSERT INTO reservations (id, driver_id, experience_id, pin, status,
                pod_number, debit_intent_id, created_at, expires_at, redeemed_at,
                cancelled_at, updated_at, venue_id)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
             ON CONFLICT(id) DO UPDATE SET
                status = excluded.status,
                pod_number = COALESCE(excluded.pod_number, reservations.pod_number),
                debit_intent_id = COALESCE(excluded.debit_intent_id, reservations.debit_intent_id),
                redeemed_at = COALESCE(excluded.redeemed_at, reservations.redeemed_at),
                cancelled_at = COALESCE(excluded.cancelled_at, reservations.cancelled_at),
                updated_at = excluded.updated_at",
        )
        .bind(id)
        .bind(r.get("driver_id").and_then(|v| v.as_str()))
        .bind(r.get("experience_id").and_then(|v| v.as_str()))
        .bind(r.get("pin").and_then(|v| v.as_str()))
        .bind(r.get("status").and_then(|v| v.as_str()).unwrap_or("pending_debit"))
        .bind(r.get("pod_number").and_then(|v| v.as_i64()))
        .bind(r.get("debit_intent_id").and_then(|v| v.as_str()))
        .bind(r.get("created_at").and_then(|v| v.as_str()))
        .bind(r.get("expires_at").and_then(|v| v.as_str()))
        .bind(r.get("redeemed_at").and_then(|v| v.as_str()))
        .bind(r.get("cancelled_at").and_then(|v| v.as_str()))
        .bind(r.get("updated_at").and_then(|v| v.as_str()))
        .bind(&state.config.venue.venue_id)
        .execute(&state.db)
        .await;
        if res.is_ok() { total += 1; }
    }
    total
}

// ─── Debit Intents ─────────────────────────────────────────────────────────

pub(crate) async fn upsert_debit_intents(state: &Arc<AppState>, intents: &[Value]) -> u64 {
    let mut total = 0u64;
    for di in intents {
        let id = di.get("id").and_then(|v| v.as_str()).unwrap_or_default();
        if id.is_empty() { continue; }
        let res = sqlx::query(
            "INSERT INTO debit_intents (id, driver_id, amount_paise, reservation_id,
                status, failure_reason, wallet_txn_id, origin, created_at,
                processed_at, updated_at, venue_id)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
             ON CONFLICT(id) DO UPDATE SET
                status = excluded.status,
                failure_reason = COALESCE(excluded.failure_reason, debit_intents.failure_reason),
                wallet_txn_id = COALESCE(excluded.wallet_txn_id, debit_intents.wallet_txn_id),
                processed_at = COALESCE(excluded.processed_at, debit_intents.processed_at),
                updated_at = excluded.updated_at",
        )
        .bind(id)
        .bind(di.get("driver_id").and_then(|v| v.as_str()))
        .bind(di.get("amount_paise").and_then(|v| v.as_i64()).unwrap_or(0))
        .bind(di.get("reservation_id").and_then(|v| v.as_str()))
        .bind(di.get("status").and_then(|v| v.as_str()).unwrap_or("pending"))
        .bind(di.get("failure_reason").and_then(|v| v.as_str()))
        .bind(di.get("wallet_txn_id").and_then(|v| v.as_str()))
        .bind(di.get("origin").and_then(|v| v.as_str()).unwrap_or("cloud"))
        .bind(di.get("created_at").and_then(|v| v.as_str()))
        .bind(di.get("processed_at").and_then(|v| v.as_str()))
        .bind(di.get("updated_at").and_then(|v| v.as_str()))
        .bind(&state.config.venue.venue_id)
        .execute(&state.db)
        .await;
        if res.is_ok() { total += 1; }
    }
    total
}

// ─── Fleet Solutions (Phase 301, LWW + venue_id tiebreaker) ────────────────

pub(crate) async fn upsert_fleet_solutions(state: &Arc<AppState>, solutions: &[Value]) -> u64 {
    let mut total = 0u64;
    let mut conflicts = 0i64;
    for sol in solutions {
        let id = sol.get("id").and_then(|v| v.as_str()).unwrap_or_default();
        if id.is_empty() { continue; }
        let ts = crate::cloud_sync::normalize_timestamp(
            sol.get("updated_at").and_then(|v| v.as_str()).unwrap_or(""),
        );
        let r = sqlx::query(
            "INSERT INTO fleet_solutions
                (id, problem_key, problem_hash, symptoms, environment, root_cause,
                 fix_action, fix_type, status, success_count, fail_count, confidence,
                 cost_to_diagnose, models_used, diagnosis_tier, source_node, venue_id,
                 created_at, updated_at, version, ttl_days, tags)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22)
             ON CONFLICT(id) DO UPDATE SET
                root_cause = excluded.root_cause,
                fix_action = excluded.fix_action,
                status = excluded.status,
                success_count = excluded.success_count,
                fail_count = excluded.fail_count,
                confidence = excluded.confidence,
                cost_to_diagnose = excluded.cost_to_diagnose,
                models_used = excluded.models_used,
                updated_at = excluded.updated_at,
                version = excluded.version,
                venue_id = excluded.venue_id
             WHERE excluded.updated_at > fleet_solutions.updated_at
                OR (excluded.updated_at = fleet_solutions.updated_at
                    AND excluded.venue_id < fleet_solutions.venue_id)",
        )
        .bind(id)
        .bind(sol.get("problem_key").and_then(|v| v.as_str()).unwrap_or(""))
        .bind(sol.get("problem_hash").and_then(|v| v.as_str()).unwrap_or(""))
        .bind(sol.get("symptoms").and_then(|v| v.as_str()).unwrap_or(""))
        .bind(sol.get("environment").and_then(|v| v.as_str()).unwrap_or(""))
        .bind(sol.get("root_cause").and_then(|v| v.as_str()).unwrap_or(""))
        .bind(sol.get("fix_action").and_then(|v| v.as_str()).unwrap_or(""))
        .bind(sol.get("fix_type").and_then(|v| v.as_str()).unwrap_or(""))
        .bind(sol.get("status").and_then(|v| v.as_str()).unwrap_or("candidate"))
        .bind(sol.get("success_count").and_then(|v| v.as_i64()).unwrap_or(1))
        .bind(sol.get("fail_count").and_then(|v| v.as_i64()).unwrap_or(0))
        .bind(sol.get("confidence").and_then(|v| v.as_f64()).unwrap_or(1.0))
        .bind(sol.get("cost_to_diagnose").and_then(|v| v.as_f64()).unwrap_or(0.0))
        .bind(sol.get("models_used").and_then(|v| v.as_str()))
        .bind(sol.get("diagnosis_tier").and_then(|v| v.as_str()).unwrap_or("deterministic"))
        .bind(sol.get("source_node").and_then(|v| v.as_str()).unwrap_or(""))
        .bind(sol.get("venue_id").and_then(|v| v.as_str()))
        .bind(sol.get("created_at").and_then(|v| v.as_str()).unwrap_or(""))
        .bind(&ts)
        .bind(sol.get("version").and_then(|v| v.as_i64()).unwrap_or(1))
        .bind(sol.get("ttl_days").and_then(|v| v.as_i64()).unwrap_or(90))
        .bind(sol.get("tags").and_then(|v| v.as_str()))
        .execute(&state.db)
        .await;
        match r {
            Ok(res) => {
                if res.rows_affected() > 0 { total += 1; }
                else { conflicts += 1; }
            }
            Err(e) => { tracing::warn!("fleet_solutions upsert error: {}", e); }
        }
    }
    if conflicts > 0 {
        let _ = sqlx::query(
            "UPDATE sync_state SET conflict_count = COALESCE(conflict_count, 0) + ?1
             WHERE table_name = 'fleet_solutions'"
        ).bind(conflicts).execute(&state.db).await;
    }
    tracing::info!("Sync push: {} fleet_solutions ({} conflicts)", solutions.len(), conflicts);
    total
}

// ─── Model Evaluations (Phase 301, LWW + venue_id tiebreaker) ──────────────

pub(crate) async fn upsert_model_evaluations(state: &Arc<AppState>, evals: &[Value]) -> u64 {
    let mut total = 0u64;
    let mut conflicts = 0i64;
    for ev in evals {
        let id = ev.get("id").and_then(|v| v.as_str()).unwrap_or_default();
        if id.is_empty() { continue; }
        let ts = crate::cloud_sync::normalize_timestamp(
            ev.get("updated_at").and_then(|v| v.as_str()).unwrap_or(""),
        );
        let r = sqlx::query(
            "INSERT INTO model_evaluations
                (id, model_name, pod_id, problem_key, prediction, actual,
                 correct, cost_usd, diagnosis_tier, created_at, updated_at, venue_id)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
             ON CONFLICT(id) DO UPDATE SET
                prediction = excluded.prediction,
                actual = excluded.actual,
                correct = excluded.correct,
                cost_usd = excluded.cost_usd,
                diagnosis_tier = excluded.diagnosis_tier,
                updated_at = excluded.updated_at,
                venue_id = excluded.venue_id
             WHERE excluded.updated_at > model_evaluations.updated_at
                OR (excluded.updated_at = model_evaluations.updated_at
                    AND excluded.venue_id < model_evaluations.venue_id)",
        )
        .bind(id)
        .bind(ev.get("model_name").and_then(|v| v.as_str()).unwrap_or(""))
        .bind(ev.get("pod_id").and_then(|v| v.as_str()))
        .bind(ev.get("problem_key").and_then(|v| v.as_str()))
        .bind(ev.get("prediction").and_then(|v| v.as_str()))
        .bind(ev.get("actual").and_then(|v| v.as_str()))
        .bind(ev.get("correct").and_then(|v| v.as_i64()).unwrap_or(0))
        .bind(ev.get("cost_usd").and_then(|v| v.as_f64()).unwrap_or(0.0))
        .bind(ev.get("diagnosis_tier").and_then(|v| v.as_str()))
        .bind(ev.get("created_at").and_then(|v| v.as_str()).unwrap_or(""))
        .bind(&ts)
        .bind(ev.get("venue_id").and_then(|v| v.as_str()))
        .execute(&state.db)
        .await;
        match r {
            Ok(res) => {
                if res.rows_affected() > 0 { total += 1; }
                else { conflicts += 1; }
            }
            Err(e) => { tracing::warn!("model_evaluations upsert error: {}", e); }
        }
    }
    if conflicts > 0 {
        let _ = sqlx::query(
            "UPDATE sync_state SET conflict_count = COALESCE(conflict_count, 0) + ?1
             WHERE table_name = 'model_evaluations'"
        ).bind(conflicts).execute(&state.db).await;
    }
    tracing::info!("Sync push: {} model_evaluations ({} conflicts)", evals.len(), conflicts);
    total
}

// ─── Metrics Rollups (Phase 301, UNIQUE constraint LWW) ────────────────────

pub(crate) async fn upsert_metrics_rollups(state: &Arc<AppState>, rollups: &[Value]) -> u64 {
    let mut total = 0u64;
    let mut conflicts = 0i64;
    for ru in rollups {
        let resolution = ru.get("resolution").and_then(|v| v.as_str()).unwrap_or_default();
        let metric_name = ru.get("metric_name").and_then(|v| v.as_str()).unwrap_or_default();
        let period_start = ru.get("period_start").and_then(|v| v.as_str()).unwrap_or_default();
        if resolution.is_empty() || metric_name.is_empty() || period_start.is_empty() { continue; }
        let ts = crate::cloud_sync::normalize_timestamp(
            ru.get("updated_at").and_then(|v| v.as_str()).unwrap_or(""),
        );
        let r = sqlx::query(
            "INSERT INTO metrics_rollups
                (resolution, metric_name, pod_id, min_value, max_value, avg_value,
                 sample_count, period_start, updated_at, venue_id)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
             ON CONFLICT(resolution, metric_name, pod_id, period_start) DO UPDATE SET
                avg_value = CASE WHEN excluded.sample_count > metrics_rollups.sample_count
                            THEN excluded.avg_value ELSE metrics_rollups.avg_value END,
                min_value = MIN(excluded.min_value, metrics_rollups.min_value),
                max_value = MAX(excluded.max_value, metrics_rollups.max_value),
                sample_count = MAX(excluded.sample_count, metrics_rollups.sample_count),
                updated_at = excluded.updated_at,
                venue_id = excluded.venue_id
             WHERE excluded.updated_at > metrics_rollups.updated_at
                OR metrics_rollups.updated_at IS NULL",
        )
        .bind(resolution)
        .bind(metric_name)
        .bind(ru.get("pod_id").and_then(|v| v.as_str()))
        .bind(ru.get("min_value").and_then(|v| v.as_f64()).unwrap_or(0.0))
        .bind(ru.get("max_value").and_then(|v| v.as_f64()).unwrap_or(0.0))
        .bind(ru.get("avg_value").and_then(|v| v.as_f64()).unwrap_or(0.0))
        .bind(ru.get("sample_count").and_then(|v| v.as_i64()).unwrap_or(0))
        .bind(period_start)
        .bind(&ts)
        .bind(ru.get("venue_id").and_then(|v| v.as_str()))
        .execute(&state.db)
        .await;
        match r {
            Ok(res) => {
                if res.rows_affected() > 0 { total += 1; }
                else { conflicts += 1; }
            }
            Err(e) => { tracing::warn!("metrics_rollups upsert error: {}", e); }
        }
    }
    if conflicts > 0 {
        let _ = sqlx::query(
            "UPDATE sync_state SET conflict_count = COALESCE(conflict_count, 0) + ?1
             WHERE table_name = 'metrics_rollups'"
        ).bind(conflicts).execute(&state.db).await;
    }
    tracing::info!("Sync push: {} metrics_rollups ({} conflicts)", rollups.len(), conflicts);
    total
}
