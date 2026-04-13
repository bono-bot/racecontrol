//! Billing background jobs — reconciliation, coupon TTL expiry, game request cleanup.
//!
//! Extracted from billing.rs (Phase 385, v49.0 Architecture Completion).
//! Self-contained spawned tasks that run on intervals.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use rc_common::protocol::DashboardEvent;

use crate::state::AppState;
use crate::whatsapp_alerter;

// ─── Reconciliation State ──────────────────────────────────────────────────

static LAST_RECONCILIATION_AT: std::sync::OnceLock<std::sync::RwLock<Option<String>>> =
    std::sync::OnceLock::new();
static LAST_DRIFT_COUNT: AtomicI64 = AtomicI64::new(-1); // -1 = never run
static LAST_DURATION_MS: AtomicI64 = AtomicI64::new(0);

fn reconciliation_status_lock() -> &'static std::sync::RwLock<Option<String>> {
    LAST_RECONCILIATION_AT.get_or_init(|| std::sync::RwLock::new(None))
}

/// Spawn background reconciliation job (FATM-12).
/// Every 30 minutes, compares wallet.balance_paise against SUM(wallet_transactions.amount_paise).
/// Logs discrepancies at ERROR and sends WhatsApp alert.
pub fn spawn_reconciliation_job(state: Arc<AppState>) {
    tokio::spawn(async move {
        // Initial delay: 60s after startup (avoid boot storm)
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        let mut interval =
            tokio::time::interval(std::time::Duration::from_secs(1800)); // 30 min
        loop {
            interval.tick().await;
            run_reconciliation(&state).await;
        }
    });
    tracing::info!(
        "FATM-12: Reconciliation job started (30-min interval, 60s initial delay)"
    );
}

/// Public wrapper so the admin endpoint can trigger an immediate reconciliation run.
pub async fn run_reconciliation_public(state: &Arc<AppState>) {
    run_reconciliation(state).await;
}

/// FATM-08: Spawn background task that expires stale coupon reservations.
/// Every 60 seconds, reverts 'reserved' coupons older than 10 minutes back to 'available'.
/// Initial delay: 120s to let the server stabilize.
pub fn spawn_coupon_ttl_expiry_job(state: Arc<AppState>) {
    tokio::spawn(async move {
        // Initial delay: 120s to let server stabilize
        tokio::time::sleep(std::time::Duration::from_secs(120)).await;
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        tracing::info!("FATM-08: Coupon TTL expiry task started (60s interval, 120s initial delay)");
        loop {
            interval.tick().await;
            // MMA-#1: Only expire reservations that are NOT linked to an active billing session.
            let result = sqlx::query(
                "UPDATE coupons SET coupon_status = 'available', reserved_at = NULL, \
                 reserved_for_session = NULL \
                 WHERE coupon_status = 'reserved' \
                 AND reserved_at < datetime('now', '-10 minutes') \
                 AND NOT EXISTS ( \
                     SELECT 1 FROM billing_sessions bs \
                     WHERE bs.id = coupons.reserved_for_session \
                     AND bs.status IN ('active', 'paused_manual', 'paused_disconnect', 'paused_game_pause', 'waiting_for_game'))",
            )
            .execute(&state.db)
            .await;
            match result {
                Ok(r) if r.rows_affected() > 0 => {
                    tracing::info!(
                        "FATM-08: Expired {} stale coupon reservation(s) (TTL 10 minutes)",
                        r.rows_affected()
                    );
                }
                Err(e) => {
                    tracing::warn!("FATM-08: Coupon TTL expiry job error: {}", e);
                }
                _ => {}
            }
        }
    });
}

/// BILL-03: Spawn background task that marks expired PWA game requests.
/// Runs every 60 seconds; marks pending requests whose expires_at < now() as 'expired'.
pub fn spawn_cleanup_expired_game_requests(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        tracing::info!("BILL-03: PWA game request TTL cleanup task started (60s interval)");
        loop {
            interval.tick().await;
            cleanup_expired_game_requests(&state).await;
        }
    });
}

/// BILL-03: Inner cleanup logic — marks pending game requests as expired and notifies dashboard.
async fn cleanup_expired_game_requests(state: &Arc<AppState>) {
    let expired: Vec<(String,)> = match sqlx::query_as(
        "SELECT id FROM game_launch_requests WHERE status = 'pending' AND expires_at < datetime('now')",
    )
    .fetch_all(&state.db)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!("BILL-03: Failed to query expired game requests: {}", e);
            return;
        }
    };

    if expired.is_empty() {
        return;
    }

    let count = expired.len();
    if let Err(e) = sqlx::query(
        "UPDATE game_launch_requests SET status = 'expired' WHERE status = 'pending' AND expires_at < datetime('now')",
    )
    .execute(&state.db)
    .await
    {
        tracing::error!("BILL-03: Failed to mark game requests as expired: {}", e);
        return;
    }

    tracing::info!("BILL-03: Marked {} PWA game request(s) as expired", count);

    for (request_id,) in expired {
        let _ = state.dashboard_tx.send(DashboardEvent::GameRequestExpired {
            request_id,
        });
    }
}

/// Inner reconciliation logic.
async fn run_reconciliation(state: &Arc<AppState>) {
    tracing::info!("RECONCILIATION: Starting wallet balance check");
    let start = std::time::Instant::now();

    let result = sqlx::query_as::<_, (String, i64, i64)>(
        "SELECT driver_id, balance_paise, computed_balance FROM (
            SELECT w.driver_id,
                   w.balance_paise,
                   COALESCE((SELECT SUM(wt.amount_paise)
                             FROM wallet_transactions wt
                             WHERE wt.driver_id = w.driver_id), 0) AS computed_balance
            FROM wallets w
         ) WHERE ABS(balance_paise - computed_balance) > 0
         LIMIT 100",
    )
    .fetch_all(&state.db)
    .await;

    match result {
        Ok(rows) if rows.is_empty() => {
            tracing::info!(
                "RECONCILIATION: All wallets balanced (took {:?})",
                start.elapsed()
            );
            update_reconciliation_status(0, start.elapsed());
        }
        Ok(rows) => {
            let count = rows.len();
            tracing::error!(
                "RECONCILIATION: {} wallet(s) with balance drift detected!",
                count
            );
            let mut details = Vec::new();
            for (driver_id, actual, computed) in &rows {
                let drift = actual - computed;
                let short_id = &driver_id[..8.min(driver_id.len())];
                tracing::error!(
                    "RECONCILIATION DRIFT: driver={}, wallet_balance={}p, txn_sum={}p, drift={}p",
                    driver_id,
                    actual,
                    computed,
                    drift
                );
                details.push(format!("{}: {}p drift", short_id, drift));
            }

            let alert_msg = format!(
                "RECONCILIATION ALERT: {} wallet(s) with balance drift.\n{}",
                count,
                details.join("\n")
            );
            whatsapp_alerter::send_whatsapp(&state.config, &alert_msg).await;

            update_reconciliation_status(count, start.elapsed());
        }
        Err(e) => {
            tracing::error!("RECONCILIATION: Query failed: {}", e);
        }
    }
}

/// Update in-memory reconciliation status (non-blocking, infallible).
fn update_reconciliation_status(drift_count: usize, duration: std::time::Duration) {
    let ts = chrono::Utc::now().to_rfc3339();
    *reconciliation_status_lock()
        .write()
        .unwrap_or_else(|e| e.into_inner()) = Some(ts);
    LAST_DRIFT_COUNT.store(drift_count as i64, Ordering::Relaxed);
    LAST_DURATION_MS.store(duration.as_millis() as i64, Ordering::Relaxed);
}

/// Returns the last reconciliation run status as JSON for the admin endpoint.
pub fn get_reconciliation_status() -> serde_json::Value {
    let last_at = reconciliation_status_lock()
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let drift_count = LAST_DRIFT_COUNT.load(Ordering::Relaxed);
    let status = if drift_count < 0 {
        "never_run"
    } else if drift_count == 0 {
        "healthy"
    } else {
        "drift_detected"
    };
    serde_json::json!({
        "last_run_at": last_at,
        "drift_count": if drift_count >= 0 { Some(drift_count) } else { None::<i64> },
        "last_duration_ms": LAST_DURATION_MS.load(Ordering::Relaxed),
        "interval_seconds": 1800,
        "status": status
    })
}
